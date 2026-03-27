use anyhow::Result;
use colored::Colorize;
use rusqlite::params;
use std::collections::HashMap;

use crate::paths;
use crate::config::Config;
use super::indexer::{open_db, vec_search_source, vec_search_doc};
use super::embed::EmbedClient;

// ─────────────────────────────────────────────────────────────────────────────
// 结果结构体
// ─────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub rank:            usize,
    pub file_path:       String,
    pub module:          String,
    pub start_line:      usize,
    pub end_line:        usize,
    pub symbol_name:     Option<String>,
    pub content_preview: String,
    pub score:           f64,
}

#[derive(Debug, Clone)]
pub struct DocSearchResult {
    pub rank:            usize,
    pub file_path:       String,
    pub module:          String,
    pub start_line:      usize,
    pub section:         Option<String>,
    pub content_preview: String,
    pub score:           f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Query 分析
// ─────────────────────────────────────────────────────────────────────────────

fn is_cjk_char(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
      | 0xAC00..=0xD7AF | 0xF900..=0xFAFF
    )
}

#[allow(dead_code)]
fn is_cjk_query(query: &str) -> bool {
    query.chars().any(is_cjk_char)
}

/// 把 query 拆成 CJK 连续段和 ASCII 词。
/// "合并module"    → (["合并"],       ["module"])
/// "合并 模块"     → (["合并","模块"], [])
/// "parse合并user" → (["合并"],       ["parse","user"])
fn split_query(query: &str) -> (Vec<String>, Vec<String>) {
    let mut cjk:   Vec<String> = Vec::new();
    let mut ascii: Vec<String> = Vec::new();

    for word in query.split_whitespace() {
        let mut cur     = String::new();
        let mut cur_cjk: Option<bool> = None;
        for c in word.chars() {
            let c_is_cjk = is_cjk_char(c);
            match cur_cjk {
                None                           => { cur_cjk = Some(c_is_cjk); cur.push(c); }
                Some(p) if p == c_is_cjk       => { cur.push(c); }
                Some(p) => {
                    push_seg(&cur, p, &mut cjk, &mut ascii);
                    cur = c.to_string(); cur_cjk = Some(c_is_cjk);
                }
            }
        }
        if let Some(p) = cur_cjk { push_seg(&cur, p, &mut cjk, &mut ascii); }
    }
    (cjk, ascii)
}

