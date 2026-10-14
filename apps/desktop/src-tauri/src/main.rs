//! Rework desktop backend (Tauri).
//!
//! Thin command layer over `rework-core` + `rework-net`. All analysis logic
//! lives in the core crates so the CLI, MCP server, and app share one engine.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use rework_core::{Analysis, AnalystStore, Evidence, KnifeRunner, Target};
use rework_net::Node;
use tokio::sync::Mutex;

struct AppState {
    node: Mutex<Option<Arc<Node>>>,
}

fn store() -> Result<AnalystStore, String> {
    AnalystStore::system_default().map_err(|e| e.to_string())
}

#[tauri::command]
async fn knife_status(knife_bin: Option<String>) -> Result<String, String> {
    let knife = match knife_bin {
        Some(bin) => KnifeRunner::with_binary(PathBuf::from(bin))
            .await
            .map_err(|e| e.to_string())?,
        None => KnifeRunner::discover().await.map_err(|e| e.to_string())?,
    };
    Ok(format!(
        "{} ({})",
        knife.version_string().await,
        knife.binary().display()
    ))
}

#[tauri::command]
async fn open_target(path: String) -> Result<Target, String> {
    Target::open(path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn analyze(
    path: String,
    tool: String,
    knife_bin: Option<String>,
) -> Result<Evidence, String> {
    let analysis = match tool.as_str() {
        "triage" => Analysis::Triage,
        "sec" => Analysis::Sec,
        "sinks" => Analysis::Sinks,
        "audit" => Analysis::Audit,
        other => return Err(format!("unknown analysis tool: {other}")),
    };
    let target = Target::open(path).await.map_err(|e| e.to_string())?;
    let knife = match knife_bin {
        Some(bin) => KnifeRunner::with_binary(PathBuf::from(bin))
            .await
            .map_err(|e| e.to_string())?,
        None => KnifeRunner::discover().await.map_err(|e| e.to_string())?,
    };
    knife
        .analyze(analysis, &target)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_annotations(path: String) -> Result<rework_core::TargetAnnotations, String> {
    let target = Target::open(path).await.map_err(|e| e.to_string())?;
    store()?
        .annotations(&target.sha256)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_name(path: String, address: String, name: String) -> Result<(), String> {
    let target = Target::open(path).await.map_err(|e| e.to_string())?;
    store()?
        .set_name(&target.sha256, &address, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_note(path: String, address: String, text: String) -> Result<(), String> {
    let target = Target::open(path).await.map_err(|e| e.to_string())?;
    store()?
        .add_note(&target.sha256, &address, &text)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn net_status(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let mut slot = state.node.lock().await;
    if slot.is_none() {
        let node = Node::bind_ephemeral().await.map_err(|e| e.to_string())?;
        *slot = Some(Arc::new(node));
    }
    let node = slot.clone().expect("just bound");
    Ok(serde_json::json!({
        "nodeId": node.id().to_string(),
        "shortId": node.short_id(),
        "addr": format!("{:?}", node.addr()),
    }))
}

#[tauri::command]
async fn net_echo_self(
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<String, String> {
    // Loopback probe: bind a second node in-process, serve echo on it, roundtrip.
    // Exercises the full QUIC stack without needing a remote peer yet.
    let server = Node::bind_ephemeral().await.map_err(|e| e.to_string())?;
    let peer = server.addr();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let serve = tokio::spawn(async move { rework_net::serve_echo(server, rx).await });

    let client = {
        let slot = state.node.lock().await;
        match slot.clone() {
            Some(n) => n,
            None => {
                drop(slot);
                let node = Node::bind_ephemeral().await.map_err(|e| e.to_string())?;
                *state.node.lock().await = Some(Arc::new(node.clone()));
                Arc::new(node)
            }
        }
    };
    let reply = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        rework_net::echo_roundtrip(&client, peer, message.as_bytes()),
    )
    .await
    .map_err(|_| "echo timed out after 30s".to_string())?
    .map_err(|e| e.to_string())?;
    let _ = tx.send(());
    let _ = serve.await;
    String::from_utf8(reply).map_err(|e| format!("non-utf8 echo reply: {e}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            node: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            knife_status,
            open_target,
            analyze,
            get_annotations,
            set_name,
            add_note,
            net_status,
            net_echo_self
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rework");
}
