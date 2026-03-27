//! 输出内容生成
//!
//! ── A. 任务提示词（init 时打印到终端，一次性使用）───────────────────────
//!    env_md_task / memory_md_task / module_analyze
//!
//! ── B. 项目文档（写入 .margi/，markdown 格式）────────────────────────────
//!    readme_md / agents_md / agents_md_snippet
//!    module_plan / module_split / module_merge

use std::path::Path;
use crate::paths::to_slash;

// ═════════════════════════════════════════════════════════════════════════════
// A. 任务提示词（init 时打印到终端）
// ═════════════════════════════════════════════════════════════════════════════

/// 任务提示词：生成 .margi/env.md（init 时打印到终端）
pub fn env_md_task(file_count: usize, languages: &[String], entry_files: &[String]) -> String {
    let entry_hint = if entry_files.is_empty() {
        String::new()
    } else {
        format!("\n     入口文件：{}\n", entry_files.join(", "))
    };

    format!(
        "任务：生成 .margi/env.md\n\
         \n\
         根据项目实际情况生成环境文档，只写实际存在的信息，\
         不确定的内容用 TODO 标记。\n\
         \n\
         必须包含的章节：\n\
         1. 开发环境启动 — 依赖安装、环境变量、本地运行命令\n\
         2. 构建 & 打包 — 各环境构建命令、产物路径\n\
         3. 测试执行 — 测试命令、覆盖率说明\n\
         4. 部署流程 — 分阶段步骤，标注需要人工介入的环节\n\
         5. 常见问题 — 已知的环境问题和解决方法\n\
         \n\
         项目信息：{file_count} 个文件，语言：{languages}{entry_hint}\
         生成完成后写入 .margi/env.md",
        languages = languages.join(", "),
    )
}

/// 任务提示词：生成 .margi/memory.md（init 时打印到终端）
pub fn memory_md_task(languages: &[String]) -> String {
    format!(
        "任务：生成 .margi/memory.md\n\
         \n\
         浏览项目目录结构后，生成 `.margi/memory.md` 初始骨架。\
         使用 `##` 作为一级章节标题。\
         无法确定的内容用 `TODO` 标记，不要填充虚假内容。\n\
         \n\
         必须包含的章节：\n\
         1. 全局注意事项 — 开发时必须始终遵守的约束（架构边界、禁止操作等）\n\
         2. 模块概览 — 每个模块一句话说明职责\n\
         3. 重要架构决策 — 关键技术选型和设计决定及原因\n\
         \n\
         语言/扩展名：{languages}\n\
         生成完成后写入 `.margi/memory.md`。",
        languages = languages.join(", "),
    )
}

