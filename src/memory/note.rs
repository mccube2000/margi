use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use std::path::Path;

use crate::paths;

pub fn add(content: &str, module: Option<&str>, tags: &[String]) -> Result<()> {
    let margi_dir = paths::margi_root()?;
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M");

    let tags_str = if tags.is_empty() { String::new() } else {
        format!("\n**{}**: {}", t!("标签", "Tags"),
            tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "))
    };

    let entry = format!(
        "\n## [{} UTC]{}\n\n{}{}\n",
        timestamp,
        module.map(|m| format!("  {}: {}", t!("模块", "Module"), m)).unwrap_or_default(),
        content,
        tags_str
    );

    match module {
        Some(mod_name) => {
            let mod_dir = margi_dir.join("modules").join(mod_name);
            if !mod_dir.exists() { std::fs::create_dir_all(&mod_dir)?; }
            append_or_create_pub(
                &mod_dir.join("notes.md"),
                &format!("# {} — {}\n", t!("注意事项", "Notes"), mod_name),
                &entry,
            )?;
            println!("{} {} '{}' → notes.md",
                "✓".green(),
                t!("已添加到模块", "Added to module"), mod_name);
        }
        None => {
            append_or_create_pub(
                &margi_dir.join("memory.md"),
                &format!("# {}\n", t!("项目全局记忆", "Project Memory")),
                &entry,
            )?;
            println!("{} {}", "✓".green(),
                t!("全局注意事项已添加", "Global note added"));
            sync_to_agents_md(&margi_dir)?;
        }
    }
    Ok(())
}

pub fn list(module: Option<&str>, tag: Option<&str>) -> Result<()> {
    let margi_dir = paths::margi_root()?;
    crate::ui::title(&t!("注意事项", "Notes"));
    println!();

    match module {
        Some(mod_name) => {
            let notes_path = margi_dir.join("modules").join(mod_name).join("notes.md");
            if !notes_path.exists() {
                println!("  {} '{}'", t!("模块暂无注意事项:", "No notes for module:"), mod_name);
                return Ok(());
            }
            let content = std::fs::read_to_string(&notes_path)?;
            println!("  {} {}", t!("模块:", "Module:").bold(), mod_name.cyan());
            println!("{}", filter_by_tag(&content, tag));
        }
        None => {
            let mem_path = margi_dir.join("memory.md");
            if mem_path.exists() {
                let content = std::fs::read_to_string(&mem_path)?;
                println!("  {}", t!("全局注意事项", "Global Notes").bold());
                println!("{}", filter_by_tag(&content, tag));
                println!();
            }
            if tag.is_none() {
                let modules_dir = margi_dir.join("modules");
                if modules_dir.exists() {
                    let mut entries: Vec<_> = std::fs::read_dir(&modules_dir)?.flatten().collect();
                    entries.sort_by_key(|e| e.file_name());
                    for entry in &entries {
                        let notes_path = entry.path().join("notes.md");
                        if notes_path.exists() {
                            let name    = entry.file_name().to_string_lossy().to_string();
                            let content = std::fs::read_to_string(&notes_path)?;
                            if content.trim().is_empty() { continue; }
                            println!("  {} {}", t!("模块:", "Module:").bold(), name.cyan());
                            let preview: String =
                                content.lines().take(10).collect::<Vec<_>>().join("\n");
                            println!("{}", preview);
                            if content.lines().count() > 10 { println!("  ..."); }
                            println!();
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn filter_by_tag(content: &str, tag: Option<&str>) -> String {
    let Some(t) = tag else { return content.to_string(); };
    let mut result = vec![];
    let mut in_matching = false;
    let mut block: Vec<String> = vec![];
    for line in content.lines() {
        if line.starts_with("## ") {
            if in_matching && !block.is_empty() {
                result.extend_from_slice(&block);
                result.push(String::new());
            }
            block.clear();
            in_matching = false;
            block.push(line.to_string());
        } else {
            block.push(line.to_string());
            if line.contains(&format!("#{}", t)) { in_matching = true; }
        }
    }
    if in_matching { result.extend_from_slice(&block); }
    result.join("\n")
}

pub fn append_or_create_pub(path: &Path, header: &str, entry: &str) -> Result<()> {
    if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        std::fs::write(path, format!("{}{}", existing, entry))?;
    } else {
        std::fs::write(path, format!("{}{}", header, entry))?;
    }
    Ok(())
}

fn sync_to_agents_md(margi_dir: &Path) -> Result<()> {
    let memory_path = margi_dir.join("memory.md");
    let agents_path = match margi_dir.parent() {
        Some(p) => p.join("AGENTS.md"),
        None    => return Ok(()),
    };
    if !agents_path.exists() || !memory_path.exists() { return Ok(()); }

    let memory_content  = std::fs::read_to_string(&memory_path)?;
    let agents_content  = std::fs::read_to_string(&agents_path)?;

    let notes: Vec<&str> = memory_content.split("\n## ").skip(1).take(10).collect();
    if notes.is_empty() { return Ok(()); }

    let summary = notes.iter()
        .filter_map(|block| {
            let line = block.lines().next()?;
            Some(format!("- {}", line.trim_start_matches('[').trim_end_matches(']')))
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 寻找全局注意事项区域并替换（匹配新旧两种 AGENTS.md 格式）
    let markers = [
        ("## 全局注意事项", "## 环境 & 构建"),
        ("## 全局注意事项", "## 环境"),
        ("## Global Notes",  "## Environment"),
    ];
    for (start_marker, end_marker) in &markers {
        if let (Some(s), Some(e)) = (agents_content.find(start_marker), agents_content.find(end_marker)) {
            if s >= e { continue; }
            let before = &agents_content[..s + start_marker.len()];
            let after  = &agents_content[e..];
            std::fs::write(&agents_path,
                format!("{}\n\n{}\n\n{}", before, summary, after))?;
            return Ok(());
        }
    }
    Ok(())
}
