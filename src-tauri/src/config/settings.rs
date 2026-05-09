//! 复刻 Python `app/config.py` 的 `Settings`：
//! - 默认值与字段名对齐（snake_case）
//! - 环境变量优先；可选 YAML 配置文件覆盖默认值
//! - 默认 data_root 走 Tauri 平台数据目录（桌面/移动统一），可被 env `DATA_ROOT` 覆盖。

use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub data_root: PathBuf,
    pub config_file: Option<PathBuf>,

    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub openai_model: String,
    pub embedding_model: String,
    pub rerank_model: String,
    pub embedding_api_key: Option<String>,
    pub rerank_api_key: Option<String>,
    pub embedding_base_url: Option<String>,
    pub rerank_base_url: Option<String>,
    pub openai_timeout_seconds: f64,
    pub openai_max_tokens: u32,
    pub embedding_timeout_seconds: f64,
    pub rerank_timeout_seconds: f64,
    pub embedding_max_tokens: u32,
    pub rerank_max_tokens: u32,

    pub api_key: Option<String>,
    pub max_file_bytes: u64,
    pub forbid_delete_wiki_glob: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8765,
            data_root: PathBuf::from("./data"),
            config_file: None,
            // 移动端打包后无 `.env` 可读，体验版直接内置默认 LLM API Key；
            // 仍可被环境变量 `OPENAI_API_KEY` 或运行时密钥文件 `.pathy/openai_api_key` 覆盖。
            openai_api_key: Some(
                "sk-iaFh54aum3HJz4klEe7801A111544aDcA8542d845c523bD8".to_string(),
            ),
            // LLM 默认走 Minimax（可被 `.env` / 环境变量 / `.pathy/llm.json` 覆盖）
            openai_base_url: Some("http://minimax.miniscoresdata.cn:3000/v1".to_string()),
            openai_model: "minimax-m2.5".to_string(),
            embedding_model: "text-embedding-3-large".to_string(),
            rerank_model: "gpt-4o-mini".to_string(),
            embedding_api_key: None,
            rerank_api_key: None,
            embedding_base_url: None,
            rerank_base_url: None,
            openai_timeout_seconds: 120.0,
            openai_max_tokens: 8192,
            embedding_timeout_seconds: 120.0,
            rerank_timeout_seconds: 120.0,
            embedding_max_tokens: 8192,
            rerank_max_tokens: 8192,
            api_key: None,
            max_file_bytes: 2_097_152,
            forbid_delete_wiki_glob: false,
        }
    }
}

impl Settings {
    /// 加载流程：默认 → `.env`（pydantic-settings 行为对齐）→ YAML 文件 → 进程环境变量；后者覆盖前者。
    /// `default_data_root` 用于在未显式配置时给定（如 Tauri 应用数据目录）。
    pub fn load(default_data_root: PathBuf) -> AppResult<Self> {
        let mut s = Self::default();
        s.data_root = default_data_root;

        // 0. 加载工作目录下的 `.env`（与 Python pydantic-settings 默认行为对齐）。
        //    使用 `dotenvy` 不会覆盖已有进程环境变量，所以"进程 env > .env"的优先级也成立。
        let _ = dotenvy::dotenv();

        // 1. CONFIG_FILE 指向的 YAML
        if let Some(cf) = env::var_os("CONFIG_FILE") {
            let path = PathBuf::from(cf);
            if path.is_file() {
                let raw = std::fs::read_to_string(&path)?;
                let yaml: serde_yaml::Value = serde_yaml::from_str(&raw)?;
                if let serde_yaml::Value::Mapping(m) = yaml {
                    apply_yaml(&mut s, &m);
                }
                s.config_file = Some(path);
            }
        }

        // 2. 环境变量（最高优先级；含 .env 中合并进来的项）
        apply_env(&mut s);

        // data_root 解析为绝对路径
        if s.data_root.is_relative() {
            if let Ok(cwd) = env::current_dir() {
                s.data_root = cwd.join(&s.data_root);
            }
        }

        Ok(s)
    }

    pub fn data_root_resolved(&self) -> PathBuf {
        std::fs::canonicalize(&self.data_root).unwrap_or_else(|_| self.data_root.clone())
    }

    pub fn pathy_dir(&self) -> PathBuf {
        self.data_root.join(".pathy")
    }

    pub fn layer_root(&self, name: &str) -> PathBuf {
        self.data_root.join(name)
    }

    pub fn layer_root_resolved(&self, name: &str) -> PathBuf {
        let p = self.layer_root(name);
        std::fs::canonicalize(&p).unwrap_or(p)
    }
}

