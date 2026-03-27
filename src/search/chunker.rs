//! # 代码分块器
//!
//! 分三层策略：
//!   1. **Tree-sitter AST**（精确）— Rust/JS/TS/Python/Go/Java/C/C++/C#/Ruby
//!   2. **Regex 边界**（兜底）  — PHP/Kotlin/Swift/Scala/Lua/Bash
//!   3. **固定行数滑窗**        — 其余所有文件
//!   4. **Vue/Svelte SFC**     — 提取 script 块后按 TS/JS 处理

use std::path::Path;
use tree_sitter::{Node, Parser};

// ─────────────────────────────────────────────────────────────────────────────
// 公共数据结构
// ─────────────────────────────────────────────────────────────────────────────

/// 一个代码块及其元数据
#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// 唯一 ID：`文件路径:起始行-结束行`
    pub id: String,
    pub file_path: String,
    pub module: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    /// 函数 / 类名，可能含父容器前缀，如 `MyClass::my_method`
    pub symbol_name: Option<String>,
}

/// 从 AST / Regex 提取到的符号边界
#[derive(Debug, Clone)]
struct Symbol {
    start_line: usize, // 0-based
    end_line: usize,   // 0-based exclusive（即下一行的行号）
    name: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// 入口
// ─────────────────────────────────────────────────────────────────────────────

/// 对一个源文件进行分块，返回 `CodeChunk` 列表。
pub fn chunk_file(
    file_path: &Path,
    project_root: &Path,
    module_name: &str,
    chunk_size: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    if content.trim().is_empty() {
        return vec![];
    }

    // 路径统一使用正斜杠（Windows 兼容）
    let rel_path = {
        use path_slash::PathExt as _;
        file_path
            .strip_prefix(project_root)
            .map(|p| p.to_slash_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_slash_lossy().to_string())
    };

    let ext = file_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // ── Vue SFC 预处理 ────────────────────────────────────────────────────────
    // 从 .vue 文件中提取 <script> 或 <script setup> 块，作为 TS/JS 处理
    if ext == "vue" || ext == "svelte" {
        return chunk_sfc(&content, &rel_path, module_name, chunk_size, overlap);
    }

    let lines: Vec<&str> = content.lines().collect();

    // ── 1. AST 分块 ──────────────────────────────────────────────────────────
    if let Some(chunks) = try_ast_chunk(&ext, &content, &lines, &rel_path, module_name, chunk_size) {
        return chunks;
    }

    // ── 2. Regex 分块 ────────────────────────────────────────────────────────
    if let Some(chunks) = try_regex_chunk(&ext, &content, &lines, &rel_path, module_name, chunk_size) {
        return chunks;
    }

    // ── 3. 固定行数兜底 ──────────────────────────────────────────────────────
    chunk_by_lines(&lines, &rel_path, module_name, chunk_size, overlap)
}

// ─────────────────────────────────────────────────────────────────────────────
// 层 1：Tree-sitter AST
// ─────────────────────────────────────────────────────────────────────────────

/// 每种语言的 AST 配置
struct LangDef {
    /// 顶层直接变成 chunk 的节点类型（函数、宏等）
    symbol_kinds: &'static [&'static str],
    /// 容器节点（class / impl / namespace / mod）——往里递归找 method
    container_kinds: &'static [&'static str],
    /// 容器内部被视为 chunk 的节点类型（方法、构造函数等）
    nested_kinds: &'static [&'static str],
    /// 独立类型定义（struct / enum / interface）无方法时也作为 chunk
    type_kinds: &'static [&'static str],
}

