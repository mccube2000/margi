use anyhow::Result;
use path_slash::PathExt as _;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// 路径辅助
// ─────────────────────────────────────────────────────────────────────────────

/// 将任意路径转为正斜杠字符串（跨平台一致）
pub fn to_slash(path: &Path) -> String {
    path.to_slash_lossy().to_string()
}

/// 相对路径转正斜杠字符串；无法 strip_prefix 时返回完整路径
pub fn rel_slash(path: &Path, base: &Path) -> String {
    to_slash(path.strip_prefix(base).unwrap_or(path))
}

// ─────────────────────────────────────────────────────────────────────────────
// 模块 key 规范
//
// 模块 key = 相对项目根的正斜杠路径，例如：
//   src/components/auth
//   src/utils
//   _root_          （项目根目录下散落的源码文件）
//
// .margi/modules/ 下的目录结构直接镜像 key：
//   .margi/modules/src/components/auth/STATUS
//   .margi/modules/_root_/STATUS
// ─────────────────────────────────────────────────────────────────────────────

/// 特殊模块 key：项目根目录下的散落源码文件
pub const ROOT_MODULE: &str = "_root_";

/// 模块的文档目录（.margi/modules/<key>/）
pub fn module_doc_dir(margi_dir: &Path, key: &str) -> PathBuf {
    // PathBuf::push 会把 "/" 分割后逐段 join，Windows 也安全
    let mut p = margi_dir.join("modules");
    for seg in key.split('/') {
        p = p.join(seg);
    }
    p
}

/// 模块的源码目录（<project_root>/<key>/，_root_ 返回 project_root）
pub fn module_source_dir(project_root: &Path, key: &str) -> PathBuf {
    if key == ROOT_MODULE {
        project_root.to_path_buf()
    } else {
        let mut p = project_root.to_path_buf();
        for seg in key.split('/') {
            p = p.join(seg);
        }
        p
    }
}

/// 将绝对路径或相对路径规范化为模块 key（相对于 project_root 的正斜杠路径）
/// 输入可以是 "src/components/auth"、"src\components\auth"、绝对路径等
pub fn normalize_module_key(input: &str, project_root: &Path) -> String {
    let norm = input.replace('\\', "/");

    // 如果是绝对路径，尝试 strip project_root
    let path = PathBuf::from(input);
    if path.is_absolute() {
        return rel_slash(&path, project_root);
    }

    // 去掉多余的前导 "./"
    let key = norm.trim_start_matches("./");
    key.to_string()
}

/// 遍历 .margi/modules/ 找出所有包含 STATUS 文件的模块，返回 (key, doc_dir)
pub fn find_all_module_keys(margi_dir: &Path) -> Vec<(String, PathBuf)> {
    let modules_root = margi_dir.join("modules");
    if !modules_root.exists() { return vec![]; }
    let mut result = vec![];
    collect_module_keys(&modules_root, &modules_root, &mut result);
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn collect_module_keys(modules_root: &Path, current: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(current) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        // 如果含有 STATUS 文件，这是一个模块
        if path.join("STATUS").exists() {
            let key = rel_slash(&path, modules_root);
            out.push((key, path.clone()));
        }
        // 继续向下找（支持 src/components/auth 这样的嵌套）
        collect_module_keys(modules_root, &path, out);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 项目根 / .margi 目录
// ─────────────────────────────────────────────────────────────────────────────

pub fn project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let mut dir = cwd.as_path();
    loop {
        if dir.join(".margi").exists() || dir.join("AGENTS.md").exists() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    Ok(cwd)
}

pub fn margi_root() -> Result<PathBuf> {
    let root = project_root()?;
    let margi = root.join(".margi");
    if !margi.exists() {
        return Err(crate::error::MargiError::NotInitialized.into());
    }
    Ok(margi)
}

pub fn index_dir(margi_root: &Path) -> PathBuf       { margi_root.join(".index") }
pub fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() { std::fs::create_dir_all(path)?; }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 源文件检测（供 planner 使用）
// ─────────────────────────────────────────────────────────────────────────────

pub fn root_has_source_files(project_root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(project_root) else { return false };
    entries.flatten().any(|e| e.path().is_file() && is_known_source_ext(&e.path()))
}

fn is_known_source_ext(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    matches!(ext.as_deref(),
        Some("rs"|"ts"|"tsx"|"js"|"jsx"|"mjs"|"cjs"
            |"vue"|"svelte"|"py"|"pyi"|"go"
            |"java"|"kt"|"kts"|"swift"
            |"cpp"|"cc"|"cxx"|"c"|"h"|"hpp"|"cs"
            |"rb"|"php"|"scala"|"lua"))
}
