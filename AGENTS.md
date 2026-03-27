# Project Context

> 完整工具说明见 `.margi/README.md`。

## margi 使用规则

本项目使用 margi 管理模块文档和搜索索引。**下列规则优先于默认的文件读取行为。**

### 任务开始前

在读取任何源文件来理解某个模块之前，先执行：

```bash
margi module status              # 查看哪些模块有现成文档
margi module load <key>          # 加载文档（已包含架构结论、接口、注意事项）
margi search "<关键词>"          # 搜索源码位置（中英文混合均可）
margi search --docs "<关键词>"   # 搜索已整理的模块文档
```

**`margi module load` 的内容是人工审核过的结论**，直接加载比阅读多个源文件更可靠、消耗更少上下文。有文档就用文档，不要重复读源码。

### 任务结束前

```bash
margi diff                                               # 检查哪些模块文档因改动变为过时
margi note add "<约束/注意事项>" --module <key>          # 持久化本次发现的重要约束
margi correct add "<错误做法 -> 正确做法>" --module <key> # 记录踩过的坑
```

对话结束后这些内容才不会丢失。不要把约束、耦合关系或设计限制只留在对话里。

### 判断表

| 需要做什么 | 正确做法 | 不要做 |
|-----------|---------|--------|
| 理解某模块的职责、接口、设计 | `margi module load <key>` | 直接打开该目录下的多个源文件 |
| 查找函数或概念在哪里 | `margi search "<关键词>"` | grep / 全局搜索 |
| 查找已知的约束或坑 | `margi search --docs` 或 `margi note list` | 假设没有相关记录 |
| 修改了某模块的接口或行为 | `margi diff` 检查文档状态 | 忽略文档更新 |
| 读完一个无文档模块的源码后 | `margi module analyze <key>` 输出文档任务 | 只在对话里总结 |
| 发现了约束、耦合或边界情况 | `margi note add` 或 `margi correct add` | 只留在当前对话 |

## 全局注意事项

<!-- 执行 `margi note list` 查看完整列表，详见 .margi/memory.md -->

## 环境 & 构建

见 `.margi/env.md`，或执行 `margi env show`
