//! `rework-mcp` — the core engine as an MCP server over stdio.
//!
//! Newline-delimited JSON-RPC (MCP stdio transport). One target per server
//! process: `open` binds it, then `file` is optional on every tool.
//!
//! Register with an agent, e.g. Claude Code:
//! `claude mcp add rework -- rework-mcp`

use std::path::PathBuf;
use std::sync::Arc;

use rework_core::{Analysis, AnalystStore, Evidence, KnifeRunner, Target, VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i64,
    message: String,
}

struct Server {
    target: Mutex<Option<Target>>,
    knife: Mutex<Option<KnifeRunner>>,
    store: AnalystStore,
}

impl Server {
    fn new(store: AnalystStore) -> Self {
        Self {
            target: Mutex::new(None),
            knife: Mutex::new(None),
            store,
        }
    }

    async fn knife(&self) -> Result<KnifeRunner, String> {
        let mut slot = self.knife.lock().await;
        if let Some(k) = slot.clone() {
            return Ok(k);
        }
        let discovered = KnifeRunner::discover().await.map_err(|e| e.to_string())?;
        *slot = Some(discovered.clone());
        Ok(discovered)
    }

    /// Resolve the session target: explicit `file` rebinds, else the bound one.
    async fn resolve_target(&self, params: &Value) -> Result<Target, String> {
        if let Some(file) = params.get("file").and_then(Value::as_str) {
            let target = Target::open(file).await.map_err(|e| e.to_string())?;
            *self.target.lock().await = Some(target.clone());
            return Ok(target);
        }
        self.target
            .lock()
            .await
            .clone()
            .ok_or_else(|| "no target bound: call `open` with a file first".to_string())
    }
}

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    let store = std::env::var_os("REWORK_STORE_DIR")
        .map(|dir| AnalystStore::at(PathBuf::from(dir)))
        .or_else(|| AnalystStore::system_default().ok())
        .unwrap_or_else(|| AnalystStore::at(std::env::temp_dir().join("rework-store")));
    let server = Arc::new(Server::new(store));

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let request: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: Some(RpcError {
                        code: -32700,
                        message: format!("parse error: {e}"),
                    }),
                };
                write_response(&mut stdout, &resp).await;
                continue;
            }
        };
        // Notifications (no id) get no response.
        let Some(id) = request.id.clone() else {
            continue;
        };
        let response = dispatch(&server, &request.method, &request.params, id).await;
        write_response(&mut stdout, &response).await;
    }
}

async fn write_response(stdout: &mut tokio::io::Stdout, resp: &Response) {
    if let Ok(mut bytes) = serde_json::to_vec(resp) {
        bytes.push(b'\n');
        let _ = stdout.write_all(&bytes).await;
        let _ = stdout.flush().await;
    }
}

fn ok(id: Value, result: Value) -> Response {
    Response {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn err(id: Value, code: i64, message: String) -> Response {
    Response {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(RpcError { code, message }),
    }
}

async fn dispatch(server: &Arc<Server>, method: &str, params: &Value, id: Value) -> Response {
    match method {
        "initialize" => ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "rework-mcp", "version": VERSION },
            }),
        ),
        "ping" => ok(id, json!({})),
        "tools/list" => ok(id, json!({ "tools": tool_definitions() })),
        "tools/call" => call_tool(server, params, id).await,
        _ => err(id, -32601, format!("method not found: {method}")),
    }
}

fn tool_definitions() -> Vec<Value> {
    let def = |name: &str, description: &str, required: Vec<&str>, extra: Value| {
        let mut props = serde_json::Map::new();
        props.insert(
            "file".to_string(),
            json!({"type": "string", "description": "Target binary path. Rebinds the session target; omit to use the bound target."}),
        );
        if let Value::Object(map) = extra {
            props.extend(map);
        }
        json!({
            "name": name,
            "description": description,
            "inputSchema": { "type": "object", "properties": props, "required": required },
            "annotations": { "readOnlyHint": !matches!(name, "set_name" | "set_note" | "set_prototype" | "record_unknown") },
        })
    };
    vec![
        def(
            "open",
            "Analyse a binary and make it the session target.",
            vec!["file"],
            json!({}),
        ),
        def(
            "session_status",
            "Bound target identity, knife availability, and store location.",
            vec![],
            json!({}),
        ),
        def(
            "triage",
            "Full static triage of the target.",
            vec![],
            json!({}),
        ),
        def(
            "sec",
            "Exploit mitigations and their consequences.",
            vec![],
            json!({}),
        ),
        def(
            "sinks",
            "Dangerous-API call sites by bug class.",
            vec![],
            json!({}),
        ),
        def(
            "audit",
            "Sink call sites ranked by argument exploitability.",
            vec![],
            json!({}),
        ),
        def(
            "set_name",
            "Persist an analyst name for an address.",
            vec!["address", "name"],
            json!({
            "address": {"type": "string"}, "name": {"type": "string"}}),
        ),
        def(
            "set_note",
            "Attach an analyst note to an address.",
            vec!["address", "text"],
            json!({
            "address": {"type": "string"}, "text": {"type": "string"}}),
        ),
        def(
            "set_prototype",
            "Set a persistent function prototype.",
            vec!["func", "proto"],
            json!({
            "func": {"type": "string"}, "proto": {"type": "string"}}),
        ),
        def(
            "get_store",
            "Everything stored for the target.",
            vec![],
            json!({}),
        ),
        def(
            "record_unknown",
            "Persist an open investigation question.",
            vec!["question"],
            json!({
            "question": {"type": "string"}, "domain": {"type": "string"}}),
        ),
        def(
            "list_unknowns",
            "List open investigation questions.",
            vec![],
            json!({}),
        ),
    ]
}

