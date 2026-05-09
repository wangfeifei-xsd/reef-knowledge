use crate::error::AppResult;
use crate::models::HealthResponse;

#[tauri::command]
pub async fn health() -> AppResult<HealthResponse> {
    Ok(HealthResponse::default())
}
