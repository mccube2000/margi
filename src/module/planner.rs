use anyhow::Result;
use colored::Colorize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{config::Config, error::MargiError, paths, prompts};
use paths::{
    find_all_module_keys, module_doc_dir, module_source_dir,
    normalize_module_key, root_has_source_files, to_slash, ROOT_MODULE,
};
use super::status::ModuleStatus;

// ─────────────────────────────────────────────────────────────────────────────
// plan
// ─────────────────────────────────────────────────────────────────────────────

pub fn cmd_plan(
    depth: Option<usize>,
    root_override: Option<&str>,
    margi_dir: &Path,
    project_root: &Path,
    config: &Config,
) -> Result<()> {
    let scan_depth = depth.unwrap_or(config.modules.scan_depth);

    let scan_root = if let Some(r) = root_override {
        let p = project_root.join(r.replace('\\', "/"));
        if !p.exists() {
            return Err(MargiError::PathNotFound(p.display().to_string()).into());
        }
        p
    } else {
        let src = config.source_root(project_root);
        if src.exists() { src } else { project_root.to_path_buf() }
    };

    eprintln!("{} {}  ({} {})",
        "→".cyan(),
        t!("扫描目录结构...", "Scanning directory structure..."),
        t!("深度", "depth"), scan_depth);

    // 构建目录树
    let tree = build_tree(&scan_root, &scan_root, scan_depth, 0);

    // 已注册模块
    let registered: Vec<(String, String)> = find_all_module_keys(margi_dir)
        .into_iter()
        .map(|(key, dir)| {
            let sf     = dir.join("STATUS");
            let status = if sf.exists() {
                ModuleStatus::parse(&std::fs::read_to_string(&sf).unwrap_or_default())
                    .label().to_string()
            } else { "unknown".to_string() };
            (key, status)
        })
        .collect();

    // 根目录散落文件提示
    let root_hint = if root_has_source_files(project_root) {
        format!("\n> {} `margi module add {}`\n",
            t!("项目根目录存在源文件，可注册为根模块：",
               "Root-level source files found. Register as root module:"),
            ROOT_MODULE)
    } else { String::new() };

    // stdout：目录树（可直接阅读，体积可控）
    println!("{}{}", tree, root_hint);

    // 详细规划模板写入文件
    let plan_path = margi_dir.join("module-plan.md");
    std::fs::write(&plan_path,
        prompts::module_plan(&tree, &registered, &scan_root, scan_depth))?;

    eprintln!();
    eprintln!("{} {} {}", "✓".green(),
        t!("规划模板:", "Plan template:"), to_slash(&plan_path));
    eprintln!();
    eprintln!("{}", t!("快速注册命令：", "Quick register:"));
    eprintln!("  margi module add <{}> ", t!("源码路径", "source-path"));
    eprintln!("  margi module add {}   {}",
        ROOT_MODULE,
        t!("# 根目录散落文件", "# root-level files"));

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// split
// ─────────────────────────────────────────────────────────────────────────────

pub fn cmd_split(
    name: &str,
    depth: usize,
    margi_dir: &Path,
    project_root: &Path,
    _config: &Config,
) -> Result<()> {
    let key        = normalize_module_key(name, project_root);
    let source_dir = module_source_dir(project_root, &key);
    let doc_dir    = module_doc_dir(margi_dir, &key);

    eprintln!("{} '{}' ...", t!("→ 准备拆分模块", "→ Preparing split for"), key);

    let sub_tree = if source_dir.exists() {
        build_tree(&source_dir, &source_dir, depth + 1, 0)
    } else {
        t!("（源码目录不存在）\n", "(source directory not found)\n").to_string()
    };

    let doc_paths: Vec<String> = ["api.md", "internals.md", "notes.md"]
        .iter().filter_map(|f| {
            let p = doc_dir.join(f);
            if p.exists() { Some(to_slash(&p)) } else { None }
        }).collect();

    let notes_path = doc_dir.join("notes.md");
    let notes_hint = if notes_path.exists() { Some(notes_path.as_path()) } else { None };

    // stdout：子目录树
    println!("{}", sub_tree);

    // 详细任务写入文件
    let safe = key.replace('/', "_");
    let task_path = margi_dir.join(format!("split-{}.md", safe));
    std::fs::write(&task_path,
        prompts::module_split(&key, &source_dir, &sub_tree, &doc_paths, notes_hint))?;

    eprintln!();
    eprintln!("{} {} {}", "✓".green(),
        t!("拆分任务:", "Split task:"), to_slash(&task_path));
    eprintln!();
    eprintln!("{}", t!("注册子模块：", "Register submodules:"));
    eprintln!("  margi module add {}/{}", key,
        t!("<子目录名>", "<subdir>"));

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// merge
// ─────────────────────────────────────────────────────────────────────────────

pub fn cmd_merge(names: &[String], target: &str, margi_dir: &Path) -> Result<()> {
    if names.len() < 2 {
        return Err(MargiError::InvalidArgs(
            t!("合并至少需要两个模块", "Merge requires at least two source modules").to_string()
        ).into());
    }

    let source_infos: Vec<(String, String, Vec<String>)> = names.iter().map(|name| {
        let doc_dir = module_doc_dir(margi_dir, name);
        let status  = {
            let sf = doc_dir.join("STATUS");
            if sf.exists() {
                ModuleStatus::parse(&std::fs::read_to_string(&sf).unwrap_or_default())
                    .label().to_string()
            } else { "unknown".to_string() }
        };
        let docs: Vec<String> = ["api.md","internals.md","notes.md"]
            .iter().filter_map(|f| {
                let p = doc_dir.join(f);
                if p.exists() { Some(to_slash(&p)) } else { None }
            }).collect();
        (name.clone(), status, docs)
    }).collect();

    eprintln!("{} {} → '{}'", "→".cyan(),
        t!("准备合并:", "Preparing merge:"), target);
    for (n, s, _) in &source_infos {
        eprintln!("  + {} ({})", n, s);
    }

    // 确保目标模块目录存在（状态 unknown）
    let target_doc_dir = module_doc_dir(margi_dir, target);
    paths::ensure_dir(&target_doc_dir)?;
    if !target_doc_dir.join("STATUS").exists() {
        std::fs::write(target_doc_dir.join("STATUS"), "unknown")?;
    }

    let safe      = target.replace('/', "_");
    let task_path = margi_dir.join(format!("merge-{}.md", safe));
    std::fs::write(&task_path,
        prompts::module_merge(names, target, &source_infos))?;

    eprintln!();
    eprintln!("{} {} {}", "✓".green(),
        t!("合并任务:", "Merge task:"), to_slash(&task_path));
    eprintln!();
    eprintln!("{}", t!("文档合并完成后：", "After consolidating docs:"));
    eprintln!("  margi module set-status {} understood", target);

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 目录树构建（输出到 stdout）
// ─────────────────────────────────────────────────────────────────────────────

const SKIP: &[&str] = &[
    "node_modules","target","dist","build",".git",".svn",
    "__pycache__",".venv","venv",".next",".nuxt","coverage",
    "vendor","out",".output",
];

pub fn build_tree(base: &Path, current: &Path, max_depth: usize, cur_depth: usize) -> String {
    let mut out = String::new();

    if cur_depth == 0 {
        let name = current.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| to_slash(current));
        out.push_str(&format!("{}/\n", name));
    }
    if cur_depth >= max_depth { return out; }

    let Ok(entries) = std::fs::read_dir(current) else { return out };
    let mut dirs:  Vec<(String, PathBuf)> = vec![];
    let mut files: BTreeMap<String, usize> = BTreeMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || SKIP.contains(&name.as_str()) { continue; }
        if path.is_dir() {
            dirs.push((name, path));
        } else if path.is_file() {
            let ext = path.extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "~".into());
            *files.entry(ext).or_insert(0) += 1;
        }
    }
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let indent = "  ".repeat(cur_depth + 1);
    let total  = dirs.len() + if files.is_empty() { 0 } else { 1 };

    for (i, (name, path)) in dirs.iter().enumerate() {
        let is_last = i == total - 1 && files.is_empty();
        let prefix  = if is_last { "└── " } else { "├── " };
        let stats   = count_files_summary(&path, 2);
        let hint    = if stats.is_empty() { String::new() }
                      else { format!("  ({})", format_stats(&stats)) };
        out.push_str(&format!("{}{}{}/{}\n", indent, prefix, name, hint));
        out.push_str(&build_tree(base, &path, max_depth, cur_depth + 1));
    }

    if !files.is_empty() {
        out.push_str(&format!("{}└── [{}]\n", indent, format_stats(&files)));
    }

    out
}

fn count_files_summary(dir: &Path, max_depth: usize) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    count_files_rec(dir, max_depth, 0, &mut m);
    m
}

fn count_files_rec(dir: &Path, max: usize, depth: usize, m: &mut BTreeMap<String, usize>) {
    if depth > max { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p    = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || SKIP.contains(&name.as_str()) { continue; }
        if p.is_file() {
            let ext = p.extension().map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "~".into());
            *m.entry(ext).or_insert(0) += 1;
        } else if p.is_dir() {
            count_files_rec(&p, max, depth + 1, m);
        }
    }
}

fn format_stats(m: &BTreeMap<String, usize>) -> String {
    m.iter().map(|(ext, n)| format!("{} .{}", n, ext)).collect::<Vec<_>>().join(", ")
}
