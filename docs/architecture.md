# margi 架构设计文档

> 面向大型项目的开发上下文管理系统

---

## 1. 项目定位

### 1.1 解决的核心问题

在辅助大型项目开发时，上下文工具面临三个根本性障碍：

1. **会话记忆断裂**：每次对话从零开始，重复分析已理解的模块，浪费大量上下文窗口
2. **上下文爆炸**：加载完整代码库超出 token 限制，工具不得不在信息残缺的情况下工作
3. **经验无法沉淀**：人工纠正过的错误、项目特有规范无处存储，反复犯同样的错误

### 1.2 设计原则

- **无状态工具**：margi 本身不持有任何运行时状态，没有后台进程、没有守护服务。所有状态存储在项目目录的文件中，工具只负责读写和计算
- **文件即状态**：所有数据以 Markdown / 纯文本存储在项目目录内，纳入版本控制，人类可直接阅读和编辑
- **引导而非代劳**：margi 不主动调用 AI。它负责收集结构信息、生成任务描述，由外部工具（Claude Code / Cursor / 任意终端工具）完成实际分析和写作
- **按需加载**：模块文档仅在需要时注入上下文，不污染无关对话
- **路径即标识**：模块 key 就是相对项目根的源码路径，文档目录直接镜像源码结构，无需任何映射
- **CLI 优先**：所有功能通过终端命令操作，便于与任何工具的 shell 调用集成
- **渐进增强**：项目从零开始逐步积累，不要求一次性完整初始化

---

## 2. 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                     外部工具层                                │
│         Claude Code / Cursor / Copilot / 任意终端工具          │
└────────┬────────────────────────────┬────────────────────────┘
         │                            │
         │ ① 直接读取文件（隐式兜底）  │ ② 调用 CLI 命令（主接口）
         ▼                            ▼
┌──────────────────┐      ┌───────────────────────────────────┐
│  .margi/         │      │         margi CLI 接口层            │
│  原始文件         │      │  init │ search │ module │ note    │
│  （直接可读）     │      └─────────────────┬─────────────────┘
└──────────────────┘                        │
         ▲                                  ▼
         │                     ┌────────────────────────┐
         │                     │      核心功能模块        │
         │                     │  初始化 │ 搜索引擎       │
         │                     │  状态机 │ 记忆管理       │
         │                     │  规划器 │ 变更感知       │
         │                     └──────────┬─────────────┘
         └──────────── 读写 ──────────────┘
                                  ▼
┌─────────────────────────────────────────────────────────────┐
│                    .margi/ 数据层                              │
│  AGENTS.md │ memory.md │ env.md │ modules/ │ .index/          │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 两条数据访问路径

**① 直接读取文件（隐式兜底）**

所有数据以纯文本 / Markdown 存储，外部工具本身就具备读取任意文件的能力，无需任何引导：

- 直接阅读 `modules/src/components/auth/api.md`
- 直接检查 `modules/src/utils/STATUS`
- 直接浏览 `corrections/2026-03-17.md`

**② 调用 CLI 命令（主接口）**

CLI 提供直接读文件做不到的增值能力：

| CLI 命令 | 增值内容 |
|----------|---------|
| `margi search "JWT 刷新逻辑"` | FTS5 全文检索 + 未来语义搜索 |
| `margi module load auth` | 跨文件聚合 + 关联纠正记录 |
| `margi module status` | 汇总所有模块状态，无需遍历 |
| `margi module plan` | 目录树构建 + 规划模板生成 |
| `margi diff` | 结合 git 变更分析文档过期 |

`AGENTS.md` 只需引导外部工具使用 CLI，不重复描述文件结构。

---

## 3. 文件结构

```
<project-root>/
├── AGENTS.md                          # 工具入口（轻量），由 margi 管理
└── .margi/
    ├── config.json                    # 项目配置
    ├── memory.md                      # 全局注意事项（高优先级同步到 AGENTS.md）
    ├── env.md                         # 开发环境 & 构建文档
    ├── module-plan.md                 # 模块规划任务（由 margi module plan 生成）
    ├── split-<safe_name>.md           # 拆分任务（由 margi module split 生成）
    ├── merge-<target>.md              # 合并任务（由 margi module merge 生成）
    ├── modules/                       # 模块文档（镜像源码目录结构）
    │   ├── _root_/                    # 项目根目录散落源码文件的文档
    │   │   ├── STATUS
    │   │   ├── api.md
    │   │   ├── internals.md
    │   │   └── notes.md
    │   └── src/
    │       ├── components/
    │       │   └── auth/              # key = "src/components/auth"
    │       │       ├── STATUS
    │       │       ├── api.md
    │       │       ├── internals.md
    │       │       └── notes.md
    │       └── utils/                 # key = "src/utils"
    │           ├── STATUS
    │           └── ...
    ├── archive/                       # 归档的已删除模块（--archive 选项）
    ├── corrections/
    │   ├── 2026-03-17.md              # 按天归档的纠正记录
    │   └── ...
    └── .index/                        # 搜索索引缓存（.gitignore，本地独立）
        ├── chunks.db
        └── meta.json
```

