//! 向量索引：与 Python `app/services/vector_index.py` 100% 兼容的 JSON 结构。
//! 文件位置：`<data_root>/.pathy/wiki_embedding_index.json`，结构：
//!
//! ```json
//! {
//!   "files": {
//!     "<rel>": { "path", "content_hash", "status", "chunk_count", "updated_at", "embedding_model" }
//!   },
//!   "chunks": {
//!     "<chunk_id>": { "chunk_id", "path", "heading_path", "body", "updated_at", "vector": [..] }
//!   }
//! }
//! ```

use async_openai::types::{CreateEmbeddingRequestArgs, EmbeddingInput};
use chrono::Utc;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::Settings;
use crate::error::{AppError, AppResult};
use crate::llm::client::build_raw_client;
use crate::llm::config::{compute_effective_embedding_model, resolve_embedding_api_key};
use crate::models::LayerName;
use crate::recall::chunking::IndexedChunk;
use crate::storage;

const INDEX_FILE: &str = "wiki_embedding_index.json";

fn pathy_dir(data_root: &Path) -> PathBuf {
    data_root.join(".pathy")
}

fn index_path(data_root: &Path) -> AppResult<PathBuf> {
    let dir = pathy_dir(data_root);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(INDEX_FILE))
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

fn chunk_id(path: &str, heading_path: &str, body: &str) -> String {
    sha256_hex(&format!("{path}\n{heading_path}\n{body}"))
}

#[derive(Debug, Clone, Default)]
pub struct IndexShape {
    pub files: Map<String, Value>,
    pub chunks: Map<String, Value>,
}

fn load_index(data_root: &Path) -> AppResult<IndexShape> {
    let p = index_path(data_root)?;
    if !p.is_file() {
        return Ok(IndexShape::default());
    }
    let text = match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) => return Ok(IndexShape::default()),
    };
    let raw: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Ok(IndexShape::default()),
    };
    let mut shape = IndexShape::default();
    if let Some(obj) = raw.as_object() {
        if let Some(files) = obj.get("files").and_then(|v| v.as_object()) {
            shape.files = files.clone();
        }
        if let Some(chunks) = obj.get("chunks").and_then(|v| v.as_object()) {
            shape.chunks = chunks.clone();
        }
    }
    Ok(shape)
}

fn save_index(data_root: &Path, shape: &IndexShape) -> AppResult<()> {
    let p = index_path(data_root)?;
    let v = json!({ "files": shape.files, "chunks": shape.chunks });
    let mut s = serde_json::to_string_pretty(&v)?;
    s.push('\n');
    std::fs::write(&p, s)?;
    Ok(())
}

pub fn mark_wiki_file_stale(data_root: &Path, rel_path: &str) -> AppResult<()> {
    let mut shape = load_index(data_root)?;
    if let Some(Value::Object(m)) = shape.files.get_mut(rel_path) {
        m.insert("status".to_string(), Value::String("stale".to_string()));
    }
    save_index(data_root, &shape)
}

pub fn delete_wiki_vectors(data_root: &Path, rel_path: &str) -> AppResult<()> {
    let mut shape = load_index(data_root)?;
    let prefix = rel_path.trim_end_matches('/').to_string();

    let keys: Vec<String> = shape
        .files
        .keys()
        .filter(|p| {
            let pp = p.as_str();
            pp == prefix.as_str() || pp.starts_with(&format!("{prefix}/"))
        })
        .cloned()
        .collect();
    for k in keys {
        shape.files.remove(&k);
    }

    let to_del: Vec<String> = shape
        .chunks
        .iter()
        .filter_map(|(k, v)| {
            let p = v.as_object()?.get("path")?.as_str()?;
            if p == rel_path
                || (!prefix.is_empty() && p.starts_with(&format!("{prefix}/")))
            {
                Some(k.clone())
            } else {
                None
            }
        })
        .collect();
    for k in to_del {
        shape.chunks.remove(&k);
    }

    save_index(data_root, &shape)
}

