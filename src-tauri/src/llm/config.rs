//! 复刻 Python `app/services/llm_config.py`：
//! - 三级合并：env > `.pathy/llm.json` > Settings 默认
//! - 三套独立 endpoint：LLM / Embedding / Rerank
//! - 密钥三处来源：env > Settings（含 .env） > `.pathy/<*>_api_key` 文件

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use crate::config::Settings;
use crate::error::AppResult;

pub const LLM_JSON_NAME: &str = "llm.json";
pub const KEY_FILE_NAME: &str = "openai_api_key";
pub const EMBEDDING_KEY_FILE_NAME: &str = "embedding_api_key";
pub const RERANK_KEY_FILE_NAME: &str = "rerank_api_key";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSource {
    Env,
    File,
    Default,
}

impl FieldSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldSource::Env => "env",
            FieldSource::File => "file",
            FieldSource::Default => "default",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EffectiveLLM {
    pub model: String,
    pub model_source: FieldSource,
    pub base_url: Option<String>,
    pub base_url_source: FieldSource,
    pub timeout_seconds: f64,
    pub timeout_source: FieldSource,
    pub max_tokens: u32,
    pub max_tokens_source: FieldSource,
}

#[derive(Debug, Clone)]
pub struct EffectiveModelEndpoint {
    pub model: String,
    pub model_source: FieldSource,
    pub base_url: Option<String>,
    pub base_url_source: FieldSource,
    pub timeout_seconds: f64,
    pub timeout_source: FieldSource,
    pub max_tokens: u32,
    pub max_tokens_source: FieldSource,
}

fn pathy_dir(data_root: &Path) -> PathBuf {
    data_root.join(".pathy")
}

pub fn llm_json_path(data_root: &Path) -> PathBuf {
    pathy_dir(data_root).join(LLM_JSON_NAME)
}

pub fn api_key_file_path(data_root: &Path) -> PathBuf {
    pathy_dir(data_root).join(KEY_FILE_NAME)
}

pub fn embedding_api_key_file_path(data_root: &Path) -> PathBuf {
    pathy_dir(data_root).join(EMBEDDING_KEY_FILE_NAME)
}

pub fn rerank_api_key_file_path(data_root: &Path) -> PathBuf {
    pathy_dir(data_root).join(RERANK_KEY_FILE_NAME)
}

pub fn load_llm_json(data_root: &Path) -> Map<String, Value> {
    let p = llm_json_path(data_root);
    if !p.is_file() {
        return Map::new();
    }
    let raw = match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) => return Map::new(),
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

pub fn save_llm_json(data_root: &Path, data: &Map<String, Value>) -> AppResult<()> {
    let dir = pathy_dir(data_root);
    std::fs::create_dir_all(&dir)?;
    let p = llm_json_path(data_root);
    let mut out = serde_json::to_string_pretty(data)?;
    out.push('\n');
    std::fs::write(&p, out)?;
    Ok(())
}

fn pick_str(
    env_name: &str,
    file: &Map<String, Value>,
    file_key: &str,
    settings_value: Option<&str>,
    default_when_missing: Option<&str>,
) -> (Option<String>, FieldSource) {
    if let Ok(v) = env::var(env_name) {
        if !v.is_empty() {
            return (Some(v), FieldSource::Env);
        }
        return (None, FieldSource::Env);
    }
    if let Some(v) = file.get(file_key) {
        if !v.is_null() {
            let s = match v {
                Value::String(s) => s.trim().to_string(),
                _ => v.to_string(),
            };
            return (
                if s.is_empty() { None } else { Some(s) },
                FieldSource::File,
            );
        }
    }
    if let Some(s) = settings_value {
        if !s.is_empty() {
            return (Some(s.to_string()), FieldSource::Default);
        }
    }
    (
        default_when_missing.map(|x| x.to_string()),
        FieldSource::Default,
    )
}

fn pick_f64(
    env_name: &str,
    file: &Map<String, Value>,
    file_key: &str,
    settings_value: f64,
) -> (f64, FieldSource) {
    if let Ok(v) = env::var(env_name) {
        if let Ok(x) = v.parse::<f64>() {
            return (x, FieldSource::Env);
        }
    }
    if let Some(v) = file.get(file_key).and_then(|v| v.as_f64()) {
        return (v, FieldSource::File);
    }
    (settings_value, FieldSource::Default)
}

