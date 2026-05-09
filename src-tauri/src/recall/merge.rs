//! 双路候选融合 + 轻量 rerank，与 Python `_merge_and_rerank_candidates` 等价。

use std::collections::BTreeMap;

use super::chunking::IndexedChunk;

#[derive(Debug, Clone)]
pub struct MergedCandidate {
    pub chunk: IndexedChunk,
    pub bm25_score: f64,
    pub vector_score: f64,
    pub rerank_score: f64,
}

fn min_max_norm(v: f64, lo: f64, hi: f64) -> f64 {
    if hi <= lo {
        return if v > 0.0 { 1.0 } else { 0.0 };
    }
    let x = (v - lo) / (hi - lo);
    x.clamp(0.0, 1.0)
}

pub fn merge_and_rerank(
    bm25_scored: &[(f64, IndexedChunk)],
    vector_scored: &[(f64, IndexedChunk)],
    terms: &[String],
) -> Vec<MergedCandidate> {
    type Key = (String, String, String);
    let key = |c: &IndexedChunk| (c.rel.clone(), c.heading_path.clone(), c.body.clone());

    // BTreeMap 让插入顺序无关，确定性输出
    let mut acc: BTreeMap<Key, (IndexedChunk, f64, f64)> = BTreeMap::new();
    for (sc, ch) in bm25_scored {
        let k = key(ch);
        let entry = acc.entry(k).or_insert_with(|| (ch.clone(), 0.0, 0.0));
        if *sc > entry.1 {
            entry.1 = *sc;
        }
    }
    for (sc, ch) in vector_scored {
        let k = key(ch);
        let entry = acc.entry(k).or_insert_with(|| (ch.clone(), 0.0, 0.0));
        if *sc > entry.2 {
            entry.2 = *sc;
        }
    }

    let bm_vals: Vec<f64> = acc.values().map(|v| v.1).collect();
    let vec_vals: Vec<f64> = acc.values().map(|v| v.2).collect();
    let (bm_lo, bm_hi) = if bm_vals.is_empty() {
        (0.0, 0.0)
    } else {
        (
            bm_vals.iter().cloned().fold(f64::INFINITY, f64::min),
            bm_vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (ve_lo, ve_hi) = if vec_vals.is_empty() {
        (0.0, 0.0)
    } else {
        (
            vec_vals.iter().cloned().fold(f64::INFINITY, f64::min),
            vec_vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };

    let mut out: Vec<MergedCandidate> = acc
        .into_iter()
        .map(|(_, (ch, bm, ve))| {
            let nb = min_max_norm(bm, bm_lo, bm_hi);
            let nv = min_max_norm(ve, ve_lo, ve_hi);
            let title_low = ch.heading_path.to_lowercase();
            let title_hit = if terms.iter().any(|t| !t.is_empty() && title_low.contains(t)) {
                1.0
            } else {
                0.0
            };
            let rr = 0.55 * nb + 0.4 * nv + 0.05 * title_hit;
            MergedCandidate {
                chunk: ch,
                bm25_score: bm,
                vector_score: ve,
                rerank_score: rr,
            }
        })
        .collect();

    out.sort_by(|a, b| {
        b.rerank_score
            .partial_cmp(&a.rerank_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.bm25_score
                    .partial_cmp(&a.bm25_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                b.vector_score
                    .partial_cmp(&a.vector_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.chunk.rel.cmp(&b.chunk.rel))
            .then_with(|| a.chunk.heading_path.cmp(&b.chunk.heading_path))
    });
    out
}
