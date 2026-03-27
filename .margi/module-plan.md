# 模块规划

扫描根目录：`C:/Users/mccube/Downloads/margi/margi/src`（深度 3）

## 目录结构

```
src/
  ├── memory/  (3 .rs)
    └── [3 .rs]
  ├── module/  (4 .rs)
    └── [4 .rs]
  ├── search/  (5 .rs)
    └── [5 .rs]
  └── [11 .rs]
```

## 当前已注册模块

- `src` — understood
- `src/memory` — understood
- `src/module` — understood
- `src/search` — understood

## 规划原则

- **独立为模块**：职责单一、内聚性强、有独立对外接口的目录
- **合并为整体**：文件少、逻辑简单、无需单独说明的目录
- **跳过**：纯资源目录（assets/public/static）、自动生成代码

## 规划结果（填写后执行下方命令）

| 源码路径 | 说明 |
|---------|------|
| TODO    | TODO |

## 执行命令

```bash
margi module add <路径>                 # 注册模块
margi module split <key> --depth 2     # 拆分大模块
margi module merge <a> <b> --into <c>  # 合并模块
margi module analyze <key>             # 为模块生成文档
```
