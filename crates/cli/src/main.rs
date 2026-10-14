//! `rework` — static RE triage from the terminal.
//!
//! Analysis commands print the canonical [`Evidence`] record as pretty JSON
//! on stdout (one record per invocation; composable in pipes) plus a one-line
//! summary on stderr.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use rework_core::{Analysis, AnalystStore, Evidence, KnifeRunner, Target};

#[derive(Debug, Parser)]
#[command(name = "rework", version, about = "Static reverse-engineering triage")]
struct Cli {
    /// Path to the `knife` binary (also: REWORK_KNIFE_BIN env).
    #[arg(long, env = "REWORK_KNIFE_BIN")]
    knife_bin: Option<PathBuf>,

    /// Override the analyst store directory (default: per-user data dir).
    #[arg(long, env = "REWORK_STORE_DIR")]
    store_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Full triage: verdict, hashes, sections, mitigations, IOCs.
    Triage { file: PathBuf },
    /// Exploit mitigations and their consequences.
    Sec { file: PathBuf },
    /// Dangerous-API call sites grouped by bug class.
    Sinks { file: PathBuf },
    /// Sink call sites ranked by argument exploitability.
    Audit { file: PathBuf },
    /// Persist an analyst name for an address.
    Name {
        file: PathBuf,
        address: String,
        name: String,
    },
    /// Attach an analyst note to an address.
    Note {
        file: PathBuf,
        address: String,
        text: String,
    },
    /// Set a persistent function prototype.
    Proto {
        file: PathBuf,
        func: String,
        proto: String,
    },
    /// Show everything stored for a binary.
    Db { file: PathBuf },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let store = match cli.store_dir {
        Some(dir) => AnalystStore::at(dir),
        None => AnalystStore::system_default()?,
    };

    match cli.command {
        Command::Triage { file } => run_analysis(cli.knife_bin, Analysis::Triage, file).await,
        Command::Sec { file } => run_analysis(cli.knife_bin, Analysis::Sec, file).await,
        Command::Sinks { file } => run_analysis(cli.knife_bin, Analysis::Sinks, file).await,
        Command::Audit { file } => run_analysis(cli.knife_bin, Analysis::Audit, file).await,
        Command::Name {
            file,
            address,
            name,
        } => {
            let target = Target::open(&file).await?;
            store.set_name(&target.sha256, &address, &name).await?;
            eprintln!("named {} {} -> {name}", target.short_id(), address.trim());
            Ok(())
        }
        Command::Note {
            file,
            address,
            text,
        } => {
            let target = Target::open(&file).await?;
            store.add_note(&target.sha256, &address, &text).await?;
            eprintln!("noted {} {}", target.short_id(), address.trim());
            Ok(())
        }
        Command::Proto { file, func, proto } => {
            let target = Target::open(&file).await?;
            store.set_prototype(&target.sha256, &func, &proto).await?;
            eprintln!("prototype {} {func} set", target.short_id());
            Ok(())
        }
        Command::Db { file } => {
            let target = Target::open(&file).await?;
            let ann = store.annotations(&target.sha256).await?;
            println!("{}", serde_json::to_string_pretty(&ann)?);
            Ok(())
        }
    }
}

async fn run_analysis(
    knife_bin: Option<PathBuf>,
    analysis: Analysis,
    file: PathBuf,
) -> anyhow::Result<()> {
    let target = Target::open(&file)
        .await
        .with_context(|| format!("opening {}", file.display()))?;
    let knife = match knife_bin {
        Some(bin) => KnifeRunner::with_binary(bin).await?,
        None => KnifeRunner::discover().await?,
    };
    let evidence: Evidence = knife.analyze(analysis, &target).await?;
    eprintln!(
        "{} {} evidence={} coverage={:?}",
        evidence.tool,
        target.short_id(),
        &evidence.evidence_id[..12],
        evidence.coverage.status,
    );
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}