### 3.1 模块 key 规范

**模块 key = 相对项目根的正斜杠路径**，直接对应 `.margi/modules/` 下的目录结构。

| 源码路径 | 模块 key | 文档目录 |
|---------|---------|---------|
| `src/components/auth/` | `src/components/auth` | `.margi/modules/src/components/auth/` |
| `src/utils.ts`（文件） | `src/utils` | `.margi/modules/src/utils/` |
| 项目根散落文件 | `_root_` | `.margi/modules/_root_/` |

Windows 路径分隔符在存储和展示时统一转为 `/`，`PathBuf` 操作通过逐段 `join` 保持跨平台兼容。

### 3.2 AGENTS.md 设计

保持轻量，只引导使用 CLI，不重复描述文件结构：

```markdown
# Project Context
> 由 margi 管理

## 使用前必读
margi module status                  # 查看所有模块理解状态
margi module load <模块名>            # 加载模块文档
margi search "<功能描述>"            # 语义搜索
margi search --exact "<函数名>"      # 精确搜索

## 全局注意事项
<!-- margi note add 后自动同步 -->

## 环境 & 构建
见 .margi/env.md，或执行 margi env show

## 工作流
1. module status → 了解哪些模块已分析
2. module load   → 加载相关模块上下文
3. search        → 定位相关代码
4. diff          → 修改后检测文档是否需要更新
```

---

## 4. 核心模块设计

### 4.1 初始化（`margi init`）

**职责**：建立 `.margi/` 脚手架，输出 env.md 和 memory.md 的生成任务描述。

**流程**：

```
margi init
   │
   ├─→ 创建目录结构（modules/ corrections/ .index/）
   ├─→ 写入 config.json（默认配置）
   ├─→ 扫描项目统计（文件数、语言、入口文件）
   ├─→ 写占位文档（env.md、memory.md 骨架）
   ├─→ 写 AGENTS.md
   ├─→ 更新 .gitignore
   └─→ stdout 输出：
          ├── env.md 生成任务（含入口文件路径列表）
          ├── memory.md 生成任务
          └── 提示：运行 margi module plan 规划模块
```

**不做**：不自动注册模块（由 `module plan` + `module add` 流程负责）。

### 4.2 模块管理状态机（`margi module`）

#### 状态定义

```
unknown ──→ analyzing ──→ partial ──→ understood
                │                          │
                └──────── outdated ◀────────┘
                              │
                              └──→ analyzing（重新分析）
```

| 状态 | 含义 | STATUS 文件内容 |
|------|------|----------------|
| `unknown` | 已注册，尚未分析 | `unknown` |
| `analyzing` | 分析中（进程锁） | `analyzing:<timestamp>` |
| `partial` | 初步分析，文档不完整 | `partial:<timestamp>` |
| `understood` | 深度分析，文档完整 | `understood:<timestamp>` |
| `outdated` | 源码变更后文档需要更新 | `outdated:<timestamp>:<reason>` |

#### 模块规划流程（`margi module plan`）

```
margi module plan [--depth N] [--root <子目录>]
   │
   ├─→ 扫描源码目录，构建目录树（含文件数统计）
   ├─→ stdout：输出目录树（体积可控，可直接阅读）
   └─→ 写入 .margi/module-plan.md：
          ├── 完整目录树
          ├── 当前已注册模块列表
          ├── 规划原则说明
          ├── 规划决策表格（待填写）
          └── 执行命令模板
```

目录树示例（stdout）：
```
src/
  ├── components/  (12 .vue)
  │   ├── auth/    (3 .vue)
  │   └── layout/  (4 .vue)
  ├── views/       (8 .vue)
  ├── stores/      (4 .ts)
  └── utils/       (6 .ts)
```

#### 模块注册（`margi module add <path>`）

路径即 key，无需额外配置：

```bash
margi module add src/components/auth    # key = src/components/auth
margi module add src/utils              # key = src/utils
margi module add _root_                 # 根目录散落文件
```

执行后创建 `.margi/modules/<key>/STATUS`，内容为 `unknown`。

#### 模块文档生成（`margi module analyze <key>`）

