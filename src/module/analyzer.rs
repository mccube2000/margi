use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::{config::Config, paths, prompts};
use paths::{module_doc_dir, module_source_dir, normalize_module_key, to_slash};
use super::status::ModuleStatus;

pub struct ModuleAnalyzer<'a> {
    pub project_root: PathBuf,
    pub margi_dir:    PathBuf,
    pub config:       &'a Config,
}

impl<'a> ModuleAnalyzer<'a> {
    pub fn new(project_root: PathBuf, margi_dir: PathBuf, config: &'a Config) -> Self {
        Self { project_root, margi_dir, config }
    }

    pub fn analyze(&self, path_or_key: &str, force: bool) -> Result<()> {
        let key        = normalize_module_key(path_or_key, &self.project_root);
        let doc_dir    = module_doc_dir(&self.margi_dir, &key);
        let source_dir = module_source_dir(&self.project_root, &key);

        paths::ensure_dir(&doc_dir)?;

        let sf     = doc_dir.join("STATUS");
        let status = if sf.exists() {
            ModuleStatus::parse(&std::fs::read_to_string(&sf)?)
        } else {
            ModuleStatus::Unknown
        };

        if matches!(status, ModuleStatus::Understood { .. }) && !force {
            eprintln!("{} {}", "!".yellow(), t!(
                format!("模块 '{}' 已是 understood，使用 --force 重新分析", key),
                format!("Module '{}' is already understood. Use --force to re-analyze.", key)
            ));
            return Ok(());
        }

        let source_files = self.collect_source_files(&source_dir);

        eprintln!("{} '{}' ...", t!("→ 准备模块", "→ Preparing module"), key);
        eprintln!("  {} {}", t!("源码目录:", "Source dir:"), to_slash(&source_dir));
        eprintln!("  {} {}", t!("源文件数:", "Files:"), source_files.len());

        let existing_notes = read_file(&doc_dir.join("notes.md"));
        let existing_api   = read_file(&doc_dir.join("api.md"));
        let corrections    = self.read_corrections(&key);

        std::fs::write(&sf,
            ModuleStatus::Analyzing { since: Utc::now() }.to_file_content())?;

        print!("{}", prompts::module_analyze(
            &key,
            &doc_dir,
            &source_dir,
            &source_files,
            &self.project_root,
            &existing_notes,
            &existing_api,
            &corrections,
        ));

        let sep = "─".repeat(60);
        eprintln!();
        eprintln!("{}", sep.dimmed());
        eprintln!("{}", t!("文档写入路径:", "Write docs to:"));
        eprintln!("  {}", to_slash(&doc_dir.join("api.md")));
        eprintln!("  {}", to_slash(&doc_dir.join("internals.md")));
        eprintln!("  {}", to_slash(&doc_dir.join("notes.md")));
        eprintln!();
        eprintln!("{}", t!("完成后更新状态:", "Update status when done:"));
        eprintln!("  margi module set-status {} understood", key);
        eprintln!("{}", sep.dimmed());

        Ok(())
    }

    fn collect_source_files(&self, dir: &Path) -> Vec<PathBuf> {
        // _root_ 模块：只取项目根目录下的直接文件，不递归
        if dir == self.project_root {
            let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
            let mut files: Vec<PathBuf> = entries.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.is_file() && !self.config.is_excluded(&p) && is_source_file(&p) {
                        Some(p)
                    } else { None }
                })
                .collect();
            files.sort();
            return files;
        }

        if !dir.exists() { return vec![]; }
        let mut files: Vec<PathBuf> = WalkDir::new(dir)
            .into_iter().flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| !self.config.is_excluded(p) && is_source_file(p))
            .collect();
        files.sort();
        files
    }

    fn read_corrections(&self, key: &str) -> String {
        let dir = self.margi_dir.join("corrections");
        if !dir.exists() { return String::new(); }
        let short = key.split('/').last().unwrap_or(key);
        let mut relevant = vec![];
        let mut files: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten()
            .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
            .collect();
        files.sort_by_key(|e| e.file_name());
        for entry in files.iter().rev().take(30) {
            if let Ok(c) = std::fs::read_to_string(entry.path()) {
                for block in c.split("\n## ") {
                    let bl = block.to_lowercase();
                    if bl.contains(&key.to_lowercase()) || bl.contains(&short.to_lowercase()) {
                        relevant.push(format!("## {}", block));
                    }
                }
            }
        }
        relevant.join("\n")
    }
}

fn read_file(path: &Path) -> String {
    if path.exists() { std::fs::read_to_string(path).unwrap_or_default() } else { String::new() }
}

fn is_source_file(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    matches!(ext.as_deref(),
        Some("rs"|"ts"|"tsx"|"js"|"jsx"|"mjs"|"cjs"
            |"vue"|"svelte"
            |"py"|"pyi"|"go"
            |"java"|"kt"|"kts"|"swift"
            |"cpp"|"cc"|"cxx"|"c"|"h"|"hpp"
            |"cs"|"rb"|"php"|"scala"|"lua"
            |"sh"|"bash"))
}