fn text_result(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn tool_error(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}

fn evidence_text(ev: &Evidence) -> String {
    serde_json::to_string_pretty(ev).unwrap_or_else(|_| "{}".to_string())
}

fn param<'a>(params: &'a Value, name: &str) -> Result<&'a str, String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required parameter: {name}"))
}

async fn call_tool(server: &Arc<Server>, params: &Value, id: Value) -> Response {
    let name = match params.get("name").and_then(Value::as_str) {
        Some(n) => n,
        None => return err(id, -32602, "tools/call requires a name".to_string()),
    };
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    let result = match name {
        "open" => {
            let file = match param(&args, "file") {
                Ok(f) => f,
                Err(e) => return ok(id, tool_error(e)),
            };
            match Target::open(file).await {
                Ok(t) => {
                    *server.target.lock().await = Some(t.clone());
                    text_result(
                        serde_json::to_string_pretty(&t).unwrap_or_else(|_| "{}".to_string()),
                    )
                }
                Err(e) => tool_error(e.to_string()),
            }
        }
        "session_status" => {
            let target = server.target.lock().await.clone();
            let knife = match server.knife().await {
                Ok(k) => k.binary().display().to_string(),
                Err(e) => format!("unavailable: {e}"),
            };
            text_result(
                serde_json::to_string_pretty(&json!({
                    "server": "rework-mcp",
                    "version": VERSION,
                    "target": target,
                    "knife": knife,
                    "store": server.store.root(),
                }))
                .unwrap_or_else(|_| "{}".to_string()),
            )
        }
        "triage" | "sec" | "sinks" | "audit" => {
            let analysis = match name {
                "triage" => Analysis::Triage,
                "sec" => Analysis::Sec,
                "sinks" => Analysis::Sinks,
                _ => Analysis::Audit,
            };
            let target = match server.resolve_target(&args).await {
                Ok(t) => t,
                Err(e) => return ok(id, tool_error(e)),
            };
            let knife = match server.knife().await {
                Ok(k) => k,
                Err(e) => return ok(id, tool_error(e)),
            };
            match knife.analyze(analysis, &target).await {
                Ok(ev) => text_result(evidence_text(&ev)),
                Err(e) => tool_error(e.to_string()),
            }
        }
        "set_name" | "set_note" | "set_prototype" => {
            let target = match server.resolve_target(&args).await {
                Ok(t) => t,
                Err(e) => return ok(id, tool_error(e)),
            };
            let outcome = match name {
                "set_name" => server
                    .store
                    .set_name(
                        &target.sha256,
                        param(&args, "address").unwrap_or(""),
                        param(&args, "name").unwrap_or(""),
                    )
                    .await
                    .map(|_| "name saved".to_string()),
                "set_note" => server
                    .store
                    .add_note(
                        &target.sha256,
                        param(&args, "address").unwrap_or(""),
                        param(&args, "text").unwrap_or(""),
                    )
                    .await
                    .map(|_| "note saved".to_string()),
                _ => server
                    .store
                    .set_prototype(
                        &target.sha256,
                        param(&args, "func").unwrap_or(""),
                        param(&args, "proto").unwrap_or(""),
                    )
                    .await
                    .map(|_| "prototype saved".to_string()),
            };
            match outcome {
                Ok(msg) => text_result(msg),
                Err(e) => tool_error(e.to_string()),
            }
        }
        "get_store" => {
            let target = match server.resolve_target(&args).await {
                Ok(t) => t,
                Err(e) => return ok(id, tool_error(e)),
            };
            match server.store.annotations(&target.sha256).await {
                Ok(ann) => text_result(
                    serde_json::to_string_pretty(&ann).unwrap_or_else(|_| "{}".to_string()),
                ),
                Err(e) => tool_error(e.to_string()),
            }
        }
        "record_unknown" => {
            let question = match param(&args, "question") {
                Ok(q) => q,
                Err(e) => return ok(id, tool_error(e)),
            };
            let domain = args.get("domain").and_then(Value::as_str).unwrap_or("");
            let target_sha = server.target.lock().await.clone().map(|t| t.sha256);
            match server
                .store
                .record_unknown(question, domain, target_sha.as_deref())
                .await
            {
                Ok(rec) => text_result(
                    serde_json::to_string_pretty(&rec).unwrap_or_else(|_| "{}".to_string()),
                ),
                Err(e) => tool_error(e.to_string()),
            }
        }
        "list_unknowns" => match server.store.list_unknowns().await {
            Ok(list) => text_result(
                serde_json::to_string_pretty(&list).unwrap_or_else(|_| "[]".to_string()),
            ),
            Err(e) => tool_error(e.to_string()),
        },
        _ => return err(id, -32602, format!("unknown tool: {name}")),
    };
    ok(id, result)
}
