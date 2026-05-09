use tauri::State;

use crate::error::AppResult;
use crate::llm::tasks::{run_compile, run_lint, run_polish_text};
use crate::models::{
    CompileTaskRequest, CompileTaskResponse, LintTaskRequest, LintTaskResponse,
    PolishTextRequest, PolishTextResponse,
};
use crate::state::AppState;

#[tauri::command]
pub async fn task_compile(
    state: State<'_, AppState>,
    body: CompileTaskRequest,
) -> AppResult<CompileTaskResponse> {
    let settings = state.settings();
    run_compile(&settings, body).await
}

#[tauri::command]
pub async fn task_lint(
    state: State<'_, AppState>,
    body: LintTaskRequest,
) -> AppResult<LintTaskResponse> {
    let settings = state.settings();
    run_lint(&settings, body).await
}

#[tauri::command]
pub async fn task_polish_text(
    state: State<'_, AppState>,
    body: PolishTextRequest,
) -> AppResult<PolishTextResponse> {
    let settings = state.settings();
    run_polish_text(&settings, body).await
}
