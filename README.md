# margi

> 面向大型项目的 AI 辅助开发上下文管理系统

## 解决的问题

| 问题 | margi 的解法 |
|------|-------------|
| 每次对话从零开始 | 模块文档持久化，按需加载 |
| 代码库太大超出 token 限制 | 语义搜索 + 分模块加载 |
| AI 反复犯同样的错误 | `corrections/` 纠正记录，自动关联到模块 |

## 安装

需要 Rust 1.70+：

```bash
cargo install --path .
```

或直接构建：

```bash
cargo build --release
# 二进制在 target/release/margi
```

## 快速上手

```bash
# 1. 在项目根目录初始化
cd my-project
margi init

# 2. 建立搜索索引
margi index build

# 3. 分析关键模块
margi module analyze src/auth
margi module analyze src/payment

# 4. 日常使用
margi module status                        # 查看所有模块状态
margi module load auth                     # 加载模块文档（供 AI 读取）
margi search "JWT token 刷新逻辑"           # 语义搜索
margi search --exact "refreshToken"        # 精确关键词搜索
```

## 命令参考

### `margi init [--force]`

扫描项目结构，生成 `.margi/` 目录、`AGENTS.md`、`env.md`。
如果设置了 `ANTHROPIC_API_KEY`，会调用 AI 生成文档内容；否则生成占位骨架。

### `margi search <query> [选项]`

```
选项:
  --exact / -e        精确关键词搜索
  --in <module>       限定模块范围
  --mode <mode>       搜索模式: semantic / keyword / hybrid（默认）
  -n <数量>           返回结果数（默认 10）

示例:
  margi search "刷新 token 的逻辑"
  margi search --exact "refreshAccessToken"
  margi search "权限校验" --in auth
  margi search "支付回调" --mode hybrid -n 5
```

### `margi module`

```
margi module status                  # 查看所有模块理解状态
margi module list                    # 列出已注册模块
margi module load <n> [n...]         # 加载模块文档到 stdout
  --include-internals                  同时加载 internals.md
margi module analyze <路径或名称>    # AI 生成/更新模块文档
  --force                              强制重新分析
margi module set-status <n> <状态>   # 手动设置状态
```

**模块状态流转：**
```
unknown → analyzing → partial → understood
                                    ↓
              outdated ←────────────┘
                 ↓
           analyzing（重新分析）
```

### `margi note`

```
margi note add "内容" [--module <n>] [--tag 标签]
margi note list [--module <n>] [--tag 标签]
```

### `margi correct`

```
margi correct add "问题描述和正确做法" [--module <n>] [--tag 标签]
margi correct list [--since YYYY-MM-DD] [--module <n>]
```

### `margi diff [--staged]`

分析 git 变更，标记受影响模块为 `outdated`。

### `margi index`

```
margi index build [--force]   # 构建/增量更新搜索索引
margi index stats             # 查看索引统计
margi index clear             # 清除索引缓存
```

### `margi env show`

显示 `.margi/env.md`（开发环境 & 构建文档）。

### `margi status`

显示整体项目状态（模块状态汇总 + 索引状态）。

## 与 AI 工具集成

### Claude Code / Codex CLI / Cursor

`margi init` 会生成 `AGENTS.md`，AI 工具启动时自动读取，了解如何使用 margi 命令。

### Shell 管道

```bash
# 搜索 + AI 解释
margi search "token 过期处理" | claude "解释这些代码的作用"

# 加载模块上下文 + AI 修改
margi module load payment | claude "帮我增加退款功能"

# 组合多模块
{ margi module load auth; margi module load payment; } | claude "review 这个跨模块的需求..."
```

## 文件结构

```
.margi/
├── config.json          # 项目配置
├── memory.md            # 全局注意事项（高优先级同步到 AGENTS.md）
├── env.md               # 开发环境文档
├── modules/
│   └── <module-name>/
│       ├── STATUS       # 理解状态（纯文本，纳入版本控制）
│       ├── api.md       # 对外接口文档
│       ├── internals.md # 内部实现说明
│       └── notes.md     # 注意事项 & 踩坑记录
├── corrections/
│   └── YYYY-MM-DD.md   # 按天归档的纠正记录
└── .index/              # 搜索索引（.gitignore，本地独立）
    ├── chunks.db
    └── meta.json
```

## 配置

`.margi/config.json` 示例：

```json
{
  "version": "1.0",
  "project": {
    "name": "my-project",
    "root": "src/",
    "exclude": ["node_modules", "dist", "target", "*.test.ts"]
  },
  "modules": {
    "auto_detect": true,
    "detection_depth": 2
  },
  "search": {
    "chunk_size": 150,
    "chunk_overlap": 20,
    "semantic_weight": 0.7,
    "embedding_model": "none"
  },
  "memory": {
    "auto_sync_to_agents_md": true,
    "max_global_notes_in_agents_md": 10
  },
  "ai": {
    "api_base": "https://api.anthropic.com",
    "model": "claude-sonnet-4-20250514"
  }
}
```

## AI 生成功能

设置 `ANTHROPIC_API_KEY` 环境变量后：

- `margi init` 自动调用 AI 生成 `env.md` 和 `memory.md`
- `margi module analyze` 调用 AI 生成 `api.md`、`internals.md`、`notes.md`

不设置时，所有命令仍然可用，只是文档内容需要手动填写。

## git hook（自动过期检测）

在 `.git/hooks/post-commit` 中添加：

```bash
#!/bin/sh
margi diff --staged
```

每次 commit 后自动检测哪些模块需要更新文档。