pub fn get_wiki_embedding_status_map(data_root: &Path) -> AppResult<HashMap<String, String>> {
    let shape = load_index(data_root)?;
    let mut out: HashMap<String, String> = HashMap::new();
    for (path, meta) in &shape.files {
        if let Some(obj) = meta.as_object() {
            let st = obj
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let normalized = if matches!(st.as_str(), "embedded" | "stale" | "not_embedded") {
                st
            } else {
                "not_embedded".to_string()
            };
            out.insert(path.clone(), normalized);
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct VectorCandidate {
    pub rel: String,
    pub heading_path: String,
    pub body: String,
    pub score: f64,
}

fn cosine_dense(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

pub fn search_wiki_vectors(
    settings: &Settings,
    query_vector: &[f64],
    wiki_prefix: &str,
    top_n: u32,
) -> AppResult<Vec<VectorCandidate>> {
    let shape = load_index(&settings.data_root)?;
    let prefix = wiki_prefix.trim().trim_end_matches('/').to_string();
    let mut out: Vec<VectorCandidate> = Vec::new();

    for (_, chunk) in shape.chunks.iter() {
        let Some(obj) = chunk.as_object() else { continue };
        let rel = obj.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if rel.is_empty() {
            continue;
        }
        if !prefix.is_empty() && !rel.starts_with(&format!("{prefix}/")) && rel != prefix {
            continue;
        }
        let file_meta = shape.files.get(rel).and_then(|v| v.as_object());
        let status_ok = file_meta
            .and_then(|m| m.get("status"))
            .and_then(|v| v.as_str())
            .map(|s| s == "embedded")
            .unwrap_or(false);
        if !status_ok {
            continue;
        }
        let vec_arr = match obj.get("vector").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => continue,
        };
        let vec_f64: Vec<f64> = vec_arr.iter().filter_map(|x| x.as_f64()).collect();
        let score = cosine_dense(query_vector, &vec_f64);
        if score > 0.0 {
            out.push(VectorCandidate {
                rel: rel.to_string(),
                heading_path: obj
                    .get("heading_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                body: obj
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                score,
            });
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.rel.cmp(&b.rel))
            .then_with(|| a.heading_path.cmp(&b.heading_path))
    });
    out.truncate(top_n as usize);
    Ok(out)
}

/// 给 recall pipeline 调用：用 query 做 embedding 然后到本地索引检索；
/// 若未配置 embedding key、模型调用失败等，返回空结果（与 Python 行为一致）。
pub async fn score_chunks_vector(
    settings: &Settings,
    query: &str,
    wiki_prefix: &str,
    top_n: u32,
) -> Vec<(f64, IndexedChunk)> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let em = compute_effective_embedding_model(settings);
    let key = match resolve_embedding_api_key(settings) {
        Some(k) => k,
        None => {
            tracing::warn!("dialogue_recall vector embedding_api_key_missing");
            return Vec::new();
        }
    };
    let client = match build_raw_client(&key, em.base_url.as_deref(), em.timeout_seconds) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e.detail(), "dialogue_recall vector client build failed");
            return Vec::new();
        }
    };
    let req = match CreateEmbeddingRequestArgs::default()
        .model(&em.model)
        .input(EmbeddingInput::String(query.to_string()))
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "dialogue_recall vector req build failed");
            return Vec::new();
        }
    };
    let resp = match client.embeddings().create(req).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, model = %em.model, "dialogue_recall vector embedding_failed");
            return Vec::new();
        }
    };
    let qv: Vec<f64> = resp
        .data
        .first()
        .map(|d| d.embedding.iter().map(|x| *x as f64).collect())
        .unwrap_or_default();
    let cands = match search_wiki_vectors(settings, &qv, wiki_prefix, top_n) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e.detail(), "dialogue_recall vector search failed");
            return Vec::new();
        }
    };
    cands
        .into_iter()
        .map(|c| {
            (
                c.score,
                IndexedChunk {
                    rel: c.rel,
                    heading_path: c.heading_path,
                    body: c.body,
                },
            )
        })
        .collect()
}