// —— Rust ——
static RUST: LangDef = LangDef {
    symbol_kinds:    &["function_item", "macro_definition"],
    container_kinds: &["impl_item", "trait_item", "mod_item"],
    nested_kinds:    &["function_item", "macro_definition"],
    type_kinds:      &["struct_item", "enum_item", "type_item", "const_item", "static_item"],
};
// —— JavaScript ——
static JS: LangDef = LangDef {
    symbol_kinds:    &["function_declaration", "generator_function_declaration"],
    container_kinds: &["class_declaration", "export_statement"],
    nested_kinds:    &["method_definition", "function_declaration"],
    type_kinds:      &[],
};
// —— TypeScript（复用 JS 定义，额外类型在 get_ts_language 中选择）——
static TS: LangDef = LangDef {
    symbol_kinds:    &["function_declaration", "generator_function_declaration",
                       "type_alias_declaration"],
    container_kinds: &["class_declaration", "abstract_class_declaration",
                       "interface_declaration", "export_statement"],
    nested_kinds:    &["method_definition", "method_signature",
                       "property_signature", "function_declaration"],
    type_kinds:      &["enum_declaration"],
};
// —— Python ——
static PYTHON: LangDef = LangDef {
    symbol_kinds:    &["function_definition", "decorated_definition"],
    container_kinds: &["class_definition"],
    nested_kinds:    &["function_definition", "decorated_definition"],
    type_kinds:      &[],
};
// —— Go ——
static GO: LangDef = LangDef {
    symbol_kinds:    &["function_declaration", "method_declaration"],
    container_kinds: &[],
    nested_kinds:    &[],
    type_kinds:      &["type_declaration", "const_declaration", "var_declaration"],
};
// —— Java ——
static JAVA: LangDef = LangDef {
    symbol_kinds:    &[],
    container_kinds: &["class_declaration", "interface_declaration",
                       "enum_declaration", "annotation_type_declaration",
                       "record_declaration"],
    nested_kinds:    &["method_declaration", "constructor_declaration",
                       "compact_constructor_declaration"],
    type_kinds:      &[],
};
// —— C ——
static C: LangDef = LangDef {
    symbol_kinds:    &["function_definition"],
    container_kinds: &[],
    nested_kinds:    &[],
    type_kinds:      &["type_definition"],
};
// —— C++ ——
static CPP: LangDef = LangDef {
    symbol_kinds:    &["function_definition", "template_declaration"],
    container_kinds: &["class_specifier", "struct_specifier",
                       "namespace_definition"],
    nested_kinds:    &["function_definition", "template_declaration"],
    type_kinds:      &["type_definition", "alias_declaration"],
};
// —— C# ——
static CSHARP: LangDef = LangDef {
    symbol_kinds:    &[],
    container_kinds: &["class_declaration", "interface_declaration",
                       "struct_declaration", "namespace_declaration",
                       "enum_declaration", "record_declaration"],
    nested_kinds:    &["method_declaration", "constructor_declaration",
                       "property_declaration", "event_declaration",
                       "operator_declaration", "conversion_operator_declaration"],
    type_kinds:      &[],
};
// —— Ruby ——
static RUBY: LangDef = LangDef {
    symbol_kinds:    &["method", "singleton_method"],
    container_kinds: &["class", "module", "singleton_class"],
    nested_kinds:    &["method", "singleton_method"],
    type_kinds:      &[],
};

fn get_ts_language(ext: &str) -> Option<(tree_sitter::Language, &'static LangDef)> {
    match ext {
        "rs"  => Some((tree_sitter_rust::language(),       &RUST)),
        "js" | "jsx" | "mjs" | "cjs"
              => Some((tree_sitter_javascript::language(), &JS)),
        "ts"  => Some((tree_sitter_typescript::language_typescript(), &TS)),
        "tsx" => Some((tree_sitter_typescript::language_tsx(),        &TS)),
        "py" | "pyi"
              => Some((tree_sitter_python::language(),     &PYTHON)),
        "go"  => Some((tree_sitter_go::language(),         &GO)),
        "java"=> Some((tree_sitter_java::language(),       &JAVA)),
        "c" | "h"
              => Some((tree_sitter_c::language(),          &C)),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx"
              => Some((tree_sitter_cpp::language(),        &CPP)),
        "cs"  => Some((tree_sitter_c_sharp::language(),   &CSHARP)),
        "rb"  => Some((tree_sitter_ruby::language(),      &RUBY)),
        _     => None,
    }
}

