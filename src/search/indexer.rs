use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::chunker::{chunk_file, CodeChunk};
use super::embed::{blob_to_vec, vec_to_blob, EmbedClient};
use crate::config::Config;
use crate::paths;

// ─────────────────────────────────────────────────────────────────────────────
// IndexMeta
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
pub struct IndexMeta {
    pub file_hashes: HashMap<String, String>,
    pub last_full_build: Option<i64>,
    pub chunk_count: usize,
    pub model: String,
    /// 向量维度（首次探测后写入，后续校验一致性）
    #[serde(default)]
    pub embedding_dim: Option<usize>,
}

impl IndexMeta {
    pub fn load(index_dir: &Path) -> Self {
        let p = index_dir.join("meta.json");
        if p.exists() {
            if let Ok(c) = std::fs::read_to_string(&p) {
                if let Ok(m) = serde_json::from_str(&c) {
                    return m;
                }
            }
        }
        IndexMeta::default()
    }
    pub fn save(&self, index_dir: &Path) -> Result<()> {
        std::fs::write(
            index_dir.join("meta.json"),
            serde_json::to_string_pretty(self)?,
        )?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Indexer
// ─────────────────────────────────────────────────────────────────────────────

pub struct Indexer {
    pub project_root: PathBuf,
    pub margi_dir: PathBuf,
    pub config: Config,
}

impl Indexer {
    pub fn new(project_root: PathBuf, margi_dir: PathBuf, config: Config) -> Self {
        Self {
            project_root,
            margi_dir,
            config,
        }
    }

    pub fn build(&self, force: bool) -> Result<()> {
        let index_dir = paths::index_dir(&self.margi_dir);
        paths::ensure_dir(&index_dir)?;

        let db_path = index_dir.join("chunks.db");
        let conn = open_db(&db_path)?;
        init_schema(&conn)?;

        let mut meta = if force {
            println!(
                "  {} {}",
                "→".cyan(),
                t!("强制全量重建", "Force full rebuild")
            );
            conn.execute("DELETE FROM chunks", [])?;
            conn.execute("DELETE FROM chunks_fts", [])?;
            conn.execute("DELETE FROM chunks_fts_cjk", [])?;
            conn.execute("DELETE FROM vec_chunks", [])?;
            IndexMeta::default()
        } else {
            IndexMeta::load(&index_dir)
        };

        // ── 向量客户端（可选）────────────────────────────────────────────────
        let embed_client = self.config.search.embedding.clone().map(EmbedClient::new);

        if embed_client.is_some() {
            println!(
                "  {} {} {}",
                "→".cyan(),
                t!("向量检索已启用，模型:", "Embedding enabled, model:"),
                self.config.search.embedding.as_ref().unwrap().model.cyan()
            );
        }

        // ── 源码文件扫描 ──────────────────────────────────────────────────────
        let source_root = self.config.source_root(&self.project_root);
        let scan_root = if source_root.exists() {
            &source_root
        } else {
            &self.project_root
        };
        let files = self.collect_files(scan_root);
        println!(
            "  {} {} {}",
            t!("扫描到", "Scanned"),
            files.len(),
            t!("个源文件", "source file(s)")
        );

        let mut updated = 0usize;
        let mut skipped = 0usize;
        let mut total_chunks = 0usize;
        // 收集需要 embed 的 (chunk_id, content) 对
        let mut pending_embed: Vec<(String, String)> = Vec::new();

        for (file_path, module_name) in &files {
            let rel = file_path
                .strip_prefix(&self.project_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

            let hash = file_hash(file_path);
            if !force
                && meta
                    .file_hashes
                    .get(&rel)
                    .map(|h| h == &hash)
                    .unwrap_or(false)
            {
                skipped += 1;
                continue;
            }

            conn.execute("DELETE FROM chunks WHERE file_path = ?1", params![rel])?;
            conn.execute("DELETE FROM chunks_fts WHERE file_path = ?1", params![rel])?;
            conn.execute(
                "DELETE FROM chunks_fts_cjk WHERE file_path = ?1",
                params![rel],
            )?;
            conn.execute(
                "DELETE FROM vec_chunks WHERE chunk_id = \
                          (SELECT id FROM chunks WHERE file_path = ?1)",
                params![rel],
            )?;

            let chunks = chunk_file(
                file_path,
                &self.project_root,
                module_name,
                self.config.search.chunk_size,
                self.config.search.chunk_overlap,
            );
            for chunk in &chunks {
                insert_chunk(&conn, chunk)?;
                if embed_client.is_some() {
                    pending_embed.push((chunk.id.clone(), chunk.content.clone()));
                }
                total_chunks += 1;
            }

            meta.file_hashes.insert(rel, hash);
            updated += 1;
        }

        meta.last_full_build = Some(Utc::now().timestamp());
        meta.chunk_count = total_chunks;
        meta.save(&index_dir)?;

        println!(
            "  {} {} {}, {} {}, {} {}",
            "✓".green(),
            updated,
            t!("文件已更新", "file(s) updated"),
            skipped,
            t!("已跳过", "skipped"),
            total_chunks,
            t!("代码块", "chunk(s)")
        );

        // ── 文档索引 ──────────────────────────────────────────────────────────
        println!(
            "  {} {}",
            "→".cyan(),
            t!("索引模块文档...", "Indexing module docs...")
        );
        let doc_count = self.index_docs(
            &conn,
            &mut meta,
            force,
            &mut pending_embed,
            embed_client.is_some(),
        )?;
        meta.save(&index_dir)?;
        println!(
            "  {} {} {}",
            "✓".green(),
            doc_count,
            t!("文档块已索引", "doc chunk(s) indexed")
        );

        // ── 批量 Embedding ────────────────────────────────────────────────────
        if let Some(client) = &embed_client {
            if !pending_embed.is_empty() {
                let dim = self.run_embedding(&conn, &mut meta, client, &pending_embed, force)?;
                meta.embedding_dim = Some(dim);
                meta.model = self
                    .config
                    .search
                    .embedding
                    .as_ref()
                    .map(|e| e.model.clone())
                    .unwrap_or_default();
                meta.save(&index_dir)?;
            }
        }

        Ok(())
    }

    /// 批量向量化并写入 vec_chunks 表
    fn run_embedding(
        &self,
        conn: &Connection,
        meta: &mut IndexMeta,
        client: &EmbedClient,
        pending: &[(String, String)],
        force: bool,
    ) -> Result<usize> {
        let cfg = self.config.search.embedding.as_ref().unwrap();
        let batch_size = cfg.batch_size.max(1);
        let total = pending.len();
        let mut done = 0usize;
        let mut detected_dim = meta.embedding_dim.unwrap_or(0);

        println!(
            "  {} {} {} ...",
            "→".cyan(),
            t!("向量化", "Embedding"),
            format!("{} chunks", total).bold()
        );

        // 检查维度一致性：若已有向量且维度不同，需要 --force 重建
        if !force && detected_dim > 0 {
            if let Some(existing_dim) = meta.embedding_dim {
                if existing_dim != detected_dim && detected_dim != 0 {
                    eprintln!(
                        "  {} {}",
                        "!".yellow(),
                        t!(
                            "向量维度与已有索引不一致，请执行 margi index build --force 重建",
                            "Embedding dim mismatch. Run: margi index build --force"
                        )
                    );
                    return Ok(existing_dim);
                }
            }
        }

        for batch in pending.chunks(batch_size) {
            let ids: Vec<&str> = batch.iter().map(|(id, _)| id.as_str()).collect();
            let texts: Vec<String> = batch.iter().map(|(_, t)| t.clone()).collect();

            let vecs = client
                .embed_batch(&texts)
                .with_context(|| format!("embedding batch {}/{}", done, total))?;

            for (id, vec) in ids.iter().zip(vecs.iter()) {
                if detected_dim == 0 {
                    detected_dim = vec.len();
                }
                let blob = vec_to_blob(vec);
                conn.execute(
                    "INSERT OR REPLACE INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
                    params![id, blob],
                )?;
            }
            done += batch.len();

            // 进度提示（每 200 个 chunk 打印一次）
            if done % 200 < batch_size {
                println!("    {}/{}", done, total);
            }
        }

        println!(
            "  {} {} {} {} {}",
            "✓".green(),
            done,
            t!("个向量已写入", "embeddings written"),
            t!("(维度", "(dim"),
            format!("{})", detected_dim)
        );

        Ok(detected_dim)
    }

    /// 索引 .margi/modules/ 下所有 .md 文档
    fn index_docs(
        &self,
        conn: &Connection,
        meta: &mut IndexMeta,
        force: bool,
        pending_embed: &mut Vec<(String, String)>,
        want_embed: bool,
    ) -> Result<usize> {
        let modules_root = self.margi_dir.join("modules");
        if !modules_root.exists() {
            return Ok(0);
        }

        if force {
            conn.execute("DELETE FROM doc_chunks", [])?;
            conn.execute("DELETE FROM doc_chunks_fts", [])?;
            conn.execute("DELETE FROM doc_chunks_fts_cjk", [])?;
        }

        let mut total = 0usize;

        for entry in walkdir::WalkDir::new(&modules_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "md" {
                continue;
            }

            use path_slash::PathExt as _;
            let rel = path
                .strip_prefix(&self.margi_dir)
                .map(|p| p.to_slash_lossy().to_string())
                .unwrap_or_else(|_| path.to_slash_lossy().to_string());

            let hash_key = format!("docs:{}", rel);
            let hash = file_hash(path);
            if !force
                && meta
                    .file_hashes
                    .get(&hash_key)
                    .map(|h| h == &hash)
                    .unwrap_or(false)
            {
                continue;
            }

            conn.execute(
                "DELETE FROM doc_chunks     WHERE file_path = ?1",
                params![rel],
            )?;
            conn.execute(
                "DELETE FROM doc_chunks_fts WHERE file_path = ?1",
                params![rel],
            )?;
            conn.execute(
                "DELETE FROM doc_chunks_fts_cjk WHERE file_path = ?1",
                params![rel],
            )?;

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let module = module_key_from_doc_rel(&rel);
            let chunks = chunk_doc(&content, &rel, &module);
            for chunk in &chunks {
                insert_doc_chunk(conn, chunk)?;
                if want_embed {
                    pending_embed.push((chunk.id.clone(), chunk.content.clone()));
                }
                total += 1;
            }

            meta.file_hashes.insert(hash_key, hash);
        }

        Ok(total)
    }

    pub fn stats(&self) -> Result<()> {
        let index_dir = paths::index_dir(&self.margi_dir);
        let meta = IndexMeta::load(&index_dir);

        crate::ui::title(&t!("索引统计", "Index Stats"));
        println!();
        println!("  {} {}", t!("代码块总数:", "Chunks:"), meta.chunk_count);
        println!(
            "  {} {}",
            t!("已索引文件:", "Files:"),
            meta.file_hashes.len()
        );
        if let Some(ts) = meta.last_full_build {
            if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
                println!(
                    "  {} {}",
                    t!("最后构建:", "Last build:"),
                    dt.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
        if meta.embedding_dim.is_some() || !meta.model.is_empty() {
            let dim_str = meta
                .embedding_dim
                .map(|d| format!(" (dim={})", d))
                .unwrap_or_default();
            println!(
                "  {} {}{}",
                t!("向量模型:", "Embedding model:"),
                meta.model,
                dim_str
            );
        } else {
            println!(
                "  {} {}",
                t!("向量检索:", "Embedding:"),
                t!(
                    "未配置（仅 BM25/bigram）",
                    "not configured (BM25/bigram only)"
                )
                .dimmed()
            );
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let index_dir = paths::index_dir(&self.margi_dir);
        for name in &["chunks.db", "meta.json"] {
            let p = index_dir.join(name);
            if p.exists() {
                std::fs::remove_file(&p)?;
            }
        }
        println!("{} {}", "✓".green(), t!("索引已清除", "Index cleared"));
        Ok(())
    }

    fn collect_files(&self, root: &Path) -> Vec<(PathBuf, String)> {
        use ignore::Walk;
        Walk::new(root)
            .flatten()
            .filter_map(|e| {
                let path = e.path().to_path_buf();
                if !path.is_file() {
                    return None;
                }
                if self.config.is_excluded(&path) {
                    return None;
                }
                if !is_indexable(&path) {
                    return None;
                }
                let module = self.infer_module(&path);
                Some((path, module))
            })
            .collect()
    }

    fn infer_module(&self, file_path: &Path) -> String {
        let src_root = self.config.source_root(&self.project_root);
        let rel = file_path
            .strip_prefix(&src_root)
            .or_else(|_| file_path.strip_prefix(&self.project_root))
            .unwrap_or(file_path);
        rel.components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_else(|| "root".to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Schema
// ─────────────────────────────────────────────────────────────────────────────

fn is_indexable(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    matches!(
        ext.as_deref(),
        Some(
            "rs" | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "vue"
                | "svelte"
                | "py"
                | "pyi"
                | "go"
                | "java"
                | "kt"
                | "kts"
                | "swift"
                | "cpp"
                | "cc"
                | "cxx"
                | "hpp"
                | "hxx"
                | "c"
                | "h"
                | "cs"
                | "rb"
                | "php"
                | "scala"
                | "sc"
                | "lua"
                | "sh"
                | "bash"
                | "md"
        )
    )
}

fn file_hash(path: &Path) -> String {
    if let Ok(content) = std::fs::read(path) {
        let mut h = Sha256::new();
        h.update(&content);
        hex::encode(h.finalize())
    } else {
        String::new()
    }
}

pub fn open_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS chunks (
            id           TEXT PRIMARY KEY,
            file_path    TEXT NOT NULL,
            module       TEXT NOT NULL DEFAULT '',
            start_line   INTEGER NOT NULL,
            end_line     INTEGER NOT NULL,
            content      TEXT NOT NULL,
            symbol_name  TEXT,
            last_indexed INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_file   ON chunks(file_path);
        CREATE INDEX IF NOT EXISTS idx_chunks_module ON chunks(module);

        -- trigram: 英文/代码符号子串搜索
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            id UNINDEXED, content, symbol_name,
            file_path UNINDEXED, module UNINDEXED,
            tokenize='trigram'
        );
        -- unicode61 + bigram: CJK 中文搜索
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts_cjk USING fts5(
            id UNINDEXED, content, symbol_name,
            file_path UNINDEXED, module UNINDEXED,
            tokenize='unicode61'
        );

        -- 向量存储：chunk_id → embedding BLOB（小端 IEEE 754 float32）
        CREATE TABLE IF NOT EXISTS vec_chunks (
            chunk_id  TEXT PRIMARY KEY,
            embedding BLOB NOT NULL
        );

        CREATE TABLE IF NOT EXISTS doc_chunks (
            id           TEXT PRIMARY KEY,
            file_path    TEXT NOT NULL,
            module       TEXT NOT NULL DEFAULT '',
            start_line   INTEGER NOT NULL,
            end_line     INTEGER NOT NULL,
            content      TEXT NOT NULL,
            section      TEXT,
            last_indexed INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_doc_chunks_module ON doc_chunks(module);
        CREATE VIRTUAL TABLE IF NOT EXISTS doc_chunks_fts USING fts5(
            id UNINDEXED, content, section,
            file_path UNINDEXED, module UNINDEXED,
            tokenize='trigram'
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS doc_chunks_fts_cjk USING fts5(
            id UNINDEXED, content, section,
            file_path UNINDEXED, module UNINDEXED,
            tokenize='unicode61'
        );
        -- 文档向量存储
        CREATE TABLE IF NOT EXISTS vec_doc_chunks (
            chunk_id  TEXT PRIMARY KEY,
            embedding BLOB NOT NULL
        );
    ",
    )?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 插入函数
// ─────────────────────────────────────────────────────────────────────────────

fn insert_chunk(conn: &Connection, chunk: &CodeChunk) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT OR REPLACE INTO chunks
            (id, file_path, module, start_line, end_line, content, symbol_name, last_indexed)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            chunk.id,
            chunk.file_path,
            chunk.module,
            chunk.start_line as i64,
            chunk.end_line as i64,
            chunk.content,
            chunk.symbol_name,
            now
        ],
    )?;
    // trigram FTS
    conn.execute(
        "INSERT OR REPLACE INTO chunks_fts (id, content, symbol_name, file_path, module)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            chunk.id,
            chunk.content,
            chunk.symbol_name.as_deref().unwrap_or(""),
            chunk.file_path,
            chunk.module
        ],
    )?;
    // bigram FTS（CJK）
    let bigrams = extract_cjk_bigrams(&chunk.content);
    let symbol_bigrams = chunk
        .symbol_name
        .as_deref()
        .map(|s| extract_cjk_bigrams(s))
        .unwrap_or_default();
    conn.execute(
        "INSERT OR REPLACE INTO chunks_fts_cjk (id, content, symbol_name, file_path, module)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            chunk.id,
            bigrams,
            symbol_bigrams,
            chunk.file_path,
            chunk.module
        ],
    )?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// CJK bigram 提取
// ─────────────────────────────────────────────────────────────────────────────

fn is_cjk_char(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
      | 0xAC00..=0xD7AF | 0xF900..=0xFAFF
    )
}

pub fn extract_cjk_bigrams(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut bigrams = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_cjk_char(chars[i]) {
            let start = i;
            while i < chars.len() && is_cjk_char(chars[i]) {
                i += 1;
            }
            let run = &chars[start..i];
            if run.len() == 1 {
                bigrams.push(run[0].to_string());
            } else {
                for k in 0..run.len() - 1 {
                    bigrams.push(format!("{}{}", run[k], run[k + 1]));
                }
            }
        } else {
            i += 1;
        }
    }
    bigrams.join(" ")
}

// ─────────────────────────────────────────────────────────────────────────────
// 文档分块与插入
// ─────────────────────────────────────────────────────────────────────────────

pub struct DocChunk {
    pub id: String,
    pub file_path: String,
    pub module: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub section: Option<String>,
}

fn module_key_from_doc_rel(rel: &str) -> String {
    let without_prefix = rel.strip_prefix("modules/").unwrap_or(rel);
    if let Some(pos) = without_prefix.rfind('/') {
        without_prefix[..pos].to_string()
    } else {
        without_prefix.trim_end_matches(".md").to_string()
    }
}

pub fn chunk_doc(content: &str, file_path: &str, module: &str) -> Vec<DocChunk> {
    let lines: Vec<&str> = content.lines().collect();
    let mut chunks = vec![];

    let mut headings: Vec<(usize, String)> = vec![];
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with('#') {
            let title = line.trim_start_matches('#').trim().to_string();
            if !title.is_empty() {
                headings.push((i, title));
            }
        }
    }

    if headings.is_empty() {
        let text = content.trim().to_string();
        if !text.is_empty() {
            chunks.push(DocChunk {
                id: format!("{}:1-{}", file_path, lines.len()),
                file_path: file_path.to_string(),
                module: module.to_string(),
                start_line: 1,
                end_line: lines.len(),
                content: text,
                section: None,
            });
        }
        return chunks;
    }

    for (i, (start, heading)) in headings.iter().enumerate() {
        let end = if i + 1 < headings.len() {
            headings[i + 1].0
        } else {
            lines.len()
        };
        let text = lines[*start..end].join("\n");
        if text.trim().is_empty() {
            continue;
        }
        chunks.push(DocChunk {
            id: format!("{}:{}-{}", file_path, start + 1, end),
            file_path: file_path.to_string(),
            module: module.to_string(),
            start_line: start + 1,
            end_line: end,
            content: text,
            section: Some(heading.clone()),
        });
    }
    chunks
}

fn insert_doc_chunk(conn: &Connection, chunk: &DocChunk) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT OR REPLACE INTO doc_chunks
            (id, file_path, module, start_line, end_line, content, section, last_indexed)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            chunk.id,
            chunk.file_path,
            chunk.module,
            chunk.start_line as i64,
            chunk.end_line as i64,
            chunk.content,
            chunk.section,
            now
        ],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO doc_chunks_fts (id, content, section, file_path, module)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            chunk.id,
            chunk.content,
            chunk.section.as_deref().unwrap_or(""),
            chunk.file_path,
            chunk.module
        ],
    )?;
    let bigrams = extract_cjk_bigrams(&chunk.content);
    let section_bigrams = chunk
        .section
        .as_deref()
        .map(|s| extract_cjk_bigrams(s))
        .unwrap_or_default();
    conn.execute(
        "INSERT OR REPLACE INTO doc_chunks_fts_cjk (id, content, section, file_path, module)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            chunk.id,
            bigrams,
            section_bigrams,
            chunk.file_path,
            chunk.module
        ],
    )?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 向量查询辅助（供 searcher 使用）
