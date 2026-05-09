//! BM25 + 标题路径命中加权，与 Python `_score_chunks_bm25` 等价。

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};

use super::chunking::IndexedChunk;

const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;
const TITLE_HIT_WEIGHT: f64 = 0.4;

static RE_WORD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[\u{4e00}-\u{9fff}]+|[a-zA-Z0-9]+").unwrap());

/// 复刻 Python `_extract_query_terms`：
/// - 切出中文连续段、英文数字段
/// - 英文数字段长度 ≥ 2 直接成词
/// - 中文：单字丢弃；长度 ≤ 8 整段成词；超过则同时生成相邻 bigram
/// - 全部小写、去重保序
pub fn extract_query_terms(q: &str) -> Vec<String> {
    let s = q.trim().to_lowercase();
    if s.is_empty() {
        return Vec::new();
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    static EN_NUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9]+$").unwrap());

    for m in RE_WORD.find_iter(&s) {
        let w = m.as_str().to_string();
        if EN_NUM.is_match(&w) {
            if w.chars().count() >= 2 && !seen.contains(&w) {
                seen.insert(w.clone());
                out.push(w);
            }
            continue;
        }
        let chars: Vec<char> = w.chars().collect();
        if chars.len() == 1 {
            continue;
        }
        if chars.len() <= 8 && !seen.contains(&w) {
            seen.insert(w.clone());
            out.push(w.clone());
        }
        for i in 0..chars.len().saturating_sub(1) {
            let bg: String = chars[i..i + 2].iter().collect();
            if !seen.contains(&bg) {
                seen.insert(bg.clone());
                out.push(bg);
            }
        }
    }
    out
}

pub fn filter_terms(terms: &[String], stopwords: &HashSet<String>) -> Vec<String> {
    terms
        .iter()
        .filter(|t| !t.is_empty() && !stopwords.contains(*t))
        .cloned()
        .collect()
}

fn bm25_idf(n_docs: usize, df: usize) -> f64 {
    ((n_docs as f64 - df as f64 + 0.5) / (df as f64 + 0.5) + 1.0).ln()
}

/// 子串频率：按 char 计的 substring count（与 Python `str.count` 等价）。
fn substring_count(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut start = 0;
    let mut cnt = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        cnt += 1;
        start += pos + needle.len();
    }
    cnt
}

pub fn score_chunks_bm25(
    chunks: &[IndexedChunk],
    terms: &[String],
) -> Vec<(f64, IndexedChunk)> {
    let n = chunks.len();
    if n == 0 || terms.is_empty() {
        return Vec::new();
    }
    let docs_lower: Vec<String> = chunks.iter().map(|c| c.doc_for_match().to_lowercase()).collect();
    let titles_lower: Vec<String> = chunks.iter().map(|c| c.heading_path.to_lowercase()).collect();
    let dls: Vec<f64> = docs_lower.iter().map(|d| d.chars().count() as f64).collect();
    let avgdl = if n > 0 {
        let sum: f64 = dls.iter().sum();
        if sum > 0.0 { sum / n as f64 } else { 1.0 }
    } else {
        1.0
    };

    let mut df_map: HashMap<String, usize> = HashMap::new();
    for t in terms {
        let df = docs_lower.iter().filter(|d| d.contains(t)).count();
        df_map.insert(t.clone(), df);
    }
    let mut idf_map: HashMap<String, f64> = HashMap::new();
    for t in terms {
        idf_map.insert(t.clone(), bm25_idf(n, *df_map.get(t).unwrap_or(&0)));
    }

    let mut scored: Vec<(f64, IndexedChunk)> = Vec::new();
    for (i, ch) in chunks.iter().enumerate() {
        let dl = dls[i];
        let doc = &docs_lower[i];
        let tit = &titles_lower[i];
        let mut s = 0.0_f64;
        for t in terms {
            let idf = *idf_map.get(t).unwrap_or(&0.0);
            let tf = substring_count(doc, t) as f64;
            if tf > 0.0 {
                let denom = tf + BM25_K1 * (1.0 - BM25_B + BM25_B * (dl / avgdl.max(1.0)));
                s += idf * (tf * (BM25_K1 + 1.0)) / denom;
            }
            let tft = substring_count(tit, t);
            if tft > 0 {
                s += TITLE_HIT_WEIGHT * idf * std::cmp::min(tft, 5) as f64;
            }
        }
        if s > 0.0 {
            scored.push((s, ch.clone()));
        }
    }
    scored
}
