## 文件：C:/Users/mccube/Downloads/margi/margi/.margi/modules/src/module/api.md

# src/module 模块 API 文档

## 对外暴露的接口

### 模块管理
- `ModuleManager::new()` - 创建模块管理器实例
- `ModuleManager::register_module()` - 注册新模块
- `ModuleManager::list_modules()` - 列出所有已注册模块
- `ModuleManager::get_module()` - 获取特定模块信息

### 模块分析
- `ModuleAnalyzer::analyze()` - 分析模块结构
- `ModuleAnalyzer::get_file_count()` - 获取模块文件数量
- `ModuleAnalyzer::get_structure()` - 获取模块目录结构

### 模板管理
- `ModulePlanner::plan()` - 生成模块规划
- `ModulePlanner::get_template()` - 获取模块文档模板
- `ModulePlanner::validate_structure()` - 验证模块结构

### 状态管理
- `ModuleStatus::new()` - 创建模块状态
- `ModuleStatus::update()` - 更新模块状态
- `ModuleStatus::check_outdated()` - 检查模块是否过时

## 核心数据结构

### 模块信息
```rust
pub struct Module {
    pub key: String,
    pub path: PathBuf,
    pub status: ModuleStatus,
    pub doc_path: PathBuf,
    pub file_count: usize,
    pub structure: ModuleStructure,
}

pub struct ModuleStructure {
    pub files: Vec<PathBuf>,
    pub directories: Vec<PathBuf>,
    pub languages: HashSet<String>,
}
```

### 模块状态
```rust
pub enum ModuleStatus {
    Fresh,          // 新注册，未分析
    Analyzed,       // 已分析，未生成文档
    Documented,     // 已生成文档，未理解
    Understood,     // 已理解
    Outdated,       // 源码已变更，文档过时
}
```

## 使用示例

### 基本模块操作
```rust
let mut manager = ModuleManager::new();
manager.register_module("example", Path::new("src/example"));

let modules = manager.list_modules();
for module in modules {
    println!("Module: {}, Files: {}", module.key, module.file_count);
}
```

### 模块分析
```rust
let analyzer = ModuleAnalyzer::new(&module);
let structure = analyzer.get_structure();
println!("Total files: {}", structure.files.len());
println!("Languages: {:?}", structure.languages);
```

### 状态管理
```rust
let mut status = ModuleStatus::new();
status.update(Documented);
let is_outdated = status.check_outdated(&module_path);
```