use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

use crate::dag::{NewQueueItem, QueueItemOption};
use crate::project::{ProjectState, QueueItemAddedEvent, QueueItemResolvedEvent};
use crate::restate::{RESTATE_ADMIN_PORT, RESTATE_SERVICES_PORT};

// ── Constants ──────────────────────────────────────────────────────────────────

pub const QUEUE_SERVICE_PORT: u16 = 9082;

// ── Error helper ──────────────────────────────────────────────────────────────

type ApiError = (StatusCode, Json<Value>);

fn internal_error(msg: impl std::fmt::Display) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": msg.to_string() })),
    )
}

fn bad_request(msg: impl std::fmt::Display) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": msg.to_string() })),
    )
}

// ── Request bodies ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateItemRequest {
    agent_id: String,
    workflow_id: Option<String>,
    question: String,
    options: Vec<QueueItemOption>,
    context_snapshot: Option<Value>,
    priority: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveItemRequest {
    item_id: String,
    resolution: Value,
}

// ── Awakeable helpers ─────────────────────────────────────────────────────────

/// Attempt to create a Restate awakeable. Returns None if Restate is unavailable.
async fn try_create_awakeable() -> Option<String> {
    let url = format!("http://127.0.0.1:{}/restate/awakeables", RESTATE_SERVICES_PORT);
    let client = reqwest::Client::new();
    match client.post(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<Value>().await {
                    Ok(body) => body
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    Err(e) => {
                        eprintln!("[queue_service] Failed to parse awakeable response: {}", e);
                        None
                    }
                }
            } else {
                eprintln!(
                    "[queue_service] Restate returned {} when creating awakeable",
                    resp.status()
                );
                None
            }
        }
        Err(e) => {
            eprintln!("[queue_service] Restate not available for awakeable creation: {}", e);
            None
        }
    }
}

