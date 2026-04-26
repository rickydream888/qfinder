use crate::error::AppResult;
use crate::platform::{self, RootEntry};

#[tauri::command]
pub fn list_roots() -> AppResult<Vec<RootEntry>> {
    platform::list_roots()
}

#[tauri::command]
pub fn os_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}
