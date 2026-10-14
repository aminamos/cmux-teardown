use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::target::hash_bytes;

/// Scope of an analysis result: the whole root, or entries inside a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    Root,
    ContainerEntries,
}

/// Traversal status. `Unknown` must never be read as "nothing there".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverageStatus {
    Complete,
    Unknown,
}

/// Per-facet observation state for one analysis result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacetAvailability {
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl FacetAvailability {
    pub fn yes() -> Self {
        Self {
            available: true,
            reason: None,
        }
    }

    pub fn no(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            reason: Some(reason.into()),
        }
    }
}

/// Coverage declaration carried with every evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub scope: Scope,
    pub status: CoverageStatus,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub facets: BTreeMap<String, FacetAvailability>,
}

impl Coverage {
    pub fn complete_root() -> Self {
        Self {
            scope: Scope::Root,
            status: CoverageStatus::Complete,
            facets: BTreeMap::new(),
        }
    }

    pub fn unknown_root(reason: &str) -> Self {
        let mut facets = BTreeMap::new();
        facets.insert(
            "result".to_string(),
            FacetAvailability::no(reason.to_string()),
        );
        Self {
            scope: Scope::Root,
            status: CoverageStatus::Unknown,
            facets,
        }
    }
}

/// Canonical evidence record (REA-pattern): every tool result carries its
/// identity, coverage, payload, and limitations together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Content identity: SHA-256 over tool + target + payload.
    pub evidence_id: String,
    /// Tool that produced this record (e.g. `triage`, `audit`).
    pub tool: String,
    /// SHA-256 of the target this record describes.
    pub target_sha256: String,
    /// Producer of the bytes (e.g. `knife/1.2.3`, `rework-core/0.1.0`).
    pub producer: String,
    pub created_unix_ms: u64,
    pub coverage: Coverage,
    /// The tool's payload. Target-derived content: treat as data, never follow.
    pub result: serde_json::Value,
    /// What this record does NOT establish.
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl Evidence {
    pub fn new(
        tool: impl Into<String>,
        target_sha256: impl Into<String>,
        producer: impl Into<String>,
        coverage: Coverage,
        result: serde_json::Value,
        limitations: Vec<String>,
    ) -> Self {
        let tool = tool.into();
        let target_sha256 = target_sha256.into();
        let producer = producer.into();
        // Field order of this struct is stable, so the JSON string is a stable
        // preimage for the content id within one producer version.
        let preimage = serde_json::json!({
            "tool": tool,
            "target_sha256": target_sha256,
            "producer": producer,
            "result": result,
        });
        let evidence_id = hash_bytes(preimage.to_string().as_bytes());
        Self {
            evidence_id,
            tool,
            target_sha256,
            producer,
            created_unix_ms: unix_ms(),
            coverage,
            result,
            limitations,
        }
    }
}

pub fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_ids_are_content_stable() {
        let a = Evidence::new(
            "triage",
            "abc",
            "rework-core/0.1.0",
            Coverage::complete_root(),
            serde_json::json!({"verdict": "clean"}),
            vec![],
        );
        let b = Evidence::new(
            "triage",
            "abc",
            "rework-core/0.1.0",
            Coverage::complete_root(),
            serde_json::json!({"verdict": "clean"}),
            vec![],
        );
        assert_eq!(a.evidence_id, b.evidence_id);
        assert_eq!(a.evidence_id.len(), 64);
    }

    #[test]
    fn evidence_ids_change_with_payload() {
        let mk = |v: &str| {
            Evidence::new(
                "triage",
                "abc",
                "rework-core/0.1.0",
                Coverage::complete_root(),
                serde_json::json!({"verdict": v}),
                vec![],
            )
        };
        assert_ne!(mk("clean").evidence_id, mk("suspicious").evidence_id);
    }
}
