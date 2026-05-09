use tauri::State;

use crate::error::AppResult;
use crate::models::{
    DialogueRecallRequest, DialogueRecallResponse, DialogueRecallTestRequest,
    DialogueRecallTestResponse, RecallStopwordsResponse, RecallStopwordsUpdateRequest,
};
use crate::recall::pipeline::{run_dialogue_recall_only, run_dialogue_recall_test};
use crate::recall::stopwords::{
    parse_stopwords_text, read_effective_stopwords, read_runtime_stopwords, runtime_stopwords_path,
    write_runtime_stopwords,
};
use crate::state::AppState;

#[tauri::command]
pub async fn dialogue_recall(
    state: State<'_, AppState>,
    body: DialogueRecallRequest,
) -> AppResult<DialogueRecallResponse> {
    let settings = state.settings();
    run_dialogue_recall_only(&settings, body).await
}

#[tauri::command]
pub async fn dialogue_recall_test(
    state: State<'_, AppState>,
    body: DialogueRecallTestRequest,
) -> AppResult<DialogueRecallTestResponse> {
    let settings = state.settings();
    run_dialogue_recall_test(&settings, body).await
}

#[tauri::command]
pub async fn get_recall_stopwords(
    state: State<'_, AppState>,
) -> AppResult<RecallStopwordsResponse> {
    let settings = state.settings();
    let runtime = read_runtime_stopwords(&settings);
    let words = if runtime.is_empty() {
        read_effective_stopwords(&settings)
    } else {
        runtime.clone()
    };
    let source = if !runtime.is_empty() {
        "runtime_file"
    } else {
        "default_builtin"
    }
    .to_string();
    let count = words.len() as u32;
    Ok(RecallStopwordsResponse {
        words,
        source,
        runtime_path: runtime_stopwords_path(&settings)
            .to_string_lossy()
            .replace('\\', "/"),
        count,
        message: "已加载召回停用词".to_string(),
    })
}

#[tauri::command]
pub async fn put_recall_stopwords(
    state: State<'_, AppState>,
    body: RecallStopwordsUpdateRequest,
) -> AppResult<RecallStopwordsResponse> {
    let settings = state.settings();
    let parsed = parse_stopwords_text(&body.words.join("\n"));
    let (n, rel) = write_runtime_stopwords(&settings, &parsed)?;
    Ok(RecallStopwordsResponse {
        words: parsed,
        source: "runtime_file".to_string(),
        runtime_path: settings
            .data_root
            .join(rel)
            .to_string_lossy()
            .replace('\\', "/"),
        count: n as u32,
        message: "已保存召回停用词".to_string(),
    })
}
