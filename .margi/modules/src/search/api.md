## 文件：C:/Users/mccube/Downloads/margi/margi/.margi/modules/src/search/api.md

# src/search 模块 API 文档

## 对外暴露的接口

### 搜索引擎
- `SearchEngine::new()` - 创建搜索引擎实例
- `SearchEngine::search()` - 执行全文搜索
- `SearchEngine::search_in_docs()` - 在文档中搜索
- `SearchEngine::build_index()` - 构建搜索索引

### 索引管理
- `IndexManager::create_index()` - 创建新索引
- `IndexManager::update_index()` - 更新现有索引
- `IndexManager::delete_index()` - 删除索引
- `IndexManager::get_index_info()` - 获取索引信息

### 文件分块
- `FileChunker::new()` - 创建文件分块器
- `FileChunker::chunk_file()` - 将文件分块
- `FileChunker::get_chunks()` - 获取所有块
- `Chunk::new()` - 创建文本块

### 搜索结果
```rust
pub struct SearchResult {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub content: String,
    pub snippet: String,
    pub score: f32,
}

pub struct SearchResults {
    pub total: usize,
    pub results: Vec<SearchResult>,
    pub query: String,
}
```

## 配置选项

### 搜索配置
```rust
pub struct SearchConfig {
    pub case_sensitive: bool,
    pub max_results: usize,
    pub min_score: f32,
    pub file_extensions: Vec<String>,
}
```

### 索引配置
```rust
pub struct IndexConfig {
    pub chunk_size: usize,
    pub max_file_size: usize,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
}
```

## 使用示例

### 基本搜索
```rust
let mut engine = SearchEngine::new();
let results = engine.search("关键词", &config);

for result in results {
    println!("Found in {}: {}", result.file_path, result.line_number);
    println!("Content: {}", result.snippet);
}
```

### 构建索引
```rust
let mut indexer = IndexManager::new();
indexer.build_index(&source_directory, &index_config);
```

### 文件分块
```rust
let chunker = FileChunker::new(&config);
let chunks = chunker.chunk_file(&file_path);
println!("Generated {} chunks", chunks.len());
```