fn try_ast_chunk(
    ext: &str,
    content: &str,
    lines: &[&str],
    rel_path: &str,
    module: &str,
    max_size: usize,
) -> Option<Vec<CodeChunk>> {
    let (ts_lang, lang_def) = get_ts_language(ext)?;

    let mut parser = Parser::new();
    parser.set_language(ts_lang).ok()?;
    let tree = parser.parse(content, None)?;

    let src = content.as_bytes();
    let root = tree.root_node();

    // 收集所有符号
    let mut symbols: Vec<Symbol> = vec![];
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        collect_symbols(child, src, lang_def, "", &mut symbols);
    }

    if symbols.is_empty() {
        return None;
    }

    symbols.sort_by_key(|s| s.start_line);
    symbols.dedup_by_key(|s| s.start_line);

    let first_sym_line = symbols[0].start_line;

    let mut chunks = symbols_to_chunks(symbols, lines, &rel_path, module, max_size);

    // 如果文件头有实质内容，单独作为一个 chunk
    if first_sym_line > 3 {
        let header_content = lines[..first_sym_line.min(lines.len())].join("\n");
        if !header_content.trim().is_empty() {
            chunks.insert(0, CodeChunk {
                id:          format!("{}:1-{}", rel_path, first_sym_line),
                file_path:   rel_path.to_string(),
                module:      module.to_string(),
                start_line:  1,
                end_line:    first_sym_line,
                content:     header_content,
                symbol_name: Some("[imports/header]".to_string()),
            });
        }
    }

    Some(chunks)
}

// ── AST 遍历 ─────────────────────────────────────────────────────────────────

fn collect_symbols(
    node: Node,
    src: &[u8],
    def: &LangDef,
    parent_ctx: &str,   // 父容器名，用于拼接 "MyClass::method"
    out: &mut Vec<Symbol>,
) {
    let kind = node.kind();

    if def.symbol_kinds.contains(&kind) {
        let name = extract_name(node, src, kind);
        let full = qualify(parent_ctx, &name);
        out.push(Symbol {
            start_line: node.start_position().row,
            end_line:   node.end_position().row + 1,
            name:       full,
        });

    } else if def.container_kinds.contains(&kind) {
        let cname = qualify(parent_ctx, &extract_container_name(node, src, kind));
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            let ck = child.kind();
            if def.nested_kinds.contains(&ck) {
                let mname = qualify(&cname, &extract_name(child, src, ck));
                out.push(Symbol {
                    start_line: child.start_position().row,
                    end_line:   child.end_position().row + 1,
                    name:       mname,
                });
            } else if def.container_kinds.contains(&ck) {
                // 嵌套容器（内部类等）
                collect_symbols(child, src, def, &cname, out);
            } else if def.symbol_kinds.contains(&ck) {
                let name = qualify(&cname, &extract_name(child, src, ck));
                out.push(Symbol {
                    start_line: child.start_position().row,
                    end_line:   child.end_position().row + 1,
                    name,
                });
            }
        }

    } else if def.type_kinds.contains(&kind) {
        let name = extract_name(node, src, kind);
        let full = qualify(parent_ctx, &name);
        out.push(Symbol {
            start_line: node.start_position().row,
            end_line:   node.end_position().row + 1,
            name:       full,
        });

    } else {
        // export_statement、decorated_definition 等包装节点 —— 透传递归
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect_symbols(child, src, def, parent_ctx, out);
        }
    }
}

// ── 名称提取 ─────────────────────────────────────────────────────────────────

