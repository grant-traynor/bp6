use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::Value;
use tauri::Manager;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::project::ProjectState;

// ── Shared state ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct McpServerInner {
    app: tauri::AppHandle,
    sessions: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
    port: u16,
}

// ── Query params ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SessionQuery {
    session: String,
}

// ── JSON-RPC helpers ──────────────────────────────────────────────────────────

fn json_rpc_error(id: Value, code: i32, message: String) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "error": { "code": code, "message": message },
        "id": id
    })
    .to_string()
}

fn json_rpc_ok(id: Value, result: Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id
    })
    .to_string()
}

// ── Tool schema definitions ────────────────────────────────────────────────────

fn tools_list() -> Value {
    serde_json::json!([
        {
            "name": "dag_list_nodes",
            "description": "List DAG nodes for a project, optionally filtered by type and/or status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "The project ID" },
                    "type": { "type": "string", "description": "Optional node type filter (e.g. Task, Epic, Feature)" },
                    "status": { "type": "string", "description": "Optional status filter on node.data.status" }
                },
                "required": ["project_id"]
            }
        },
        {
            "name": "dag_get_node",
            "description": "Retrieve a single DAG node by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string" }
                },
                "required": ["project_id", "node_id"]
            }
        },
        {
            "name": "dag_get_dependencies",
            "description": "Get the incoming and outgoing edges (dependencies) for a DAG node.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string" }
                },
                "required": ["project_id", "node_id"]
            }
        },
        {
            "name": "dag_search",
            "description": "Case-insensitive full-text search over node data JSON for a project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "query": { "type": "string", "description": "Search string" }
                },
                "required": ["project_id", "query"]
            }
        },
        {
            "name": "dag_get_artefact",
            "description": "Retrieve a KnowledgeArtifact DAG node by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "artefact_id": { "type": "string" }
                },
                "required": ["project_id", "artefact_id"]
            }
        },
        {
            "name": "dag_list_queue",
            "description": "List queue items for a project, optionally filtered by status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "status": { "type": "string", "description": "Optional status filter (pending, resolved)" }
                },
                "required": ["project_id"]
            }
        }
    ])
}

// ── Tool dispatch ─────────────────────────────────────────────────────────────

