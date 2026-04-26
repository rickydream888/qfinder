use std::path::PathBuf;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::platform::{self, ensure_exists};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryDto {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub size: Option<u64>,
}

#[tauri::command]
pub fn read_dir(path: String, show_hidden: bool) -> AppResult<Vec<DirEntryDto>> {
    let p = PathBuf::from(&path);
    ensure_exists(&p)?;
    if !p.is_dir() {
        return Err(AppError::IllegalTarget(format!("not a directory: {path}")));
    }

    let read = std::fs::read_dir(&p).map_err(AppError::from)?;
    let mut out: Vec<DirEntryDto> = Vec::new();
    for entry in read {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let full = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        // Always skip Windows backwards-compatibility junctions.
        if platform::is_compat_junction(&meta) {
            continue;
        }
        let is_dir = meta.is_dir();
        let is_hidden = platform::is_hidden(&full, &name, Some(&meta));
        if is_hidden && !show_hidden {
            continue;
        }
        let size = if is_dir { None } else { Some(meta.len()) };
        out.push(DirEntryDto {
            name,
            path: platform::path_to_string(&full),
            is_dir,
            is_hidden,
            size,
        });
    }

    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}