/// Attempt to resolve a Restate awakeable. Best-effort — logs on failure.
async fn try_resolve_awakeable(awakeable_id: &str, resolution: &Value) {
    let url = format!(
        "http://127.0.0.1:{}/restate/awakeables/{}/resolve",
        RESTATE_SERVICES_PORT, awakeable_id
    );
    let client = reqwest::Client::new();
    match client.post(&url).json(resolution).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                eprintln!(
                    "[queue_service] Restate resolve awakeable '{}' returned {}",
                    awakeable_id,
                    resp.status()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "[queue_service] Failed to resolve awakeable '{}': {}",
                awakeable_id, e
            );
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /poe-queue/:project_id/create_item
async fn handle_create_item(
    Path(project_id): Path<String>,
    State(app): State<Arc<AppHandle>>,
    Json(body): Json<CreateItemRequest>,
) -> Result<Json<Value>, ApiError> {
    // Step 1: Try to create a Restate awakeable (non-fatal if unavailable)
    let awakeable_id = try_create_awakeable().await;

    // Step 2: Persist to SQLite via ProjectState
    let ps = app.state::<ProjectState>();
    let item = {
        let lock = ps.store.lock().map_err(|e| internal_error(e.to_string()))?;
        let store = lock.as_ref().ok_or_else(|| bad_request("No project open"))?;
        store
            .create_queue_item(NewQueueItem {
                project_id,
                agent_id: body.agent_id,
                workflow_id: body.workflow_id,
                awakeable_id: awakeable_id.clone(),
                question: body.question,
                options: body.options,
                context_snapshot: body.context_snapshot.unwrap_or(json!({})),
                priority: body.priority.unwrap_or(2),
            })
            .map_err(|e| internal_error(e))?
    };

    // Step 3: Emit Tauri event
    app.emit("queue:item:added", QueueItemAddedEvent { item: item.clone() })
        .map_err(|e| internal_error(format!("Failed to emit queue:item:added: {}", e)))?;

    // Step 4: Return IDs
    Ok(Json(json!({
        "awakeableId": item.awakeable_id,
        "itemId": item.id,
    })))
}

/// POST /poe-queue/:project_id/resolve_item
async fn handle_resolve_item(
    Path(_project_id): Path<String>,
    State(app): State<Arc<AppHandle>>,
    Json(body): Json<ResolveItemRequest>,
) -> Result<Json<Value>, ApiError> {
    let ps = app.state::<ProjectState>();

    // Step 1: Fetch the item from SQLite
    let item = {
        let lock = ps.store.lock().map_err(|e| internal_error(e.to_string()))?;
        let store = lock.as_ref().ok_or_else(|| bad_request("No project open"))?;
        store
            .get_queue_item(&body.item_id)
            .map_err(|e| internal_error(e))?
    };

    // Step 2: Complete the Restate awakeable if present
    if let Some(ref awk_id) = item.awakeable_id {
        try_resolve_awakeable(awk_id, &body.resolution).await;
    }

    // Step 3: Mark resolved in SQLite
    {
        let lock = ps.store.lock().map_err(|e| internal_error(e.to_string()))?;
        let store = lock.as_ref().ok_or_else(|| bad_request("No project open"))?;
        store
            .resolve_queue_item_in_db(&body.item_id, body.resolution)
            .map_err(|e| internal_error(e))?;
    }

    // Step 4: Emit Tauri event
    app.emit(
        "queue:item:resolved",
        QueueItemResolvedEvent {
            item_id: body.item_id.clone(),
        },
    )
    .map_err(|e| internal_error(format!("Failed to emit queue:item:resolved: {}", e)))?;

    Ok(Json(json!({ "ok": true })))
}

/// GET /poe-queue/:project_id/pending
async fn handle_list_pending(
    Path(project_id): Path<String>,
    State(app): State<Arc<AppHandle>>,
) -> Result<Json<Value>, ApiError> {
    let ps = app.state::<ProjectState>();
    let lock = ps.store.lock().map_err(|e| internal_error(e.to_string()))?;
    let store = lock.as_ref().ok_or_else(|| bad_request("No project open"))?;
    let items = store
        .list_queue_items(&project_id)
        .map_err(|e| internal_error(e))?;
    Ok(Json(json!(items)))
}

/// GET /poe-queue/discover — Restate service discovery endpoint
async fn handle_discover() -> Json<Value> {
    Json(json!({
        "protocolMode": "REQUEST_RESPONSE",
        "services": [{
            "name": "poe-queue",
            "ty": "VIRTUAL_OBJECT",
            "handlers": [
                { "name": "createItem", "ty": "EXCLUSIVE" },
                { "name": "resolveItem", "ty": "EXCLUSIVE" },
                { "name": "listPending", "ty": "SHARED" }
            ]
        }]
    }))
}

// ── Restate registration ──────────────────────────────────────────────────────

async fn register_with_restate(port: u16) {
    let url = format!("http://127.0.0.1:{}/deployments", RESTATE_ADMIN_PORT);
    let body = json!({ "uri": format!("http://localhost:{}", port) });
    let client = reqwest::Client::new();

    for attempt in 1..=5u8 {
        match client.post(&url).json(&body).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    eprintln!(
                        "[queue_service] Registered with Restate at {} (attempt {})",
                        url, attempt
                    );
                    return;
                } else {
                    eprintln!(
                        "[queue_service] Restate registration returned {} (attempt {})",
                        resp.status(),
                        attempt
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "[queue_service] Restate registration failed (attempt {}): {}",
                    attempt, e
                );
            }
        }

        if attempt < 5 {
            sleep(Duration::from_secs(1)).await;
        }
    }

    eprintln!(
        "[queue_service] Could not register with Restate after 5 attempts — continuing without it"
    );
}

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the Axum queue service in a background tokio task.
pub fn spawn_queue_service(app: AppHandle) {
    let app = Arc::new(app);

    tokio::spawn(async move {
        let router = Router::new()
            .route("/poe-queue/discover", get(handle_discover))
            .route(
                "/poe-queue/:project_id/create_item",
                post(handle_create_item),
            )
            .route(
                "/poe-queue/:project_id/resolve_item",
                post(handle_resolve_item),
            )
            .route("/poe-queue/:project_id/pending", get(handle_list_pending))
            .with_state(app);

        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], QUEUE_SERVICE_PORT));
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "[queue_service] Failed to bind on port {}: {}",
                    QUEUE_SERVICE_PORT, e
                );
                return;
            }
        };

        eprintln!(
            "[queue_service] Listening on http://127.0.0.1:{}",
            QUEUE_SERVICE_PORT
        );

        // Register with Restate in the background — non-fatal
        tokio::spawn(register_with_restate(QUEUE_SERVICE_PORT));

        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("[queue_service] Server error: {}", e);
        }
    });
}
