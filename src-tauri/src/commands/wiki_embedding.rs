use tauri::State;

use crate::error::AppResult;
use crate::models::{WikiEmbedRequest, WikiEmbedResponse};
use crate::state::AppState;
use crate::vector_index;

#[tauri::command]
pub async fn embed_wiki_file(
    state: State<'_, AppState>,
    body: WikiEmbedRequest,
) -> AppResult<WikiEmbedResponse> {
    let settings = state.settings();
    let (cnt, model, updated_at) = vector_index::embed_wiki_file(&settings, &body.path).await?;
    Ok(WikiEmbedResponse {
        path: body.path,
        chunk_count: cnt,
        model,
        updated_at,
        message: "已完成嵌入并写入向量索引".to_string(),
    })
}
