use tauri::State;

use crate::error::AppResult;
use crate::models::{
    FileContentResponse, LayerFileListResponse, LayerName, ListLayerResponse,
};
use crate::state::AppState;
use crate::storage;
use crate::vector_index;

#[tauri::command]
pub async fn list_entries(
    state: State<'_, AppState>,
    layer: LayerName,
    prefix: Option<String>,
) -> AppResult<ListLayerResponse> {
    let settings = state.settings();
    let prefix = prefix.unwrap_or_default();
    storage::ensure_layer_tree(&settings.data_root)?;
    let status_map = if layer == LayerName::Wiki {
        Some(vector_index::get_wiki_embedding_status_map(&settings.data_root)?)
    } else {
        None
    };
    let (pfx, entries) = storage::list_dir(
        &settings.data_root,
        layer,
        &prefix,
        status_map.as_ref(),
    )?;
    Ok(ListLayerResponse {
        layer,
        prefix: pfx,
        entries,
    })
}

#[tauri::command]
pub async fn list_layer_files(
    state: State<'_, AppState>,
    layer: LayerName,
    suffix: Option<String>,
    max_files: Option<u32>,
) -> AppResult<LayerFileListResponse> {
    let settings = state.settings();
    let max_files_raw = max_files.unwrap_or(5000);
    if !(1..=20_000).contains(&max_files_raw) {
        return Err(crate::error::AppError::BadRequest(
            "max_files 需在 [1, 20000] 范围内".to_string(),
        ));
    }
    let max_files = max_files_raw as usize;
    storage::ensure_layer_tree(&settings.data_root)?;
    let (paths, truncated) = storage::list_all_file_paths(
        &settings.data_root,
        layer,
        suffix.as_deref(),
        max_files,
    )?;
    Ok(LayerFileListResponse {
        layer,
        paths,
        truncated,
    })
}

#[tauri::command]
pub async fn read_layer_file(
    state: State<'_, AppState>,
    layer: LayerName,
    path: String,
) -> AppResult<FileContentResponse> {
    let settings = state.settings();
    storage::ensure_layer_tree(&settings.data_root)?;
    let (text, size) = storage::read_file(&settings.data_root, layer, &path, settings.max_file_bytes)?;
    Ok(FileContentResponse {
        layer,
        path,
        content: text,
        size,
    })
}

#[tauri::command]
pub async fn write_layer_file(
    state: State<'_, AppState>,
    layer: LayerName,
    path: String,
    content: String,
) -> AppResult<FileContentResponse> {
    let settings = state.settings();
    storage::ensure_layer_tree(&settings.data_root)?;
    let size = storage::write_file(
        &settings.data_root,
        layer,
        &path,
        &content,
        settings.max_file_bytes,
    )?;
    if layer == LayerName::Wiki {
        vector_index::delete_wiki_vectors(&settings.data_root, &path)?;
    }
    Ok(FileContentResponse {
        layer,
        path,
        content,
        size,
    })
}

#[tauri::command]
pub async fn upload_layer_file(
    state: State<'_, AppState>,
    layer: LayerName,
    path: Option<String>,
    filename: Option<String>,
    bytes: Vec<u8>,
) -> AppResult<FileContentResponse> {
    let settings = state.settings();
    storage::ensure_layer_tree(&settings.data_root)?;
    if (bytes.len() as u64) > settings.max_file_bytes {
        return Err(crate::error::AppError::PayloadTooLarge(
            "文件超过大小限制".to_string(),
        ));
    }
    let mut rel = path.unwrap_or_default().trim().replace('\\', "/");
    rel = rel.trim_start_matches('/').to_string();
    if rel.is_empty() {
        let fn_name = filename.unwrap_or_else(|| "uploaded.txt".to_string());
        let basename = std::path::Path::new(&fn_name)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "uploaded.txt".to_string());
        rel = if basename.is_empty() || basename == "." || basename == ".." {
            "uploaded.txt".to_string()
        } else {
            basename
        };
    }
    let text = storage::decode_upload_text(&bytes)?;
    let size = storage::write_file(
        &settings.data_root,
        layer,
        &rel,
        &text,
        settings.max_file_bytes,
    )?;
    if layer == LayerName::Wiki {
        vector_index::delete_wiki_vectors(&settings.data_root, &rel)?;
    }
    Ok(FileContentResponse {
        layer,
        path: rel,
        content: text,
        size,
    })
}

#[tauri::command]
pub async fn delete_layer_file(
    state: State<'_, AppState>,
    layer: LayerName,
    path: String,
) -> AppResult<serde_json::Value> {
    let settings = state.settings();
    storage::ensure_layer_tree(&settings.data_root)?;
    storage::delete_path(
        &settings.data_root,
        layer,
        &path,
        settings.forbid_delete_wiki_glob,
    )?;
    if layer == LayerName::Wiki {
        vector_index::delete_wiki_vectors(&settings.data_root, path.trim_end_matches('/'))?;
    }
    Ok(serde_json::json!({ "ok": true, "deleted": path }))
}

#[tauri::command]
pub async fn archive_layer(
    state: State<'_, AppState>,
    layer: LayerName,
    prefix: Option<String>,
) -> AppResult<Vec<u8>> {
    let settings = state.settings();
    storage::ensure_layer_tree(&settings.data_root)?;
    storage::zip_layer_bytes(&settings.data_root, layer, prefix.as_deref().unwrap_or(""))
}
