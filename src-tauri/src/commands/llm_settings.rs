//! 模型配置 IPC：完全覆盖 Python `app/routers/llm_settings.py` 全部能力。

use serde_json::{json, Map, Value};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::llm::config::{
    api_key_configured, compute_effective_embedding_model, compute_effective_llm,
    compute_effective_rerank_model, embedding_api_key_configured, env_locks, patch_llm_json,
    rerank_api_key_configured, resolve_embedding_api_key, resolve_rerank_api_key,
    write_api_key_file, write_embedding_api_key_file, write_rerank_api_key_file,
};
use crate::llm::test::{run_connection_test, run_connection_test_raw, RawTestKind};
use crate::models::{
    BasicModelSettingsResponse, BasicModelSettingsUpdateRequest, BasicModelSettingsUpdateResult,
    LLMConnectionTestRequest, LLMFieldSource, LLMSettingsResponse, LLMSettingsUpdateRequest,
    LLMSettingsUpdateResult, LLMTestResponse,
};
use crate::state::AppState;

fn to_field_source(s: crate::llm::config::FieldSource) -> LLMFieldSource {
    match s {
        crate::llm::config::FieldSource::Env => LLMFieldSource::Env,
        crate::llm::config::FieldSource::File => LLMFieldSource::File,
        crate::llm::config::FieldSource::Default => LLMFieldSource::Default,
    }
}

fn to_llm_response(state: &AppState) -> LLMSettingsResponse {
    let settings = state.settings();
    let eff = compute_effective_llm(&settings);
    LLMSettingsResponse {
        openai_model: eff.model,
        openai_model_source: to_field_source(eff.model_source),
        openai_base_url: eff.base_url,
        openai_base_url_source: to_field_source(eff.base_url_source),
        openai_timeout_seconds: eff.timeout_seconds,
        openai_timeout_source: to_field_source(eff.timeout_source),
        openai_max_tokens: eff.max_tokens,
        openai_max_tokens_source: to_field_source(eff.max_tokens_source),
        openai_api_key_configured: api_key_configured(&settings),
        env_locks: env_locks(),
        runtime_llm_json: ".pathy/llm.json".to_string(),
    }
}

#[tauri::command]
pub async fn get_llm_settings(state: State<'_, AppState>) -> AppResult<LLMSettingsResponse> {
    Ok(to_llm_response(&state))
}