```
margi module analyze src/components/auth
   │
   ├─→ 解析 key → 找到源码目录和文档目录
   ├─→ 收集源文件路径列表（不读取文件内容）
   ├─→ 读取已有 notes.md、api.md（小体积上下文文档）
   ├─→ 读取相关 corrections/ 记录
   ├─→ STATUS 置为 analyzing
   └─→ stdout 输出任务描述：
          ├── 源文件路径列表（引导外部工具自行读取）
          ├── 三个目标文件的内容要求（api.md / internals.md / notes.md）
          ├── 已有上下文（notes + corrections）
          └── 完成后执行命令：margi module set-status <key> understood
```

**关键设计**：任务描述中只包含**文件路径列表**，不嵌入源码内容，避免超出上下文。外部工具根据路径自行读取所需文件。

#### 模块拆分与合并

`margi module split <key> [--depth N]`
- stdout：子目录树
- 写入 `.margi/split-<safe_name>.md`：包含子目录结构、现有文档路径、拆分决策表格

`margi module merge <key1> <key2> --into <target>`
- 写入 `.margi/merge-<target>.md`：包含各源模块状态和文档路径、合并任务说明

`margi module remove <key> [--archive|--hard]`
- 默认：清空文档文件，STATUS 置 unknown
- `--archive`：移动到 `.margi/archive/<key>/`
- `--hard`：彻底删除目录

### 4.3 搜索引擎（`margi search`）

#### 索引构建分层策略

```
源码文件
   │
   ├─→ 层 1：Tree-sitter AST（精确边界）
   │         Rust / JS / TS / Python / Go / Java / C / C++ / C# / Ruby
   │         → 精确到函数/方法级，处理嵌套，提取完整符号名
   │
   ├─→ 层 2：Regex 边界（兜底）
   │         PHP / Kotlin / Swift / Scala / Lua / Bash / Haskell
   │
   ├─→ 层 3：Vue/Svelte SFC 预处理
   │         提取 <script>/<script setup> 块 → 以 TS/JS 处理
   │         <template> 块整体作为一个 chunk
   │
   └─→ 层 4：固定行数滑窗（最终兜底）
             其余所有文件，chunk_size 行，overlap 行重叠
```

**文件头注入**：每个函数 chunk 的开头附加文件的 import/use 段落，保留上下文。

#### 索引存储

`chunks.db`（SQLite + FTS5）：

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT | `文件路径:起始行-结束行` |
| file_path | TEXT | 正斜杠相对路径 |
| module | TEXT | 所属模块 key |
| start_line / end_line | INT | 行号范围 |
| content | TEXT | 代码块内容 |
| symbol_name | TEXT | 函数/类名（含父容器，如 `MyClass::my_method`） |
| embedding | BLOB | 向量嵌入（预留，当前未填充） |

#### 搜索模式

| 模式 | 命令 | 实现 |
|------|------|------|
| 关键词 | `--exact` 或 `--mode keyword` | FTS5 全文检索 |
| 混合（默认） | `--mode hybrid` | 当前同关键词；预留语义融合接口 |
| 语义 | `--mode semantic` | 预留，配置嵌入模型后启用 |
| 模块内 | `--in <key>` | FTS5 + module 字段过滤 |

#### 增量更新

`meta.json` 记录每个文件的 SHA-256 哈希，变更检测成本极低：

```json
{
  "file_hashes": { "src/auth/token.ts": "abc123..." },
  "last_full_build": 1710000000,
  "chunk_count": 1247
}
```

`.index/` 加入 `.gitignore`，各开发者本地独立维护。

### 4.4 记忆管理（`margi note` / `margi correct`）

#### 两类记录的区别

| 类型 | 命令 | 存储位置 | 含义 |
|------|------|----------|------|
| 注意事项 | `margi note add` | 模块 `notes.md` 或全局 `memory.md` | 预防性说明 |
| 纠正记录 | `margi correct add` | `corrections/<date>.md` | AI 做错后人工纠正的记录 |

#### 记忆注入时机

- 全局 `memory.md` 的高优先级条目自动同步到 `AGENTS.md`（每次 `margi note add` 触发）
- 模块 `notes.md` 仅在 `margi module load` 时注入
- 纠正记录在 `margi module analyze` 时按 key 匹配后附加到任务描述

### 4.5 变更感知（`margi diff`）

```bash
margi diff           # 检查工作区 + 暂存区变更
margi diff --staged  # 只检查暂存区
```

通过 `libgit2` 获取变更文件列表，使用**路径前缀匹配**判断归属模块：

```
变更文件: src/components/auth/LoginForm.vue
  → starts_with(module_source_dir("src/components/auth"))
  → 匹配模块 src/components/auth
  → 状态 understood → 更新为 outdated
```

`_root_` 模块匹配逻辑：只匹配直接在项目根目录下的文件（`parent == project_root`）。

---

## 5. 数据流

### 5.1 典型工作流：首次接入新项目