fn extract_name(node: Node, src: &[u8], kind: &str) -> String {
    // 特殊情况处理
    match kind {
        // Python 装饰器定义：外层是 decorated_definition，内层才有名字
        "decorated_definition" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                let ck = child.kind();
                if ck == "function_definition" || ck == "class_definition" {
                    let inner = extract_name(child, src, ck);
                    return format!("@{}", inner);
                }
            }
            return "<decorated>".to_string();
        }
        // C/C++ 函数定义：名字藏在 declarator 树里
        "function_definition" | "template_declaration" => {
            if let Some(decl) = node.child_by_field_name("declarator") {
                if let Some(name) = dig_declarator_name(decl, src) {
                    return name;
                }
            }
            // template_declaration: 内部通常是 function_definition 或 class_specifier
            if kind == "template_declaration" {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    let ck = child.kind();
                    if ck == "function_definition" || ck == "class_specifier" {
                        return format!("template<>{}", extract_name(child, src, ck));
                    }
                }
            }
        }
        // Go method_declaration: receiver + name
        "method_declaration" => {
            let recv = node.child_by_field_name("receiver")
                .and_then(|r| {
                    // receiver 里找 type_identifier
                    find_type_ident(r, src)
                })
                .unwrap_or_default();
            let fname = node.child_by_field_name("name")
                .and_then(|n| n.utf8_text(src).ok())
                .unwrap_or("<method>")
                .to_string();
            return if recv.is_empty() { fname } else { format!("({}).{}", recv, fname) };
        }
        // Rust impl_item: 没有 name 字段，用 type 字段
        "impl_item" => {
            return extract_container_name(node, src, kind);
        }
        _ => {}
    }

    // 通用：child_by_field_name("name")
    if let Some(n) = node.child_by_field_name("name") {
        return n.utf8_text(src).unwrap_or("<unknown>").to_string();
    }

    // JS/TS: lexical_declaration → variable_declarator.name（仅当值为函数时）
    if kind == "lexical_declaration" || kind == "variable_declaration" {
        if let Some(vd) = find_named_child_of_kind(node, "variable_declarator") {
            if let Some(val) = vd.child_by_field_name("value") {
                let vk = val.kind();
                if vk.contains("function") || vk.contains("arrow") {
                    if let Some(n) = vd.child_by_field_name("name") {
                        return n.utf8_text(src).unwrap_or("<anon>").to_string();
                    }
                }
            }
        }
        return "<var>".to_string();  // 普通变量，不应作为 symbol
    }

    "<unknown>".to_string()
}

fn extract_container_name(node: Node, src: &[u8], kind: &str) -> String {
    // Rust impl_item: impl Trait for Type
    if kind == "impl_item" {
        let type_name = node.child_by_field_name("type")
            .and_then(|n| n.utf8_text(src).ok())
            .unwrap_or("impl")
            .to_string();
        let trait_name = node.child_by_field_name("trait")
            .and_then(|n| n.utf8_text(src).ok())
            .map(|t| format!("<{}>", t));
        return match trait_name {
            Some(t) => format!("{}{}", type_name, t),
            None    => type_name,
        };
    }
    // Ruby class / module：scope_resolution or constant
    if kind == "class" || kind == "module" {
        if let Some(n) = node.child_by_field_name("name") {
            return n.utf8_text(src).unwrap_or("<class>").to_string();
        }
    }
    // C++ namespace_definition: name field
    if kind == "namespace_definition" {
        if let Some(n) = node.child_by_field_name("name") {
            return n.utf8_text(src).unwrap_or("namespace").to_string();
        }
        return "<anonymous namespace>".to_string();
    }
    // General
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(src).ok())
        .unwrap_or("<unknown>")
        .to_string()
}

/// 递归挖 C/C++ declarator 树，找到最终的标识符名
fn dig_declarator_name(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" | "destructor_name" => {
            node.utf8_text(src).ok().map(|s| s.to_string())
        }
        "qualified_identifier" => {
            // Foo::bar — 直接取全文
            node.utf8_text(src).ok().map(|s| s.to_string())
        }
        "operator_name" => {
            node.utf8_text(src).ok().map(|s| s.to_string())
        }
        _ => {
            // function_declarator / pointer_declarator / reference_declarator 等
            // 优先看 declarator 字段，否则取第一个 named child
            if let Some(inner) = node.child_by_field_name("declarator") {
                if let Some(name) = dig_declarator_name(inner, src) {
                    return Some(name);
                }
            }
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(name) = dig_declarator_name(child, src) {
                    return Some(name);
                }
            }
            None
        }
    }
}

fn find_named_child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let result: Option<Node<'a>> = node.named_children(&mut cursor).find(|c| c.kind() == kind);
    result
}

