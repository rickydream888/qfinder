use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::platform::ensure_exists;
use crate::task::{TaskInfo, TaskKind, TaskManager};

fn parent_or_err(p: &Path) -> AppResult<&Path> {
    p.parent()
        .ok_or_else(|| AppError::IllegalTarget(format!("no parent for: {}", p.display())))
}

fn join_dst(dst_dir: &Path, src: &Path) -> AppResult<PathBuf> {
    let name = src
        .file_name()
        .ok_or_else(|| AppError::IllegalTarget(format!("no file name: {}", src.display())))?;
    Ok(dst_dir.join(name))
}

fn ensure_not_self_or_descendant(src: &Path, dst_dir: &Path) -> AppResult<()> {
    let src_canon = std::fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    let dst_canon = std::fs::canonicalize(dst_dir).unwrap_or_else(|_| dst_dir.to_path_buf());
    if dst_canon == src_canon || dst_canon.starts_with(&src_canon) {
        return Err(AppError::IllegalTarget(
            "cannot move/copy a directory into itself or its descendant".into(),
        ));
    }
    Ok(())
}

fn copy_recursive(src: &Path, dst: &Path) -> AppResult<()> {
    let meta = std::fs::metadata(src).map_err(AppError::from)?;
    if meta.is_dir() {
        std::fs::create_dir(dst).map_err(AppError::from)?;
        for entry in std::fs::read_dir(src).map_err(AppError::from)? {
            let entry = entry.map_err(AppError::from)?;
            let from = entry.path();
            let to = dst.join(entry.file_name());
            copy_recursive(&from, &to)?;
        }
    } else {
        std::fs::copy(src, dst).map_err(AppError::from)?;
    }
    Ok(())
}

fn remove_path(p: &Path) -> AppResult<()> {
    let meta = std::fs::symlink_metadata(p).map_err(AppError::from)?;
    if meta.is_dir() {
        std::fs::remove_dir_all(p).map_err(AppError::from)?;
    } else {
        std::fs::remove_file(p).map_err(AppError::from)?;
    }
    Ok(())
}

