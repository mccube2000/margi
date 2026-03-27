## 文件：C:/Users/mccube/Downloads/margi/margi/.margi/modules/src/memory/api.md

# src/memory 模块 API 文档

## 对外暴露的接口

### 记录管理
- `MemoryManager::new()` - 创建记忆管理器实例
- `MemoryManager::add_note()` - 添加新的笔记记录
- `MemoryManager::get_notes()` - 获取所有笔记
- `MemoryManager::search_notes()` - 搜索笔记内容

### 纠正记录管理
- `CorrectRecord::new()` - 创建新的纠正记录
- `CorrectionManager::add_correction()` - 添加纠正记录
- `CorrectionManager::get_corrections()` - 获取所有纠正记录
- `CorrectionManager::apply_correction()` - 应用纠正记录

### 数据结构
```rust
pub struct Note {
    pub id: String,
    pub content: String,
    pub module_key: String,
    pub timestamp: DateTime<Utc>,
    pub tags: Vec<String>,
}

pub struct CorrectRecord {
    pub id: String,
    pub wrong_way: String,
    pub right_way: String,
    pub module_key: String,
    pub timestamp: DateTime<Utc>,
}
```

### 核心功能
- 笔记的增删改查
- 纠正记录的持久化存储
- 模块级别的记忆分类
- 时间戳记录和检索

## 使用示例

### 基本笔记操作
```rust
let mut memory = MemoryManager::new();
let note = Note {
    id: uuid::Uuid::new_v4().to_string(),
    content: "这是一个重要笔记".to_string(),
    module_key: "example".to_string(),
    timestamp: Utc::now(),
    tags: vec!["important".to_string()],
};
memory.add_note(note);
```

### 纠正记录使用
```rust
let correction = CorrectRecord {
    id: uuid::Uuid::new_v4().to_string(),
    wrong_way: "不要这样做".to_string(),
    right_way: "应该这样做".to_string(),
    module_key: "example".to_string(),
    timestamp: Utc::now(),
};
correction_manager.add_correction(correction);
```