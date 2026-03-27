pub mod chunker;
pub mod embed;
pub mod indexer;
pub mod searcher;

use anyhow::Result;

use crate::{cli::IndexCommands, config::Config, paths};
use indexer::Indexer;
use searcher::Searcher;

pub fn run_search(
    query: &str, exact: bool, module: Option<&str>, mode: &str, limit: usize, docs: bool,
) -> Result<()> {
    let margi_dir = paths::margi_root()?;
    let config    = Config::load(&margi_dir)?;
    let searcher  = Searcher::new(margi_dir, config);

    // 从 anyhow::Error 中提取 MargiError，打印本地化消息
    let print_margi_err = |e: &anyhow::Error| -> bool {
        if let Some(me) = e.downcast_ref::<crate::error::MargiError>() {
            crate::ui::err(&me.localized());
            true
        } else {
            false
        }
    };

    if docs {
        match searcher.search_docs(query, module, limit) {
            Ok(results) => searcher::print_doc_results(query, &results),
            Err(e) if print_margi_err(&e) => {}
            Err(e) => return Err(e),
        }
    } else {
        match searcher.search(query, exact, module, mode, limit) {
            Ok(results) => searcher::print_results(query, &results),
            Err(e) if print_margi_err(&e) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn run_index(command: IndexCommands) -> Result<()> {
    let margi_dir    = paths::margi_root()?;
    let project_root = paths::project_root()?;
    let config       = Config::load(&margi_dir)?;
    let indexer      = Indexer::new(project_root, margi_dir, config);

    match command {
        IndexCommands::Build { force } => {
            crate::ui::info(&t!("构建搜索索引...", "Building search index..."));
            indexer.build(force)?;
        }
        IndexCommands::Stats => indexer.stats()?,
        IndexCommands::Clear => indexer.clear()?,
    }
    Ok(())
}