#[tauri::command]
pub async fn put_llm_settings(
    state: State<'_, AppState>,
    body: LLMSettingsUpdateRequest,
) -> AppResult<LLMSettingsUpdateResult> {
    let settings = state.settings();
    let locks = env_locks();
    let mut warnings: Vec<String> = Vec::new();
    let mut patch: Map<String, Value> = Map::new();
    let root = settings.data_root.clone();

    if let Some(m) = body.openai_model.as_deref() {
        let m = m.trim();
        if m.is_empty() {
            return Err(AppError::BadRequest("openai_model 不能为空".to_string()));
        }
        if locks.get("openai_model").copied().unwrap_or(false) {
            warnings.push("OPENAI_MODEL 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            patch.insert("openai_model".to_string(), json!(m));
        }
    }
    if let Some(u) = body.openai_base_url.as_deref() {
        if locks.get("openai_base_url").copied().unwrap_or(false) {
            warnings.push("OPENAI_BASE_URL 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            let u = u.trim();
            patch.insert(
                "openai_base_url".to_string(),
                if u.is_empty() {
                    Value::Null
                } else {
                    json!(u)
                },
            );
        }
    }
    if let Some(t) = body.openai_timeout_seconds {
        if locks.get("openai_timeout_seconds").copied().unwrap_or(false) {
            warnings.push("OPENAI_TIMEOUT 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            if t <= 0.0 || t > 3600.0 {
                return Err(AppError::BadRequest(
                    "openai_timeout_seconds 需在 (0, 3600] 内".to_string(),
                ));
            }
            patch.insert("openai_timeout_seconds".to_string(), json!(t));
        }
    }
    if let Some(n) = body.openai_max_tokens {
        if locks.get("openai_max_tokens").copied().unwrap_or(false) {
            warnings.push("OPENAI_MAX_TOKENS 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            if n < 1 || n > 200_000 {
                return Err(AppError::BadRequest(
                    "openai_max_tokens 超出合理范围".to_string(),
                ));
            }
            patch.insert("openai_max_tokens".to_string(), json!(n));
        }
    }
    if !patch.is_empty() {
        patch_llm_json(&root, &patch)?;
    }
    if let Some(k) = body.openai_api_key.as_deref() {
        if locks.get("openai_api_key").copied().unwrap_or(false) {
            warnings.push("OPENAI_API_KEY 已由环境变量锁定，未写入密钥文件".to_string());
        } else {
            let k = k.trim();
            write_api_key_file(&root, if k.is_empty() { None } else { Some(k) })?;
        }
    }
    Ok(LLMSettingsUpdateResult {
        settings: to_llm_response(&state),
        warnings,
    })
}

#[tauri::command]
pub async fn test_llm_connection(
    state: State<'_, AppState>,
    body: Option<LLMConnectionTestRequest>,
) -> AppResult<LLMTestResponse> {
    let settings = state.settings();
    let body = body.unwrap_or_default();
    run_connection_test(&settings, body.openai_model, body.openai_base_url).await
}

// ----- Embedding -----

fn embedding_response(state: &AppState) -> BasicModelSettingsResponse {
    let settings = state.settings();
    let eff = compute_effective_embedding_model(&settings);
    let locks = env_locks();
    let mut env_locks_map = std::collections::HashMap::<String, bool>::new();
    for k in [
        "embedding_model",
        "embedding_base_url",
        "embedding_timeout_seconds",
        "embedding_max_tokens",
        "embedding_api_key",
    ] {
        env_locks_map.insert(k.to_string(), locks.get(k).copied().unwrap_or(false));
    }
    // Python 用了简化的 key 名称（model / openai_base_url / ...）；这里同样保留 openai_* 命名以兼容前端
    let mut alias_map = std::collections::HashMap::<String, bool>::new();
    alias_map.insert("embedding_model".into(), env_locks_map["embedding_model"]);
    alias_map.insert(
        "openai_base_url".into(),
        env_locks_map["embedding_base_url"],
    );
    alias_map.insert(
        "openai_timeout_seconds".into(),
        env_locks_map["embedding_timeout_seconds"],
    );
    alias_map.insert(
        "openai_max_tokens".into(),
        env_locks_map["embedding_max_tokens"],
    );
    alias_map.insert("openai_api_key".into(), env_locks_map["embedding_api_key"]);

    BasicModelSettingsResponse {
        model: eff.model,
        model_source: to_field_source(eff.model_source),
        openai_base_url: eff.base_url,
        openai_base_url_source: to_field_source(eff.base_url_source),
        openai_timeout_seconds: eff.timeout_seconds,
        openai_timeout_source: to_field_source(eff.timeout_source),
        openai_max_tokens: eff.max_tokens,
        openai_max_tokens_source: to_field_source(eff.max_tokens_source),
        openai_api_key_configured: embedding_api_key_configured(&settings),
        env_locks: alias_map,
        runtime_llm_json: ".pathy/llm.json".to_string(),
    }
}

#[tauri::command]
pub async fn get_embedding_settings(
    state: State<'_, AppState>,
) -> AppResult<BasicModelSettingsResponse> {
    Ok(embedding_response(&state))
}

#[tauri::command]
pub async fn put_embedding_settings(
    state: State<'_, AppState>,
    body: BasicModelSettingsUpdateRequest,
) -> AppResult<BasicModelSettingsUpdateResult> {
    let settings = state.settings();
    let locks = env_locks();
    let mut warnings: Vec<String> = Vec::new();
    let mut patch: Map<String, Value> = Map::new();
    if let Some(m) = body.model.as_deref() {
        let m = m.trim();
        if m.is_empty() {
            return Err(AppError::BadRequest("model 不能为空".to_string()));
        }
        if locks.get("embedding_model").copied().unwrap_or(false) {
            warnings.push("EMBEDDING_MODEL 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            patch.insert("embedding_model".to_string(), json!(m));
        }
    }
    if let Some(u) = body.openai_base_url.as_deref() {
        if locks.get("embedding_base_url").copied().unwrap_or(false) {
            warnings.push("EMBEDDING_BASE_URL 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            let u = u.trim();
            patch.insert(
                "embedding_base_url".to_string(),
                if u.is_empty() {
                    Value::Null
                } else {
                    json!(u)
                },
            );
        }
    }
    if let Some(t) = body.openai_timeout_seconds {
        if locks
            .get("embedding_timeout_seconds")
            .copied()
            .unwrap_or(false)
        {
            warnings.push("EMBEDDING_TIMEOUT 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            if t <= 0.0 || t > 3600.0 {
                return Err(AppError::BadRequest(
                    "openai_timeout_seconds 需在 (0, 3600] 内".to_string(),
                ));
            }
            patch.insert("embedding_timeout_seconds".to_string(), json!(t));
        }
    }
    if let Some(n) = body.openai_max_tokens {
        if locks.get("embedding_max_tokens").copied().unwrap_or(false) {
            warnings.push("EMBEDDING_MAX_TOKENS 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            if n < 1 || n > 200_000 {
                return Err(AppError::BadRequest(
                    "openai_max_tokens 超出合理范围".to_string(),
                ));
            }
            patch.insert("embedding_max_tokens".to_string(), json!(n));
        }
    }
    if !patch.is_empty() {
        patch_llm_json(&settings.data_root, &patch)?;
    }
    if let Some(k) = body.openai_api_key.as_deref() {
        if locks.get("embedding_api_key").copied().unwrap_or(false) {
            warnings.push("EMBEDDING_API_KEY 已由环境变量锁定，未写入密钥文件".to_string());
        } else {
            let k = k.trim();
            write_embedding_api_key_file(&settings.data_root, if k.is_empty() { None } else { Some(k) })?;
        }
    }
    Ok(BasicModelSettingsUpdateResult {
        settings: embedding_response(&state),
        warnings,
    })
}

#[tauri::command]
pub async fn test_embedding_connection(
    state: State<'_, AppState>,
    body: Option<LLMConnectionTestRequest>,
) -> AppResult<LLMTestResponse> {
    let settings = state.settings();
    let body = body.unwrap_or_default();
    let eff = compute_effective_embedding_model(&settings);
    let model = body
        .openai_model
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(eff.model);
    let base_url = match body.openai_base_url {
        Some(s) => {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        }
        None => eff.base_url,
    };
    run_connection_test_raw(
        model,
        base_url,
        eff.timeout_seconds,
        eff.max_tokens,
        resolve_embedding_api_key(&settings),
        "未配置 Embedding API 密钥（环境变量 EMBEDDING_API_KEY 或 .pathy/embedding_api_key）"
            .to_string(),
        RawTestKind::Embedding,
    )
    .await
}

// ----- Rerank -----

fn rerank_response(state: &AppState) -> BasicModelSettingsResponse {
    let settings = state.settings();
    let eff = compute_effective_rerank_model(&settings);
    let locks = env_locks();
    let mut alias_map = std::collections::HashMap::<String, bool>::new();
    alias_map.insert(
        "rerank_model".into(),
        locks.get("rerank_model").copied().unwrap_or(false),
    );
    alias_map.insert(
        "openai_base_url".into(),
        locks.get("rerank_base_url").copied().unwrap_or(false),
    );
    alias_map.insert(
        "openai_timeout_seconds".into(),
        locks.get("rerank_timeout_seconds").copied().unwrap_or(false),
    );
    alias_map.insert(
        "openai_max_tokens".into(),
        locks.get("rerank_max_tokens").copied().unwrap_or(false),
    );
    alias_map.insert(
        "openai_api_key".into(),
        locks.get("rerank_api_key").copied().unwrap_or(false),
    );
    BasicModelSettingsResponse {
        model: eff.model,
        model_source: to_field_source(eff.model_source),
        openai_base_url: eff.base_url,
        openai_base_url_source: to_field_source(eff.base_url_source),
        openai_timeout_seconds: eff.timeout_seconds,
        openai_timeout_source: to_field_source(eff.timeout_source),
        openai_max_tokens: eff.max_tokens,
        openai_max_tokens_source: to_field_source(eff.max_tokens_source),
        openai_api_key_configured: rerank_api_key_configured(&settings),
        env_locks: alias_map,
        runtime_llm_json: ".pathy/llm.json".to_string(),
    }
}

#[tauri::command]
pub async fn get_rerank_settings(
    state: State<'_, AppState>,
) -> AppResult<BasicModelSettingsResponse> {
    Ok(rerank_response(&state))
}

#[tauri::command]
pub async fn put_rerank_settings(
    state: State<'_, AppState>,
    body: BasicModelSettingsUpdateRequest,
) -> AppResult<BasicModelSettingsUpdateResult> {
    let settings = state.settings();
    let locks = env_locks();
    let mut warnings: Vec<String> = Vec::new();
    let mut patch: Map<String, Value> = Map::new();
    if let Some(m) = body.model.as_deref() {
        let m = m.trim();
        if m.is_empty() {
            return Err(AppError::BadRequest("model 不能为空".to_string()));
        }
        if locks.get("rerank_model").copied().unwrap_or(false) {
            warnings.push("RERANK_MODEL 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            patch.insert("rerank_model".to_string(), json!(m));
        }
    }
    if let Some(u) = body.openai_base_url.as_deref() {
        if locks.get("rerank_base_url").copied().unwrap_or(false) {
            warnings.push("RERANK_BASE_URL 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            let u = u.trim();
            patch.insert(
                "rerank_base_url".to_string(),
                if u.is_empty() {
                    Value::Null
                } else {
                    json!(u)
                },
            );
        }
    }
    if let Some(t) = body.openai_timeout_seconds {
        if locks.get("rerank_timeout_seconds").copied().unwrap_or(false) {
            warnings.push("RERANK_TIMEOUT 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            if t <= 0.0 || t > 3600.0 {
                return Err(AppError::BadRequest(
                    "openai_timeout_seconds 需在 (0, 3600] 内".to_string(),
                ));
            }
            patch.insert("rerank_timeout_seconds".to_string(), json!(t));
        }
    }
    if let Some(n) = body.openai_max_tokens {
        if locks.get("rerank_max_tokens").copied().unwrap_or(false) {
            warnings.push("RERANK_MAX_TOKENS 已由环境变量锁定，未写入运行时文件".to_string());
        } else {
            if n < 1 || n > 200_000 {
                return Err(AppError::BadRequest(
                    "openai_max_tokens 超出合理范围".to_string(),
                ));
            }
            patch.insert("rerank_max_tokens".to_string(), json!(n));
        }
    }
    if !patch.is_empty() {
        patch_llm_json(&settings.data_root, &patch)?;
    }
    if let Some(k) = body.openai_api_key.as_deref() {
        if locks.get("rerank_api_key").copied().unwrap_or(false) {
            warnings.push("RERANK_API_KEY 已由环境变量锁定，未写入密钥文件".to_string());
        } else {
            let k = k.trim();
            write_rerank_api_key_file(&settings.data_root, if k.is_empty() { None } else { Some(k) })?;
        }
    }
    Ok(BasicModelSettingsUpdateResult {
        settings: rerank_response(&state),
        warnings,
    })
}

#[tauri::command]
pub async fn test_rerank_connection(
    state: State<'_, AppState>,
    body: Option<LLMConnectionTestRequest>,
) -> AppResult<LLMTestResponse> {
    let settings = state.settings();
    let body = body.unwrap_or_default();
    let eff = compute_effective_rerank_model(&settings);
    let model = body
        .openai_model
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(eff.model);
    let base_url = match body.openai_base_url {
        Some(s) => {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        }
        None => eff.base_url,
    };
    run_connection_test_raw(
        model,
        base_url,
        eff.timeout_seconds,
        eff.max_tokens,
        resolve_rerank_api_key(&settings),
        "未配置 Rerank API 密钥（环境变量 RERANK_API_KEY 或 .pathy/rerank_api_key）".to_string(),
        RawTestKind::Chat,
    )
    .await
}