/// 单文件嵌入：与 Python `embed_wiki_file` 行为等价。
pub async fn embed_wiki_file(
    settings: &Settings,
    rel_path: &str,
) -> AppResult<(u32, String, String)> {
    let rel = rel_path
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();
    if !rel.to_lowercase().ends_with(".md") {
        return Err(AppError::BadRequest("仅支持 .md 文件嵌入".to_string()));
    }
    let data_root = settings.data_root.clone();
    let (text, _) = storage::read_file(&data_root, LayerName::Wiki, &rel, settings.max_file_bytes)?;
    let model_cfg = compute_effective_embedding_model(settings);

    // 1200 与 Python 一致
    let chunks = wiki_chunks(&rel, &text, 1200);
    if chunks.is_empty() {
        delete_wiki_vectors(&data_root, &rel)?;
        return Ok((0, model_cfg.model.clone(), now_iso()));
    }

    let key = resolve_embedding_api_key(settings).ok_or_else(|| {
        AppError::ServiceUnavailable(
            "未配置 embedding API 密钥（EMBEDDING_API_KEY 或数据目录 .pathy/embedding_api_key）"
                .to_string(),
        )
    })?;
    let client = build_raw_client(&key, model_cfg.base_url.as_deref(), model_cfg.timeout_seconds)?;
    let inputs: Vec<String> = chunks
        .iter()
        .map(|(_, h, b)| {
            let s = format!("{h}\n\n{b}");
            // Python 截断为 8000 字符
            let chars: Vec<char> = s.trim().chars().collect();
            if chars.len() > 8000 {
                chars.into_iter().take(8000).collect::<String>()
            } else {
                s.trim().to_string()
            }
        })
        .collect();
    let req = CreateEmbeddingRequestArgs::default()
        .model(&model_cfg.model)
        .input(EmbeddingInput::StringArray(inputs))
        .build()
        .map_err(|e| AppError::Internal(format!("embedding req 构建失败：{e}")))?;
    let resp = client
        .embeddings()
        .create(req)
        .await
        .map_err(|e| AppError::BadGateway(format!("embedding 调用失败：{e}")))?;

    let mut shape = load_index(&data_root)?;
    // 删除旧 chunks
    let to_del: Vec<String> = shape
        .chunks
        .iter()
        .filter_map(|(k, v)| {
            let p = v.as_object()?.get("path")?.as_str()?;
            if p == rel {
                Some(k.clone())
            } else {
                None
            }
        })
        .collect();
    for k in to_del {
        shape.chunks.remove(&k);
    }

    let ts = now_iso();
    for (i, (_, hpath, body)) in chunks.iter().enumerate() {
        let cid = chunk_id(&rel, hpath, body);
        let vec: Vec<Value> = resp
            .data
            .get(i)
            .map(|d| d.embedding.iter().map(|x| Value::from(*x as f64)).collect())
            .unwrap_or_default();
        shape.chunks.insert(
            cid.clone(),
            json!({
                "chunk_id": cid,
                "path": rel,
                "heading_path": hpath,
                "body": body,
                "updated_at": ts,
                "vector": vec
            }),
        );
    }
    shape.files.insert(
        rel.clone(),
        json!({
            "path": rel,
            "content_hash": sha256_hex(&text),
            "status": "embedded",
            "chunk_count": chunks.len(),
            "updated_at": ts,
            "embedding_model": model_cfg.model
        }),
    );
    save_index(&data_root, &shape)?;
    Ok((chunks.len() as u32, model_cfg.model, ts))
}

fn wiki_chunks(rel: &str, full: &str, max_chars: usize) -> Vec<(String, String, String)> {
    // 嵌入路径同样过滤"只有标题、无正文"以及"全为分隔符"的 section：
    // - 避免把无意义片段写进向量索引（节省 embedding API 调用、降低召回噪声）
    // - 与召回路径行为一致
    // 注：这是对 Python `vector_index._wiki_chunks` 旧实现的修复——Python 端只在召回处过滤，
    // 嵌入处漏过滤，导致索引堆积"空标题"chunk。Python 同步修复见同文件。
    use crate::recall::chunking::wiki_indexed_chunks;
    wiki_indexed_chunks(rel, full, max_chars)
        .into_iter()
        .map(|c| (c.rel, c.heading_path, c.body))
        .collect()
}
