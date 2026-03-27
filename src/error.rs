use thiserror::Error;

#[derive(Error, Debug)]
pub enum MargiError {
    #[error("NotInitialized")]
    NotInitialized,

    #[error("{0}")]
    ModuleNotFound(String),

    #[error("IndexNotBuilt")]
    IndexNotBuilt,

    #[error("{0}")]
    ConfigError(String),

    #[error("{0}")]
    GitError(String),

    #[error("{0}")]
    PathNotFound(String),

    #[error("{0}")]
    InvalidArgs(String),
}

impl MargiError {
    /// 返回本地化的用户友好错误消息
    pub fn localized(&self) -> String {
        match self {
            MargiError::NotInitialized =>
                t!("未找到 .margi 目录，请先运行 `margi init`",
                   "No .margi directory found. Run `margi init` first.").to_string(),
            MargiError::IndexNotBuilt =>
                t!("搜索索引未建立，请先运行 `margi index build`",
                   "Search index not built. Run `margi index build` first.").to_string(),
            MargiError::ModuleNotFound(name) =>
                t!(format!("模块 '{}' 不存在", name),
                   format!("Module '{}' not found", name)),
            MargiError::PathNotFound(p) =>
                t!(format!("路径不存在: {}", p),
                   format!("Path not found: {}", p)),
            MargiError::InvalidArgs(msg) | MargiError::ConfigError(msg) | MargiError::GitError(msg) =>
                msg.clone(),
        }
    }
}
