use std::path::PathBuf;

use thiserror::Error;

/// Core error type. Variants are user-facing; keep messages actionable.
#[derive(Debug, Error)]
pub enum Error {
    #[error("target not found: {0}")]
    TargetNotFound(PathBuf),

    #[error("target is not a regular file: {0}")]
    TargetNotFile(PathBuf),

    #[error("knife binary not found (set REWORK_KNIFE_BIN or put `knife` on PATH): {0}")]
    KnifeNotFound(String),

    #[error("knife `{args}` failed with status {status}: {stderr}")]
    KnifeFailed {
        args: String,
        status: String,
        stderr: String,
    },

    #[error("knife output was not valid JSON: {0}")]
    KnifeBadJson(#[from] serde_json::Error),

    #[error("store error at {path}: {message}")]
    Store { path: PathBuf, message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
