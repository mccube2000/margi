## 文件：C:/Users/mccube/Downloads/margi/margi/.margi/modules/src/api.md

# src 模块 API 文档

## 对外暴露的接口

### 主入口
- `main()` - 程序主入口，解析命令行参数并执行相应命令

### CLI 接口
- `run()` - CLI 命令执行主函数

### 模块管理接口
- `ModuleStatus` - 模块状态枚举
- `Module` - 模块信息结构体
- `list_modules()` - 列出所有已注册模块
- `analyze_module()` - 分析模块并生成文档任务

### 命令行接口
- `Command` - CLI 命令枚举
  - `Init` - 初始化命令
  - `Module` - 模块管理命令
  - `Search` - 搜索命令
  - `Note` - 笔记管理命令
  - `Diff` - 差异比较命令
  - `Correct` - 纠正记录命令

### 核心数据结构
```rust
pub struct Module {
    pub key: String,
    pub path: PathBuf,
    pub status: ModuleStatus,
    pub doc_path: PathBuf,
    pub file_count: usize,
}

pub enum ModuleStatus {
    Fresh,
    Analyzed,
    Documented,
    Understood,
    Outdated,
}
```

### 错误处理
- `MargiError` - 自定义错误类型
- `MargiResult<T>` - 结果类型别名

## 使用示例

### 基本使用
```rust
// 初始化项目
margi init

// 注册模块
margi module add src/path

// 分析模块
margi module analyze module_key
```

### 命令行使用
```bash
# 查看所有模块
margi module list

# 加载模块文档
margi module load module_key

# 搜索源码
margi search "关键词"
```