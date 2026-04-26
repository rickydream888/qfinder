mod commands;
mod error;
mod platform;
mod task;

use std::sync::Arc;

use task::TaskManager;

fn main() {
    tauri::Builder::default()
        .manage::<Arc<TaskManager>>(TaskManager::new())
        .invoke_handler(tauri::generate_handler![
            commands::roots::list_roots,
            commands::roots::os_family,
            commands::fs_tree::read_dir,
            commands::preview::preview,
            commands::ops::op_rename,
            commands::ops::op_copy,
            commands::ops::op_move,
            commands::ops::op_delete,
            commands::ops::current_task,
            commands::ops::open_default,
        ])
        .run(tauri::generate_context!())
        .expect("error while running qfinder");
}
