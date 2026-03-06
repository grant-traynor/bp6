pub mod advisor;
pub mod agents;
pub mod conversation;
pub mod dag;
pub mod lifecycle;
pub mod mcp_server;
pub mod project;
pub mod queue_service;
pub mod restate;
pub mod skills;

use tauri::Manager;

use advisor::{confirm_advisor_session, start_advisor_query, send_advisor_message, reset_advisor_session, get_advisor_state};
use agents::{kill_agent, spawn_agent, stop_agent_graceful, write_to_agent, AgentState};
use conversation::{start_conversation_turn, continue_conversation_turn};
use project::{
    close_project, create_edge, create_lifecycle_artifact, create_node, create_queue_item,
    delete_edge, delete_node, get_artefacts_for_step,
    get_provenance, get_snapshot, get_stage_metrics, list_open_projects, list_queue_items,
    load_app_state, open_project, probe_node, promote_skill, redirect_workflow, resolve_queue_item,
    save_app_state, stop_workflow, switch_project, update_node, ProjectState,
};
use lifecycle::{approve_lifecycle_step, get_lifecycle_status, spawn_lifecycle_service, start_lifecycle, submit_rework_items};
use mcp_server::spawn_mcp_server;
use queue_service::spawn_queue_service;
use restate::{is_restate_healthy, spawn_restate, stop_restate, wait_for_restate_healthy, RestateState};
use skills::{get_skill_content, list_skills};

pub struct McpServerState {
    pub port: u16,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ProjectState::new())
        .manage(RestateState::new())
        .manage(AgentState::new())
        .setup(|app| {
            // Resolve app data directory for Restate state
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir");

            let restate_state = app.state::<RestateState>();

            // If Restate is already healthy (e.g. left over from a previous launch), reuse it.
            if is_restate_healthy() {
                eprintln!("✅ Restate already running and healthy — skipping spawn");
            } else {
                match spawn_restate(&data_dir) {
                    Ok(child) => {
                        *restate_state.child.lock().unwrap() = Some(child);

                        // Health-check in a background thread so we don't block the event loop
                        std::thread::spawn(move || {
                            match wait_for_restate_healthy() {
                                Ok(_) => eprintln!("✅ Restate ready"),
                                Err(e) => eprintln!("⚠️  Restate health check failed: {}", e),
                            }
                        });
                    }
                    Err(e) => {
                        // Restate is optional — log but don't crash the app
                        eprintln!("⚠️  Could not start Restate: {}", e);
                    }
                }
            }

            // Spawn POE queue service (Restate virtual object HTTP server on port 9082)
            spawn_queue_service(app.handle().clone());

            // Spawn lifecycle workflow (Restate durable workflow HTTP endpoint on port 9083)
            spawn_lifecycle_service(app.handle().clone());

            // Register lifecycle workflow endpoint with Restate admin API
            // Small delay allows the HTTP server to start before registration
            tauri::async_runtime::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let client = reqwest::Client::new();
                match client
                    .post("http://localhost:9070/deployments")
                    .json(&serde_json::json!({"uri": "http://localhost:9083"}))
                    .send()
                    .await
                {
                    Ok(_) => eprintln!("✅ Lifecycle workflow registered with Restate"),
                    Err(e) => eprintln!("⚠️  Lifecycle workflow registration failed: {}", e),
                }
            });

            // Spawn MCP HTTP/SSE server (port 9084+)
            let mcp_port = spawn_mcp_server(app.handle().clone());
            app.manage(McpServerState { port: mcp_port });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    // Clean up Restate on main window close
                    let app = window.app_handle();
                    let restate_state = app.state::<RestateState>();
                    // guard declared after restate_state → dropped before it (reverse order)
                    let mut guard = restate_state
                        .child
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if let Some(child) = guard.as_mut() {
                        stop_restate(child);
                    }
                    *guard = None;
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            open_project,
            close_project,
            create_node,
            update_node,
            delete_node,
            get_snapshot,
            create_edge,
            delete_edge,
            create_queue_item,
            list_queue_items,
            resolve_queue_item,
            probe_node,
            get_provenance,
            stop_workflow,
            redirect_workflow,
            spawn_agent,
            kill_agent,
            stop_agent_graceful,
            write_to_agent,
            get_lifecycle_status,
            start_lifecycle,
            approve_lifecycle_step,
            submit_rework_items,
            list_skills,
            get_skill_content,
            switch_project,
            list_open_projects,
            load_app_state,
            save_app_state,
            get_artefacts_for_step,
            start_advisor_query,
            send_advisor_message,
            reset_advisor_session,
            get_advisor_state,
            confirm_advisor_session,
            get_stage_metrics,
            promote_skill,
            create_lifecycle_artifact,
            start_conversation_turn,
            continue_conversation_turn,
        ])
        .run(tauri::generate_context!())
        .expect("error while running POE application");
}
