use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::error::{AppError, AppResult};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum TaskKind {
    Rename,
    Copy,
    Move,
    Delete,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Running,
    Done,
    Failed,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub id: String,
    pub kind: TaskKind,
    pub description: String,
    pub started_at_ms: u64,
    pub status: TaskStatus,
}

#[derive(Default)]
pub struct TaskManager {
    inner: Mutex<Option<TaskInfo>>,
}

impl TaskManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub async fn current(&self) -> Option<TaskInfo> {
        self.inner.lock().await.clone()
    }

    /// Run the future as a single in-flight task. Returns the started task info.
    pub async fn run<F, Fut>(
        self: Arc<Self>,
        app: AppHandle,
        kind: TaskKind,
        description: String,
        work: F,
    ) -> AppResult<TaskInfo>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = AppResult<()>> + Send + 'static,
    {
        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Err(AppError::BusyTask);
        }
        let info = TaskInfo {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.clone(),
            description: description.clone(),
            started_at_ms: now_ms(),
            status: TaskStatus::Running,
        };
        *guard = Some(info.clone());
        drop(guard);

        let _ = app.emit("task://started", info.clone());

        let mgr = Arc::clone(&self);
        let info_clone = info.clone();
        let app_for_spawn = app.clone();
        tokio::spawn(async move {
            let res = work().await;
            let mut g = mgr.inner.lock().await;
            let mut finished = info_clone.clone();
            match res {
                Ok(()) => {
                    finished.status = TaskStatus::Done;
                    let _ = app_for_spawn.emit("task://finished", finished.clone());
                }
                Err(e) => {
                    finished.status = TaskStatus::Failed;
                    let _ = app_for_spawn.emit(
                        "task://failed",
                        serde_json::json!({
                            "id": finished.id,
                            "code": e.code(),
                            "message": e.to_string(),
                        }),
                    );
                }
            }
            *g = None;
        });

        Ok(info)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