fn find_type_ident(node: Node, src: &[u8]) -> Option<String> {
    if node.kind() == "type_identifier" || node.kind() == "pointer_type" {
        return node.utf8_text(src).ok().map(|s| s.trim_start_matches('*').to_string());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(n) = find_type_ident(child, src) {
            return Some(n);
        }
    }
    None
}

fn qualify(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", parent, name)
    }
}

// ── Symbols → Chunks ─────────────────────────────────────────────────────────

fn symbols_to_chunks(
    symbols: Vec<Symbol>,
    lines: &[&str],
    rel_path: &str,
    module: &str,
    max_size: usize,
) -> Vec<CodeChunk> {
    let mut chunks = vec![];

    for sym in symbols {
        // 过滤掉没有实质内容的 symbol（如普通变量）
        if sym.name == "<var>" || sym.name == "<unknown>" {
            continue;
        }

        let start = sym.start_line.min(lines.len());
        let end   = sym.end_line.min(lines.len());
        if start >= end { continue; }

        let sym_lines = &lines[start..end];
        let total = sym_lines.len();

        // 超长符号按 max_size 行分割（不做 overlap，因为已有 AST 边界）
        let mut offset = 0;
        while offset < total {
            let slice_end = (offset + max_size).min(total);
            // 直接存储原始源码，不注入 header，保证行号与内容对应
            let final_content = sym_lines[offset..slice_end].join("\n");

            let sym_label = if offset == 0 {
                sym.name.clone()
            } else {
                format!("{}(cont.+{})", sym.name, offset)
            };

            chunks.push(CodeChunk {
                id:          format!("{}:{}-{}", rel_path, start + offset + 1, start + slice_end),
                file_path:   rel_path.to_string(),
                module:      module.to_string(),
                start_line:  start + offset + 1,
                end_line:    start + slice_end,
                content:     final_content,
                symbol_name: Some(sym_label),
            });

            if slice_end >= total { break; }
            offset = slice_end;
        }
    }

    chunks
}

// ─────────────────────────────────────────────────────────────────────────────
// 层 2：Regex 边界（兜底语言）
// ─────────────────────────────────────────────────────────────────────────────

struct RegexDef {
    /// 匹配"符号起始行"的正则，第 1 个捕获组为名字
    pattern: &'static str,
}

static PHP_RE: RegexDef = RegexDef {
    pattern:   r"(?m)^(?:(?:public|protected|private|static|abstract|final)\s+)*(?:function\s+(\w+)|class\s+(\w+)|interface\s+(\w+)|trait\s+(\w+))",
};
static KOTLIN_RE: RegexDef = RegexDef {
    pattern:   r"(?m)^(?:(?:public|private|protected|internal|open|abstract|override|inline|suspend|data|sealed|enum|companion|object|inner)\s+)*(?:fun\s+(?:<[^>]*>\s*)?(\w+)|class\s+(\w+)|object\s+(\w+)|interface\s+(\w+))",
};
static SWIFT_RE: RegexDef = RegexDef {
    pattern:   r"(?m)^(?:(?:public|private|internal|open|fileprivate|final|override|static|class|mutating|nonmutating|required|convenience|weak|lazy|dynamic)\s+)*(?:func\s+(\w+)|class\s+(\w+)|struct\s+(\w+)|enum\s+(\w+)|protocol\s+(\w+)|extension\s+(\w+))",
};
static SCALA_RE: RegexDef = RegexDef {
    pattern:   r"(?m)^(?:\s*)(?:override\s+)?(?:def\s+(\w+)|class\s+(\w+)|object\s+(\w+)|trait\s+(\w+)|case\s+class\s+(\w+))",
};
static LUA_RE: RegexDef = RegexDef {
    pattern:   r"(?m)^(?:local\s+)?function\s+(\w[\w.]*)\s*\(|^(\w[\w.]*)\s*=\s*function\s*\(",
};
static BASH_RE: RegexDef = RegexDef {
    pattern:   r"(?m)^(?:function\s+(\w+)\s*\{?|(\w+)\s*\(\s*\)\s*\{)",
};
static HASKELL_RE: RegexDef = RegexDef {
    // 顶层类型签名行作为边界
    pattern:   r"(?m)^([a-z_]\w*)\s*::\s*",
};

