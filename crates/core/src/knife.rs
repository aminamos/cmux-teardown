use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::error::Error;
use crate::evidence::{Coverage, Evidence, FacetAvailability};
use crate::target::Target;

/// Analysis entry points. Each maps to a `knife` invocation; every command
/// runs with `--json` and is wrapped in an [`Evidence`] record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Analysis {
    /// Full triage: verdict, hashes, sections, mitigations, IOCs, entry disasm.
    Triage,
    /// Exploit mitigations and their consequences.
    Sec,
    /// Dangerous-API call sites grouped by bug class.
    Sinks,
    /// Sink call sites ranked by argument exploitability.
    Audit,
}

impl Analysis {
    fn tool_name(self) -> &'static str {
        match self {
            Analysis::Triage => "triage",
            Analysis::Sec => "sec",
            Analysis::Sinks => "sinks",
            Analysis::Audit => "audit",
        }
    }

    fn as_args(self) -> &'static [&'static str] {
        match self {
            // `knife FILE` (bare) is the full triage; the rest are subcommands.
            Analysis::Triage => &[],
            Analysis::Sec => &["sec"],
            Analysis::Sinks => &["sinks"],
            Analysis::Audit => &["audit"],
        }
    }
}

/// Runs the `knife` binary and wraps results in evidence records.
///
/// M0 strategy: wrap the binary (`cargo install reknife`) instead of linking
/// the engine. The evidence contract stays ours either way, so a native port
/// later changes no API surface.
#[derive(Debug, Clone)]
pub struct KnifeRunner {
    bin: PathBuf,
}

impl KnifeRunner {
    /// Locate `knife`: `REWORK_KNIFE_BIN` wins, else `knife` on `PATH`.
    /// The path is validated by running `knife --version`.
    pub async fn discover() -> Result<Self, Error> {
        let bin = std::env::var_os("REWORK_KNIFE_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("knife"));
        Self::with_binary(bin).await
    }

    pub async fn with_binary(bin: PathBuf) -> Result<Self, Error> {
        let probe = Command::new(&bin).arg("--version").output().await;
        match probe {
            Ok(out) if out.status.success() => Ok(Self { bin }),
            _ => Err(Error::KnifeNotFound(bin.display().to_string())),
        }
    }

    pub fn binary(&self) -> &Path {
        &self.bin
    }

    /// Best-effort `knife --version` string for evidence `producer` fields.
    pub async fn version_string(&self) -> String {
        match Command::new(&self.bin).arg("--version").output().await {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            }
            _ => "knife/unknown".to_string(),
        }
    }

    /// Run one analysis over a target and wrap it in an evidence record.
    pub async fn analyze(&self, analysis: Analysis, target: &Target) -> Result<Evidence, Error> {
        let mut args: Vec<String> = analysis.as_args().iter().map(|s| s.to_string()).collect();
        args.push(target.path.display().to_string());
        args.push("--json".to_string());

        let output = Command::new(&self.bin).args(&args).output().await?;
        if !output.status.success() {
            return Err(Error::KnifeFailed {
                args: args.join(" "),
                status: output.status.to_string(),
                stderr: truncate(String::from_utf8_lossy(&output.stderr).trim(), 2000),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let value: serde_json::Value = serde_json::from_str(&stdout)?;

        let producer = self.version_string().await;
        let coverage = coverage_for(analysis, &value);
        let limitations = limitations_for(analysis);
        Ok(Evidence::new(
            analysis.tool_name(),
            target.sha256.clone(),
            producer,
            coverage,
            value,
            limitations,
        ))
    }
}

/// Declare per-analysis facet availability from the returned payload.
/// Unknown shapes stay `Unknown` — never claim coverage we didn't observe.
fn coverage_for(analysis: Analysis, value: &serde_json::Value) -> Coverage {
    use std::collections::BTreeMap;

    let mut facets = BTreeMap::new();
    let mut facet = |name: &str, present: bool| {
        facets.insert(
            name.to_string(),
            if present {
                FacetAvailability::yes()
            } else {
                FacetAvailability::no("not present in knife output".to_string())
            },
        );
    };

    match analysis {
        Analysis::Triage => {
            facet("verdict", value.get("verdict").is_some());
            facet("hashes", value.get("hashes").is_some());
            facet("sections", value.get("sections").is_some());
        }
        Analysis::Sec => {
            facet("mitigations", value.get("mitigations").is_some());
        }
        Analysis::Sinks => {
            facet("sinks", value.get("sinks").is_some());
        }
        Analysis::Audit => {
            facet("findings", value.get("findings").is_some());
        }
    }

    let status = if facets.values().all(|f| f.available) {
        crate::evidence::CoverageStatus::Complete
    } else {
        crate::evidence::CoverageStatus::Unknown
    };
    Coverage {
        scope: crate::evidence::Scope::Root,
        status,
        facets,
    }
}

fn limitations_for(analysis: Analysis) -> Vec<String> {
    let mut out =
        vec!["Static analysis only: reads bytes on disk, never executes the target.".to_string()];
    match analysis {
        Analysis::Triage => out.push(
            "Triage verdict is heuristic; absence of findings does not establish safety."
                .to_string(),
        ),
        Analysis::Sec => out
            .push("Mitigation readings describe on-disk records, not loader behavior.".to_string()),
        Analysis::Sinks => out
            .push("Sink list is direct-call only; indirect dispatch is out of scope.".to_string()),
        Analysis::Audit => {
            out.push("Data-flow walk is bounded and x86/x64-only.".to_string());
            out.push("Empty results do not establish unreachability or safety.".to_string());
        }
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…[truncated]", &s[..max])
    }
}
