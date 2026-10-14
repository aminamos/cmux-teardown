//! Rework core: targets, evidence records, knife runner, analyst store.
//!
//! One engine, every frontend: the CLI, MCP server, and Tauri app all call
//! into this crate. Static analysis only — nothing here executes the target.

pub mod error;
pub mod evidence;
pub mod knife;
pub mod store;
pub mod target;

pub use error::Error;
pub use evidence::{Coverage, CoverageStatus, Evidence, Scope};
pub use knife::{Analysis, KnifeRunner};
pub use store::{AnalystStore, TargetAnnotations, UnknownRecord, UnknownStatus};
pub use target::Target;

/// Crate version, also reported by the MCP server and Tauri backend.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
