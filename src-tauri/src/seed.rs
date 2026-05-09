//! 首次启动时若 `wiki/` 下无 Markdown，则从内置资源或开发目录拷贝默认 wiki。

use std::path::Path;

use tauri::path::BaseDirectory;
use tauri::App;
use tauri::Manager;

fn wiki_dir_has_md(wiki: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(wiki) else {
        return false;
    };
    rd.filter_map(|e| e.ok()).any(|e| {
        e.path()
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("md"))
    })
}

fn copy_md_files(src: &Path, dest: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dest)?;
    let mut n = 0;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("md"))
        {
            std::fs::copy(&p, dest.join(entry.file_name()))?;
            n += 1;
        }
    }
    Ok(n)
}

/// 与 `pathy-knowledge-server` 的 `data/wiki` 对齐：仅在目标 wiki 尚无 `.md` 时写入。
pub fn seed_default_wiki_if_empty(app: &App, data_root: &Path) -> std::io::Result<()> {
    #[cfg(mobile)]
    {
        let _ = (app, data_root);
        return Ok(());
    }

    #[cfg(not(mobile))]
    {
        let wiki = data_root.join("wiki");
        if wiki_dir_has_md(&wiki) {
            return Ok(());
        }

        if let Ok(bundle_dir) = app.path().resolve("default-wiki", BaseDirectory::Resource) {
            if bundle_dir.is_dir() {
                let n = copy_md_files(&bundle_dir, &wiki)?;
                if n > 0 {
                    tracing::info!(
                        from = %bundle_dir.display(),
                        count = n,
                        "已从应用资源种子化默认 wiki"
                    );
                    return Ok(());
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("data")
                .join("wiki");
            if dev.is_dir() {
                let n = copy_md_files(&dev, &wiki)?;
                if n > 0 {
                    tracing::info!(
                        from = %dev.display(),
                        count = n,
                        "已从开发目录种子化默认 wiki"
                    );
                }
            }
        }

        Ok(())
    }
}
