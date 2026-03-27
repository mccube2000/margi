# margi — margi

> margi 是项目上下文管理工具，帮助理解和维护大型代码库。
> 本文件记录功能参考和目录结构，供日常查阅。

---

## 核心概念

- **模块（Module）**：一个源码目录对应一个模块，有独立的文档（api / internals / notes）
- **索引（Index）**：对源码和模块文档的全文搜索索引，支持中英文混合搜索
- **memory.md**：项目全局记忆，记录约束、架构决策等高优先级信息

---

## 日常开发场景

### 开始一个任务前

```bash
margi module status                  # 查看哪些模块已分析
margi module load src/auth           # 加载相关模块文档到上下文
margi search "关键词"               # 搜索源码定位相关代码
margi search --docs "关键词"        # 搜索模块文档
```

### 修改完成后

```bash
margi diff                           # 检测代码变更，标记过时的模块文档
margi note add "注意事项" --module src/auth     # 记录注意事项
margi correct add "错误→正确" --module src/auth  # 记录纠正
```

### 新加入代码库

```bash
margi status                         # 整体状态总览
margi module plan                    # 查看目录结构
margi module list                    # 列出已注册模块
margi env show                       # 查看环境和构建说明
```

---

## 模块管理

```bash
margi module add src/feature         # 注册新模块（路径即 key，必须存在）
margi module analyze src/feature     # 生成模块文档（输出任务描述到 stdout）
margi module load src/feature --include-internals   # 加载含内部实现的完整文档
margi module set-status src/feature outdated        # 手动标记过时

margi module split src/feature --depth 2            # 拆分大模块
margi module merge src/a src/b --into src/combined  # 合并模块
margi module remove src/old --archive               # 归档注销
```

模块状态：`unknown` → `analyzing` → `partial` / `understood` / `outdated`

---

## 搜索

```bash
# 源码搜索（默认）
margi search "合并"                 # 中文（bigram 索引，2字词精确命中）
margi search "cmd_merge"            # 函数名（trigram 子串匹配）
margi search "parseUser"            # 驼峰命名
margi search "AI"                   # 短词（< 3字符自动 LIKE 兜底）
margi search "合并module"           # 中英文混合（双路 + RRF 合并）

# 文档搜索
margi search --docs "模块状态"      # 搜索 .margi/modules/ 下的文档

# 过滤选项
margi search "token" --in src/auth  # 限定模块
margi search "fn" -n 20             # 返回更多结果（默认 10）
margi search --exact "cmd_merge"    # 精确短语匹配
```

---

## 搜索索引

```bash
margi index build          # 增量构建（源码 + 模块文档）
margi index build --force  # 全量重建（首次或 clear 后必须执行）
margi index stats          # 查看统计
margi index clear          # 清空索引（清空后需 build --force）
```

索引技术：英文/代码使用 trigram（支持子串），中文使用 bigram（相邻2字组）。

---

## 注意事项与纠正记录

```bash
margi note add "内容" [--module <key>] [--tag <标签>]
margi note list [--module <key>] [--tag <标签>]

margi correct add "问题→正确做法" [--module <key>]
margi correct list [--since 2024-01-01] [--module <key>]
```

---

## 文件结构

```
<项目根>/
  AGENTS.md              — 工具入口（margi init 生成，可手动补充）
  .margi/
    README.md            — 本文件
    config.json          — 配置（source_root、排除规则等）
    env.md               — 环境 & 构建说明
    memory.md            — 项目全局记忆
    modules/
      src/auth/
        STATUS           — 模块状态
        api.md           — 对外接口文档
        internals.md     — 内部实现说明
        notes.md         — 注意事项
    .index/              — 搜索索引（已加入 .gitignore）
      chunks.db
      meta.json
    corrections/
      YYYY-MM-DD.md      — 纠正记录（按日期）
    archive/             — 已归档的模块文档
```

---

## 环境变量

| 变量 | 说明 |
|------|------|
| `MARGI_LANG` | 强制指定语言（`zh` / `en`），优先级最高 |
| `LANG` / `LANGUAGE` / `LC_ALL` | 系统语言检测（`zh_*` → 中文） |