fn push_seg(seg: &str, is_cjk: bool, cjk_out: &mut Vec<String>, ascii_out: &mut Vec<String>) {
    let s = seg.trim();
    if s.is_empty() { return; }
    if is_cjk {
        cjk_out.push(s.to_string());
    } else {
        let clean: String = s.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
        if !clean.is_empty() { ascii_out.push(clean); }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 查询字符串构造
//
// CJK 使用 bigram 策略：
//   "合并至少模块"  →  sliding bigrams: ["合并","并至","至少","少模","模块"]
//
//   AND 查询（严格，等价于子串搜索）:
//     "合并 AND 并至 AND 至少 AND 少模 AND 模块"
//     只有包含"合并至少模块"连续串的 chunk 才能命中。
//
//   OR 查询（宽松，BM25 按命中 bigram 数自然排名）:
//     "合并 OR 并至 OR 至少 OR 少模 OR 模块"
//     cmd_merge 里有"合并"和"模块"两个 bigram → 命中，得分低于精确匹配但排名靠前。
//
// ASCII 使用 trigram 表的 AND/OR 查询（与原逻辑一致）。
// ─────────────────────────────────────────────────────────────────────────────

/// 从 CJK 段列表提取滑动 bigram。
/// ["合并模块"] → ["合并","并模","模块"]
/// ["合并","模块"] → ["合并","模块"]  (两段各自提取)
fn sliding_bigrams(segs: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for seg in segs {
        let chars: Vec<char> = seg.chars().collect();
        if chars.is_empty() { continue; }
        if chars.len() == 1 {
            out.push(chars[0].to_string());
        } else {
            for i in 0..chars.len() - 1 {
                out.push(format!("{}{}", chars[i], chars[i + 1]));
            }
        }
    }
    out
}

fn cjk_and_query(segs: &[String]) -> String {
    let bg = sliding_bigrams(segs);
    if bg.is_empty() { return String::new(); }
    bg.join(" AND ")
}

fn cjk_or_query(segs: &[String]) -> String {
    let bg = sliding_bigrams(segs);
    if bg.is_empty() { return String::new(); }
    bg.join(" OR ")
}

fn ascii_and_query(words: &[String]) -> String {
    words.join(" AND ")
}

fn ascii_or_query(words: &[String]) -> String {
    words.join(" OR ")
}

// ─────────────────────────────────────────────────────────────────────────────
// RRF 合并（Reciprocal Rank Fusion, k=60）
// 用于混合查询（CJK 路 + ASCII 路）结果的合并。
// ─────────────────────────────────────────────────────────────────────────────

const RRF_K: f64 = 60.0;

fn rrf_source(lists: Vec<Vec<SearchResult>>, limit: usize) -> Vec<SearchResult> {
    if lists.len() == 1 {
        // 单路直接截取，保留原始 BM25 分数
        return lists.into_iter().next().unwrap()
            .into_iter().take(limit)
            .enumerate()
            .map(|(i, mut r)| { r.rank = i + 1; r })
            .collect();
    }
    let mut scores:  HashMap<String, f64>          = HashMap::new();
    let mut results: HashMap<String, SearchResult> = HashMap::new();
    for list in lists {
        for (rank, r) in list.into_iter().enumerate() {
            let key = format!("{}:{}", r.file_path, r.start_line);
            *scores.entry(key.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
            results.entry(key).or_insert(r);
        }
    }
    let mut merged: Vec<(f64, SearchResult)> = scores.into_iter()
        .filter_map(|(k, s)| results.remove(&k).map(|r| (s, r)))
        .collect();
    merged.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    merged.into_iter().take(limit)
        .enumerate()
        .map(|(i, (score, mut r))| { r.rank = i + 1; r.score = score; r })
        .collect()
}

fn rrf_doc(lists: Vec<Vec<DocSearchResult>>, limit: usize) -> Vec<DocSearchResult> {
    if lists.len() == 1 {
        return lists.into_iter().next().unwrap()
            .into_iter().take(limit)
            .enumerate()
            .map(|(i, mut r)| { r.rank = i + 1; r })
            .collect();
    }
    let mut scores:  HashMap<String, f64>             = HashMap::new();
    let mut results: HashMap<String, DocSearchResult> = HashMap::new();
    for list in lists {
        for (rank, r) in list.into_iter().enumerate() {
            let key = format!("{}:{}", r.file_path, r.start_line);
            *scores.entry(key.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
            results.entry(key).or_insert(r);
        }
    }
    let mut merged: Vec<(f64, DocSearchResult)> = scores.into_iter()
        .filter_map(|(k, s)| results.remove(&k).map(|r| (s, r)))
        .collect();
    merged.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    merged.into_iter().take(limit)
        .enumerate()
        .map(|(i, (score, mut r))| { r.rank = i + 1; r.score = score; r })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Searcher
// ─────────────────────────────────────────────────────────────────────────────

pub struct Searcher {
    margi_dir: std::path::PathBuf,
    config:    Config,
}

impl Searcher {
    pub fn new(margi_dir: std::path::PathBuf, config: Config) -> Self {
        Self { margi_dir, config }
    }

    pub fn search(
        &self, query: &str, exact: bool,
        module_filter: Option<&str>, mode: &str, limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let db_path = paths::index_dir(&self.margi_dir).join("chunks.db");
        if !db_path.exists() {
            return Err(crate::error::MargiError::IndexNotBuilt.into());
        }
        let conn = open_db(&db_path)?;

        if exact {
            let fts = format!("\"{}\"", query.replace('"', ""));
            return Ok(self.raw_source(&conn, &fts, "chunks_fts", module_filter, limit)?
                .into_iter().enumerate()
                .map(|(i, mut r)| { r.rank = i + 1; r })
                .collect());
        }
        if mode == "semantic" {
            eprintln!("{} {}",
                "→".yellow(),
                t!("语义搜索需要配置 embedding，当前使用关键词模式",
                   "Semantic search requires embedding config. Falling back to keyword mode."));
        }

        // 有 embedding 配置 → 混合检索
        if self.config.has_embedding() && mode != "keyword" {
            if let Some(emb_cfg) = &self.config.search.embedding {
                let client = EmbedClient::new(emb_cfg.clone());
                return self.hybrid_source(&conn, query, &client, module_filter, limit);
            }
        }

        self.smart_source(&conn, query, module_filter, limit)
    }

    pub fn search_docs(
        &self, query: &str,
        module_filter: Option<&str>, limit: usize,
    ) -> Result<Vec<DocSearchResult>> {
        let db_path = paths::index_dir(&self.margi_dir).join("chunks.db");
        if !db_path.exists() {
            return Err(crate::error::MargiError::IndexNotBuilt.into());
        }
        let conn = open_db(&db_path)?;

        // 有 embedding 配置 → 混合检索
        if self.config.has_embedding() {
            if let Some(emb_cfg) = &self.config.search.embedding {
                let client = EmbedClient::new(emb_cfg.clone());
                return self.hybrid_doc(&conn, query, &client, module_filter, limit);
            }
        }

        self.smart_doc(&conn, query, module_filter, limit)
    }

    // ── 混合检索（向量 + BM25，RRF 合并）────────────────────────────────────
    //
    // 策略：
    //   1. query embed → 余弦相似度扫描 vec_chunks → 向量候选
    //   2. smart_source BM25/bigram → 关键词候选
    //   3. RRF 合并（向量对语义近义词好，BM25 对精确名称/短词好）

    fn hybrid_source(
        &self,
        conn:          &rusqlite::Connection,
        query:         &str,
        client:        &EmbedClient,
        module_filter: Option<&str>,
        limit:         usize,
    ) -> Result<Vec<SearchResult>> {
        let fetch = (limit * 3).max(30);

        let vec_results = match client.embed_one(query) {
            Ok(qvec) => {
                let ids = vec_search_source(conn, &qvec, module_filter, fetch)?;
                self.ids_to_source_results(conn, &ids)?
            }
            Err(e) => {
                eprintln!("{} {}: {}", "!".yellow(),
                    t!("向量检索失败，回退到关键词", "Vector search failed, falling back"), e);
                vec![]
            }
        };

        let kw_results = self.smart_source(conn, query, module_filter, fetch)?;

        if vec_results.is_empty() {
            return Ok(kw_results.into_iter().take(limit)
                .enumerate().map(|(i, mut r)| { r.rank = i + 1; r }).collect());
        }
        if kw_results.is_empty() {
            return Ok(vec_results.into_iter().take(limit)
                .enumerate().map(|(i, mut r)| { r.rank = i + 1; r }).collect());
        }
        Ok(rrf_source(vec![vec_results, kw_results], limit))
    }

    fn hybrid_doc(
        &self,
        conn:          &rusqlite::Connection,
        query:         &str,
        client:        &EmbedClient,
        module_filter: Option<&str>,
        limit:         usize,
    ) -> Result<Vec<DocSearchResult>> {
        let fetch = (limit * 3).max(30);

        let vec_results = match client.embed_one(query) {
            Ok(qvec) => {
                let ids = vec_search_doc(conn, &qvec, module_filter, fetch)?;
                self.ids_to_doc_results(conn, &ids)?
            }
            Err(e) => {
                eprintln!("{} {}: {}", "!".yellow(),
                    t!("向量检索失败", "Vector search failed"), e);
                vec![]
            }
        };

        let kw_results = self.smart_doc(conn, query, module_filter, fetch)?;

        if vec_results.is_empty() {
            return Ok(kw_results.into_iter().take(limit)
                .enumerate().map(|(i, mut r)| { r.rank = i + 1; r }).collect());
        }
        if kw_results.is_empty() {
            return Ok(vec_results.into_iter().take(limit)
                .enumerate().map(|(i, mut r)| { r.rank = i + 1; r }).collect());
        }
        Ok(rrf_doc(vec![vec_results, kw_results], limit))
    }

    /// 把 vec_search 返回的 (id, score) 列表转成完整 SearchResult
    fn ids_to_source_results(
        &self,
        conn: &rusqlite::Connection,
        ids:  &[(String, f32)],
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::with_capacity(ids.len());
        for (id, score) in ids {
            let sql = "SELECT id, file_path, module, start_line, end_line, symbol_name, content \
                       FROM chunks WHERE id = ?1";
            if let Ok(row_r) = conn.query_row(sql, params![id], |row| {
                let content: String = row.get(6)?;
                Ok(SearchResult {
                    rank:            0,
                    file_path:       row.get(1)?,
                    module:          row.get(2)?,
                    start_line:      row.get::<_, i64>(3)? as usize,
                    end_line:        row.get::<_, i64>(4)? as usize,
                    symbol_name:     row.get(5)?,
                    content_preview: content.lines().take(5).collect::<Vec<_>>().join("\n"),
                    score:           *score as f64,
                })
            }) {
                results.push(row_r);
            }
        }
        Ok(results)
    }

    fn ids_to_doc_results(
        &self,
        conn: &rusqlite::Connection,
        ids:  &[(String, f32)],
    ) -> Result<Vec<DocSearchResult>> {
        let mut results = Vec::with_capacity(ids.len());
        for (id, score) in ids {
            let sql = "SELECT id, file_path, module, start_line, section, content \
                       FROM doc_chunks WHERE id = ?1";
            if let Ok(row_r) = conn.query_row(sql, params![id], |row| {
                let content: String = row.get(5)?;
                Ok(DocSearchResult {
                    rank:            0,
                    file_path:       row.get(1)?,
                    module:          row.get(2)?,
                    start_line:      row.get::<_, i64>(3)? as usize,
                    section:         row.get(4)?,
                    content_preview: content.lines().take(6).collect::<Vec<_>>().join("\n"),
                    score:           *score as f64,
                })
            }) {
                results.push(row_r);
            }
        }
        Ok(results)
    }

    // ── 主路由 ──────────────────────────────────────────────────────────────

    fn smart_source(
        &self, conn: &rusqlite::Connection,
        query: &str, module_filter: Option<&str>, limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let (cjk, ascii) = split_query(query);
        let fetch = (limit * 3).max(30);
        let mut lists: Vec<Vec<SearchResult>> = Vec::new();

        // ── CJK 路：AND 精确 → OR 宽松（bigram 表）─────────────────────────
        if !cjk.is_empty() {
            let q_and = cjk_and_query(&cjk);
            let q_or  = cjk_or_query(&cjk);
            if !q_and.is_empty() {
                let r = self.raw_source(conn, &q_and, "chunks_fts_cjk", module_filter, fetch)?;
                if !r.is_empty() {
                    lists.push(r);
                } else if q_or != q_and {
                    // AND 无结果，OR 宽松兜底
                    let r2 = self.raw_source(conn, &q_or, "chunks_fts_cjk", module_filter, fetch)?;
                    if !r2.is_empty() { lists.push(r2); }
                }
            }
        }

        // ── ASCII 路：AND 精确 → OR 宽松（trigram 表）──────────────────────
        // 短词（< 3 字符）trigram 无法索引，直接走 LIKE 全表扫描兜底
        if !ascii.is_empty() {
            if ascii.iter().all(|w| w.chars().count() < 3) {
                let r = self.like_source(conn, &ascii, module_filter, fetch)?;
                if !r.is_empty() { lists.push(r); }
            } else {
                let q_and = ascii_and_query(&ascii);
                let q_or  = ascii_or_query(&ascii);
                let r = self.raw_source(conn, &q_and, "chunks_fts", module_filter, fetch)?;
                if !r.is_empty() {
                    lists.push(r);
                } else {
                    let r2 = self.raw_source(conn, &q_or, "chunks_fts", module_filter, fetch)?;
                    if !r2.is_empty() { lists.push(r2); }
                }
            }
        }

        Ok(rrf_source(lists, limit))
    }

    fn smart_doc(
        &self, conn: &rusqlite::Connection,
        query: &str, module_filter: Option<&str>, limit: usize,
    ) -> Result<Vec<DocSearchResult>> {
        let (cjk, ascii) = split_query(query);
        let fetch = (limit * 3).max(30);
        let mut lists: Vec<Vec<DocSearchResult>> = Vec::new();

        if !cjk.is_empty() {
            let q_and = cjk_and_query(&cjk);
            let q_or  = cjk_or_query(&cjk);
            if !q_and.is_empty() {
                let r = self.raw_doc(conn, &q_and, "doc_chunks_fts_cjk", module_filter, fetch)?;
                if !r.is_empty() {
                    lists.push(r);
                } else if q_or != q_and {
                    let r2 = self.raw_doc(conn, &q_or, "doc_chunks_fts_cjk", module_filter, fetch)?;
                    if !r2.is_empty() { lists.push(r2); }
                }
            }
        }

        if !ascii.is_empty() {
            if ascii.iter().all(|w| w.chars().count() < 3) {
                let r = self.like_doc(conn, &ascii, module_filter, fetch)?;
                if !r.is_empty() { lists.push(r); }
            } else {
                let q_and = ascii_and_query(&ascii);
                let q_or  = ascii_or_query(&ascii);
                let r = self.raw_doc(conn, &q_and, "doc_chunks_fts", module_filter, fetch)?;
                if !r.is_empty() {
                    lists.push(r);
                } else {
                    let r2 = self.raw_doc(conn, &q_or, "doc_chunks_fts", module_filter, fetch)?;
                    if !r2.is_empty() { lists.push(r2); }
                }
            }
        }

        Ok(rrf_doc(lists, limit))
    }

    // ── 底层查询（操作单张 FTS 表）──────────────────────────────────────────

    fn raw_source(
        &self, conn: &rusqlite::Connection,
        fts_query: &str, table: &str,
        module_filter: Option<&str>, limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // symbol_name 权重 10×（列顺序：id UNINDEXED, content, symbol_name, ...）
        let sql_mod = format!("
            SELECT c.id, c.file_path, c.module, c.start_line, c.end_line,
                   c.symbol_name, c.content,
                   bm25({t}, 0.0, 1.0, 10.0, 0.0, 0.0) AS score
            FROM {t} f JOIN chunks c ON c.id = f.id
            WHERE {t} MATCH ?1 AND c.module = ?2
            ORDER BY score LIMIT ?3", t = table);
        let sql_all = format!("
            SELECT c.id, c.file_path, c.module, c.start_line, c.end_line,
                   c.symbol_name, c.content,
                   bm25({t}, 0.0, 1.0, 10.0, 0.0, 0.0) AS score
            FROM {t} f JOIN chunks c ON c.id = f.id
            WHERE {t} MATCH ?1
            ORDER BY score LIMIT ?2", t = table);

        if let Some(m) = module_filter {
            let mut s = conn.prepare(&sql_mod)?;
            let x: Vec<_> = s.query_map(params![fts_query, m, limit as i64], row_to_source)?
                .flatten().collect();
            Ok(x)
        } else {
            let mut s = conn.prepare(&sql_all)?;
            let x: Vec<_> = s.query_map(params![fts_query, limit as i64], row_to_source)?
                .flatten().collect();
            Ok(x)
        }
    }

    fn raw_doc(
        &self, conn: &rusqlite::Connection,
        fts_query: &str, table: &str,
        module_filter: Option<&str>, limit: usize,
    ) -> Result<Vec<DocSearchResult>> {
        // section 权重 5×
        let sql_mod = format!("
            SELECT d.id, d.file_path, d.module, d.start_line,
                   d.section, d.content,
                   bm25({t}, 0.0, 1.0, 5.0, 0.0, 0.0) AS score
            FROM {t} f JOIN doc_chunks d ON d.id = f.id
            WHERE {t} MATCH ?1 AND d.module = ?2
            ORDER BY score LIMIT ?3", t = table);
        let sql_all = format!("
            SELECT d.id, d.file_path, d.module, d.start_line,
                   d.section, d.content,
                   bm25({t}, 0.0, 1.0, 5.0, 0.0, 0.0) AS score
            FROM {t} f JOIN doc_chunks d ON d.id = f.id
            WHERE {t} MATCH ?1
            ORDER BY score LIMIT ?2", t = table);

        if let Some(m) = module_filter {
            let mut s = conn.prepare(&sql_mod)?;
            let x: Vec<_> = s.query_map(params![fts_query, m, limit as i64], row_to_doc)?
                .flatten().collect();
            Ok(x)
        } else {
            let mut s = conn.prepare(&sql_all)?;
            let x: Vec<_> = s.query_map(params![fts_query, limit as i64], row_to_doc)?
                .flatten().collect();
            Ok(x)
        }
    }

    // ── LIKE 兜底（短 ASCII 词 < 3 字符，trigram 无法索引）────────────────
    // 每个词要求同时出现在 content 或 symbol_name 中（大小写不敏感）。
    // 使用 INSTR(LOWER(...), LOWER(?)) 逐词 AND，无需改 schema。

    fn like_source(
        &self, conn: &rusqlite::Connection,
        words: &[String], module_filter: Option<&str>, limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let word_conds: String = (0..words.len())
            .map(|i| format!(
                "(INSTR(LOWER(c.content), LOWER(?{idx})) > 0 \
                  OR INSTR(LOWER(COALESCE(c.symbol_name,'')), LOWER(?{idx})) > 0)",
                idx = i + 1
            ))
            .collect::<Vec<_>>()
            .join(" AND ");

        let (module_cond, limit_idx) = if module_filter.is_some() {
            (format!(" AND c.module = ?{}", words.len() + 1), words.len() + 2)
        } else {
            (String::new(), words.len() + 1)
        };

        let sql = format!(
            "SELECT c.id, c.file_path, c.module, c.start_line, c.end_line,
                    c.symbol_name, c.content, 1.0 AS score
             FROM chunks c
             WHERE {word_conds}{module_cond}
             LIMIT ?{limit_idx}"
        );

        use rusqlite::types::ToSql;
        let mut bind: Vec<Box<dyn ToSql>> = words.iter()
            .map(|w| Box::new(w.clone()) as Box<dyn ToSql>)
            .collect();
        if let Some(m) = module_filter { bind.push(Box::new(m.to_string())); }
        bind.push(Box::new(limit as i64));

        let mut stmt = conn.prepare(&sql)?;
        let x: Vec<SearchResult> = stmt
            .query_map(rusqlite::params_from_iter(bind.iter().map(|b| b.as_ref())),
                       row_to_source)?
            .flatten().collect();
        Ok(x)
    }

    fn like_doc(
        &self, conn: &rusqlite::Connection,
        words: &[String], module_filter: Option<&str>, limit: usize,
    ) -> Result<Vec<DocSearchResult>> {
        let word_conds: String = (0..words.len())
            .map(|i| format!(
                "(INSTR(LOWER(d.content), LOWER(?{idx})) > 0 \
                  OR INSTR(LOWER(COALESCE(d.section,'')), LOWER(?{idx})) > 0)",
                idx = i + 1
            ))
            .collect::<Vec<_>>()
            .join(" AND ");

        let (module_cond, limit_idx) = if module_filter.is_some() {
            (format!(" AND d.module = ?{}", words.len() + 1), words.len() + 2)
        } else {
            (String::new(), words.len() + 1)
        };

        let sql = format!(
            "SELECT d.id, d.file_path, d.module, d.start_line,
                    d.section, d.content, 1.0 AS score
             FROM doc_chunks d
             WHERE {word_conds}{module_cond}
             LIMIT ?{limit_idx}"
        );

        use rusqlite::types::ToSql;
        let mut bind: Vec<Box<dyn ToSql>> = words.iter()
            .map(|w| Box::new(w.clone()) as Box<dyn ToSql>)
            .collect();
        if let Some(m) = module_filter { bind.push(Box::new(m.to_string())); }
        bind.push(Box::new(limit as i64));

        let mut stmt = conn.prepare(&sql)?;
        let x: Vec<DocSearchResult> = stmt
            .query_map(rusqlite::params_from_iter(bind.iter().map(|b| b.as_ref())),
                       row_to_doc)?
            .flatten().collect();
        Ok(x)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 行映射
// ─────────────────────────────────────────────────────────────────────────────

fn row_to_source(row: &rusqlite::Row) -> rusqlite::Result<SearchResult> {
    let content: String = row.get(6)?;
    Ok(SearchResult {
        rank:            0,
        file_path:       row.get(1)?,
        module:          row.get(2)?,
        start_line:      row.get::<_, i64>(3)? as usize,
        end_line:        row.get::<_, i64>(4)? as usize,
        symbol_name:     row.get(5)?,
        content_preview: content.lines().take(5).collect::<Vec<_>>().join("\n"),
        score:           row.get::<_, f64>(7).unwrap_or(0.0).abs(),
    })
}

fn row_to_doc(row: &rusqlite::Row) -> rusqlite::Result<DocSearchResult> {
    let content: String = row.get(5)?;
    Ok(DocSearchResult {
        rank:            0,
        file_path:       row.get(1)?,
        module:          row.get(2)?,
        start_line:      row.get::<_, i64>(3)? as usize,
        section:         row.get(4)?,
        content_preview: content.lines().take(6).collect::<Vec<_>>().join("\n"),
        score:           row.get::<_, f64>(6).unwrap_or(0.0).abs(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 结果打印（委托给 crate::ui）
// ─────────────────────────────────────────────────────────────────────────────

pub fn print_results(query: &str, results: &[SearchResult]) {
    crate::ui::print_source_results(query, results);
}

pub fn print_doc_results(query: &str, results: &[DocSearchResult]) {
    crate::ui::print_doc_results(query, results);
}
