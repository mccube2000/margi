use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use git2::Repository;
use std::path::{Path, PathBuf};

use crate::{module::status::ModuleStatus, paths};
use paths::{find_all_module_keys, rel_slash, to_slash, ROOT_MODULE};

pub fn run(staged: bool) -> Result<()> {
    let project_root = paths::project_root()?;
    let margi_dir    = paths::margi_root()?;

    let changed_files = get_changed_files(&project_root, staged)?;
    if changed_files.is_empty() {
        println!("{} {}", "✓".green(),
            t!("没有检测到代码变更", "No code changes detected."));
        return Ok(());
    }

    println!("{}", t!("变更分析", "Change Analysis").bold().underline());
    println!();
    println!("{} {} {}:",
        t!("检测到", "Detected"), changed_files.len(),
        t!("个文件变更", "file change(s)"));
    for f in &changed_files {
        println!("  · {}", to_slash(f));
    }
    println!();

    // 把变更文件映射到模块：使用路径前缀匹配
    let all_modules = find_all_module_keys(&margi_dir);
    let mut outdated_count = 0usize;

    // 对每个注册模块，检查是否有变更文件属于它
    for (key, doc_dir) in &all_modules {
        let module_src = paths::module_source_dir(&project_root, key);

        let matched: Vec<&PathBuf> = changed_files.iter().filter(|f| {
            if key == ROOT_MODULE {
                // root 模块：只匹配直接在项目根的文件
                f.parent().map(|p| p == project_root).unwrap_or(false)
            } else {
                // 其他模块：文件路径以模块源码目录为前缀
                f.starts_with(&module_src)
            }
        }).collect();

        if matched.is_empty() { continue; }

        let sf     = doc_dir.join("STATUS");
        let status = if sf.exists() {
            ModuleStatus::parse(&std::fs::read_to_string(&sf)?)
        } else { ModuleStatus::Unknown };

        if matches!(status, ModuleStatus::Understood { .. } | ModuleStatus::Partial { .. }) {
            let reason = format!("{} {}: {}",
                matched.len(),
                t!("个文件变更", "file(s) changed"),
                matched.iter().take(3)
                    .map(|f| f.file_name().unwrap_or_default().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(", "));

            if doc_dir.exists() {
                std::fs::write(&sf,
                    ModuleStatus::Outdated {
                        since: Utc::now(), reason: reason.clone(),
                    }.to_file_content())?;
            }

            println!("  {} {}  → {} ({})",
                "!".red(), key.bold(),
                t!("outdated", "outdated").red(), reason);
            println!("    margi module analyze {}", key);
            outdated_count += 1;
        } else {
            println!("  {} {} — {} ({})",
                "·".dimmed(), key,
                status.label(),
                t!("无需更新文档", "no doc update needed"));
        }
    }

    // 未匹配到任何模块的文件
    for file in &changed_files {
        let rel   = rel_slash(file, &project_root);
        let any   = all_modules.iter().any(|(key, _)| {
            let src = paths::module_source_dir(&project_root, key);
            if key == ROOT_MODULE {
                file.parent().map(|p| p == project_root).unwrap_or(false)
            } else {
                file.starts_with(&src)
            }
        });
        if !any {
            println!("  {} {} — {}",
                "✓".green().dimmed(), rel,
                t!("无关联模块", "no linked module"));
        }
    }

    println!();
    if outdated_count > 0 {
        println!("{} {} {}", "!".yellow(), outdated_count,
            t!("个模块文档已过时", "module doc(s) marked outdated"));
    } else {
        println!("{} {}", "✓".green(),
            t!("所有模块文档均为最新", "All module docs are up to date."));
    }
    Ok(())
}

fn get_changed_files(project_root: &Path, staged: bool) -> Result<Vec<PathBuf>> {
    let repo = match Repository::discover(project_root) {
        Ok(r)  => r,
        Err(e) => {
            crate::ui::warn(&crate::error::MargiError::GitError(
                t!(format!("不是 git 仓库或无法访问：{}，跳过变更检测", e),
                   format!("Not a git repository or inaccessible: {}, skipping.", e))
            ).localized());
            return Ok(vec![]);
        }
    };

    let mut files: Vec<PathBuf> = vec![];

    macro_rules! collect {
        ($diff:expr) => {
            $diff.foreach(
                &mut |delta, _| {
                    if let Some(p) = delta.new_file().path() {
                        files.push(project_root.join(p));
                    }
                    true
                },
                None, None, None,
            )?;
        };
    }

    if staged {
        if let Ok(head) = repo.head().and_then(|h| h.peel_to_tree()) {
            let diff = repo.diff_tree_to_index(Some(&head), None, None)?;
            collect!(diff);
        }
    } else {
        let workdir_diff = repo.diff_index_to_workdir(None, None)?;
        collect!(workdir_diff);

        if let Ok(head) = repo.head().and_then(|h| h.peel_to_tree()) {
            if let Ok(staged_diff) = repo.diff_tree_to_index(Some(&head), None, None) {
                staged_diff.foreach(
                    &mut |delta, _| {
                        if let Some(p) = delta.new_file().path() {
                            let full = project_root.join(p);
                            if !files.contains(&full) { files.push(full); }
                        }
                        true
                    },
                    None, None, None,
                )?;
            }
        }
    }
    Ok(files)
}
