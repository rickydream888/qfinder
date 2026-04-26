use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(String),
    #[error("permission denied: {0}")]
    Permission(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("conflict at: {path}")]
    Conflict { path: String, is_dir: bool },
    #[error("illegal target: {0}")]
    IllegalTarget(String),
    #[error("a background task is running")]
    BusyTask,
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Io(_) => "IO",
            AppError::Permission(_) => "PERMISSION",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::AlreadyExists(_) => "ALREADY_EXISTS",
            AppError::Conflict { .. } => "CONFLICT",
            AppError::IllegalTarget(_) => "ILLEGAL_TARGET",
            AppError::BusyTask => "BUSY_TASK",
            AppError::Internal(_) => "INTERNAL",
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind::*;
        match err.kind() {
            NotFound => AppError::NotFound(err.to_string()),
            PermissionDenied => AppError::Permission(err.to_string()),
            AlreadyExists => AppError::AlreadyExists(err.to_string()),
            _ => AppError::Io(err.to_string()),
        }
    }
}

impl From<trash::Error> for AppError {
    fn from(err: trash::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("code", self.code())?;
        m.serialize_entry("message", &self.to_string())?;
        if let AppError::Conflict { path, is_dir } = self {
            m.serialize_entry("path", path)?;
            m.serialize_entry("isDir", is_dir)?;
        }
        m.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
