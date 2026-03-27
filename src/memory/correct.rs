use anyhow::Result;
use chrono::{NaiveDate, Utc};
use colored::Colorize;
use std::cmp::Reverse;

use crate::paths;

pub fn add(description: &str, module: Option<&str>, tags: &[String]) -> Result<()> {
    let margi_dir      = paths::margi_root()?;
    let corrections_dir = margi_dir.join("corrections");
    paths::ensure_dir(&corrections_dir)?;

    let now      = Utc::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let time_str = now.format("%Y-%m-%d %H:%M").to_string();
    let file_path = corrections_dir.join(format!("{}.md", date_str));

    let tags_str = if tags.is_empty() { String::new() } else {
        format!("\n**{}**: {}\n",
            t!("标签", "Tags"),
            tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "))
    };

    let module_str = module
        .map(|m| format!("  {}: {}", t!("模块", "Module"), m))
        .unwrap_or_default();

    let entry = format!(
        "\n## [{} UTC]{}\n\n**{}**: {}{}\n",
        time_str, module_str,
        t!("问题", "Issue"),
        description, tags_str
    );

    let header = format!("# {} {}\n", t!("纠正记录", "Corrections"), date_str);
    crate::memory::note::append_or_create_pub(&file_path, &header, &entry)?;

    // 同时追加到模块 notes.md
    if let Some(mod_name) = module {
        let mod_dir = margi_dir.join("modules").join(mod_name);
        if mod_dir.exists() {
            let note_entry = format!(
                "\n## [{} {} {}]\n\n{}\n",
                t!("纠正", "Correction"), date_str, mod_name,
                description
            );
            crate::memory::note::append_or_create_pub(
                &mod_dir.join("notes.md"),
                &format!("# {} — {}\n", t!("注意事项", "Notes"), mod_name),
                &note_entry,
            )?;
        }
    }

    println!("{} {} corrections/{}.md",
        "✓".green(),
        t!("纠正记录已保存:", "Correction saved to"), date_str);
    if let Some(m) = module {
        println!("  {} '{}' notes.md",
            t!("同时追加到模块", "Also appended to module"), m);
    }
    Ok(())
}

pub fn list(since: Option<&str>, module_filter: Option<&str>) -> Result<()> {
    let margi_dir       = paths::margi_root()?;
    let corrections_dir = margi_dir.join("corrections");

    if !corrections_dir.exists() {
        println!("{}", t!("尚无纠正记录", "No corrections yet."));
        return Ok(());
    }

    let since_date = since.and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let mut files: Vec<_> = std::fs::read_dir(&corrections_dir)?
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    files.sort_by_key(|e| Reverse(e.file_name()));

    crate::ui::title(&t!("纠正记录", "Corrections"));
    println!();

    let mut total = 0usize;
    for entry in &files {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let date_str  = file_name.trim_end_matches(".md");

        if let Some(since) = since_date {
            if let Ok(fd) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                if fd < since { continue; }
            }
        }

        let content = std::fs::read_to_string(entry.path())?;
        let output  = if let Some(m) = module_filter {
            let relevant: Vec<&str> = content.split("\n## ")
                .filter(|b| b.to_lowercase().contains(&m.to_lowercase()))
                .collect();
            if relevant.is_empty() { continue; }
            format!("# {} {}\n\n## {}",
                t!("纠正记录", "Corrections"), date_str,
                relevant.join("\n\n## "))
        } else {
            content
        };

        println!("{}", output);
        total += 1;
    }

    if total == 0 {
        println!("{}", t!("没有符合条件的记录", "No matching records found."));
    }
    Ok(())
}
