//! 构造 async-openai 客户端，复刻 Python `_build_openai_client` 行为。

use async_openai::config::OpenAIConfig;
use async_openai::Client;
use std::time::Duration;

use crate::config::Settings;
use crate::error::{AppError, AppResult};
use crate::llm::config::{compute_effective_llm, resolve_openai_api_key, EffectiveLLM};

pub struct BuiltClient {
    pub client: Client<OpenAIConfig>,
    pub effective: EffectiveLLM,
}

pub fn build_openai_client(settings: &Settings) -> AppResult<BuiltClient> {
    let key = resolve_openai_api_key(settings).ok_or_else(|| {
        AppError::ServiceUnavailable(
            "未配置 API 密钥（环境变量 OPENAI_API_KEY 或数据目录 .pathy/openai_api_key）".to_string(),
        )
    })?;
    let effective = compute_effective_llm(settings);
    let mut config = OpenAIConfig::new().with_api_key(&key);
    if let Some(url) = effective.base_url.as_deref() {
        config = config.with_api_base(url);
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs_f64(effective.timeout_seconds.max(1.0)))
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP client 构建失败：{e}")))?;
    let client = Client::with_config(config).with_http_client(http);
    Ok(BuiltClient { client, effective })
}

pub fn build_raw_client(
    api_key: &str,
    base_url: Option<&str>,
    timeout_seconds: f64,
) -> AppResult<Client<OpenAIConfig>> {
    let mut config = OpenAIConfig::new().with_api_key(api_key);
    if let Some(url) = base_url {
        config = config.with_api_base(url);
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs_f64(timeout_seconds.max(1.0)))
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP client 构建失败：{e}")))?;
    Ok(Client::with_config(config).with_http_client(http))
}
