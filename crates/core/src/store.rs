use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::evidence::unix_ms;

/// Analyst annotations for one target, keyed by content hash.
/// Follows the binary, not the path.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetAnnotations {
    /// address -> analyst-assigned name
    #[serde(default)]
    pub names: BTreeMap<String, String>,
    /// address -> notes in insertion order
    #[serde(default)]
    pub notes: BTreeMap<String, Vec<String>>,
    /// function -> C prototype string
    #[serde(default)]
    pub prototypes: BTreeMap<String, String>,
    #[serde(default)]
    pub updated_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnknownStatus {
    Open,
    Answered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnknownRecord {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub target_sha256: Option<String>,
    pub status: UnknownStatus,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    pub created_unix_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UnknownLedgerFile {
    #[serde(default)]
    unknowns: Vec<UnknownRecord>,
}

/// On-disk analyst store: `targets/<sha256>.json` + a global `unknowns.json`.
#[derive(Debug, Clone)]
pub struct AnalystStore {
    root: PathBuf,
}

impl AnalystStore {
    /// System store: `{data_dir}/rework` (`%APPDATA%/rework` on Windows).
    pub fn system_default() -> Result<Self, Error> {
        let dir = dirs::data_dir()
            .ok_or_else(|| Error::Store {
                path: PathBuf::from("<data_dir>"),
                message: "no per-user data directory on this platform".to_string(),
            })?
            .join("rework");
        Ok(Self::at(dir))
    }

    /// Store rooted at an explicit directory (tests, portable installs).
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn targets_dir(&self) -> PathBuf {
        self.root.join("targets")
    }

    fn target_file(&self, sha256: &str) -> PathBuf {
        self.targets_dir().join(format!("{sha256}.json"))
    }

    fn unknowns_file(&self) -> PathBuf {
        self.root.join("unknowns.json")
    }

    async fn read_json<T: Default + serde::de::DeserializeOwned>(
        &self,
        path: &Path,
    ) -> Result<T, Error> {
        match tokio::fs::read(path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| Error::Store {
                path: path.to_path_buf(),
                message: format!("invalid JSON: {e}"),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
            Err(e) => Err(Error::Store {
                path: path.to_path_buf(),
                message: e.to_string(),
            }),
        }
    }

    async fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::Store {
                    path: parent.to_path_buf(),
                    message: e.to_string(),
                })?;
        }
        // Write-temp-then-rename so a crash never leaves a half-written file.
        let tmp = path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| Error::Store {
            path: path.to_path_buf(),
            message: format!("serialize failed: {e}"),
        })?;
        tokio::fs::write(&tmp, &bytes)
            .await
            .map_err(|e| Error::Store {
                path: tmp.clone(),
                message: e.to_string(),
            })?;
        tokio::fs::rename(&tmp, path)
            .await
            .map_err(|e| Error::Store {
                path: path.to_path_buf(),
                message: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn annotations(&self, sha256: &str) -> Result<TargetAnnotations, Error> {
        self.read_json(&self.target_file(sha256)).await
    }

    pub async fn set_name(&self, sha256: &str, address: &str, name: &str) -> Result<(), Error> {
        let address = normalize_address(address)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(Error::Store {
                path: self.target_file(sha256),
                message: "name must not be empty".to_string(),
            });
        }
        let mut ann = self.annotations(sha256).await?;
        ann.names.insert(address, name);
        ann.updated_unix_ms = unix_ms();
        self.write_json(&self.target_file(sha256), &ann).await
    }

    pub async fn add_note(&self, sha256: &str, address: &str, text: &str) -> Result<(), Error> {
        let address = normalize_address(address)?;
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(Error::Store {
                path: self.target_file(sha256),
                message: "note must not be empty".to_string(),
            });
        }
        let mut ann = self.annotations(sha256).await?;
        ann.notes.entry(address).or_default().push(text);
        ann.updated_unix_ms = unix_ms();
        self.write_json(&self.target_file(sha256), &ann).await
    }

    pub async fn set_prototype(&self, sha256: &str, func: &str, proto: &str) -> Result<(), Error> {
        let func = func.trim().to_string();
        let proto = proto.trim().to_string();
        if func.is_empty() || proto.is_empty() {
            return Err(Error::Store {
                path: self.target_file(sha256),
                message: "function and prototype must not be empty".to_string(),
            });
        }
        let mut ann = self.annotations(sha256).await?;
        ann.prototypes.insert(func, proto);
        ann.updated_unix_ms = unix_ms();
        self.write_json(&self.target_file(sha256), &ann).await
    }

    pub async fn record_unknown(
        &self,
        question: &str,
        domain: &str,
        target_sha256: Option<&str>,
    ) -> Result<UnknownRecord, Error> {
        let question = question.trim().to_string();
        if question.is_empty() {
            return Err(Error::Store {
                path: self.unknowns_file(),
                message: "question must not be empty".to_string(),
            });
        }
        let mut file: UnknownLedgerFile = self.read_json(&self.unknowns_file()).await?;
        let id = format!("u{}", file.unknowns.len() + 1);
        let record = UnknownRecord {
            id,
            question,
            domain: domain.trim().to_string(),
            target_sha256: target_sha256.map(|s| s.to_string()),
            status: UnknownStatus::Open,
            evidence_ids: vec![],
            created_unix_ms: unix_ms(),
        };
        file.unknowns.push(record.clone());
        self.write_json(&self.unknowns_file(), &file).await?;
        Ok(record)
    }

    pub async fn list_unknowns(&self) -> Result<Vec<UnknownRecord>, Error> {
        let file: UnknownLedgerFile = self.read_json(&self.unknowns_file()).await?;
        Ok(file.unknowns)
    }
}

fn normalize_address(address: &str) -> Result<String, Error> {
    let a = address.trim().to_lowercase();
    if a.is_empty() {
        return Err(Error::Store {
            path: PathBuf::from("<address>"),
            message: "address must not be empty".to_string(),
        });
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn names_notes_prototypes_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = AnalystStore::at(dir.path().to_path_buf());
        let sha = "abc123";

        store
            .set_name(sha, "0x4017A0", "parse_record")
            .await
            .unwrap();
        store
            .add_note(sha, "0x4017a0", "len from packet")
            .await
            .unwrap();
        store
            .set_prototype(sha, "parse_record", "int parse_record(char *buf, int len)")
            .await
            .unwrap();

        let ann = store.annotations(sha).await.unwrap();
        assert_eq!(ann.names.get("0x4017a0").unwrap(), "parse_record");
        assert_eq!(ann.notes.get("0x4017a0").unwrap(), &vec!["len from packet"]);
        assert!(ann.prototypes.contains_key("parse_record"));

        // A fresh handle sees the same data (persistence, not memory).
        let store2 = AnalystStore::at(dir.path().to_path_buf());
        let ann2 = store2.annotations(sha).await.unwrap();
        assert_eq!(ann2.names, ann.names);
    }

    #[tokio::test]
    async fn unknowns_ledger_appends() {
        let dir = tempfile::tempdir().unwrap();
        let store = AnalystStore::at(dir.path().to_path_buf());
        let r1 = store
            .record_unknown("who calls parse_record?", "callers", None)
            .await
            .unwrap();
        let r2 = store
            .record_unknown("is len bounded?", "taint", Some("abc"))
            .await
            .unwrap();
        assert_eq!(r1.id, "u1");
        assert_eq!(r2.id, "u2");
        assert_eq!(store.list_unknowns().await.unwrap().len(), 2);
    }
}
