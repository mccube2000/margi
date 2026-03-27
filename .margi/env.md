# 环境 & 构建 — margi

> 由 `margi init` 生成骨架，请补充实际内容

## 开发环境启动

```bash
# 克隆项目后初始化
margi init

# 构建搜索索引
margi index build
```

## 构建 & 打包

```bash
# 发布版本构建（使用 locked 依赖版本）
cargo build --release --locked

# 安装到系统（或使用 --root 指定目录）
cargo install --path . --force --locked
```

## 测试执行

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test search::searcher
```

## 部署流程

1. 构建发布版本：`cargo build --release --locked`
2. 安装二进制文件：`cargo install --path . --force --locked`
3. 在目标机器上运行 `margi init` 初始化项目

## 常见问题

**Q: 索引构建失败？**
A: 确保 SQLite 扩展可用，且有足够的磁盘空间。

**Q: embedding 功能无法使用？**
A: 在 `.margi/config.json` 中配置正确的 embedding 服务地址和 API 密钥。

**Q: 搜索未返回预期结果？**
A: 运行 `margi index build --force` 重建索引，检查 `.margi/modules/` 中的模块配置。
