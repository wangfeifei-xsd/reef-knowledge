//! 复刻 Python `app/services/recall_stopwords.py` 的内置 + 运行时停用词。

use once_cell::sync::Lazy;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::config::Settings;
use crate::error::AppResult;

const RUNTIME_STOPWORDS_REL: &str = ".pathy/recall_stopwords.txt";

pub static DEFAULT_STOPWORDS: Lazy<BTreeSet<&'static str>> = Lazy::new(|| {
    [
        // English
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "as", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "can",
        "this", "that", "these", "those", "it", "its", "i", "you", "he", "she", "we", "they",
        "what", "which", "who", "when", "where", "why", "how", "all", "each", "every", "both",
        "few", "more", "most", "other", "some", "such", "no", "nor", "not", "only", "own",
        "same", "so", "than", "too", "very", "just", "also", "now", "here", "there", "then",
        "if", "about", "into", "through", "during", "before", "after", "above", "below",
        "between", "under", "again", "further", "once", "any", "me", "him", "her", "us", "them",
        "my", "your", "his", "our", "their",
        // 常见英文技术泛词
        "api", "app",
        // 中文（高频虚词、疑问与口语）
        "的", "了", "和", "是", "在", "有", "就", "不", "人", "都", "一", "一个", "上", "也",
        "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这", "那",
        "这个", "那个", "这样", "什么", "怎么", "为什么", "哪", "哪些", "哪里", "谁", "几", "多",
        "少", "能", "可以", "应该", "如果", "因为", "所以", "但是", "而且", "或者", "还是", "与",
        "及", "等", "之", "为", "以", "对", "从", "把", "被", "让", "向", "将", "已", "还",
        "又", "再", "更", "最", "请", "问", "想", "知道", "告诉", "一下", "如何", "怎样", "是否",
        "有没有", "吗", "呢", "吧", "啊", "嘛", "呀", "哦", "嗯", "哈", "唉", "哎", "的话",
        "来说", "方面", "时候", "情况", "问题", "内容", "东西", "进行", "通过", "使用", "需要",
        "认为", "觉得", "希望", "帮助", "谢谢", "感谢",
    ]
    .into_iter()
    .collect()
});

pub fn runtime_stopwords_path(settings: &Settings) -> PathBuf {
    settings.data_root.join(RUNTIME_STOPWORDS_REL)
}

/// 与 Python `parse_stopwords_text` 等价：忽略空行与 `#` 注释；统一小写并去重保序。
pub fn parse_stopwords_text(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for raw in text.lines() {
        let w = raw.trim().to_lowercase();
        if w.is_empty() || w.starts_with('#') {
            continue;
        }
        if seen.insert(w.clone()) {
            out.push(w);
        }
    }
    out
}

pub fn read_runtime_stopwords(settings: &Settings) -> Vec<String> {
    let p = runtime_stopwords_path(settings);
    if !p.is_file() {
        return Vec::new();
    }
    match std::fs::read_to_string(&p) {
        Ok(s) => parse_stopwords_text(&s),
        Err(_) => Vec::new(),
    }
}

pub fn read_effective_stopwords(settings: &Settings) -> Vec<String> {
    let runtime = read_runtime_stopwords(settings);
    if !runtime.is_empty() {
        return runtime;
    }
    DEFAULT_STOPWORDS.iter().map(|s| s.to_string()).collect()
}

pub fn write_runtime_stopwords(settings: &Settings, words: &[String]) -> AppResult<(usize, String)> {
    let p = runtime_stopwords_path(settings);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut lines: Vec<String> = vec![
        "# Recall stopwords (one per line)".to_string(),
        "# Empty file means fallback to built-in defaults".to_string(),
        String::new(),
    ];
    lines.extend(words.iter().cloned());
    let mut text = lines.join("\n").trim_end().to_string();
    text.push('\n');
    std::fs::write(&p, text.as_bytes())?;
    Ok((words.len(), RUNTIME_STOPWORDS_REL.to_string()))
}

#[allow(dead_code)]
pub fn _ensure_path(p: &Path) {
    let _ = p;
}
