use anyhow::Result;
use colored::Colorize;
use std::path::Path;
use walkdir::WalkDir;

use crate::{config::Config, paths, prompts};

pub fn run(force: bool) -> Result<()> {
    let project_root = paths::project_root()?;
    let margi_dir    = project_root.join(".margi");

    if margi_dir.exists() && !force {
        println!("{} {}", "!".yellow(),
            t!(".margi/ 已存在，使用 --force 重新初始化",
               ".margi/ already exists. Use --force to reinitialize."));
        return Ok(());
    }

    crate::ui::info(&t!("初始化...", "Initializing..."));
    println!("  {} {}", t!("项目根目录:", "Project root:"), project_root.display());

    // ── 1. 目录结构 ──────────────────────────────────────────────────────────
    for dir in &[
        margi_dir.clone(),
        margi_dir.join("modules"),
        margi_dir.join("corrections"),
        margi_dir.join(".index"),
    ] {
        std::fs::create_dir_all(dir)?;
        println!("  {} {}", "✓".green(),
            dir.strip_prefix(&project_root)
               .map(|p| paths::to_slash(p))
               .unwrap_or_else(|_| paths::to_slash(dir)));
    }

    // ── 2. 配置文件 ───────────────────────────────────────────────────────────
    let config = Config::default();
    config.save(&margi_dir)?;
    crate::ui::ok(".margi/config.json");

    // ── 3. 扫描项目 ───────────────────────────────────────────────────────────
    println!();
    crate::ui::info(&t!("扫描项目结构...", "Scanning project structure..."));
    let summary = scan_project(&project_root, &config)?;

    // ── 4. 占位文档 ───────────────────────────────────────────────────────────
    write_scaffold_docs(&margi_dir, &project_root)?;
    crate::ui::ok(".margi/env.md");
    crate::ui::ok(".margi/memory.md");

    // ── 5. .margi/README.md ───────────────────────────────────────────────────
    let project_name = project_root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    std::fs::write(margi_dir.join("README.md"), prompts::readme_md(&project_name))?;
    crate::ui::ok(".margi/README.md");

    // ── 6. AGENTS.md ─────────────────────────────────────────────────────────
    let agents_path    = project_root.join("AGENTS.md");
    let agents_existed = agents_path.exists();
    if agents_existed {
        // 已存在：不覆盖，终端提示用户追加 margi 引导段
        println!("  {} AGENTS.md {}",
            "!".yellow(),
            t!("已存在，跳过（见下方提示）", "already exists, skipped (see hint below)"));
    } else {
        std::fs::write(&agents_path, prompts::agents_md())?;
        crate::ui::ok("AGENTS.md");
    }

    // ── 7. .gitignore ─────────────────────────────────────────────────────────
    update_gitignore(&project_root)?;

    // ── 8. 终端引导（一次性，简洁）───────────────────────────────────────────
    println!();
    crate::ui::ok(&t!("初始化完成", "Initialization complete"));
    print_next_steps(&summary, agents_existed);

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 项目扫描
// ─────────────────────────────────────────────────────────────────────────────

pub struct ProjectSummary {
    pub file_count:  usize,
    pub languages:   Vec<String>,
    pub entry_files: Vec<String>,
}

pub fn scan_project(root: &Path, config: &Config) -> Result<ProjectSummary> {
    let mut file_count = 0usize;
    let mut lang_counts: std::collections::HashMap<String, usize> = Default::default();
    let mut entry_files = vec![];

    for entry in WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !n.starts_with('.') && !matches!(n.as_ref(),
                "node_modules"|"target"|"dist"|"build"|"vendor")
        })
        .flatten()
    {
        let path = entry.path();
        if config.is_excluded(path) { continue; }
        if path.is_file() {
            file_count += 1;
            if let Some(ext) = path.extension() {
                *lang_counts.entry(ext.to_string_lossy().to_string()).or_insert(0) += 1;
            }
            let name = entry.file_name().to_string_lossy();
            if matches!(name.as_ref(),
                "main.rs"|"lib.rs"|"index.ts"|"index.js"|"main.py"|"app.py"
                |"main.go"|"App.vue"|"App.tsx"|"server.ts"|"server.js"
                |"main.kt"|"Main.java"|"vite.config.ts"|"vite.config.js"
            ) {
                if let Ok(rel) = path.strip_prefix(root) {
                    entry_files.push(paths::to_slash(rel));
                }
            }
        }
    }

    let mut languages: Vec<String> = lang_counts.into_iter()
        .filter(|(_, v)| *v > 2)
        .map(|(k, _)| k)
        .collect();
    languages.sort();

    println!("  {} {}", t!("文件数:", "Files:"), file_count);
    println!("  {} {}", t!("语言:", "Languages:"), languages.join(", "));
    if !entry_files.is_empty() {
        println!("  {} {}", t!("入口:", "Entry:"), entry_files.join(", "));
    }

    Ok(ProjectSummary { file_count, languages, entry_files })
}

// ─────────────────────────────────────────────────────────────────────────────
// 占位文档
// ─────────────────────────────────────────────────────────────────────────────

