//! HTTP Embedding 客户端
//!
//! 支持两种接口格式：
//! - OpenAI 兼容（/v1/embeddings）：OpenAI、vLLM、LocalAI 等
//! - Ollama（/api/embed）
//!
//! 自动按响应体结构区分，无需额外配置。

use anyhow::{anyhow, Context, Result};
use serde_json::json;

use crate::config::EmbeddingConfig;

// ─────────────────────────────────────────────────────────────────────────────
// 请求 / 响应结构体
// ─────────────────────────────────────────────────────────────────────────────

/// 内部统一结果
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// EmbedClient
// ─────────────────────────────────────────────────────────────────────────────

pub struct EmbedClient {
    cfg: EmbeddingConfig,
    client: reqwest::blocking::Client,
}

impl EmbedClient {
    pub fn new(cfg: EmbeddingConfig) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client");
        Self { cfg, client }
    }

    /// 对单条文本生成 embedding。
    /// 一般只在查询时用，建索引时用 `embed_batch`。
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        let mut results = self.embed_batch(&[text.to_string()])?;
        results
            .pop()
            .ok_or_else(|| anyhow!("embedding service returned empty result"))
    }

    /// 批量生成 embedding，一次 HTTP 请求发送 `texts` 中所有文本。
    /// 调用方应按 `cfg.batch_size` 拆分大批次。
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let resp = self
            .call_api(texts)
            .with_context(|| format!("embedding API call failed (url={})", self.cfg.url))?;

        if resp.embeddings.len() != texts.len() {
            return Err(anyhow!(
                "embedding count mismatch: sent {}, got {}",
                texts.len(),
                resp.embeddings.len()
            ));
        }
        Ok(resp.embeddings)
    }

    // ── 底层 HTTP 调用 ────────────────────────────────────────────────────────

    fn call_api(&self, texts: &[String]) -> Result<EmbedResponse> {
        // 构造请求体：Ollama 用 "input" 数组，OpenAI 也用 "input"
        let body = json!({
            "model": self.cfg.model,
            "input": texts,
        });

        let mut req = self
            .client
            .post(&self.cfg.url)
            .header("Content-Type", "application/json")
            .json(&body);

        if let Some(key) = &self.cfg.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().context("HTTP request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(anyhow!("embedding API returned {}: {}", status, body));
        }

        // 按响应结构自动判断接口类型
        let raw: serde_json::Value = response
            .json()
            .context("failed to parse embedding response as JSON")?;

        // Ollama: { "embeddings": [[...], ...] }
        if let Some(arr) = raw.get("embeddings").and_then(|v| v.as_array()) {
            let embeddings: Vec<Vec<f32>> = arr
                .iter()
                .map(|v| {
                    v.as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
                .collect();
            return Ok(EmbedResponse { embeddings });
        }

        // OpenAI: { "data": [{ "embedding": [...] }, ...] }
        if let Some(data) = raw.get("data").and_then(|v| v.as_array()) {
            let embeddings: Vec<Vec<f32>> = data
                .iter()
                .filter_map(|item| {
                    item.get("embedding")?.as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_f64().map(|f| f as f32))
                            .collect()
                    })
                })
                .collect();
            return Ok(EmbedResponse { embeddings });
        }

        Err(anyhow!(
            "unrecognized embedding API response format. Expected \
             Ollama {{embeddings:[...]}} or OpenAI {{data:[{{embedding:[...]}}]}}. \
             Got: {}",
            raw
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 向量工具函数
// ─────────────────────────────────────────────────────────────────────────────

/// 余弦相似度（向量已归一化时等价于点积）
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// 将 Vec<f32> 序列化为 BLOB（小端 IEEE 754）
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// 将 BLOB 反序列化为 Vec<f32>
pub fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
