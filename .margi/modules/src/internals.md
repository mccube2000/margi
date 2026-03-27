## 文件：C:/Users/mccube/Downloads/margi/margi/.margi/modules/src/internals.md

# src 模块内部实现

## 核心数据流

### 程序执行流程
```
main()
  ↓
parse_command_line()
  ↓
match Command enum
  ↓
run() → execute_command()
  ↓
[各个命令的执行]
```

### 模块处理流程
```
模块扫描
  ↓
目录结构分析
  ↓
源文件识别
  ↓
模块信息创建
  ↓
状态更新
```

## 关键设计决策

### 1. 命令行架构设计
- 使用枚举类型定义所有命令，便于扩展和维护
- 每个命令都有独立的处理函数，职责分离
- 错误处理使用统一的 `MargiError` 类型

### 2. 模块管理机制
- 模块状态机设计：Fresh → Analyzed → Documented → Understood → Outdated
- 文档与源码分离存储，通过 `.margi/modules/` 目录管理
- 使用持久化存储记录模块状态和文档路径

### 3. 目录扫描策略
- 递归扫描目录，支持任意深度的项目结构
- 自动识别 Rust 源码文件（.rs）
- 统计文件数量用于模块文档生成

## 依赖关系

### 主要依赖
- `clap` - 命令行参数解析
- `anyhow` - 错误处理
- `serde` - 序列化支持
- `serde_json` - JSON 格式数据处理

### 模块间依赖
```
src (主模块)
├── src/memory - 记忆和纠正管理
├── src/module - 模块分析和管理
└── src/search - 搜索功能
```

### 状态管理
- 模块状态存储在 `.margi/config.json` 中
- 文档路径信息与模块信息关联
- 状态变更时自动更新配置文件

## 性能考虑

### 目录扫描优化
- 使用并行处理提高大项目扫描速度
- 缓存扫描结果避免重复计算

### 文档生成
- 按需生成文档，避免不必要的 I/O 操作
- 文档内容结构化存储，便于检索和更新