/// 任务提示词：为模块生成文档（只含路径列表，不内嵌源码）
pub fn module_analyze(
    module_name:    &str,
    doc_dir:        &Path,
    source_dir:     &Path,
    source_files:   &[std::path::PathBuf],
    _project_root:  &Path,
    existing_notes: &str,
    existing_api:   &str,
    corrections:    &str,
) -> String {
    let api_path       = to_slash(&doc_dir.join("api.md"));
    let internals_path = to_slash(&doc_dir.join("internals.md"));
    let notes_path     = to_slash(&doc_dir.join("notes.md"));
    let src_dir_slash  = to_slash(source_dir);

    let file_list = if source_files.is_empty() {
        "  （目录为空或无源文件）".to_string()
    } else {
        source_files.iter()
            .map(|p| format!("  - {}", to_slash(p)))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let context_section = build_context_section(existing_notes, existing_api, corrections);

    let vue_hint = if source_files.iter().any(|p| {
        p.extension().map(|e| e.eq_ignore_ascii_case("vue")).unwrap_or(false)
    }) {
        "\n> 本模块包含 `.vue` 文件。分析时请注意 `<script setup>` 中的 \
         `defineProps` / `defineEmits` / `defineExpose` 即为对外接口。\n"
    } else { "" };

    format!(
        "# 任务：为模块 `{module_name}` 生成文档\n\
         \n\
         读取下方列出的源文件，然后生成以下三个文档文件。\n\
         每个文件用 `## 文件：<路径>` 作为分隔标题输出内容。\n\
         {vue_hint}\
         \n\
         ## 源文件列表\n\
         \n\
         模块目录：`{src_dir_slash}`\n\
         \n\
         {file_list}\n\
         \n\
         ---\n\
         \n\
         ## 文件 1：{api_path}\n\
         \n\
         - 对外暴露的函数 / 类 / 接口 / 组件 Props（参数类型、返回值、副作用）\n\
         - 主要数据结构和类型定义（字段含义、约束条件）\n\
         - 使用示例（仅在存在非直觉用法时附上）\n\
         - **不包含**内部实现细节\n\
         \n\
         ## 文件 2：{internals_path}\n\
         \n\
         - 核心数据流和处理流程（ASCII 图或有序列表）\n\
         - 关键算法和设计决策（说明*为什么*这样做）\n\
         - 模块内部重要依赖关系和状态管理逻辑\n\
         \n\
         ## 文件 3：{notes_path}\n\
         \n\
         写操作性结论，不写描述性文字。每条格式为：\n\
         「不要做 X，因为 Y」或「做 Z 之前必须先做 W」或「修改 A 时注意 B」\n\
         \n\
         内容包含：\n\
         - 不能做什么、容易误用的地方（**最重要**）\n\
         - 已知的坑、边界情况、竞态条件\n\
         - 与其他模块的耦合约束（改这里会影响哪里）\n\
         - 性能 / 安全注意事项（如有）\n\
         \n\
         ---\n\
         {context_section}\
         ## 完成后\n\
         \n\
         ```bash\n\
         margi module set-status {module_name} understood\n\
         ```\n",
    )
}

fn build_context_section(existing_notes: &str, existing_api: &str, corrections: &str) -> String {
    let mut parts = vec![];
    if !existing_notes.is_empty() {
        parts.push(format!("### 已有 notes.md（保留并补充）\n\n{}", existing_notes));
    }
    if !existing_api.is_empty() {
        parts.push(format!("### 已有 api.md（参考并更新）\n\n{}", existing_api));
    }
    if !corrections.is_empty() {
        parts.push(format!("### 历史纠正记录（必须体现在 notes.md 中）\n\n{}", corrections));
    }
    if parts.is_empty() { return String::new(); }
    format!("\n## 已有上下文\n\n{}\n\n---\n\n", parts.join("\n\n---\n\n"))
}

// ═════════════════════════════════════════════════════════════════════════════
// B. 项目文档（写入磁盘的 .md 文件）
// ═════════════════════════════════════════════════════════════════════════════

/// .margi/README.md — 功能参考和目录说明
pub fn readme_md(project_name: &str) -> String {
    format!(
"# margi — {project_name}

> margi 是项目上下文管理工具，帮助理解和维护大型代码库。
> 本文件记录功能参考和目录结构，供日常查阅。

---

## 核心概念

- **模块（Module）**：一个源码目录对应一个模块，有独立的文档（api / internals / notes）
- **索引（Index）**：对源码和模块文档的全文搜索索引，支持中英文混合搜索
- **memory.md**：项目全局记忆，记录约束、架构决策等高优先级信息

---

## 日常开发场景

### 开始一个任务前

```bash
margi module status                  # 查看哪些模块已分析
margi module load src/auth           # 加载相关模块文档到上下文
margi search \"关键词\"               # 搜索源码定位相关代码
margi search --docs \"关键词\"        # 搜索模块文档
```

### 修改完成后

```bash
margi diff                           # 检测代码变更，标记过时的模块文档
margi note add \"注意事项\" --module src/auth     # 记录注意事项
margi correct add \"错误→正确\" --module src/auth  # 记录纠正
```

### 新加入代码库

```bash
margi status                         # 整体状态总览
margi module plan                    # 查看目录结构
margi module list                    # 列出已注册模块
margi env show                       # 查看环境和构建说明
```

---

## 模块管理

```bash
margi module add src/feature         # 注册新模块（路径即 key，必须存在）
margi module analyze src/feature     # 生成模块文档（输出任务描述到 stdout）
margi module load src/feature --include-internals   # 加载含内部实现的完整文档
margi module set-status src/feature outdated        # 手动标记过时

margi module split src/feature --depth 2            # 拆分大模块
margi module merge src/a src/b --into src/combined  # 合并模块
margi module remove src/old --archive               # 归档注销
```

模块状态：`unknown` → `analyzing` → `partial` / `understood` / `outdated`

---

## 搜索

```bash
# 源码搜索（默认）
margi search \"合并\"                 # 中文（bigram 索引，2字词精确命中）
margi search \"cmd_merge\"            # 函数名（trigram 子串匹配）
margi search \"parseUser\"            # 驼峰命名
margi search \"AI\"                   # 短词（< 3字符自动 LIKE 兜底）
margi search \"合并module\"           # 中英文混合（双路 + RRF 合并）

# 文档搜索
margi search --docs \"模块状态\"      # 搜索 .margi/modules/ 下的文档

# 过滤选项
margi search \"token\" --in src/auth  # 限定模块
margi search \"fn\" -n 20             # 返回更多结果（默认 10）
margi search --exact \"cmd_merge\"    # 精确短语匹配
```

---

## 搜索索引

```bash
margi index build          # 增量构建（源码 + 模块文档）
margi index build --force  # 全量重建（首次或 clear 后必须执行）
margi index stats          # 查看统计
margi index clear          # 清空索引（清空后需 build --force）
```

索引技术：英文/代码使用 trigram（支持子串），中文使用 bigram（相邻2字组）。

---

## 注意事项与纠正记录

```bash
margi note add \"内容\" [--module <key>] [--tag <标签>]
margi note list [--module <key>] [--tag <标签>]

margi correct add \"问题→正确做法\" [--module <key>]
margi correct list [--since 2024-01-01] [--module <key>]
```

---

## 文件结构

```
<项目根>/
  AGENTS.md              — 工具入口（margi init 生成，可手动补充）
  .margi/
    README.md            — 本文件
    config.json          — 配置（source_root、排除规则等）
    env.md               — 环境 & 构建说明
    memory.md            — 项目全局记忆
    modules/
      src/auth/
        STATUS           — 模块状态
        api.md           — 对外接口文档
        internals.md     — 内部实现说明
        notes.md         — 注意事项
    .index/              — 搜索索引（已加入 .gitignore）
      chunks.db
      meta.json
    corrections/
      YYYY-MM-DD.md      — 纠正记录（按日期）
    archive/             — 已归档的模块文档
```

---

## 环境变量

| 变量 | 说明 |
|------|------|
| `MARGI_LANG` | 强制指定语言（`zh` / `en`），优先级最高 |
| `LANG` / `LANGUAGE` / `LC_ALL` | 系统语言检测（`zh_*` → 中文） |
"
    )
}


/// AGENTS.md（项目根目录）—— 新建时生成
pub fn agents_md() -> String {
    r#"# Project Context

> 完整工具说明见 `.margi/README.md`。

## margi 使用规则

本项目使用 margi 管理模块文档和搜索索引。**下列规则优先于默认的文件读取行为。**

### 任务开始前

在读取任何源文件来理解某个模块之前，先执行：

```bash
margi module status              # 查看哪些模块有现成文档
margi module load <key>          # 加载文档（已包含架构结论、接口、注意事项）
margi search "<关键词>"          # 搜索源码位置（中英文混合均可）
margi search --docs "<关键词>"   # 搜索已整理的模块文档
```

**`margi module load` 的内容是人工审核过的结论**，直接加载比阅读多个源文件更可靠、消耗更少上下文。有文档就用文档，不要重复读源码。

### 任务结束前

```bash
margi diff                                               # 检查哪些模块文档因改动变为过时
margi note add "<约束/注意事项>" --module <key>          # 持久化本次发现的重要约束
margi correct add "<错误做法 -> 正确做法>" --module <key> # 记录踩过的坑
```

对话结束后这些内容才不会丢失。不要把约束、耦合关系或设计限制只留在对话里。

### 判断表

| 需要做什么 | 正确做法 | 不要做 |
|-----------|---------|--------|
| 理解某模块的职责、接口、设计 | `margi module load <key>` | 直接打开该目录下的多个源文件 |
| 查找函数或概念在哪里 | `margi search "<关键词>"` | grep / 全局搜索 |
| 查找已知的约束或坑 | `margi search --docs` 或 `margi note list` | 假设没有相关记录 |
| 修改了某模块的接口或行为 | `margi diff` 检查文档状态 | 忽略文档更新 |
| 读完一个无文档模块的源码后 | `margi module analyze <key>` 输出文档任务 | 只在对话里总结 |
| 发现了约束、耦合或边界情况 | `margi note add` 或 `margi correct add` | 只留在当前对话 |

## 全局注意事项

<!-- 执行 `margi note list` 查看完整列表，详见 .margi/memory.md -->

## 环境 & 构建

见 `.margi/env.md`，或执行 `margi env show`
"#.to_string()
}

/// AGENTS.md 已存在时，向用户展示需要手动追加的片段
pub fn agents_md_snippet() -> &'static str {
r#"## margi 使用规则

本项目使用 margi 管理模块文档和搜索索引。**下列规则优先于默认的文件读取行为。**

任务开始前，在读源码之前先查文档：

```bash
margi module status              # 查看哪些模块有现成文档
margi module load <key>          # 加载文档（已包含架构结论、接口、注意事项）
margi search "<关键词>"          # 搜索源码（中英文混合均可）
margi search --docs "<关键词>"   # 搜索已整理的模块文档
```

任务结束前，持久化本次发现：

```bash
margi diff                                               # 检查文档是否因改动变为过时
margi note add "<约束/注意事项>" --module <key>          # 记录重要约束
margi correct add "<错误做法 -> 正确做法>" --module <key> # 记录踩过的坑
```

有模块文档就用 `margi module load` 而不读源码；搜索用 `margi search` 而不用 grep；发现的约束用 `margi note add` 持久化而不只留在对话里。

完整说明见 `.margi/README.md`"#
}

