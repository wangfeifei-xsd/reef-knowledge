use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::TaskUsage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMFieldSource {
    Env,
    File,
    Default,
}

impl LLMFieldSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            LLMFieldSource::Env => "env",
            LLMFieldSource::File => "file",
            LLMFieldSource::Default => "default",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMSettingsResponse {
    pub openai_model: String,
    pub openai_model_source: LLMFieldSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_base_url: Option<String>,
    pub openai_base_url_source: LLMFieldSource,
    pub openai_timeout_seconds: f64,
    pub openai_timeout_source: LLMFieldSource,
    pub openai_max_tokens: u32,
    pub openai_max_tokens_source: LLMFieldSource,
    pub openai_api_key_configured: bool,
    pub env_locks: HashMap<String, bool>,
    pub runtime_llm_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LLMSettingsUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_timeout_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_max_tokens: Option<u32>,
    /// 写入 .pathy/openai_api_key；传空字符串可删除该文件
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMSettingsUpdateResult {
    pub settings: LLMSettingsResponse,
    pub warnings: Vec<String>,
}

/// Embedding / Rerank 共享的"基础模型配置"响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicModelSettingsResponse {
    pub model: String,
    pub model_source: LLMFieldSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_base_url: Option<String>,
    pub openai_base_url_source: LLMFieldSource,
    pub openai_timeout_seconds: f64,
    pub openai_timeout_source: LLMFieldSource,
    pub openai_max_tokens: u32,
    pub openai_max_tokens_source: LLMFieldSource,
    pub openai_api_key_configured: bool,
    pub env_locks: HashMap<String, bool>,
    pub runtime_llm_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BasicModelSettingsUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_timeout_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicModelSettingsUpdateResult {
    pub settings: BasicModelSettingsResponse,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LLMConnectionTestRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMTestResponse {
    pub ok: bool,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub elapsed_ms: f64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TaskUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
