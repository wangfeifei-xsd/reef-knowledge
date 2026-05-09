use tauri::State;

use crate::error::AppResult;
use crate::llm::config::{api_key_configured, compute_effective_llm};
use crate::models::ConfigSummaryResponse;
use crate::state::AppState;

#[tauri::command]
pub async fn get_config_summary(state: State<'_, AppState>) -> AppResult<ConfigSummaryResponse> {
    let settings = state.settings();
    let eff = compute_effective_llm(&settings);
    Ok(ConfigSummaryResponse {
        data_root: settings.data_root.to_string_lossy().to_string(),
        data_root_resolved: settings.data_root_resolved().to_string_lossy().to_string(),
        openai_base_url_configured: eff.base_url.is_some(),
        openai_model: eff.model,
        openai_timeout_seconds: eff.timeout_seconds,
        openai_max_tokens: eff.max_tokens,
        openai_api_key_configured: api_key_configured(&settings),
        layers: vec!["raw".into(), "wiki".into(), "schema".into()],
        auth_enabled: settings.api_key.as_deref().map(|s| !s.is_empty()).unwrap_or(false),
    })
}
