## 文件：C:/Users/mccube/Downloads/margi/margi/.margi/modules/src/search/internals.md

# src/search 模块内部实现

## 核心数据流

### 搜索执行流程
```
搜索请求
  ↓
查询预处理
  ↓
索引查找
  ↓
结果排序
  ↓
分页返回
```

### 索引构建流程
```
源码扫描
  ↓
文件分块
  ↓
内容提取
  ↓
倒排索引构建
  ↓
索引持久化
```

### 文件处理流程
```
文件发现
  ↓
格式识别
  ↓
文本提取
  ↓
分块处理
  ↓
索引更新
```

## 关键设计决策

### 1. 倒排索引
- 使用高效的倒排索引结构
- 支持多字段搜索（文件名、内容、路径）
- TF-IDF 算法计算相关度得分

### 2. 文件分块策略
- 智能分块，考虑代码结构
- 支持自定义分块大小
- 保留上下文信息

### 3. 搜索算法
- 使用 BM25 算法改进 TF-IDF
- 支持模糊搜索和拼写纠错
- 多线程并行处理提高性能

## 数据结构设计

### 倒排索引结构
```rust
struct InvertedIndex {
    terms: HashMap<String, Vec<Posting>>,
    documents: HashMap<PathBuf, DocumentInfo>,
}

struct Posting {
    doc_id: usize,
    positions: Vec<usize>,
    frequency: usize,
}

struct DocumentInfo {
    path: PathBuf,
    chunk_count: usize,
    last_modified: SystemTime,
}
```

### 内存缓存
```rust
struct SearchCache {
    index: Arc<InvertedIndex>,
    file_list: Vec<PathBuf>,
    config: SearchConfig,
}
```

## 性能优化

### 索引优化
- 增量更新索引，避免全量重建
- 内存映射文件减少 I/O 开销
- 压缩索引节省存储空间

### 搜索优化
- 使用布隆过滤器快速排除不相关文件
- 缓存热门查询结果
- 并行处理多个查询

### 文件处理优化
- 支持大文件的流式处理
- 延迟加载不常用的文件内容
- 批量处理文件减少系统调用

## 存储结构

### 索引文件存储
```
.margi/index/
├── main.index     # 主索引文件
├── documents.db   # 文档信息
├── chunks/        # 文件块数据
│   ├── chunk_1.db
│   └── chunk_2.db
└── meta.json      # 元数据
```