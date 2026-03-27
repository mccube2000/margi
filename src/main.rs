// i18n 必须是第一个 mod 声明，#[macro_use] 才能对后续所有模块生效
#[macro_use]
mod i18n;

mod cli;
mod config;
mod diff;
mod error;
mod init;
mod memory;
mod module;
mod paths;
mod prompts;
mod search;
mod ui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    i18n::init();
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { force }                                    => init::run(force),
        Commands::Search { query, exact, module, mode, limit, docs } => search::run_search(&query, exact, module.as_deref(), &mode, limit, docs),
        Commands::Module { command }                                => module::run(command),
        Commands::Note { command }                                  => memory::run_note(command),
        Commands::Correct { command }                               => memory::run_correct(command),
        Commands::Diff { staged }                                   => diff::run(staged),
        Commands::Env { command }                                   => run_env(command),
        Commands::Status                                            => run_status(),
        Commands::Index { command }                                 => search::run_index(command),
    };

    if let Err(e) = result {
        // 优先尝试显示本地化的 MargiError 消息，避免打印原始变体名
        if let Some(me) = e.downcast_ref::<error::MargiError>() {
            ui::err(&me.localized());
        } else {
            // 非预期错误（IO、serde 等）打印完整错误链
            eprintln!("{} {:?}", "✗", e);
        }
        std::process::exit(1);
    }
}

fn run_env(command: cli::EnvCommands) -> Result<()> {
    use cli::EnvCommands;
    let root = paths::margi_root()?;
    match command {
        EnvCommands::Show => {
            let env_path = root.join("env.md");
            if env_path.exists() {
                print!("{}", std::fs::read_to_string(&env_path)?);
            } else {
                ui::err(&error::MargiError::NotInitialized.localized());
            }
        }
    }
    Ok(())
}

fn run_status() -> Result<()> {
    use colored::Colorize;
    let root = paths::margi_root()?;
    let modules_dir = root.join("modules");

    if !modules_dir.exists() {
        ui::err(&error::MargiError::NotInitialized.localized());
        return Ok(());
    }

    ui::title(&t!("margi 项目状态", "margi project status"));
    println!();

    let mut rows     = Vec::new();
    let mut counts   = [0usize; 5]; // unknown, analyzing, partial, understood, outdated
    let mut outdated = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&modules_dir)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        if !entry.file_type()?.is_dir() { continue; }
        let sf     = entry.path().join("STATUS");
        let status = if sf.exists() {
            module::status::ModuleStatus::parse(&std::fs::read_to_string(&sf)?)
        } else {
            module::status::ModuleStatus::Unknown
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let (icon, label, idx) = match &status {
            module::status::ModuleStatus::Unknown          => ("○", status.label_i18n().dimmed().to_string(),  0),
            module::status::ModuleStatus::Analyzing { .. } => ("◐", status.label_i18n().yellow().to_string(), 1),
            module::status::ModuleStatus::Partial { .. }   => ("◑", status.label_i18n().cyan().to_string(),   2),
            module::status::ModuleStatus::Understood { .. }=> ("●", status.label_i18n().green().to_string(),  3),
            module::status::ModuleStatus::Outdated { .. }  => { outdated += 1; ("!", status.label_i18n().red().to_string(), 4) },
        };
        counts[idx] += 1;
        rows.push((icon.to_string(), name, label));
    }

    ui::print_module_status_table(&rows, outdated);

    println!("  unknown {}, analyzing {}, partial {}, understood {}, outdated {}",
        counts[0], counts[1], counts[2], counts[3], counts[4]);
    println!();

    if root.join(".index").join("meta.json").exists() {
        ui::ok(&t!("搜索索引：已建立", "Search index: ready"));
    } else {
        ui::err(&error::MargiError::IndexNotBuilt.localized());
    }
    Ok(())
}