fn apply_yaml(s: &mut Settings, m: &serde_yaml::Mapping) {
    fn s_string(v: &serde_yaml::Value) -> Option<String> {
        v.as_str().map(|x| x.to_string())
    }
    fn s_bool(v: &serde_yaml::Value) -> Option<bool> {
        v.as_bool()
    }
    fn s_f64(v: &serde_yaml::Value) -> Option<f64> {
        v.as_f64().or_else(|| v.as_i64().map(|x| x as f64))
    }
    fn s_u64(v: &serde_yaml::Value) -> Option<u64> {
        v.as_u64().or_else(|| v.as_i64().and_then(|x| u64::try_from(x).ok()))
    }
    fn s_path(v: &serde_yaml::Value) -> Option<PathBuf> {
        v.as_str().map(PathBuf::from)
    }

    for (k, v) in m {
        let Some(key) = k.as_str() else { continue };
        match key {
            "host" => {
                if let Some(x) = s_string(v) {
                    s.host = x;
                }
            }
            "port" => {
                if let Some(x) = s_u64(v) {
                    s.port = x as u16;
                }
            }
            "data_root" => {
                if let Some(x) = s_path(v) {
                    s.data_root = x;
                }
            }
            "openai_api_key" => s.openai_api_key = s_string(v),
            "openai_base_url" => s.openai_base_url = s_string(v),
            "openai_model" => {
                if let Some(x) = s_string(v) {
                    s.openai_model = x;
                }
            }
            "embedding_model" => {
                if let Some(x) = s_string(v) {
                    s.embedding_model = x;
                }
            }
            "rerank_model" => {
                if let Some(x) = s_string(v) {
                    s.rerank_model = x;
                }
            }
            "embedding_api_key" => s.embedding_api_key = s_string(v),
            "rerank_api_key" => s.rerank_api_key = s_string(v),
            "embedding_base_url" => s.embedding_base_url = s_string(v),
            "rerank_base_url" => s.rerank_base_url = s_string(v),
            "openai_timeout_seconds" => {
                if let Some(x) = s_f64(v) {
                    s.openai_timeout_seconds = x;
                }
            }
            "openai_max_tokens" => {
                if let Some(x) = s_u64(v) {
                    s.openai_max_tokens = x as u32;
                }
            }
            "embedding_timeout_seconds" => {
                if let Some(x) = s_f64(v) {
                    s.embedding_timeout_seconds = x;
                }
            }
            "rerank_timeout_seconds" => {
                if let Some(x) = s_f64(v) {
                    s.rerank_timeout_seconds = x;
                }
            }
            "embedding_max_tokens" => {
                if let Some(x) = s_u64(v) {
                    s.embedding_max_tokens = x as u32;
                }
            }
            "rerank_max_tokens" => {
                if let Some(x) = s_u64(v) {
                    s.rerank_max_tokens = x as u32;
                }
            }
            "api_key" => s.api_key = s_string(v),
            "max_file_bytes" => {
                if let Some(x) = s_u64(v) {
                    s.max_file_bytes = x;
                }
            }
            "forbid_delete_wiki_glob" => {
                if let Some(x) = s_bool(v) {
                    s.forbid_delete_wiki_glob = x;
                }
            }
            _ => {}
        }
    }
}

fn apply_env(s: &mut Settings) {
    if let Ok(v) = env::var("DATA_ROOT") {
        s.data_root = PathBuf::from(v);
    }
    if let Ok(v) = env::var("OPENAI_API_KEY") {
        s.openai_api_key = Some(v);
    }
    if let Ok(v) = env::var("OPENAI_BASE_URL") {
        s.openai_base_url = Some(v);
    }
    if let Ok(v) = env::var("OPENAI_MODEL") {
        s.openai_model = v;
    }
    if let Ok(v) = env::var("EMBEDDING_MODEL") {
        s.embedding_model = v;
    }
    if let Ok(v) = env::var("RERANK_MODEL") {
        s.rerank_model = v;
    }
    if let Ok(v) = env::var("EMBEDDING_API_KEY") {
        s.embedding_api_key = Some(v);
    }
    if let Ok(v) = env::var("RERANK_API_KEY") {
        s.rerank_api_key = Some(v);
    }
    if let Ok(v) = env::var("EMBEDDING_BASE_URL") {
        s.embedding_base_url = Some(v);
    }
    if let Ok(v) = env::var("RERANK_BASE_URL") {
        s.rerank_base_url = Some(v);
    }
    if let Some(v) = parse_env_f64("OPENAI_TIMEOUT") {
        s.openai_timeout_seconds = v;
    }
    if let Some(v) = parse_env_u32("OPENAI_MAX_TOKENS") {
        s.openai_max_tokens = v;
    }
    if let Some(v) = parse_env_f64("EMBEDDING_TIMEOUT") {
        s.embedding_timeout_seconds = v;
    }
    if let Some(v) = parse_env_f64("RERANK_TIMEOUT") {
        s.rerank_timeout_seconds = v;
    }
    if let Some(v) = parse_env_u32("EMBEDDING_MAX_TOKENS") {
        s.embedding_max_tokens = v;
    }
    if let Some(v) = parse_env_u32("RERANK_MAX_TOKENS") {
        s.rerank_max_tokens = v;
    }
    if let Ok(v) = env::var("API_KEY") {
        s.api_key = Some(v);
    }
}

fn parse_env_f64(key: &str) -> Option<f64> {
    env::var(key).ok().and_then(|v| v.trim().parse::<f64>().ok())
}

fn parse_env_u32(key: &str) -> Option<u32> {
    env::var(key).ok().and_then(|v| v.trim().parse::<u32>().ok())
}

/// 在数据根下创建三层目录（与原 Python 启动钩子等价）。
pub fn ensure_layer_tree(data_root: &Path) -> AppResult<()> {
    std::fs::create_dir_all(data_root)?;
    for name in ["raw", "wiki", "schema"] {
        std::fs::create_dir_all(data_root.join(name))?;
    }
    Ok(())
}