fn write_scaffold_docs(margi_dir: &Path, project_root: &Path) -> Result<()> {
    let project_name = project_root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    std::fs::write(margi_dir.join("env.md"), format!(
        "# {heading} — {project_name}\n\n\
         > {note}\n\n\
         ## {s1}\n\n```bash\n# TODO\n```\n\n\
         ## {s2}\n\n```bash\n# TODO\n```\n\n\
         ## {s3}\n\n```bash\n# TODO\n```\n\n\
         ## {s4}\n\nTODO\n\n## {s5}\n\nTODO\n",
        heading = t!("环境 & 构建", "Environment & Build"),
        note    = t!("由 `margi init` 生成骨架，请补充实际内容",
                     "Scaffold generated by `margi init`. Please fill in."),
        s1 = t!("开发环境启动", "Dev Setup"),
        s2 = t!("构建 & 打包",  "Build & Package"),
        s3 = t!("测试执行",      "Testing"),
        s4 = t!("部署流程",      "Deployment"),
        s5 = t!("常见问题",      "FAQ"),
    ))?;

    std::fs::write(margi_dir.join("memory.md"), format!(
        "# {heading} — {project_name}\n\n\
         > {note}\n\n\
         ## {s1}\n\nTODO\n\n\
         ## {s2}\n\nTODO\n\n\
         ## {s3}\n\nTODO\n",
        heading = t!("项目全局记忆", "Project Memory"),
        note    = t!("高优先级注意事项会自动同步到 AGENTS.md",
                     "High-priority notes are auto-synced to AGENTS.md."),
        s1 = t!("全局注意事项",  "Global Notes"),
        s2 = t!("模块概览",      "Module Overview"),
        s3 = t!("重要架构决策",  "Key Architecture Decisions"),
    ))?;

    Ok(())
}

fn update_gitignore(project_root: &Path) -> Result<()> {
    let path  = project_root.join(".gitignore");
    let entry = ".margi/.index/\n";
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if !content.contains(".margi/.index") {
            std::fs::write(&path,
                format!("{}\n# margi index cache\n{}", content.trim_end(), entry))?;
            crate::ui::ok(".gitignore");
        }
    } else {
        std::fs::write(&path, format!("# margi index cache\n{}", entry))?;
        println!("  {} .gitignore", "✓".green());
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 终端引导（init 后一次性显示，简洁）
// ─────────────────────────────────────────────────────────────────────────────

fn print_next_steps(summary: &ProjectSummary, agents_exists: bool) {
    let sep = "─".repeat(60);
    println!();
    println!("{}", sep.dimmed());
    println!();

    // ── 立即执行的任务（一次性，直接输出任务描述）──────────────────────────
    println!("{}", t!("立即执行：生成项目文档", "Next: generate project docs").bold());
    println!();

    // env.md 任务描述
    println!("  {} {}", "1".bold().dimmed(),
        t!("生成环境文档 (.margi/env.md)", "Generate env doc (.margi/env.md)"));
    println!();
    let env_prompt = prompts::env_md_task(summary.file_count, &summary.languages, &summary.entry_files);
    for line in env_prompt.lines() {
        println!("     {}", line);
    }
    println!();

    // memory.md 任务描述
    println!("  {} {}", "2".bold().dimmed(),
        t!("生成项目记忆文档 (.margi/memory.md)", "Generate memory doc (.margi/memory.md)"));
    println!();
    let mem_prompt = prompts::memory_md_task(&summary.languages);
    for line in mem_prompt.lines() {
        println!("     {}", line);
    }
    println!();

    // ── AGENTS.md 已存在时的追加提示 ─────────────────────────────────────────
    if agents_exists {
        println!("{}", sep.dimmed());
        println!();
        println!("  {} {}",
            "!".yellow(),
            t!("AGENTS.md 已存在，请手动在文件中添加以下内容：",
               "AGENTS.md already exists. Please append this to the file:"));
        println!();
        let snippet = prompts::agents_md_snippet();
        for line in snippet.lines() {
            println!("     {}", line);
        }
        println!();
    }

    // ── 后续步骤 ─────────────────────────────────────────────────────────────
    println!("{}", sep.dimmed());
    println!();
    println!("{}", t!("后续步骤", "Next steps").bold());
    println!();

    let steps_zh = [
        ("margi module plan",                "规划模块结构"),
        ("margi module add <源码路径>",      "注册模块"),
        ("margi module analyze <key>",       "生成模块文档（输出任务描述到 stdout）"),
        ("margi index build",                "构建搜索索引"),
        ("margi search \"<关键词>\"",        "搜索源码"),
    ];
    let steps_en = [
        ("margi module plan",                "plan module structure"),
        ("margi module add <path>",          "register a module"),
        ("margi module analyze <key>",       "generate module docs (outputs task description to stdout)"),
        ("margi index build",                "build search index"),
        ("margi search \"<keyword>\"",       "search source code"),
    ];

    let steps = t!(steps_zh, steps_en);
    for (i, (cmd, desc)) in steps.iter().enumerate() {
        println!("  {}  {}  {}",
            format!("{}", i + 1).bold().dimmed(),
            cmd.cyan(),
            desc.dimmed());
    }

    println!();
    println!("  {}", t!(
        "完整功能说明见 .margi/README.md",
        "Full reference: .margi/README.md"
    ).dimmed());
    println!();
    println!("{}", sep.dimmed());
    println!();
}
