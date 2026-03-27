use clap::{Parser, Subcommand};

/// 面向大型项目的开发上下文管理系统
#[derive(Parser)]
#[command(name = "margi", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 初始化项目，生成 .margi/ 脚手架和文档骨架
    Init {
        #[arg(long)]
        force: bool,
    },
    /// 搜索代码库（默认搜索源码，--docs 搜索模块文档）
    Search {
        query: String,
        #[arg(long, short = 'e')]
        exact: bool,
        #[arg(long, short = 'i', value_name = "MODULE")]
        module: Option<String>,
        #[arg(long, default_value = "hybrid")]
        mode: String,
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,
        /// 搜索模块文档（api.md / notes.md / internals.md）而非源码
        #[arg(long, short = 'd')]
        docs: bool,
    },
    /// 模块管理
    Module {
        #[command(subcommand)]
        command: ModuleCommands,
    },
    /// 注意事项管理
    Note {
        #[command(subcommand)]
        command: NoteCommands,
    },
    /// 纠正记录管理
    Correct {
        #[command(subcommand)]
        command: CorrectCommands,
    },
    /// 分析 git 变更，检测过期文档
    Diff {
        #[arg(long)]
        staged: bool,
    },
    /// 环境 & 构建文档
    Env {
        #[command(subcommand)]
        command: EnvCommands,
    },
    /// 显示整体项目状态
    Status,
    /// 管理搜索索引
    Index {
        #[command(subcommand)]
        command: IndexCommands,
    },
}

#[derive(Subcommand)]
pub enum ModuleCommands {
    /// 查看所有模块的理解状态
    Status,

    /// 加载模块文档到 stdout
    Load {
        #[arg(required = true)]
        names: Vec<String>,
        #[arg(long)]
        include_internals: bool,
    },

    /// 输出模块文档生成任务（含文件路径列表）
    Analyze {
        /// 模块路径，如 src/components/auth
        path: String,
        #[arg(long)]
        force: bool,
    },

    /// 手动设置模块状态
    SetStatus {
        name:   String,
        status: String,
    },

    /// 列出所有已注册模块
    List,

    /// 注册一个模块（路径即 key，如 src/components/auth）
    Add {
        /// 相对于项目根的源码路径，同时作为模块 key
        path: String,
    },

    /// 注销模块（默认保留文档并置为 unknown，--archive 归档，--hard 彻底删除）
    Remove {
        /// 模块 key
        name: String,
        /// 归档到 .margi/archive/ 而不是删除文档
        #[arg(long)]
        archive: bool,
        /// 彻底删除模块目录及所有文档
        #[arg(long)]
        hard: bool,
    },

    /// 扫描目录结构，输出模块规划任务到 .margi/module-plan.md
    Plan {
        /// 扫描深度（默认使用 config.modules.scan_depth）
        #[arg(long, short = 'd')]
        depth: Option<usize>,
        /// 限定扫描的子目录（相对项目根，不指定则扫描 source root）
        #[arg(long)]
        root: Option<String>,
    },

    /// 拆分模块：输出子目录结构和拆分任务到 .margi/split-<safe_name>.md
    Split {
        /// 要拆分的模块 key
        name: String,
        /// 子目录展开深度
        #[arg(long, short = 'd', default_value = "1")]
        depth: usize,
    },

    /// 合并模块：输出合并任务到 .margi/merge-<target>.md
    Merge {
        /// 要合并的模块（至少两个）
        #[arg(required = true)]
        names: Vec<String>,
        /// 合并后的目标模块名
        #[arg(long)]
        into: String,
    },
}

#[derive(Subcommand)]
pub enum NoteCommands {
    Add {
        content: String,
        #[arg(long, short = 'm')]
        module: Option<String>,
        #[arg(long, short = 't')]
        tag: Vec<String>,
    },
    List {
        #[arg(long, short = 'm')]
        module: Option<String>,
        #[arg(long, short = 't')]
        tag: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CorrectCommands {
    Add {
        description: String,
        #[arg(long, short = 'm')]
        module: Option<String>,
        #[arg(long, short = 't')]
        tag: Vec<String>,
    },
    List {
        #[arg(long)]
        since: Option<String>,
        #[arg(long, short = 'm')]
        module: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum EnvCommands {
    Show,
}

#[derive(Subcommand)]
pub enum IndexCommands {
    Build {
        #[arg(long)]
        force: bool,
    },
    Stats,
    Clear,
}
