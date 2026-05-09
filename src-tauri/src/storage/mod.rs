//! 三层存储：raw / wiki / schema。
//!
//! 对齐 Python `app/services/storage.py`：
//! - `safe_resolve_under` 防 `../` 越界
//! - `list_dir` / `list_all_file_paths`
//! - `read_file` / `write_file` / `delete_path`
//! - `zip_layer_bytes`

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

use crate::error::{AppError, AppResult};
use crate::models::{DirEntry, LayerName};

pub fn layer_root(data_root: &Path, layer: LayerName) -> PathBuf {
    let p = data_root.join(layer.as_str());
    std::fs::canonicalize(&p).unwrap_or(p)
}

/// 将相对路径解析到 `base` 内，禁止跳出目录。复刻 Python `safe_resolve_under`。
pub fn safe_resolve_under(base: &Path, rel: &str) -> AppResult<PathBuf> {
    let trimmed = rel.trim();
    if trimmed.is_empty() {
        return Ok(base.to_path_buf());
    }
    let normalized = trimmed.replace('\\', "/");
    let mut cur = base.to_path_buf();
    for part in Path::new(&normalized).components() {
        match part {
            Component::CurDir | Component::RootDir | Component::Prefix(_) => continue,
            Component::ParentDir => {
                return Err(AppError::BadRequest("路径不允许包含 '..'".to_string()));
            }
            Component::Normal(p) => {
                if p.is_empty() {
                    continue;
                }
                cur.push(p);
            }
        }
    }
    let resolved = std::fs::canonicalize(&cur).unwrap_or(cur.clone());
    let canon_base = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    if !resolved.starts_with(&canon_base) {
        return Err(AppError::BadRequest("路径越界".to_string()));
    }
    Ok(resolved)
}

pub fn ensure_layer_tree(data_root: &Path) -> AppResult<()> {
    std::fs::create_dir_all(data_root)?;
    for name in ["raw", "wiki", "schema"] {
        std::fs::create_dir_all(data_root.join(name))?;
    }
    Ok(())
}

/// 列出某层下指定 prefix 的子项。
pub fn list_dir(
    data_root: &Path,
    layer: LayerName,
    prefix: &str,
    embedding_status: Option<&HashMap<String, String>>,
) -> AppResult<(String, Vec<DirEntry>)> {
    let base = layer_root(data_root, layer);
    let target = if prefix.is_empty() {
        base.clone()
    } else {
        safe_resolve_under(&base, prefix)?
    };
    if !target.is_dir() {
        return Err(AppError::NotFound("目录不存在".to_string()));
    }
    let mut entries: Vec<DirEntry> = Vec::new();
    for child in std::fs::read_dir(&target)? {
        let child = child?;
        let path = child.path();
        let is_dir = path.is_dir();
        let name = child.file_name().to_string_lossy().to_string();
        let rel = path.strip_prefix(&base).unwrap_or(&path);
        let posix = path_to_posix(rel);
        let display_path = if is_dir {
            format!("{posix}/")
        } else {
            posix.clone()
        };
        let size = if is_dir {
            None
        } else {
            child.metadata().ok().map(|m| m.len())
        };
        let embedding = if !is_dir && layer == LayerName::Wiki {
            Some(
                embedding_status
                    .and_then(|m| m.get(&posix).cloned())
                    .unwrap_or_else(|| "not_embedded".to_string()),
            )
        } else {
            None
        };
        entries.push(DirEntry {
            name,
            path: display_path,
            is_dir,
            size,
            embedding_status: embedding,
        });
    }
    // 与 Python 一致：先目录后文件，再按 lower(name) 排序
    entries.sort_by(|a, b| {
        let ka = (!a.is_dir, a.name.to_lowercase());
        let kb = (!b.is_dir, b.name.to_lowercase());
        ka.cmp(&kb)
    });
    Ok((prefix.trim_end_matches('/').to_string(), entries))
}

/// 递归列出层内所有文件相对路径，可按后缀过滤。
pub fn list_all_file_paths(
    data_root: &Path,
    layer: LayerName,
    suffix: Option<&str>,
    max_files: usize,
) -> AppResult<(Vec<String>, bool)> {
    let base = layer_root(data_root, layer);
    if !base.is_dir() {
        return Ok((Vec::new(), false));
    }
    let mut collected: Vec<String> = Vec::new();
    let suffix_lower = suffix.map(|s| s.to_lowercase()).filter(|s| !s.is_empty());
    for entry in WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(&base).unwrap_or(entry.path());
        let posix = path_to_posix(rel);
        if let Some(sfx) = &suffix_lower {
            if !posix.to_lowercase().ends_with(sfx) {
                continue;
            }
        }
        collected.push(posix);
    }
    collected.sort();
    let truncated = collected.len() > max_files;
    if truncated {
        collected.truncate(max_files);
    }
    Ok((collected, truncated))
}