/// Merge contents of `src` (a directory) into `dst` (an existing directory),
/// overwriting files on conflict and recursing into matching subdirectories.
fn merge_copy(src: &Path, dst: &Path) -> AppResult<()> {
    for entry in std::fs::read_dir(src).map_err(AppError::from)? {
        let entry = entry.map_err(AppError::from)?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ft = entry.file_type().map_err(AppError::from)?;
        if ft.is_dir() {
            if to.is_dir() {
                merge_copy(&from, &to)?;
            } else {
                if to.exists() {
                    remove_path(&to)?;
                }
                copy_recursive(&from, &to)?;
            }
        } else {
            if to.exists() {
                remove_path(&to)?;
            }
            std::fs::copy(&from, &to).map_err(AppError::from)?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ConflictStrategy {
    None,
    Merge,
    Replace,
}

fn parse_strategy(s: Option<String>) -> AppResult<ConflictStrategy> {
    match s.as_deref() {
        None => Ok(ConflictStrategy::None),
        Some("merge") => Ok(ConflictStrategy::Merge),
        Some("replace") => Ok(ConflictStrategy::Replace),
        Some(other) => Err(AppError::IllegalTarget(format!(
            "unknown conflict strategy: {other}"
        ))),
    }
}

#[tauri::command]
pub async fn op_rename(
    app: AppHandle,
    manager: State<'_, Arc<TaskManager>>,
    path: String,
    new_name: String,
) -> AppResult<TaskInfo> {
    if new_name.is_empty() || new_name.contains('/') || new_name.contains('\\') {
        return Err(AppError::IllegalTarget(format!("invalid name: {new_name}")));
    }
    let p = PathBuf::from(&path);
    ensure_exists(&p)?;
    let parent = parent_or_err(&p)?.to_path_buf();
    let dst = parent.join(&new_name);
    if dst.exists() {
        return Err(AppError::AlreadyExists(format!("{}", dst.display())));
    }
    let desc = format!(
        "重命名 {} 到 {}",
        p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or(path.clone()),
        new_name
    );
    let mgr = Arc::clone(&manager);
    mgr.run(app, TaskKind::Rename, desc, move || async move {
        std::fs::rename(&p, &dst).map_err(AppError::from)
    })
    .await
}

#[tauri::command]
pub async fn op_copy(
    app: AppHandle,
    manager: State<'_, Arc<TaskManager>>,
    src: String,
    dst_dir: String,
    on_conflict: Option<String>,
) -> AppResult<TaskInfo> {
    let s = PathBuf::from(&src);
    let d = PathBuf::from(&dst_dir);
    ensure_exists(&s)?;
    ensure_exists(&d)?;
    if !d.is_dir() {
        return Err(AppError::IllegalTarget(format!(
            "not a directory: {dst_dir}"
        )));
    }
    if s.is_dir() {
        ensure_not_self_or_descendant(&s, &d)?;
    }
    let dst = join_dst(&d, &s)?;
    let strategy = parse_strategy(on_conflict)?;
    if dst.exists() && matches!(strategy, ConflictStrategy::None) {
        return Err(AppError::Conflict {
            path: dst.display().to_string(),
            is_dir: dst.is_dir(),
        });
    }
    if matches!(strategy, ConflictStrategy::Merge) && (!s.is_dir() || !dst.is_dir()) {
        return Err(AppError::IllegalTarget(
            "合并仅适用于源与目标都为目录的场景".into(),
        ));
    }
    let desc = format!("复制 {} 到 {}", s.display(), d.display());
    let mgr = Arc::clone(&manager);
    mgr.run(app, TaskKind::Copy, desc, move || async move {
        match strategy {
            ConflictStrategy::None => copy_recursive(&s, &dst),
            ConflictStrategy::Replace => {
                if dst.exists() {
                    remove_path(&dst)?;
                }
                copy_recursive(&s, &dst)
            }
            ConflictStrategy::Merge => merge_copy(&s, &dst),
        }
    })
    .await
}

#[tauri::command]
pub async fn op_move(
    app: AppHandle,
    manager: State<'_, Arc<TaskManager>>,
    src: String,
    dst_dir: String,
    on_conflict: Option<String>,
) -> AppResult<TaskInfo> {
    let s = PathBuf::from(&src);
    let d = PathBuf::from(&dst_dir);
    ensure_exists(&s)?;
    ensure_exists(&d)?;
    if !d.is_dir() {
        return Err(AppError::IllegalTarget(format!(
            "not a directory: {dst_dir}"
        )));
    }
    if s.is_dir() {
        ensure_not_self_or_descendant(&s, &d)?;
    }
    let dst = join_dst(&d, &s)?;
    let strategy = parse_strategy(on_conflict)?;
    if dst.exists() && matches!(strategy, ConflictStrategy::None) {
        return Err(AppError::Conflict {
            path: dst.display().to_string(),
            is_dir: dst.is_dir(),
        });
    }
    if matches!(strategy, ConflictStrategy::Merge) && (!s.is_dir() || !dst.is_dir()) {
        return Err(AppError::IllegalTarget(
            "合并仅适用于源与目标都为目录的场景".into(),
        ));
    }
    let desc = format!("移动 {} 到 {}", s.display(), d.display());
    let mgr = Arc::clone(&manager);
    mgr.run(app, TaskKind::Move, desc, move || async move {
        match strategy {
            ConflictStrategy::None => match std::fs::rename(&s, &dst) {
                Ok(()) => Ok(()),
                Err(_) => {
                    // Cross-device fallback: copy then remove. Semantics remain Move.
                    copy_recursive(&s, &dst)?;
                    remove_path(&s)
                }
            },
            ConflictStrategy::Replace => {
                if dst.exists() {
                    remove_path(&dst)?;
                }
                match std::fs::rename(&s, &dst) {
                    Ok(()) => Ok(()),
                    Err(_) => {
                        copy_recursive(&s, &dst)?;
                        remove_path(&s)
                    }
                }
            }
            ConflictStrategy::Merge => {
                merge_copy(&s, &dst)?;
                remove_path(&s)
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn op_delete(
    app: AppHandle,
    manager: State<'_, Arc<TaskManager>>,
    path: String,
) -> AppResult<TaskInfo> {
    let p = PathBuf::from(&path);
    ensure_exists(&p)?;
    let desc = format!("删除 {}", p.display());
    let mgr = Arc::clone(&manager);
    mgr.run(app, TaskKind::Delete, desc, move || async move {
        trash::delete(&p).map_err(AppError::from)
    })
    .await
}

#[tauri::command]
pub async fn current_task(manager: State<'_, Arc<TaskManager>>) -> AppResult<Option<TaskInfo>> {
    Ok(manager.current().await)
}

#[tauri::command]
pub fn open_default(path: String) -> AppResult<()> {
    let p = PathBuf::from(&path);
    ensure_exists(&p)?;
    open::that(&p).map_err(|e| AppError::Io(e.to_string()))
}
