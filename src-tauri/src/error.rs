//! Application error types for AgentDock.

use std::path::PathBuf;
use thiserror::Error;

/// Unified error type returned across backend services and Tauri commands.
#[derive(Debug, Error)]
pub enum AppError {
    /// Underlying filesystem I/O failure with path context.
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// JSON serialization or deserialization failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Home or Application Support directory could not be resolved.
    #[error("could not resolve system directory: {0}")]
    DirectoryUnavailable(&'static str),

    /// Invalid configuration or input path supplied to a command.
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Failed to launch an external macOS helper such as Finder or Open.
    #[error("command execution failed: {0}")]
    Command(String),
}

impl AppError {
    /// Constructs an [`AppError::Io`] carrying the path that failed.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}
