pub mod analyzer;
pub mod planner;
pub mod status;

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;

use crate::{cli::ModuleCommands, config::Config, error::MargiError, paths};
use paths::{find_all_module_keys, module_doc_dir, module_source_dir, to_slash, ROOT_MODULE};
use status::ModuleStatus;

pub fn run(command: ModuleCommands) -> Result<()> {
    match command {
        ModuleCommands::Status                              => cmd_status(),
        ModuleCommands::Load { names, include_internals }  => cmd_load(&names, include_internals),
        ModuleCommands::Analyze { path, force }            => cmd_analyze(&path, force),
        ModuleCommands::SetStatus { name, status }         => cmd_set_status(&name, &status),
        ModuleCommands::List                               => cmd_list(),
        ModuleCommands::Add { path }                       => cmd_add(&path),
        ModuleCommands::Remove { name, archive, hard }     => cmd_remove(&name, archive, hard),
        ModuleCommands::Plan { depth, root }               => cmd_plan(depth, root.as_deref()),
        ModuleCommands::Split { name, depth }              => cmd_split(&name, depth),
        ModuleCommands::Merge { names, into }              => cmd_merge(&names, &into),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// status
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_status() -> Result<()> {
    let margi_dir = paths::margi_root()?;
    let modules   = find_all_module_keys(&margi_dir);

    if modules.is_empty() {
        crate::ui::warn(&t!("尚未注册任何模块，运行 `margi module plan` 开始规划",
                            "No modules registered. Run `margi module plan` to start."));
        return Ok(());
    }

    let mut rows    = Vec::new();
    let mut outdated = 0usize;

    for (key, doc_dir) in &modules {
        let sf     = doc_dir.join("STATUS");
        let status = if sf.exists() {
            ModuleStatus::parse(&std::fs::read_to_string(&sf)?)
        } else {
            ModuleStatus::Unknown
        };
        let (icon, label) = match &status {
            ModuleStatus::Unknown          => ("○", status.label_i18n().dimmed().to_string()),
            ModuleStatus::Analyzing { .. } => ("◐", status.label_i18n().yellow().to_string()),
            ModuleStatus::Partial { .. }   => ("◑", status.label_i18n().cyan().to_string()),
            ModuleStatus::Understood { .. }=> ("●", status.label_i18n().green().to_string()),
            ModuleStatus::Outdated { .. }  => { outdated += 1; ("!", status.label_i18n().red().to_string()) },
        };
        rows.push((icon.to_string(), key.clone(), label));
    }

    crate::ui::print_module_status_table(&rows, outdated);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// load（输出 markdown 文档内容供 AI 使用，框架用终端格式）
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_load(names: &[String], include_internals: bool) -> Result<()> {
    let margi_dir = paths::margi_root()?;

    for name in names {
        let key     = paths::normalize_module_key(name, &paths::project_root()?);
        let doc_dir = module_doc_dir(&margi_dir, &key);

        if !doc_dir.exists() {
            crate::ui::err(&MargiError::ModuleNotFound(key.clone()).localized());
            continue;
        }

        let sf     = doc_dir.join("STATUS");
        let status = if sf.exists() {
            ModuleStatus::parse(&std::fs::read_to_string(&sf)?)
        } else { ModuleStatus::Unknown };

        // 模块标题
        println!("{} {}", t!("模块:", "Module:").bold(), key.cyan().bold());

        if matches!(status, ModuleStatus::Unknown) {
            println!("{} {} margi module analyze {}",
                "→".yellow(),
                t!("尚未分析，运行:", "Not yet analyzed. Run:"),
                key);
            println!();
        } else if matches!(status, ModuleStatus::Outdated { .. }) {
            println!("{} {} margi module analyze {}",
                "→".yellow(),
                t!("文档可能已过时，建议重新运行:", "Docs may be outdated. Run:"),
                key);
            println!();
        }

        for (label, file) in &[
            (t!("对外接口", "Public API"),   "api.md"),
            (t!("注意事项", "Notes"),         "notes.md"),
        ] {
            let p = doc_dir.join(file);
            if p.exists() {
                // 小节标题用终端格式，内容是 md（供 AI 读取）
                println!("{} {} ({})", "──".dimmed(), label.bold(), file);
                println!();
                println!("{}", std::fs::read_to_string(&p)?);
                println!();
            }
        }

        if include_internals {
            let p = doc_dir.join("internals.md");
            if p.exists() {
                println!("{} {} (internals.md)", "──".dimmed(), t!("内部实现", "Internals").bold());
                println!();
                println!("{}", std::fs::read_to_string(&p)?);
                println!();
            }
        }

        let corrections = load_corrections_for_module(&margi_dir, &key);
        if !corrections.is_empty() {
            println!("{} {}", "──".dimmed(), t!("相关纠正记录", "Related Corrections").bold());
            println!();
            for c in &corrections { println!("  · {}", c); }
            println!();
        }

        if names.len() > 1 {
            println!("{}", "─".repeat(60).dimmed());
            println!();
        }
    }
    Ok(())
}

fn load_corrections_for_module(margi_dir: &std::path::Path, key: &str) -> Vec<String> {
    let dir = margi_dir.join("corrections");
    if !dir.exists() { return vec![]; }
    let mut relevant = vec![];
    let mut files: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten().collect();
    files.sort_by_key(|e| e.file_name());
    let short = key.split('/').last().unwrap_or(key);
    for entry in files.iter().rev().take(10) {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for block in content.split("\n## ") {
                let bl = block.to_lowercase();
                if bl.contains(&key.to_lowercase()) || bl.contains(&short.to_lowercase()) {
                    if let Some(line) = block.lines().next() {
                        relevant.push(line.trim_start_matches('[').to_string());
                    }
                }
            }
        }
    }
    relevant
}

// ─────────────────────────────────────────────────────────────────────────────
// analyze
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_analyze(path: &str, force: bool) -> Result<()> {
    let margi_dir    = paths::margi_root()?;
    let project_root = paths::project_root()?;
    let config       = Config::load(&margi_dir)?;
    analyzer::ModuleAnalyzer::new(project_root, margi_dir, &config)
        .analyze(path, force)
}

// ─────────────────────────────────────────────────────────────────────────────
// set-status
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_set_status(name: &str, status_str: &str) -> Result<()> {
    let margi_dir    = paths::margi_root()?;
    let project_root = paths::project_root()?;
    let key     = paths::normalize_module_key(name, &project_root);
    let doc_dir = module_doc_dir(&margi_dir, &key);

    if !doc_dir.exists() {
        crate::ui::err(&MargiError::ModuleNotFound(key.clone()).localized());
        return Ok(());
    }

    let now    = Utc::now();
    let status = match status_str {
        "unknown"    => ModuleStatus::Unknown,
        "partial"    => ModuleStatus::Partial    { since: now },
        "understood" => ModuleStatus::Understood { since: now },
        "outdated"   => ModuleStatus::Outdated   { since: now, reason: String::new() },
        other => {
            eprintln!("{} {}: {}. {}",
                "✗".red(),
                t!("未知状态", "Unknown status"), other,
                t!("可选: unknown / partial / understood / outdated",
                   "Valid: unknown / partial / understood / outdated"));
            return Ok(());
        }
    };

    std::fs::write(doc_dir.join("STATUS"), status.to_file_content())?;
    println!("{} {} '{}' → {}", "✓".green(),
        t!("模块", "Module"), key, status_str);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// list
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_list() -> Result<()> {
    let margi_dir = paths::margi_root()?;
    let modules   = find_all_module_keys(&margi_dir);

    if modules.is_empty() {
        crate::ui::warn(&t!("尚未注册任何模块。", "No modules registered."));
        return Ok(());
    }

    let rows: Vec<(String, Vec<String>)> = modules.iter()
        .map(|(key, doc_dir)| {
            let docs: Vec<String> = ["api.md", "internals.md", "notes.md", "STATUS"]
                .iter()
                .filter(|f| doc_dir.join(f).exists())
                .map(|f| f.to_string())
                .collect();
            (key.clone(), docs)
        })
        .collect();

    crate::ui::print_module_list(&rows);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// add
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_add(path: &str) -> Result<()> {
    let margi_dir    = paths::margi_root()?;
    let project_root = paths::project_root()?;
    let key          = paths::normalize_module_key(path, &project_root);
    let doc_dir      = module_doc_dir(&margi_dir, &key);

    if key != ROOT_MODULE {
        let source_dir = module_source_dir(&project_root, &key);
        if !source_dir.exists() {
            eprintln!("{} {} '{}'",
                "✗".red(),
                t!("源码路径不存在:", "Source path does not exist:"),
                to_slash(&source_dir));
            return Ok(());
        }
    }

    if doc_dir.join("STATUS").exists() {
        println!("{} {} '{}'", "!".yellow(),
            t!("模块已存在:", "Module already exists:"), key);
        return Ok(());
    }

    paths::ensure_dir(&doc_dir)?;
    std::fs::write(doc_dir.join("STATUS"), "unknown")?;

    println!("{} {} '{}'", "✓".green(),
        t!("已注册模块", "Module registered:"), key);
    println!("  {} {}", t!("文档目录:", "Docs:"), to_slash(&doc_dir));
    println!("  {}",
        t!("运行 `margi module analyze <key>` 生成文档",
           "Run `margi module analyze <key>` to generate docs"));
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// remove
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_remove(name: &str, archive: bool, hard: bool) -> Result<()> {
    let margi_dir    = paths::margi_root()?;
    let project_root = paths::project_root()?;
    let key          = paths::normalize_module_key(name, &project_root);
    let doc_dir      = module_doc_dir(&margi_dir, &key);

    if !doc_dir.exists() {
        crate::ui::err(&MargiError::ModuleNotFound(key.clone()).localized());
        return Ok(());
    }

    if hard {
        std::fs::remove_dir_all(&doc_dir)?;
        println!("{} {} '{}'", "✓".green(),
            t!("已彻底删除模块", "Module permanently removed:"), key);
    } else if archive {
        let archive_dir = margi_dir.join("archive");
        let dest = {
            let mut p = archive_dir.clone();
            for seg in key.split('/') { p = p.join(seg); }
            p
        };
        paths::ensure_dir(&archive_dir)?;
        if dest.exists() { std::fs::remove_dir_all(&dest)?; }
        std::fs::rename(&doc_dir, &dest)?;
        println!("{} '{}' → {}", t!("✓ 已归档模块", "✓ Module archived:"),
            key, to_slash(&dest));
    } else {
        for doc in &["api.md", "internals.md", "notes.md"] {
            let p = doc_dir.join(doc);
            if p.exists() { std::fs::remove_file(&p)?; }
        }
        std::fs::write(doc_dir.join("STATUS"), "unknown")?;
        println!("{} '{}' {}", "✓".green(), key,
            t!("文档已清除，状态重置为 unknown（目录保留）",
               "docs cleared, status reset to unknown (directory kept)"));
        println!("  {} margi module add {}", t!("重新注册:", "Re-register:"), key);
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// plan / split / merge
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_plan(depth: Option<usize>, root: Option<&str>) -> Result<()> {
    let margi_dir    = paths::margi_root()?;
    let project_root = paths::project_root()?;
    let config       = Config::load(&margi_dir)?;
    planner::cmd_plan(depth, root, &margi_dir, &project_root, &config)
}

fn cmd_split(name: &str, depth: usize) -> Result<()> {
    let margi_dir    = paths::margi_root()?;
    let project_root = paths::project_root()?;
    let config       = Config::load(&margi_dir)?;
    planner::cmd_split(name, depth, &margi_dir, &project_root, &config)
}

fn cmd_merge(names: &[String], target: &str) -> Result<()> {
    let margi_dir = paths::margi_root()?;
    planner::cmd_merge(names, target, &margi_dir)
}
