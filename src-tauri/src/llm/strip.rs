//! 复刻 Python `_strip_think_blocks`：去除 think / reasoning / 思考 等推理片段。

use once_cell::sync::Lazy;
use regex::Regex;

static THINK_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // (?si) = case insensitive + dotall
        Regex::new(r"(?si)<redacted_thinking\b[^>]*>.*?</think>").unwrap(),
        Regex::new(r"(?si)<think\b[^>]*>.*?</think>").unwrap(),
        Regex::new(r"(?si)<reasoning\b[^>]*>.*?</reasoning>").unwrap(),
        Regex::new(r"(?s)<思考[^>]*>.*?</思考>").unwrap(),
        // 整段 ``` think ``` 块
        Regex::new(r"(?ism)^\s*```\s*think\s*\r?\n.*?^\s*```\s*$").unwrap(),
    ]
});

static MULTI_NL: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

pub fn strip_think_blocks(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for _ in 0..64 {
        let prev = out.clone();
        for pat in THINK_PATTERNS.iter() {
            out = pat.replace_all(&out, "").into_owned();
        }
        if out == prev {
            break;
        }
    }
    let trimmed = out.trim();
    MULTI_NL.replace_all(trimmed, "\n\n").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_think_tags() {
        let s = "正文一<think>secret reasoning</think>正文二";
        assert_eq!(strip_think_blocks(s), "正文一正文二");
    }

    #[test]
    fn strips_chinese_think_block() {
        let s = "前\n<思考 a=1>多行\n推理</思考>\n后";
        assert_eq!(strip_think_blocks(s), "前\n\n后");
    }
}
