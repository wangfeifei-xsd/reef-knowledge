//! 「海洋知识库」桌面/移动端应用入口（Tauri 2 共享 lib）。
//!
//! 通过 `reef_knowledge_lib::run()` 同时供桌面 `main.rs` 与移动端 `mobile_entry_point` 使用。

pub mod commands;
pub mod config;
pub mod error;
pub mod seed;
pub mod llm;
pub mod models;
pub mod recall;
pub mod state;
pub mod storage;
pub mod vector_index;

use tauri::Manager;

use crate::config::{ensure_layer_tree, Settings};
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 默认数据根目录：<app_data_dir>/reef
            let default_root = match app.path().app_data_dir() {
                Ok(d) => d.join("reef"),
                Err(_) => std::path::PathBuf::from("./data"),
            };
            std::fs::create_dir_all(&default_root).ok();

            let settings = Settings::load(default_root)
                .map_err(|e| -> Box<dyn std::error::Error> { Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.detail())) })?;
            ensure_layer_tree(&settings.data_root)
                .map_err(|e| -> Box<dyn std::error::Error> { Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.detail())) })?;

            if let Err(e) = seed::seed_default_wiki_if_empty(app, &settings.data_root) {
                tracing::warn!(error = %e, "默认 wiki 种子化失败（可忽略）");
            }

            tracing::info!(
                data_root = %settings.data_root.display(),
                openai_model = %settings.openai_model,
                "reef-knowledge-desktop ready"
            );

            app.manage(AppState::new(settings));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // health & meta
            commands::health::health,
            commands::meta::get_config_summary,
            // layers
            commands::layers::list_entries,
            commands::layers::list_layer_files,
            commands::layers::read_layer_file,
            commands::layers::write_layer_file,
            commands::layers::upload_layer_file,
            commands::layers::delete_layer_file,
            commands::layers::archive_layer,
            // llm settings
            commands::llm_settings::get_llm_settings,
            commands::llm_settings::put_llm_settings,
            commands::llm_settings::test_llm_connection,
            commands::llm_settings::get_embedding_settings,
            commands::llm_settings::put_embedding_settings,
            commands::llm_settings::test_embedding_connection,
            commands::llm_settings::get_rerank_settings,
            commands::llm_settings::put_rerank_settings,
            commands::llm_settings::test_rerank_connection,
            // tasks
            commands::tasks::task_compile,
            commands::tasks::task_lint,
            commands::tasks::task_polish_text,
            // dialogue recall
            commands::dialogue::dialogue_recall,
            commands::dialogue::dialogue_recall_test,
            commands::dialogue::get_recall_stopwords,
            commands::dialogue::put_recall_stopwords,
            // wiki embedding
            commands::wiki_embedding::embed_wiki_file,
        ])
        .run(tauri::generate_context!())
        .expect("启动「海洋知识库」应用失败");
}