/// .margi/module-plan.md — 模块规划模板（由 module plan 写入）
pub fn module_plan(
    tree: &str,
    registered: &[(String, String)],
    scan_root: &Path,
    scan_depth: usize,
) -> String {
    let registered_section = if registered.is_empty() {
        "（当前无已注册模块）".to_string()
    } else {
        registered.iter()
            .map(|(n, s)| format!("- `{}` — {}", n, s))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "# 模块规划\n\
         \n\
         扫描根目录：`{scan_root}`（深度 {scan_depth}）\n\
         \n\
         ## 目录结构\n\
         \n\
         ```\n\
         {tree}\
         ```\n\
         \n\
         ## 当前已注册模块\n\
         \n\
         {registered_section}\n\
         \n\
         ## 规划原则\n\
         \n\
         - **独立为模块**：职责单一、内聚性强、有独立对外接口的目录\n\
         - **合并为整体**：文件少、逻辑简单、无需单独说明的目录\n\
         - **跳过**：纯资源目录（assets/public/static）、自动生成代码\n\
         \n\
         ## 规划结果（填写后执行下方命令）\n\
         \n\
         | 源码路径 | 说明 |\n\
         |---------|------|\n\
         | TODO    | TODO |\n\
         \n\
         ## 执行命令\n\
         \n\
         ```bash\n\
         margi module add <路径>                 # 注册模块\n\
         margi module split <key> --depth 2     # 拆分大模块\n\
         margi module merge <a> <b> --into <c>  # 合并模块\n\
         margi module analyze <key>             # 为模块生成文档\n\
         ```\n",
        scan_root = to_slash(scan_root),
    )
}

