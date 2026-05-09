//! Markdown 标题切块 + 滑窗，与 Python `_parse_md_sections` / `_split_chunks` 等价。

use once_cell::sync::Lazy;
use regex::Regex;

static HEADING_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(#{1,6})\s+(.+?)\s*$").unwrap());
static MULTI_BLANK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

#[derive(Debug, Clone)]
pub struct IndexedChunk {
    pub rel: String,
    pub heading_path: String,
    pub body: String,
}

impl IndexedChunk {
    pub fn doc_for_match(&self) -> String {
        if self.heading_path.is_empty() {
            self.body.clone()
        } else {
            format!("{}\n\n{}", self.heading_path, self.body)
        }
    }
}

pub fn markdown_heading_present(text: &str) -> bool {
    static HEADING_ANY: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^#{1,6}\s+\S").unwrap());
    HEADING_ANY.is_match(text)
}

/// 字符级切分（按 Python 行为，按 char 计长度，避免中文被字节切碎）。
pub fn split_chunks(text: &str, mut max_chars: usize) -> Vec<String> {
    if max_chars < 200 {
        max_chars = 200;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&str> = MULTI_BLANK.split(trimmed).collect();
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        let chars: Vec<char> = p.chars().collect();
        if chars.len() <= max_chars {
            out.push(p.to_string());
            continue;
        }
        let step = std::cmp::max(max_chars.saturating_sub(120), 120);
        let mut i = 0;
        while i < chars.len() {
            let end = std::cmp::min(i + max_chars, chars.len());
            let piece: String = chars[i..end].iter().collect::<String>().trim().to_string();
            if !piece.is_empty() {
                out.push(piece);
            }
            i += step;
        }
    }
    out
}

pub fn parse_md_sections(text: &str) -> Vec<(String, String)> {
    let normalized = text.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut buffer: Vec<&str> = Vec::new();
    let mut out: Vec<(String, String)> = Vec::new();

    let path_str = |stack: &Vec<(usize, String)>| -> String {
        stack
            .iter()
            .map(|(_, t)| t.clone())
            .collect::<Vec<_>>()
            .join(" > ")
    };

    let flush = |stack: &Vec<(usize, String)>,
                 buffer: &mut Vec<&str>,
                 out: &mut Vec<(String, String)>| {
        if buffer.is_empty() && stack.is_empty() {
            return;
        }
        if stack.is_empty() {
            let body = buffer.join("\n").trim_end().to_string();
            if !body.is_empty() {
                out.push((String::new(), body));
            }
            buffer.clear();
            return;
        }
        let p = path_str(stack);
        let body = buffer.join("\n").trim_end().to_string();
        out.push((p, body));
        buffer.clear();
    };

    for line in lines {
        if let Some(caps) = HEADING_LINE.captures(line) {
            let level = caps.get(1).unwrap().as_str().len();
            let title = caps.get(2).unwrap().as_str().trim().to_string();
            flush(&stack, &mut buffer, &mut out);
            while let Some(&(lvl, _)) = stack.last() {
                if lvl >= level {
                    stack.pop();
                } else {
                    break;
                }
            }
            stack.push((level, title));
        } else {
            buffer.push(line);
        }
    }
    flush(&stack, &mut buffer, &mut out);
    out
}

/// 过滤空正文 / 仅水平分隔符（--- *** ___）的无意义片段。
pub fn is_meaningful_body(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return false;
    }
    static HR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[\-*_]{3,}$").unwrap());
    for ln in &lines {
        if HR.is_match(ln) {
            continue;
        }
        return true;
    }
    false
}

/// 切块通用实现。`filter_meaningless` 控制是否过滤"水平分隔线 / 空"等无意义片段。
/// 召回与嵌入两条路径都启用过滤（`true`），与 `dialogue_recall._wiki_indexed_chunks`
/// 以及修复后的 `vector_index._wiki_chunks` 行为一致。
pub fn wiki_indexed_chunks_with(
    rel: &str,
    full: &str,
    max_chars: usize,
    filter_meaningless: bool,
) -> Vec<IndexedChunk> {
    let full = full.replace("\r\n", "\n");
    if full.trim().is_empty() {
        return Vec::new();
    }
    if markdown_heading_present(&full) {
        let sections = parse_md_sections(&full);
        let mut out: Vec<IndexedChunk> = Vec::new();
        for (path, body) in sections {
            if filter_meaningless && !is_meaningful_body(&body) {
                continue;
            }
            let body_chars: Vec<char> = body.chars().collect();
            if body_chars.len() <= max_chars {
                out.push(IndexedChunk {
                    rel: rel.to_string(),
                    heading_path: path,
                    body,
                });
            } else {
                for piece in split_chunks(&body, max_chars) {
                    out.push(IndexedChunk {
                        rel: rel.to_string(),
                        heading_path: path.clone(),
                        body: piece,
                    });
                }
            }
        }
        return out;
    }
    split_chunks(&full, max_chars)
        .into_iter()
        .map(|c| IndexedChunk {
            rel: rel.to_string(),
            heading_path: String::new(),
            body: c,
        })
        .collect()
}

/// 召回与嵌入两条路径共用的切块入口：过滤"只有标题、全分隔符"等无意义片段。
pub fn wiki_indexed_chunks(rel: &str, full: &str, max_chars: usize) -> Vec<IndexedChunk> {
    wiki_indexed_chunks_with(rel, full, max_chars, true)
}