```
1. margi init
   └─→ 脚手架 + 输出 env.md / memory.md 任务描述

2. 外部工具读取任务描述，写入 .margi/env.md 和 .margi/memory.md

3. margi module plan [--depth 2]
   └─→ stdout 目录树 + 写 .margi/module-plan.md

4. 外部工具或用户填写规划表，确定模块粒度

5. margi module add src/components/auth
   margi module add src/stores
   margi module add _root_

6. margi index build

7. margi module analyze src/components/auth
   └─→ stdout 任务描述（含文件路径列表）
   外部工具读取源文件，写入 api.md / internals.md / notes.md

8. margi module set-status src/components/auth understood
```

### 5.2 典型工作流：日常开发

```
外部工具开始任务
   │
   ├─→ margi module status           → 了解已分析模块
   ├─→ margi module load <key>       → 加载模块上下文（stdout）
   ├─→ margi search "<描述>"         → 定位相关代码
   │
   ├─→ 修改代码
   │
   └─→ 用户 review 后：
          ├── margi diff             → 检测过时文档
          ├── margi note add / margi correct add  → 记录经验
          └── margi module analyze <key>          → 更新过时文档
```

### 5.3 模块粒度调整工作流

```
# 拆分：components 下每个子目录独立为模块
margi module split src/components --depth 1
└─→ stdout 子目录树 + .margi/split-src_components.md

# 外部工具或用户决定拆分边界后：
margi module add src/components/auth
margi module add src/components/layout
margi module remove src/components   # 原粗粒度模块降级或归档

# 合并：两个小模块合并
margi module merge src/utils src/helpers --into src/shared
└─→ .margi/merge-src_shared.md
# 写入合并后文档，然后：
margi module set-status src/shared understood
margi module remove src/utils
margi module remove src/helpers
```

---

## 6. 配置规范

`.margi/config.json`：

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
    "scan_depth": 3,
    "manual": []
  },
  "search": {
    "chunk_size": 150,
    "chunk_overlap": 20,
    "embedding": {}
  },
  "memory": {
    "auto_sync_to_agents_md": true,
    "max_global_notes_in_agents_md": 10
  },
  "hooks": {
    "post_commit": true,
    "outdated_check_on_diff": true
  }
}
```

`scan_depth` 控制 `margi module plan` 和 `margi init` 时的目录扫描深度。对超大型项目，设置较小值（如 2）可加快速度，防止上下文过长；对需要细粒度文档的项目，设置较大值（如 4-5）。

---

## 7. 国际化

用户界面消息（终端输出）支持中英文，通过 `t!("中文", "English")` 宏在调用处声明。语言检测优先级：`MARGI_LANG` > `LANG` > `LANGUAGE` > `LC_ALL`，`zh_*` 系统自动切换中文，其余使用英文。

提示词（输出到 stdout 的任务描述）固定使用中文，不参与国际化。

---

## 8. 设计边界

以下场景**不在**设计范围内：

- **主动调用 AI**：margi 只生成结构化任务描述，不持有 API key，不发起网络请求
- **实时 IDE 集成**：不提供 LSP、IDE 插件或 GUI，专注 CLI 和文件系统
- **代码执行 / 沙箱**：不运行或测试代码，只管理上下文文档
- **权限 / 认证管理**：不处理多用户权限，适用于个人或小团队开发场景
- **实时协作**：不支持多人同时编辑，依赖 git 进行异步协作
- **语义搜索**（当前版本）：索引结构已预留 embedding 字段，但当前只实现 FTS5 关键词搜索；语义搜索需配置外部嵌入模型后启用

---

## 9. 扩展性

### 9.1 多仓库 / Monorepo

每个子包可以有独立的 `.margi/`，子包模块 key 相对于子包根目录：

```bash
# packages/api 子包
cd packages/api && margi module plan
# packages/web 子包
cd packages/web && margi module plan
```

### 9.2 团队协作

- `.margi/modules/`、`.margi/corrections/`、`AGENTS.md` 纳入版本控制，共享模块文档和纠正经验
- `.margi/.index/` 本地独立，不提交

### 9.3 文档腐化治理

- `margi diff` 在每次 commit 后（git hook）自动将受影响模块标记为 `outdated`
- `margi module status` 展示所有过期模块
- 长期未更新的文档可通过 `scan_depth` + `module plan` 重新规划

### 9.4 Vue 3 / 前端项目

- `.vue` 和 `.svelte` 文件完整支持索引和分块（提取 `<script setup>` 块进行 AST 解析）
- `margi module analyze` 对含 `.vue` 文件的模块自动在任务描述中附加 `defineProps` / `defineEmits` / `defineExpose` 分析提示
- 默认排除目录包含 `.nuxt`、`.next`、`.output`、`coverage`
