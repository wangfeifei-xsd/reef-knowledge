//! 与 Python 参考实现的对齐校验。
//! 用法：
//! ```bash
//! # 1) 在 pathy-knowledge-server 项目下生成金标准 JSON
//! python ../reef-knowledge/scripts/verify_parity.py --emit-golden /tmp/parity.json
//!
//! # 2) 在 reef-knowledge/src-tauri 下跑此测试
//! PARITY_GOLDEN=/tmp/parity.json cargo test --test parity_test -- --nocapture
//! ```
//! 若环境变量 `PARITY_GOLDEN` 未设置，测试会被跳过（不影响 CI 默认行为）。

use std::collections::HashSet;
use std::path::PathBuf;

use reef_knowledge_lib::llm::strip::strip_think_blocks;
use reef_knowledge_lib::recall::bm25::{
    extract_query_terms, filter_terms, score_chunks_bm25,
};
use reef_knowledge_lib::recall::chunking::{
    is_meaningful_body, markdown_heading_present, parse_md_sections, split_chunks,
    wiki_indexed_chunks, IndexedChunk,
};
use reef_knowledge_lib::recall::stopwords::DEFAULT_STOPWORDS;
use serde_json::Value;

const SAMPLE_QUERIES: &[&str] = &[
    "如何配置 OpenAI API base_url 和 model 参数？",
    "What is BM25?",
    "向量召回的 top_k_chunks 怎么调整",
    "",
    "the and or",
    "RAG 与 wiki 编译流程",
];

const SAMPLE_MARKDOWN: &str = "# 标题一\n\n\
正文一段，不含标题之外内容。\n\n\
## 子标题\n\n\
- 列表项 A\n- 列表项 B\n\n\
---\n\n\
# 标题二\n\n\
```python\nprint(\"hello\")\n```\n\n\
正文二\n";

const SAMPLE_THINK: &str =
    "前缀<think>这是隐藏推理</think>正文一<reasoning attr='x'>多行\n推理</reasoning>正文二\n<思考>中文思考块</思考>结尾";

fn no_heading_sample() -> String {
    let one = "没有标题的纯文本。\n\n第二段，应当作为单独的滑窗块。\n\n第三段。\n";
    one.repeat(30)
}

fn chunk_to_json(c: &IndexedChunk) -> Value {
    serde_json::json!({
        "rel": c.rel,
        "heading_path": c.heading_path,
        "body": c.body,
    })
}

fn round6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

#[test]
fn parity_with_python_reference() {
    let Some(golden_path) = std::env::var_os("PARITY_GOLDEN") else {
        eprintln!("[skip] PARITY_GOLDEN 未设置，跳过对齐测试。");
        return;
    };
    let path = PathBuf::from(golden_path);
    let raw = std::fs::read_to_string(&path).expect("读取 golden 文件失败");
    let golden: Value = serde_json::from_str(&raw).expect("golden JSON 解析失败");

    // 1) query terms
    let stopwords: HashSet<String> = DEFAULT_STOPWORDS
        .iter()
        .map(|s| s.to_string())
        .collect();
    let qt = golden.get("query_terms").and_then(|v| v.as_array()).unwrap();
    assert_eq!(qt.len(), SAMPLE_QUERIES.len(), "query 用例数量不一致");
    for (i, q) in SAMPLE_QUERIES.iter().enumerate() {
        let raw_terms = extract_query_terms(q);
        let kept = filter_terms(&raw_terms, &stopwords);
        let g = &qt[i];
        assert_eq!(g["query"].as_str().unwrap(), *q);
        let g_raw: Vec<String> = g["raw"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        assert_eq!(raw_terms, g_raw, "query[{i}] raw terms 不一致");
        let g_kept: Vec<String> = g["kept"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        assert_eq!(kept, g_kept, "query[{i}] kept terms 不一致");
    }

    // 2) markdown
    assert_eq!(
        markdown_heading_present(SAMPLE_MARKDOWN),
        golden["markdown_heading_present"].as_bool().unwrap(),
    );
    let sections = parse_md_sections(SAMPLE_MARKDOWN);
    let g_sections = golden["md_sections"].as_array().unwrap();
    assert_eq!(sections.len(), g_sections.len(), "md_sections 段数不一致");
    for (i, (path, body)) in sections.iter().enumerate() {
        let g = g_sections[i].as_array().unwrap();
        assert_eq!(path, g[0].as_str().unwrap(), "section[{i}].path");
        assert_eq!(body, g[1].as_str().unwrap(), "section[{i}].body");
    }

    let chunks = wiki_indexed_chunks("doc.md", SAMPLE_MARKDOWN, 1200);
    let g_chunks = golden["chunking_with_filter"].as_array().unwrap();
    assert_eq!(chunks.len(), g_chunks.len(), "chunking 数量不一致");
    for (i, c) in chunks.iter().enumerate() {
        assert_eq!(chunk_to_json(c), g_chunks[i], "chunk[{i}] 不一致");
    }

    // 3) split no heading
    let no_h = no_heading_sample();
    let split = split_chunks(&no_h, 1200);
    let g_split: Vec<String> = golden["split_no_heading"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(split, g_split, "split_chunks 不一致");

    // 4) is_meaningful_body
    let mb = &golden["is_meaningful_body"];
    assert_eq!(is_meaningful_body(""), mb["empty"].as_bool().unwrap());
    assert_eq!(is_meaningful_body("   \n  "), mb["blank"].as_bool().unwrap());
    assert_eq!(is_meaningful_body("---"), mb["rule_only"].as_bool().unwrap());
    assert_eq!(
        is_meaningful_body("***\n___"),
        mb["stars_only"].as_bool().unwrap()
    );
    assert_eq!(is_meaningful_body("实际正文"), mb["real"].as_bool().unwrap());

    // 5) strip think
    let stripped = strip_think_blocks(SAMPLE_THINK);
    assert_eq!(stripped, golden["strip_think"].as_str().unwrap());

    // 6) BM25
    let bm = score_chunks_bm25(
        &chunks,
        &["标题".to_string(), "正文".to_string(), "列表".to_string()],
    );
    let g_bm = golden["bm25_scores"].as_array().unwrap();
    assert_eq!(bm.len(), g_bm.len(), "BM25 候选数不一致");
    for (i, (score, c)) in bm.iter().enumerate() {
        let g = &g_bm[i];
        assert_eq!(round6(*score), g["score"].as_f64().unwrap(), "BM25[{i}] score");
        assert_eq!(c.rel, g["rel"].as_str().unwrap());
        assert_eq!(c.heading_path, g["heading_path"].as_str().unwrap());
        assert_eq!(c.body, g["body"].as_str().unwrap());
    }

    eprintln!("[ok] 与 Python 参考实现完全一致。");
}