// ─────────────────────────────────────────────────────────────────────────────

/// 在 vec_chunks 中执行暴力余弦相似度扫描，返回 (chunk_id, score) top-N
/// 对中小型代码库（< 10万 chunk）性能足够；大型项目可后续换 ANN 索引
pub fn vec_search_source(
    conn: &Connection,
    query_vec: &[f32],
    module_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    vec_search_inner(
        conn,
        query_vec,
        module_filter,
        limit,
        "vec_chunks",
        "chunk_id",
        "chunks",
        "id",
    )
}

pub fn vec_search_doc(
    conn: &Connection,
    query_vec: &[f32],
    module_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    vec_search_inner(
        conn,
        query_vec,
        module_filter,
        limit,
        "vec_doc_chunks",
        "chunk_id",
        "doc_chunks",
        "id",
    )
}

fn vec_search_inner(
    conn: &Connection,
    query_vec: &[f32],
    module_filter: Option<&str>,
    limit: usize,
    vec_table: &str,
    vec_id_col: &str,
    data_table: &str,
    data_id_col: &str,
) -> Result<Vec<(String, f32)>> {
    // 读取所有向量，内存计算余弦相似度
    // 对 < 50k chunk 的代码库，全扫描 < 100ms，可接受
    let sql = format!(
        "SELECT v.{vec_id_col}, v.embedding \
         FROM {vec_table} v \
         JOIN {data_table} d ON d.{data_id_col} = v.{vec_id_col} \
         {}",
        if let Some(m) = module_filter {
            format!("WHERE d.module = '{}'", m.replace('\'', "''"))
        } else {
            String::new()
        }
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut scores: Vec<(String, f32)> = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, blob))
        })?
        .flatten()
        .map(|(id, blob)| {
            let vec = blob_to_vec(&blob);
            let score = crate::search::embed::cosine_similarity(query_vec, &vec);
            (id, score)
        })
        .collect();

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores.truncate(limit);
    Ok(scores)
}