fn get_regex_def(ext: &str) -> Option<&'static RegexDef> {
    match ext {
        "php"  => Some(&PHP_RE),
        "kt" | "kts" => Some(&KOTLIN_RE),
        "swift" => Some(&SWIFT_RE),
        "scala" | "sc" => Some(&SCALA_RE),
        "lua"  => Some(&LUA_RE),
        "sh" | "bash" | "zsh" => Some(&BASH_RE),
        "hs" | "lhs" => Some(&HASKELL_RE),
        _ => None,
    }
}

fn try_regex_chunk(
    ext: &str,
    content: &str,
    lines: &[&str],
    rel_path: &str,
    module: &str,
    max_size: usize,
) -> Option<Vec<CodeChunk>> {
    let def = get_regex_def(ext)?;
    let re  = regex::Regex::new(def.pattern).ok()?;

    let mut boundaries: Vec<(usize, String)> = vec![];

    for cap in re.captures_iter(content) {
        // 找第一个有值的捕获组
        let name = (1..cap.len())
            .find_map(|i| cap.get(i))
            .map(|m| {
                let line = content[..m.start()].chars().filter(|&c| c == '\n').count();
                (line, m.as_str().to_string())
            });
        if let Some((line, name)) = name {
            boundaries.push((line, name));
        }
    }

    if boundaries.is_empty() {
        return None;
    }

    let chunks = boundaries_to_chunks(&boundaries, lines, rel_path, module, max_size);
    Some(chunks)
}

fn boundaries_to_chunks(
    boundaries: &[(usize, String)],
    lines: &[&str],
    rel_path: &str,
    module: &str,
    max_size: usize,
) -> Vec<CodeChunk> {
    let mut chunks = vec![];
    let n = boundaries.len();

    for (i, (start_line, symbol)) in boundaries.iter().enumerate() {
        let end_line = if i + 1 < n { boundaries[i + 1].0 } else { lines.len() };

        let mut offset = *start_line;
        while offset < end_line {
            let slice_end = (offset + max_size).min(end_line);
            let content   = lines[offset..slice_end].join("\n");
            if content.trim().is_empty() { offset = slice_end; continue; }

            let sym_label = if offset == *start_line {
                symbol.clone()
            } else {
                format!("{}(cont.)", symbol)
            };

            chunks.push(CodeChunk {
                id:          format!("{}:{}-{}", rel_path, offset + 1, slice_end),
                file_path:   rel_path.to_string(),
                module:      module.to_string(),
                start_line:  offset + 1,
                end_line:    slice_end,
                content,
                symbol_name: Some(sym_label),
            });

            if slice_end >= end_line { break; }
            offset = slice_end;
        }
    }
    chunks
}

// ─────────────────────────────────────────────────────────────────────────────
// 层 3：固定行数滑窗（最终兜底）
// ─────────────────────────────────────────────────────────────────────────────

fn chunk_by_lines(
    lines: &[&str],
    rel_path: &str,
    module: &str,
    chunk_size: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let mut chunks = vec![];
    let mut start  = 0;

    while start < lines.len() {
        let end = (start + chunk_size).min(lines.len());
        let content = lines[start..end].join("\n");

        if !content.trim().is_empty() {
            chunks.push(CodeChunk {
                id:          format!("{}:{}-{}", rel_path, start + 1, end),
                file_path:   rel_path.to_string(),
                module:      module.to_string(),
                start_line:  start + 1,
                end_line:    end,
                content,
                symbol_name: None,
            });
        }

        if end >= lines.len() { break; }
        start = end.saturating_sub(overlap);
    }

    chunks
}

// ─────────────────────────────────────────────────────────────────────────────
// Vue / Svelte SFC 分块
//
// 从单文件组件中提取 <script> / <script setup> 块，以 TS/JS 进行 AST 分块。
// <template> 块作为单独的一个整体 chunk（不做 AST 解析）。
// ─────────────────────────────────────────────────────────────────────────────

