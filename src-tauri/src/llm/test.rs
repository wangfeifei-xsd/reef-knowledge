//! 复刻 Python `app/services/llm_test.py`：探测 OpenAI 兼容端点连通性。

use async_openai::types::{
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs,
    EmbeddingInput,
};
use std::time::Instant;

use crate::config::Settings;
use crate::error::AppResult;
use crate::llm::client::build_raw_client;
use crate::llm::config::{compute_effective_llm, resolve_openai_api_key};
use crate::models::{LLMTestResponse, TaskUsage};

fn usage_chat(u: Option<async_openai::types::CompletionUsage>) -> Option<TaskUsage> {
    u.map(|u| TaskUsage {
        prompt_tokens: Some(u.prompt_tokens),
        completion_tokens: Some(u.completion_tokens),
        total_tokens: Some(u.total_tokens),
    })
}

fn usage_emb(u: Option<async_openai::types::EmbeddingUsage>) -> Option<TaskUsage> {
    u.map(|u| TaskUsage {
        prompt_tokens: Some(u.prompt_tokens),
        completion_tokens: None,
        total_tokens: Some(u.total_tokens),
    })
}

fn truncate_err(e: impl std::fmt::Display) -> String {
    let s = e.to_string();
    let s = s.trim();
    if s.chars().count() <= 800 {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(800).collect::<String>())
    }
}

pub async fn run_connection_test(
    settings: &Settings,
    draft_model: Option<String>,
    draft_base_url: Option<String>,
) -> AppResult<LLMTestResponse> {
    let eff = compute_effective_llm(settings);
    let model = draft_model
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(eff.model.clone());
    let bu = match draft_base_url {
        Some(s) => {
            let s = s.trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        }
        None => eff.base_url.clone(),
    };

    let key = match resolve_openai_api_key(settings) {
        Some(k) => k,
        None => {
            return Ok(LLMTestResponse {
                ok: false,
                model,
                base_url: bu,
                elapsed_ms: 0.0,
                message: String::new(),
                usage: None,
                error: Some("未配置 API 密钥（环境变量或 .pathy/openai_api_key）".to_string()),
            });
        }
    };
    let client = build_raw_client(&key, bu.as_deref(), eff.timeout_seconds)?;

    let t0 = Instant::now();
    let req = match CreateChatCompletionRequestArgs::default()
        .model(&model)
        .max_tokens(std::cmp::min(16u32, eff.max_tokens))
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content("ping")
            .build()
            .unwrap()
            .into()])
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(LLMTestResponse {
                ok: false,
                model,
                base_url: bu,
                elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0,
                message: String::new(),
                usage: None,
                error: Some(truncate_err(e)),
            });
        }
    };

    match client.chat().create(req).await {
        Ok(resp) => Ok(LLMTestResponse {
            ok: true,
            model,
            base_url: bu,
            elapsed_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            message: "Chat Completions 调用成功".to_string(),
            usage: usage_chat(resp.usage),
            error: None,
        }),
        Err(e) => Ok(LLMTestResponse {
            ok: false,
            model,
            base_url: bu,
            elapsed_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            message: String::new(),
            usage: None,
            error: Some(truncate_err(e)),
        }),
    }
}

pub enum RawTestKind {
    Chat,
    Embedding,
}

#[allow(clippy::too_many_arguments)]
pub async fn run_connection_test_raw(
    model: String,
    base_url: Option<String>,
    timeout_seconds: f64,
    max_tokens: u32,
    api_key: Option<String>,
    missing_key_error: String,
    kind: RawTestKind,
) -> AppResult<LLMTestResponse> {
    let key = match api_key {
        Some(k) if !k.is_empty() => k,
        _ => {
            return Ok(LLMTestResponse {
                ok: false,
                model,
                base_url,
                elapsed_ms: 0.0,
                message: String::new(),
                usage: None,
                error: Some(missing_key_error),
            });
        }
    };
    let client = build_raw_client(&key, base_url.as_deref(), timeout_seconds)?;
    let t0 = Instant::now();
    match kind {
        RawTestKind::Chat => {
            let req = CreateChatCompletionRequestArgs::default()
                .model(&model)
                .max_tokens(std::cmp::min(16u32, max_tokens))
                .messages([ChatCompletionRequestUserMessageArgs::default()
                    .content("ping")
                    .build()
                    .unwrap()
                    .into()])
                .build();
            match req {
                Ok(r) => match client.chat().create(r).await {
                    Ok(resp) => Ok(LLMTestResponse {
                        ok: true,
                        model,
                        base_url,
                        elapsed_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
                        message: "Chat Completions 调用成功".to_string(),
                        usage: usage_chat(resp.usage),
                        error: None,
                    }),
                    Err(e) => Ok(LLMTestResponse {
                        ok: false,
                        model,
                        base_url,
                        elapsed_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
                        message: String::new(),
                        usage: None,
                        error: Some(truncate_err(e)),
                    }),
                },
                Err(e) => Ok(LLMTestResponse {
                    ok: false,
                    model,
                    base_url,
                    elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0,
                    message: String::new(),
                    usage: None,
                    error: Some(truncate_err(e)),
                }),
            }
        }
        RawTestKind::Embedding => {
            let req = CreateEmbeddingRequestArgs::default()
                .model(&model)
                .input(EmbeddingInput::String("ping".to_string()))
                .build();
            match req {
                Ok(r) => match client.embeddings().create(r).await {
                    Ok(resp) => Ok(LLMTestResponse {
                        ok: true,
                        model,
                        base_url,
                        elapsed_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
                        message: "Embeddings 调用成功".to_string(),
                        usage: usage_emb(Some(resp.usage)),
                        error: None,
                    }),
                    Err(e) => Ok(LLMTestResponse {
                        ok: false,
                        model,
                        base_url,
                        elapsed_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
                        message: String::new(),
                        usage: None,
                        error: Some(truncate_err(e)),
                    }),
                },
                Err(e) => Ok(LLMTestResponse {
                    ok: false,
                    model,
                    base_url,
                    elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0,
                    message: String::new(),
                    usage: None,
                    error: Some(truncate_err(e)),
                }),
            }
        }
    }
}
