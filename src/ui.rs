//! 终端输出格式化
//!
//! 面向人类阅读的终端输出统一在此处理。
//! 原则：不使用 Markdown 符号（#、##、---），用颜色和缩进区分层次。
//! Markdown 格式只用于写入磁盘的文档文件。

#![allow(dead_code)]

use colored::Colorize;

// ─────────────────────────────────────────────────────────────────────────────
// 标题与分隔
// ─────────────────────────────────────────────────────────────────────────────

/// 打印页级标题，附带下划线
pub fn title(text: &str) {
    let line = "─".repeat(text.chars().count().min(56));
    println!("{}", text.bold());
    println!("{}", line.dimmed());
}

/// 打印节标题（无下划线，用于子节）
pub fn section(label: &str) {
    println!();
    println!("{}", label.bold());
}

/// 打印视觉分隔线
pub fn divider() {
    println!("{}", "─".repeat(60).dimmed());
}

// ─────────────────────────────────────────────────────────────────────────────
// 状态行
// ─────────────────────────────────────────────────────────────────────────────

pub fn ok(msg: &str) {
    println!("{} {}", "✓".green(), msg);
}

pub fn info(msg: &str) {
    println!("{} {}", "→".cyan(), msg);
}

pub fn warn(msg: &str) {
    println!("{} {}", "!".yellow(), msg);
}

pub fn err(msg: &str) {
    eprintln!("{} {}", "✗".red(), msg);
}

pub fn step_hint(cmd: &str) {
    println!("  {}", cmd.cyan());
}

// ─────────────────────────────────────────────────────────────────────────────
// 搜索结果展示
// ─────────────────────────────────────────────────────────────────────────────

pub fn print_source_results(query: &str, results: &[super::search::searcher::SearchResult]) {
    if results.is_empty() {
        println!("{} {}", "→".dimmed(),
            t!(format!("未找到「{}」相关结果", query),
               format!("No results for \"{}\"", query)));
        return;
    }

    title(&t!(
        format!("源码搜索  \"{}\"  ({} 条)", query, results.len()),
        format!("Source Search  \"{}\"  ({} results)", query, results.len())
    ));

    for r in results {
        println!();
        // 序号 + 文件位置（高亮文件名部分）
        let file = &r.file_path;
        let loc  = format!("{}:{}", file, r.start_line);
        print!("  {}  {}", format!("{:2}", r.rank).bold(), loc.cyan());

        if let Some(sym) = &r.symbol_name {
            print!("  {}", sym.yellow());
        }
        println!();

        // 元信息行（相关度、模块）
        print!("     {}: {:.4}", t!("相关度", "score"), r.score);
        if !r.module.is_empty() {
            print!("  {}: {}", t!("模块", "module"), r.module.dimmed());
        }
        println!();

        // 代码预览（带行号）
        for (i, line) in r.content_preview.lines().enumerate() {
            println!("   {:>4}  {}", r.start_line + i, line.dimmed());
        }
    }
    println!();
}

pub fn print_doc_results(query: &str, results: &[super::search::searcher::DocSearchResult]) {
    if results.is_empty() {
        println!("{} {}", "→".dimmed(),
            t!(format!("文档中未找到「{}」相关内容", query),
               format!("No doc results for \"{}\"", query)));
        return;
    }

    title(&t!(
        format!("文档搜索  \"{}\"  ({} 条)", query, results.len()),
        format!("Doc Search  \"{}\"  ({} results)", query, results.len())
    ));

    for r in results {
        println!();
        let display = r.file_path.strip_prefix("modules/").unwrap_or(&r.file_path);
        print!("  {}  {}", format!("{:2}", r.rank).bold(), display.cyan());
        if let Some(sec) = &r.section {
            print!("  § {}", sec.yellow());
        }
        println!();

        print!("     {}: {:.4}", t!("相关度", "score"), r.score);
        if !r.module.is_empty() {
            print!("  {}: {}", t!("模块", "module"), r.module.dimmed());
        }
        println!();

        for line in r.content_preview.lines() {
            println!("     {}", line.dimmed());
        }
    }
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// 模块状态展示
// ─────────────────────────────────────────────────────────────────────────────

pub fn print_module_status_table(rows: &[(String, String, String)], outdated: usize) {
    // rows: (icon, key, label_colored)
    title(&t!("模块理解状态", "Module Status"));
    println!();
    for (icon, key, label) in rows {
        println!("  {} {:<38} {}", icon, key, label);
    }
    println!();
    if outdated > 0 {
        warn(&t!(
            format!("{} 个模块文档已过时，建议重新运行 margi module analyze <key>", outdated),
            format!("{} module(s) outdated — run: margi module analyze <key>", outdated)
        ));
    }
}

pub fn print_module_list(rows: &[(String, Vec<String>)]) {
    title(&t!("已注册模块", "Registered Modules"));
    println!();
    for (key, docs) in rows {
        if docs.is_empty() {
            println!("  {}", key.cyan());
        } else {
            println!("  {}  {}", key.cyan(), docs.join(", ").dimmed());
        }
    }
    println!();
    println!("  {}  {}", t!("共", "total"), format!("{}", rows.len()).bold());
}
