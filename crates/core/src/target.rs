use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use crate::error::Error;

/// A binary under analysis. Identity is the SHA-256 of its bytes, never the path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    /// Display path the target was opened from.
    pub path: PathBuf,
    /// Lowercase hex SHA-256 of the file bytes.
    pub sha256: String,
    /// File size in bytes.
    pub size: u64,
}

impl Target {
    /// Open and hash a target file.
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let meta = tokio::fs::metadata(&path)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Error::TargetNotFound(path.clone()),
                _ => Error::Io(e),
            })?;
        if !meta.is_file() {
            return Err(Error::TargetNotFile(path));
        }
        let sha256 = hash_file(&path).await?;
        Ok(Self {
            path,
            sha256,
            size: meta.len(),
        })
    }

    /// Short identity for UI/log lines: first 12 hex chars of the SHA-256.
    pub fn short_id(&self) -> String {
        self.sha256.chars().take(12).collect()
    }
}

/// SHA-256 hex digest of a file's bytes (streamed, 64 KiB chunks).
pub async fn hash_file(path: &Path) -> Result<String, Error> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// SHA-256 hex digest of in-memory bytes.
pub fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.bin");
        tokio::fs::write(&path, b"abc").await.unwrap();
        let target = Target::open(&path).await.unwrap();
        assert_eq!(
            target.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(target.size, 3);
        assert_eq!(target.short_id(), "ba7816bf8f01");
    }

    #[tokio::test]
    async fn missing_file_is_not_found() {
        let err = Target::open("/nonexistent/rework-sample.bin")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::TargetNotFound(_)));
    }
}