/// .margi/split-<module>.md — 模块拆分任务（由 module split 写入）
pub fn module_split(
    module_name: &str,
    module_source_dir: &Path,
    sub_tree: &str,
    doc_paths: &[String],
    existing_notes_path: Option<&Path>,
) -> String {
    let docs_section = if doc_paths.is_empty() {
        "（该模块暂无文档）".to_string()
    } else {
        doc_paths.iter()
            .map(|p| format!("- `{}`", p))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let notes_hint = existing_notes_path
        .map(|p| format!("\n已有 notes.md 可供参考：`{}`\n", to_slash(p)))
        .unwrap_or_default();

    format!(
        "# 模块拆分：`{module_name}`\n\
         \n\
         源码目录：`{source_dir}`\n\
         {notes_hint}\
         \n\
         ## 子目录结构\n\
         \n\
         ```\n\
         {sub_tree}\
         ```\n\
         \n\
         ## 当前文档文件\n\
         \n\
         {docs_section}\n\
         \n\
         ## 任务\n\
         \n\
         1. 阅读上方目录结构，决定哪些子目录应独立为模块\n\
         2. 为每个子模块注册并生成文档：\n\
         \n\
         ```bash\n\
         margi module add {module_name}/<子目录名>\n\
         margi module analyze {module_name}/<子目录名>\n\
         margi module set-status {module_name} outdated   # 原模块标记为过时\n\
         ```\n\
         \n\
         ## 拆分决策（填写）\n\
         \n\
         | 子目录 | 处理方式 | 备注 |\n\
         |--------|----------|------|\n\
         | TODO   | 独立/合并/跳过 | |\n",
        source_dir = to_slash(module_source_dir),
    )
}

/// .margi/merge-<target>.md — 模块合并任务（由 module merge 写入）
pub fn module_merge(
    source_names: &[String],
    target_name: &str,
    source_infos: &[(String, String, Vec<String>)],
) -> String {
    let sources_section = source_infos.iter().map(|(name, status, docs)| {
        let doc_list = if docs.is_empty() {
            "  （无文档）".to_string()
        } else {
            docs.iter().map(|d| format!("  - `{}`", d)).collect::<Vec<_>>().join("\n")
        };
        format!("### `{}` — {}\n\n{}", name, status, doc_list)
    }).collect::<Vec<_>>().join("\n\n");

    format!(
        "# 模块合并：{sources} → `{target_name}`\n\
         \n\
         ## 待合并模块\n\
         \n\
         {sources_section}\n\
         \n\
         ## 任务\n\
         \n\
         1. 阅读各模块文档，将内容合并整理\n\
         2. 在以下路径生成合并后的文档：\n\
         \n\
         ```\n\
         .margi/modules/{target_name}/api.md\n\
         .margi/modules/{target_name}/internals.md\n\
         .margi/modules/{target_name}/notes.md\n\
         ```\n\
         \n\
         3. 完成后执行：\n\
         \n\
         ```bash\n\
         margi module add {target_name} <源码路径>\n\
         margi module set-status {target_name} understood\n\
         margi module set-status {src} outdated\n\
         ```\n\
         \n\
         ## 合并要点\n\
         \n\
         - api.md：合并对外接口，去除重复，保持清晰分节\n\
         - internals.md：整合内部实现说明，说明模块间的关系\n\
         - notes.md：合并所有注意事项，按重要性排序\n",
        sources = source_names.join(" + "),
        src = source_names.first().cloned().unwrap_or_default(),
    )
}