fn handle_tool_call(
    app: &tauri::AppHandle,
    id: Value,
    tool_name: &str,
    args: &Value,
) -> String {
    let project_state = app.state::<ProjectState>();

    match tool_name {
        "dag_list_nodes" => {
            let project_id = match args["project_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing project_id".to_string()),
            };
            let type_filter = args["type"].as_str().map(|s| s.to_lowercase());
            let status_filter = args["status"].as_str().map(|s| s.to_lowercase());

            let projects = project_state.projects.lock().unwrap();
            let proj = match projects.get(&project_id).or_else(|| {
                // Also allow lookup by project_id UUID (not directory path)
                projects.values().find(|p| p.project_id == project_id)
            }) {
                Some(p) => p,
                None => {
                    return json_rpc_error(
                        id,
                        -32000,
                        format!("Project {} not open", project_id),
                    )
                }
            };

            let pid = proj.project_id.clone();
            match proj.store.list_nodes(&pid) {
                Err(e) => json_rpc_error(id, -32000, e),
                Ok(nodes) => {
                    let filtered: Vec<_> = nodes
                        .into_iter()
                        .filter(|n| {
                            if let Some(ref tf) = type_filter {
                                if n.node_type.to_lowercase() != *tf {
                                    return false;
                                }
                            }
                            if let Some(ref sf) = status_filter {
                                let node_status = n.data["status"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_lowercase();
                                if node_status != *sf {
                                    return false;
                                }
                            }
                            true
                        })
                        .collect();
                    json_rpc_ok(id, serde_json::json!({ "nodes": filtered }))
                }
            }
        }

        "dag_get_node" => {
            let project_id = match args["project_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing project_id".to_string()),
            };
            let node_id = match args["node_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing node_id".to_string()),
            };

            let projects = project_state.projects.lock().unwrap();
            let proj = match projects.get(&project_id).or_else(|| {
                projects.values().find(|p| p.project_id == project_id)
            }) {
                Some(p) => p,
                None => {
                    return json_rpc_error(
                        id,
                        -32000,
                        format!("Project {} not open", project_id),
                    )
                }
            };

            match proj.store.get_node(&node_id) {
                Err(_) => json_rpc_error(id, -32001, format!("Node {} not found", node_id)),
                Ok(node) => json_rpc_ok(id, serde_json::to_value(node).unwrap_or(Value::Null)),
            }
        }

        "dag_get_dependencies" => {
            let project_id = match args["project_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing project_id".to_string()),
            };
            let node_id = match args["node_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing node_id".to_string()),
            };

            let projects = project_state.projects.lock().unwrap();
            let proj = match projects.get(&project_id).or_else(|| {
                projects.values().find(|p| p.project_id == project_id)
            }) {
                Some(p) => p,
                None => {
                    return json_rpc_error(
                        id,
                        -32000,
                        format!("Project {} not open", project_id),
                    )
                }
            };

            let pid = proj.project_id.clone();
            let incoming = match proj.store.get_incoming_edges(&node_id) {
                Err(e) => return json_rpc_error(id, -32000, e),
                Ok(v) => v,
            };
            let all_edges = match proj.store.list_edges(&pid) {
                Err(e) => return json_rpc_error(id, -32000, e),
                Ok(v) => v,
            };
            let outgoing: Vec<_> = all_edges
                .into_iter()
                .filter(|e| e.from_id == node_id)
                .collect();

            json_rpc_ok(
                id,
                serde_json::json!({ "incoming": incoming, "outgoing": outgoing }),
            )
        }

        "dag_search" => {
            let project_id = match args["project_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing project_id".to_string()),
            };
            let query = match args["query"].as_str() {
                Some(v) => v.to_lowercase(),
                None => return json_rpc_error(id, -32602, "Missing query".to_string()),
            };

            let projects = project_state.projects.lock().unwrap();
            let proj = match projects.get(&project_id).or_else(|| {
                projects.values().find(|p| p.project_id == project_id)
            }) {
                Some(p) => p,
                None => {
                    return json_rpc_error(
                        id,
                        -32000,
                        format!("Project {} not open", project_id),
                    )
                }
            };

            let pid = proj.project_id.clone();
            match proj.store.list_nodes(&pid) {
                Err(e) => json_rpc_error(id, -32000, e),
                Ok(nodes) => {
                    let matched: Vec<_> = nodes
                        .into_iter()
                        .filter(|n| {
                            let data_str = serde_json::to_string(&n.data)
                                .unwrap_or_default()
                                .to_lowercase();
                            data_str.contains(&query)
                        })
                        .collect();
                    json_rpc_ok(id, serde_json::json!({ "nodes": matched }))
                }
            }
        }

        "dag_get_artefact" => {
            let project_id = match args["project_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing project_id".to_string()),
            };
            let artefact_id = match args["artefact_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing artefact_id".to_string()),
            };

            let projects = project_state.projects.lock().unwrap();
            let proj = match projects.get(&project_id).or_else(|| {
                projects.values().find(|p| p.project_id == project_id)
            }) {
                Some(p) => p,
                None => {
                    return json_rpc_error(
                        id,
                        -32000,
                        format!("Project {} not open", project_id),
                    )
                }
            };

            match proj.store.get_node(&artefact_id) {
                Err(_) => {
                    json_rpc_error(id, -32001, format!("Node {} not found", artefact_id))
                }
                Ok(node) => json_rpc_ok(id, serde_json::to_value(node).unwrap_or(Value::Null)),
            }
        }

        "dag_list_queue" => {
            let project_id = match args["project_id"].as_str() {
                Some(v) => v.to_string(),
                None => return json_rpc_error(id, -32602, "Missing project_id".to_string()),
            };
            let status_filter = args["status"].as_str().map(|s| s.to_lowercase());

            let projects = project_state.projects.lock().unwrap();
            let proj = match projects.get(&project_id).or_else(|| {
                projects.values().find(|p| p.project_id == project_id)
            }) {
                Some(p) => p,
                None => {
                    return json_rpc_error(
                        id,
                        -32000,
                        format!("Project {} not open", project_id),
                    )
                }
            };

            let pid = proj.project_id.clone();
            match proj.store.list_queue_items(&pid) {
                Err(e) => json_rpc_error(id, -32000, e),
                Ok(items) => {
                    let filtered: Vec<_> = items
                        .into_iter()
                        .filter(|item| {
                            if let Some(ref sf) = status_filter {
                                return item.status.to_lowercase() == *sf;
                            }
                            true
                        })
                        .collect();
                    json_rpc_ok(id, serde_json::json!({ "items": filtered }))
                }
            }
        }

        other => json_rpc_error(id, -32601, format!("Unknown tool: {}", other)),
    }
}

