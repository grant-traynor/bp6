use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::{get, post},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use crate::dag::{NewQueueItem, QueueItemOption};
use crate::project::{ProjectState, QueueItemAddedEvent, QueueItemResolvedEvent};
use crate::restate::RESTATE_SERVICES_PORT;

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
    let item = ps
        .with_active(|store, _| {
            store.create_queue_item(NewQueueItem {
                project_id: project_id.clone(),
                agent_id: body.agent_id.clone(),
                workflow_id: body.workflow_id.clone(),
                awakeable_id: awakeable_id.clone(),
                question: body.question.clone(),
                options: body.options.clone(),
                context_snapshot: body.context_snapshot.clone().unwrap_or(json!({})),
                priority: body.priority.unwrap_or(2),
            })
        })
        .map_err(|e| bad_request(e))?;

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
    Path(_): Path<String>,
    State(app): State<Arc<AppHandle>>,
    Json(body): Json<ResolveItemRequest>,
) -> Result<Json<Value>, ApiError> {
    let ps = app.state::<ProjectState>();

    // Step 1: Fetch the item from SQLite
    let item = ps
        .with_active(|store, _| store.get_queue_item(&body.item_id))
        .map_err(|e| bad_request(e))?;

    // Step 2: Complete the Restate awakeable if present
    if let Some(awk_id) = item.awakeable_id.as_deref() {
        try_resolve_awakeable(awk_id, &body.resolution).await;
    }

    // Step 3: Mark resolved in SQLite
    ps.with_active(|store, _| store.resolve_queue_item_in_db(&body.item_id, body.resolution.clone()))
        .map_err(|e| internal_error(e))?;

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
    Path(_): Path<String>,
    State(app): State<Arc<AppHandle>>,
) -> Result<Json<Value>, ApiError> {
    let ps = app.state::<ProjectState>();
    let items = ps
        .with_active(|store, proj_id| store.list_queue_items(proj_id))
        .map_err(|e| bad_request(e))?;
    Ok(Json(json!(items)))
}

/// GET /discover (and /poe-queue/discover) — Restate endpoint manifest
///
/// Restate 1.x requires:
///   Content-Type: application/vnd.restate.endpointmanifest.v1+json
/// and the manifest must include minProtocolVersion / maxProtocolVersion.
async fn handle_discover() -> (HeaderMap, String) {
    let manifest = json!({
        "minProtocolVersion": 1,
        "maxProtocolVersion": 1,
        "protocolMode": "REQUEST_RESPONSE",
        "services": [{
            "name": "poe-queue",
            "ty": "VIRTUAL_OBJECT",
            "handlers": [
                { "name": "createItem", "ty": "EXCLUSIVE" },
                { "name": "resolveItem", "ty": "EXCLUSIVE" },
                { "name": "listPending",  "ty": "SHARED"    }
            ]
        }]
    });

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.restate.endpointmanifest.v1+json"),
    );
    (headers, manifest.to_string())
}

// Note: We intentionally do NOT register with Restate's /deployments API.
// The queue service uses Restate only for its awakeables REST API (durable promises).
// Full service registration requires implementing the Restate invocation binary protocol
// (versions 5-6 as of Restate 1.6.x), which is out of scope here. The /discover endpoint
// exists for introspection only.

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the Axum queue service in a background OS thread with its own Tokio runtime.
/// Using std::thread avoids the "no reactor running" panic when called from Tauri's
/// synchronous setup closure (before the main Tokio runtime is active).
pub fn spawn_queue_service(app: AppHandle) {
    let app = Arc::new(app);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime for queue_service");
        rt.block_on(async move {
        let router = Router::new()
            // Restate calls {uri}/discover for service discovery
            .route("/discover", get(handle_discover))
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
            "[queue_service] Listening on http://127.0.0.1:{} (awakeables mode — no Restate registration)",
            QUEUE_SERVICE_PORT
        );

        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("[queue_service] Server error: {}", e);
        }
        });
    });
}