fn chunk_sfc(
    content: &str,
    rel_path: &str,
    module: &str,
    chunk_size: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let mut chunks = vec![];
    let all_lines: Vec<&str> = content.lines().collect();

    // 提取各 block：<script ...>, <template>, <style>
    let blocks = extract_sfc_blocks(content);

    for block in &blocks {
        let block_lines: Vec<&str> = block.content.lines().collect();

        match block.tag.as_str() {
            "script" => {
                // 确定语言：lang="ts" 用 TS，否则 JS
                let lang = if block.attrs.contains("lang=\"ts\"")
                    || block.attrs.contains("lang='ts'")
                {
                    "ts"
                } else {
                    "js"
                };

                // 构造虚拟文件名供 AST 选择
                let virtual_ext = if block.attrs.contains("setup") {
                    format!("{rel_path}[script setup].{lang}")
                } else {
                    format!("{rel_path}[script].{lang}")
                };

                // 尝试 AST 分块
                let ast_chunks = try_ast_chunk(
                    lang,
                    &block.content,
                    &block_lines,
                    &virtual_ext,
                    module,
                    chunk_size,
                );

                if let Some(mut ac) = ast_chunks {
                    // 修正行号偏移（block 在文件中的起始行）
                    for c in &mut ac {
                        c.start_line += block.start_line;
                        c.end_line   += block.start_line;
                        c.id = format!("{}:{}-{}", rel_path, c.start_line, c.end_line);
                        c.file_path = rel_path.to_string();
                        c.module    = module.to_string();
                    }
                    chunks.extend(ac);
                } else {
                    // 行数兜底
                    let mut lc = chunk_by_lines(
                        &block_lines, &virtual_ext, module, chunk_size, overlap,
                    );
                    for c in &mut lc {
                        c.start_line += block.start_line;
                        c.end_line   += block.start_line;
                        c.id = format!("{}:{}-{}", rel_path, c.start_line, c.end_line);
                        c.file_path = rel_path.to_string();
                    }
                    chunks.extend(lc);
                }
            }
            "template" => {
                // template 整体作为一个 chunk，便于搜索组件结构
                let template_content = block_lines.join("\n");
                if !template_content.trim().is_empty() {
                    chunks.push(CodeChunk {
                        id:          format!("{}:{}:{}", rel_path, block.start_line, block.end_line),
                        file_path:   rel_path.to_string(),
                        module:      module.to_string(),
                        start_line:  block.start_line + 1,
                        end_line:    block.end_line,
                        content:     template_content,
                        symbol_name: Some("<template>".to_string()),
                    });
                }
            }
            _ => {} // <style> 略过
        }
    }

    // 如果没解析到任何 block，整文件行数兜底
    if chunks.is_empty() {
        return chunk_by_lines(&all_lines, rel_path, module, chunk_size, overlap);
    }

    chunks
}

struct SfcBlock {
    tag:        String, // "script" / "template" / "style"
    attrs:      String, // raw attribute string
    content:    String, // block 内部文本（不含开闭标签行）
    start_line: usize,  // 0-based，开标签所在行
    end_line:   usize,  // 0-based，闭标签所在行
}

fn extract_sfc_blocks(content: &str) -> Vec<SfcBlock> {
    use regex::Regex;
    let open_re  = Regex::new(r"(?i)^<(script|template|style)(\b[^>]*)?>").unwrap();
    let close_re = Regex::new(r"(?i)^</(script|template|style)>").unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = vec![];
    let mut i = 0;

    while i < lines.len() {
        if let Some(cap) = open_re.captures(lines[i].trim_start()) {
            let tag   = cap[1].to_lowercase();
            let attrs = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("").to_string();
            let start = i;
            i += 1;
            let mut inner: Vec<&str> = vec![];
            while i < lines.len() {
                if close_re.is_match(lines[i].trim_start()) {
                    let end = i;
                    blocks.push(SfcBlock {
                        tag,
                        attrs,
                        content:    inner.join("\n"),
                        start_line: start,
                        end_line:   end,
                    });
                    i += 1;
                    break;
                }
                inner.push(lines[i]);
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    blocks
}