fn pick_u32(
    env_name: &str,
    file: &Map<String, Value>,
    file_key: &str,
    settings_value: u32,
) -> (u32, FieldSource) {
    if let Ok(v) = env::var(env_name) {
        if let Ok(x) = v.parse::<u32>() {
            return (x, FieldSource::Env);
        }
    }
    if let Some(v) = file.get(file_key).and_then(|v| v.as_u64()) {
        return (v as u32, FieldSource::File);
    }
    (settings_value, FieldSource::Default)
}

pub fn compute_effective_llm(settings: &Settings) -> EffectiveLLM {
    let f = load_llm_json(&settings.data_root);
    let (model, ms) = pick_str(
        "OPENAI_MODEL",
        &f,
        "openai_model",
        Some(&settings.openai_model),
        Some(&settings.openai_model),
    );
    let model = model.expect("openai_model must have default");
    let (base_url, bs) = pick_str(
        "OPENAI_BASE_URL",
        &f,
        "openai_base_url",
        settings.openai_base_url.as_deref(),
        None,
    );
    let (timeout, ts) = pick_f64(
        "OPENAI_TIMEOUT",
        &f,
        "openai_timeout_seconds",
        settings.openai_timeout_seconds,
    );
    let (max_tokens, mt) = pick_u32(
        "OPENAI_MAX_TOKENS",
        &f,
        "openai_max_tokens",
        settings.openai_max_tokens,
    );
    EffectiveLLM {
        model,
        model_source: ms,
        base_url,
        base_url_source: bs,
        timeout_seconds: timeout,
        timeout_source: ts,
        max_tokens,
        max_tokens_source: mt,
    }
}

pub fn compute_effective_embedding_model(settings: &Settings) -> EffectiveModelEndpoint {
    let f = load_llm_json(&settings.data_root);
    let (model, ms) = pick_str(
        "EMBEDDING_MODEL",
        &f,
        "embedding_model",
        Some(&settings.embedding_model),
        Some(&settings.embedding_model),
    );
    let (base_url, bs) = pick_str(
        "EMBEDDING_BASE_URL",
        &f,
        "embedding_base_url",
        settings.embedding_base_url.as_deref(),
        None,
    );
    let (timeout, ts) = pick_f64(
        "EMBEDDING_TIMEOUT",
        &f,
        "embedding_timeout_seconds",
        settings.embedding_timeout_seconds,
    );
    let (max_tokens, mt) = pick_u32(
        "EMBEDDING_MAX_TOKENS",
        &f,
        "embedding_max_tokens",
        settings.embedding_max_tokens,
    );
    EffectiveModelEndpoint {
        model: model.unwrap_or_default(),
        model_source: ms,
        base_url,
        base_url_source: bs,
        timeout_seconds: timeout,
        timeout_source: ts,
        max_tokens,
        max_tokens_source: mt,
    }
}

pub fn compute_effective_rerank_model(settings: &Settings) -> EffectiveModelEndpoint {
    let f = load_llm_json(&settings.data_root);
    let (model, ms) = pick_str(
        "RERANK_MODEL",
        &f,
        "rerank_model",
        Some(&settings.rerank_model),
        Some(&settings.rerank_model),
    );
    let (base_url, bs) = pick_str(
        "RERANK_BASE_URL",
        &f,
        "rerank_base_url",
        settings.rerank_base_url.as_deref(),
        None,
    );
    let (timeout, ts) = pick_f64(
        "RERANK_TIMEOUT",
        &f,
        "rerank_timeout_seconds",
        settings.rerank_timeout_seconds,
    );
    let (max_tokens, mt) = pick_u32(
        "RERANK_MAX_TOKENS",
        &f,
        "rerank_max_tokens",
        settings.rerank_max_tokens,
    );
    EffectiveModelEndpoint {
        model: model.unwrap_or_default(),
        model_source: ms,
        base_url,
        base_url_source: bs,
        timeout_seconds: timeout,
        timeout_source: ts,
        max_tokens,
        max_tokens_source: mt,
    }
}