pub fn read_file(
    data_root: &Path,
    layer: LayerName,
    rel_path: &str,
    max_bytes: u64,
) -> AppResult<(String, u64)> {
    let base = layer_root(data_root, layer);
    let path = safe_resolve_under(&base, rel_path)?;
    if !path.is_file() {
        return Err(AppError::NotFound("文件不存在".to_string()));
    }
    let meta = std::fs::metadata(&path)?;
    if meta.len() > max_bytes {
        return Err(AppError::PayloadTooLarge("文件超过大小限制".to_string()));
    }
    let bytes = std::fs::read(&path)?;
    let text = String::from_utf8(bytes.clone())
        .map_err(|_| AppError::UnsupportedMedia("仅支持 UTF-8 文本".to_string()))?;
    Ok((text, bytes.len() as u64))
}

pub fn write_file(
    data_root: &Path,
    layer: LayerName,
    rel_path: &str,
    content: &str,
    max_bytes: u64,
) -> AppResult<u64> {
    let bytes = content.as_bytes();
    if (bytes.len() as u64) > max_bytes {
        return Err(AppError::PayloadTooLarge("内容超过大小限制".to_string()));
    }
    let base = layer_root(data_root, layer);
    // 写入路径可能尚不存在，因此不能直接 canonicalize；改为对父目录做 safe 解析。
    let p = sanitize_relative(rel_path)?;
    let absolute = base.join(&p);
    if let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&absolute, bytes)?;
    Ok(bytes.len() as u64)
}

pub fn delete_path(
    data_root: &Path,
    layer: LayerName,
    rel_path: &str,
    forbid_wiki_delete: bool,
) -> AppResult<()> {
    if forbid_wiki_delete && layer == LayerName::Wiki {
        return Err(AppError::Forbidden("已禁止删除编译层".to_string()));
    }
    let base = layer_root(data_root, layer);
    let path = safe_resolve_under(&base, rel_path)?;
    if path == base {
        return Err(AppError::BadRequest("不能删除层根目录".to_string()));
    }
    if !path.exists() {
        return Err(AppError::NotFound("路径不存在".to_string()));
    }
    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// 打包整层或子目录为 ZIP（返回字节）。
pub fn zip_layer_bytes(
    data_root: &Path,
    layer: LayerName,
    prefix: &str,
) -> AppResult<Vec<u8>> {
    let base = layer_root(data_root, layer);
    let root = if prefix.is_empty() {
        base.clone()
    } else {
        safe_resolve_under(&base, prefix)?
    };
    if !root.exists() {
        return Err(AppError::NotFound("路径不存在".to_string()));
    }
    let mut buf: Vec<u8> = Vec::new();
    let cursor = Cursor::new(&mut buf);
    let mut zw = zip::ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if root.is_file() {
        let arcname = format!("{}/{}", layer.as_str(), prefix);
        zw.start_file::<_, ()>(arcname, opts)?;
        let mut f = std::fs::File::open(&root)?;
        let mut content = Vec::new();
        f.read_to_end(&mut content)?;
        zw.write_all(&content)?;
    } else {
        for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            let rel = p.strip_prefix(&base).unwrap_or(p);
            let arcname = format!("{}/{}", layer.as_str(), path_to_posix(rel));
            zw.start_file::<_, ()>(arcname, opts)?;
            let mut f = std::fs::File::open(p)?;
            let mut content = Vec::new();
            f.read_to_end(&mut content)?;
            zw.write_all(&content)?;
        }
    }
    zw.finish()?;
    Ok(buf)
}

fn sanitize_relative(rel: &str) -> AppResult<PathBuf> {
    let trimmed = rel.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("路径为空".to_string()));
    }
    let normalized = trimmed.replace('\\', "/");
    let mut out = PathBuf::new();
    for part in Path::new(&normalized).components() {
        match part {
            Component::CurDir | Component::RootDir | Component::Prefix(_) => continue,
            Component::ParentDir => {
                return Err(AppError::BadRequest("路径不允许包含 '..'".to_string()));
            }
            Component::Normal(p) => out.push(p),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(AppError::BadRequest("路径为空".to_string()));
    }
    Ok(out)
}

/// 上传文件：按 utf-8-sig / utf-8 / gb18030 解码，与 Python `_decode_upload_text` 等价。
pub fn decode_upload_text(raw: &[u8]) -> AppResult<String> {
    // 1) UTF-8 BOM
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(s) = std::str::from_utf8(&raw[3..]) {
            return Ok(s.to_string());
        }
    }
    // 2) 普通 UTF-8
    if let Ok(s) = std::str::from_utf8(raw) {
        return Ok(s.to_string());
    }
    // 3) GB18030（用 encoding_rs 解码）
    let (cow, _enc, had_errors) = encoding_rs::GB18030.decode(raw);
    if had_errors {
        return Err(AppError::UnsupportedMedia(
            "无法作为文本解码：请用 UTF-8 保存后再上传；PDF/图片等二进制不适用本接口。".to_string(),
        ));
    }
    Ok(cow.into_owned())
}

fn path_to_posix(p: &Path) -> String {
    p.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
