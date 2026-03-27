use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub version: String,
    pub project: ProjectConfig,
    pub modules: ModulesConfig,
    pub search: SearchConfig,
    pub memory: MemoryConfig,
    pub hooks: HooksConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub root: String,
    pub exclude: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModulesConfig {
    pub auto_detect: bool,
    pub scan_depth: usize,
    pub manual: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    /// 向量检索配置（可选）。未配置时只使用 BM25/bigram 全文检索。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<EmbeddingConfig>,
}

/// 向量检索配置
///
/// 支持任何 OpenAI 兼容的 embedding 接口，包括 Ollama / vLLM / OpenAI。
///
/// 示例（Ollama）：
/// ```json
/// {
///   "url":   "http://localhost:11434/api/embed",
///   "model": "nomic-embed-text"
/// }
/// ```
///
/// 示例（OpenAI）：
/// ```json
/// {
///   "url":    "https://api.openai.com/v1/embeddings",
///   "model":  "text-embedding-3-small",
///   "api_key": "sk-..."
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmbeddingConfig {
    /// embedding 服务端点
    pub url: String,
    /// 模型名称，传给服务端
    pub model: String,
    /// 可选 API Key（Bearer token）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// 批量请求大小，默认 32
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// 向量维度（首次 build 时自动探测并写入 meta.json，无需手填）
    #[serde(default)]
    pub dim: Option<usize>,
}

fn default_batch_size() -> usize {
    32
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConfig {
    pub auto_sync_to_agents_md: bool,
    pub max_global_notes_in_agents_md: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HooksConfig {
    pub post_commit: bool,
    pub outdated_check_on_diff: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: "1.0".to_string(),
            project: ProjectConfig {
                name: detect_project_name(),
                root: "src/".to_string(),
                exclude: vec![
                    "node_modules".to_string(),
                    "dist".to_string(),
                    "target".to_string(),
                    ".git".to_string(),
                    ".nuxt".to_string(),
                    ".next".to_string(),
                    ".output".to_string(),
                    "coverage".to_string(),
                    "*.test.ts".to_string(),
                    "*.spec.ts".to_string(),
                    "*.test.rs".to_string(),
                ],
            },
            modules: ModulesConfig {
                auto_detect: true,
                scan_depth: 3, // 支持 components/auth 等多层结构
                manual: vec![],
            },
            search: SearchConfig {
                chunk_size: 150,
                chunk_overlap: 20,
                embedding: None,
            },
            memory: MemoryConfig {
                auto_sync_to_agents_md: true,
                max_global_notes_in_agents_md: 10,
            },
            hooks: HooksConfig {
                post_commit: true,
                outdated_check_on_diff: true,
            },
        }
    }
}

fn detect_project_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "my-project".to_string())
}

impl Config {
    pub fn load(margi_root: &Path) -> Result<Self> {
        let config_path = margi_root.join("config.json");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            serde_json::from_str(&content).map_err(|e| {
                crate::error::MargiError::ConfigError(t!(
                    format!("config.json 解析失败: {}", e),
                    format!("Failed to parse config.json: {}", e)
                ))
                .into()
            })
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self, margi_root: &Path) -> Result<()> {
        let config_path = margi_root.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        for pattern in &self.project.exclude {
            if pattern.starts_with('*') {
                let ext = &pattern[1..];
                if path_str.ends_with(ext) {
                    return true;
                }
            } else if path_str.contains(pattern.as_str()) {
                return true;
            }
        }
        false
    }

    pub fn source_root(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.project.root)
    }

    /// 是否已配置向量检索
    pub fn has_embedding(&self) -> bool {
        self.search
            .embedding
            .as_ref()
            .map(|e| !e.url.is_empty() && !e.model.is_empty())
            .unwrap_or(false)
    }
}