// ── MCP method dispatch ───────────────────────────────────────────────────────

fn dispatch_request(state: &McpServerInner, body: &str) -> String {
    let req: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({
                "jsonrpc": "2.0",
                "error": { "code": -32700, "message": format!("Parse error: {}", e) },
                "id": null
            })
            .to_string()
        }
    };

    let id = req["id"].clone();
    let method = match req["method"].as_str() {
        Some(m) => m,
        None => return json_rpc_error(id, -32600, "Invalid Request: missing method".to_string()),
    };

    match method {
        "initialize" => json_rpc_ok(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "poe", "version": "0.1.0" }
            }),
        ),

        "notifications/initialized" => json_rpc_ok(id, serde_json::json!({})),

        "tools/list" => json_rpc_ok(
            id,
            serde_json::json!({ "tools": tools_list() }),
        ),

        "tools/call" => {
            let tool_name = match req["params"]["name"].as_str() {
                Some(n) => n.to_string(),
                None => {
                    return json_rpc_error(id, -32602, "Missing tool name".to_string())
                }
            };
            let args = req["params"]["arguments"].clone();
            handle_tool_call(&state.app, id, &tool_name, &args)
        }

        other => json_rpc_error(id, -32601, format!("Method not found: {}", other)),
    }
}

// ── HTTP handlers ─────────────────────────────────────────────────────────────

async fn handle_sse(
    State(state): State<McpServerInner>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let session_id = Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::channel::<String>(64);

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), tx);
    }

    let endpoint_url = format!(
        "http://127.0.0.1:{}/message?session={}",
        state.port, session_id
    );

    // Send the endpoint event immediately, then forward channel messages
    let endpoint_event = async_stream::stream! {
        // First event: tell client where to POST
        yield Ok::<Event, Infallible>(
            Event::default()
                .event("endpoint")
                .data(endpoint_url)
        );

        // Forward subsequent messages from the channel
        let mut stream = ReceiverStream::new(rx);
        use tokio_stream::StreamExt;
        while let Some(msg) = stream.next().await {
            yield Ok(Event::default().data(msg));
        }
    };

    Sse::new(endpoint_event).keep_alive(KeepAlive::default())
}

async fn handle_message(
    State(state): State<McpServerInner>,
    Query(params): Query<SessionQuery>,
    body: String,
) -> axum::response::Response {
    let response = dispatch_request(&state, &body);

    let tx = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&params.session).cloned()
    };

    match tx {
        None => axum::response::Response::builder()
            .status(404)
            .body(axum::body::Body::from("Session not found"))
            .unwrap(),
        Some(sender) => {
            if sender.send(response).await.is_err() {
                eprintln!("[mcp_server] Failed to send response to session {}", params.session);
            }
            axum::response::Response::builder()
                .status(202)
                .body(axum::body::Body::empty())
                .unwrap()
        }
    }
}

// ── Public startup ────────────────────────────────────────────────────────────

pub fn spawn_mcp_server(app: tauri::AppHandle) -> u16 {
    // Try ports 9084..=9090; pick the first available
    let listener = (9084u16..=9090)
        .find_map(|port| {
            std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).ok()
        })
        .expect("[mcp_server] No available port in range 9084-9090");

    let port = listener.local_addr().unwrap().port();

    let inner = McpServerInner {
        app,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        port,
    };

    let router = Router::new()
        .route("/sse", get(handle_sse))
        .route("/message", post(handle_message))
        .with_state(inner);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .expect("[mcp_server] Failed to create Tokio runtime");
        rt.block_on(async move {
            let tokio_listener = tokio::net::TcpListener::from_std(listener)
                .expect("[mcp_server] Failed to convert std listener to tokio listener");

            eprintln!("[mcp_server] Listening on http://127.0.0.1:{}", port);

            if let Err(e) = axum::serve(tokio_listener, router).await {
                eprintln!("[mcp_server] Server error: {}", e);
            }
        });
    });

    port
}