pub fn resolve_openai_api_key(settings: &Settings) -> Option<String> {
    if let Ok(v) = env::var("OPENAI_API_KEY") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    if let Some(k) = settings.openai_api_key.as_ref() {
        let v = k.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    let p = api_key_file_path(&settings.data_root);
    if p.is_file() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            let v = s.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub fn resolve_embedding_api_key(settings: &Settings) -> Option<String> {
    if let Ok(v) = env::var("EMBEDDING_API_KEY") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    if let Some(k) = settings.embedding_api_key.as_ref() {
        let v = k.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    let p = embedding_api_key_file_path(&settings.data_root);
    if p.is_file() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            let v = s.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub fn resolve_rerank_api_key(settings: &Settings) -> Option<String> {
    if let Ok(v) = env::var("RERANK_API_KEY") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    if let Some(k) = settings.rerank_api_key.as_ref() {
        let v = k.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    let p = rerank_api_key_file_path(&settings.data_root);
    if p.is_file() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            let v = s.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub fn api_key_configured(settings: &Settings) -> bool {
    resolve_openai_api_key(settings).is_some()
}

pub fn embedding_api_key_configured(settings: &Settings) -> bool {
    resolve_embedding_api_key(settings).is_some()
}

pub fn rerank_api_key_configured(settings: &Settings) -> bool {
    resolve_rerank_api_key(settings).is_some()
}

pub fn env_locks() -> HashMap<String, bool> {
    let keys = [
        ("openai_model", "OPENAI_MODEL"),
        ("embedding_model", "EMBEDDING_MODEL"),
        ("rerank_model", "RERANK_MODEL"),
        ("openai_base_url", "OPENAI_BASE_URL"),
        ("embedding_base_url", "EMBEDDING_BASE_URL"),
        ("rerank_base_url", "RERANK_BASE_URL"),
        ("openai_timeout_seconds", "OPENAI_TIMEOUT"),
        ("embedding_timeout_seconds", "EMBEDDING_TIMEOUT"),
        ("rerank_timeout_seconds", "RERANK_TIMEOUT"),
        ("openai_max_tokens", "OPENAI_MAX_TOKENS"),
        ("embedding_max_tokens", "EMBEDDING_MAX_TOKENS"),
        ("rerank_max_tokens", "RERANK_MAX_TOKENS"),
        ("openai_api_key", "OPENAI_API_KEY"),
        ("embedding_api_key", "EMBEDDING_API_KEY"),
        ("rerank_api_key", "RERANK_API_KEY"),
    ];
    keys.iter()
        .map(|(k, e)| (k.to_string(), env::var_os(e).is_some()))
        .collect()
}

pub fn patch_llm_json(data_root: &Path, patch: &Map<String, Value>) -> AppResult<Map<String, Value>> {
    const ALLOWED: [&str; 12] = [
        "openai_model",
        "embedding_model",
        "rerank_model",
        "openai_base_url",
        "embedding_base_url",
        "rerank_base_url",
        "openai_timeout_seconds",
        "embedding_timeout_seconds",
        "rerank_timeout_seconds",
        "openai_max_tokens",
        "embedding_max_tokens",
        "rerank_max_tokens",
    ];
    let mut cur = load_llm_json(data_root);
    for (k, v) in patch {
        if !ALLOWED.contains(&k.as_str()) {
            continue;
        }
        if v.is_null() {
            cur.remove(k);
        } else {
            cur.insert(k.clone(), v.clone());
        }
    }
    save_llm_json(data_root, &cur)?;
    Ok(cur)
}

fn write_key_file(path: &Path, api_key: Option<&str>) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match api_key {
        Some(k) if !k.trim().is_empty() => {
            let mut s = k.trim().to_string();
            s.push('\n');
            std::fs::write(path, s.as_bytes())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            }
            Ok(())
        }
        _ => {
            if path.is_file() {
                std::fs::remove_file(path)?;
            }
            Ok(())
        }
    }
}

pub fn write_api_key_file(data_root: &Path, api_key: Option<&str>) -> AppResult<()> {
    write_key_file(&api_key_file_path(data_root), api_key)
}

pub fn write_embedding_api_key_file(data_root: &Path, api_key: Option<&str>) -> AppResult<()> {
    write_key_file(&embedding_api_key_file_path(data_root), api_key)
}

pub fn write_rerank_api_key_file(data_root: &Path, api_key: Option<&str>) -> AppResult<()> {
    write_key_file(&rerank_api_key_file_path(data_root), api_key)
}
