use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::platform::{self, ensure_exists};

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "html", "htm", "css", "js", "mjs", "cjs", "ts", "tsx", "jsx", "json",
    "xml", "yaml", "yml", "toml", "ini", "conf", "cfg", "log", "csv", "tsv", "rs", "py", "rb",
    "go", "java", "kt", "swift", "c", "cpp", "cc", "cxx", "h", "hpp", "hh", "sh", "bash", "zsh",
    "fish", "bat", "ps1", "psm1", "sql", "lua", "pl", "php", "vue", "svelte", "scss", "sass",
    "less", "env", "gitignore", "gitattributes", "lock", "tex", "diff", "patch",
];

const IMAGE_EXTENSIONS: &[&str] =
    &["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif"];

const TEXT_PREVIEW_LIMIT: u64 = 10 * 1024;
const IMAGE_PREVIEW_LIMIT: u64 = 20 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PreviewPayload {
    #[serde(rename_all = "camelCase")]
    Directory {
        sub_dirs: u64,
        sub_files: u64,
        total_size: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Text {
        content: String,
        truncated: bool,
        total_size: u64,
    },
    #[serde(rename_all = "camelCase")]
    Image { path: String, size: u64 },
    #[serde(rename_all = "camelCase")]
    ImageTooLarge { size: u64 },
    #[serde(rename_all = "camelCase")]
    Other { size: u64 },
}

#[tauri::command]
pub async fn preview(path: String) -> AppResult<PreviewPayload> {
    tokio::task::spawn_blocking(move || preview_blocking(path))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

fn preview_blocking(path: String) -> AppResult<PreviewPayload> {
    let p = PathBuf::from(&path);
    ensure_exists(&p)?;
    let meta = std::fs::metadata(&p).map_err(AppError::from)?;
    if meta.is_dir() {
        return preview_dir(&p);
    }
    let size = meta.len();
    if is_image(&p) {
        if size > IMAGE_PREVIEW_LIMIT {
            return Ok(PreviewPayload::ImageTooLarge { size });
        }
        return Ok(PreviewPayload::Image {
            path: platform::path_to_string(&p),
            size,
        });
    }
    if is_text(&p) {
        return preview_text(&p, size);
    }
    Ok(PreviewPayload::Other { size })
}

fn preview_dir(p: &Path) -> AppResult<PreviewPayload> {
    let mut sub_dirs: u64 = 0;
    let mut sub_files: u64 = 0;
    let read = std::fs::read_dir(p).map_err(AppError::from)?;
    for entry in read.flatten() {
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => sub_dirs += 1,
            Ok(_) => sub_files += 1,
            Err(_) => {}
        }
    }

    let total_size = du_size(p);
    Ok(PreviewPayload::Directory {
        sub_dirs,
        sub_files,
        total_size,
    })
}

fn du_size(p: &Path) -> Option<u64> {
    if cfg!(target_os = "windows") {
        return None;
    }
    if !platform::has_command("du") {
        return None;
    }
    let output = Command::new("du").arg("-sk").arg(p).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.split_whitespace().next()?;
    let kb: u64 = first.parse().ok()?;
    Some(kb.saturating_mul(1024))
}

fn preview_text(p: &Path, size: u64) -> AppResult<PreviewPayload> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).map_err(AppError::from)?;
    let limit = TEXT_PREVIEW_LIMIT;
    let mut buf = Vec::with_capacity(limit.min(size) as usize);
    let mut handle = f.by_ref().take(limit);
    handle.read_to_end(&mut buf).map_err(AppError::from)?;
    let truncated = size > limit;
    let content = String::from_utf8_lossy(&buf).into_owned();
    Ok(PreviewPayload::Text {
        content,
        truncated,
        total_size: size,
    })
}

fn ext_lower(p: &Path) -> Option<String> {
    p.extension().map(|e| e.to_string_lossy().to_lowercase())
}

fn is_image(p: &Path) -> bool {
    matches!(ext_lower(p), Some(ref e) if IMAGE_EXTENSIONS.contains(&e.as_str()))
}

fn is_text(p: &Path) -> bool {
    if let Some(ref e) = ext_lower(p) {
        if TEXT_EXTENSIONS.contains(&e.as_str()) {
            return true;
        }
    }
    // Fallback: use `file --mime` on Unix when available.
    if cfg!(unix) && platform::has_command("file") {
        if let Ok(output) = Command::new("file").arg("--mime").arg(p).output() {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                return s.contains("text/")
                    || s.contains("application/json")
                    || s.contains("application/xml");
            }
        }
    }
    false
}
