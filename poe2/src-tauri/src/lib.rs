pub mod agent;
pub mod agent_lifecycle;
pub mod dag_store;
pub mod event_ingester;
pub mod event_sink;
pub mod orchestrator;
pub mod skills;

pub use event_sink::{EventSink, TauriEventSink};

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::mpsc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Create the DagChanged channel
            let (dag_tx, dag_rx) = mpsc::unbounded_channel::<event_ingester::DagChanged>();

            // Register shared state
            let registry = dag_store::new_registry();
            let agent_map = agent_lifecycle::new_agent_map();
            let limits = orchestrator::ConcurrencyLimits::new();
            app.manage(registry.clone());
            app.manage(agent_map.clone());
            app.manage(limits.clone());
            app.manage(agent::pty_handover::new_handover_map());
            app.manage(dag_tx.clone());

            // Start orchestrator loop with the receiver
            let app_handle = app.handle().clone();
            let sink: Arc<dyn EventSink> = Arc::new(TauriEventSink(app_handle));
            tauri::async_runtime::spawn(async move {
                orchestrator::start(sink, registry, limits, agent_map, dag_tx, dag_rx).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // dag_store commands
            dag_store::commands::open_project,
            dag_store::commands::close_project,
            dag_store::commands::list_projects,
            dag_store::commands::create_node,
            dag_store::commands::update_node,
            dag_store::commands::cancel_node,
            dag_store::commands::get_node,
            dag_store::commands::list_nodes,
            dag_store::commands::create_edge,
            dag_store::commands::remove_edge,
            dag_store::commands::create_artifact,
            dag_store::commands::list_artifacts,
            dag_store::commands::create_knowledge,
            dag_store::commands::list_knowledge,
            dag_store::commands::list_queue_items,
            dag_store::commands::resolve_queue_item,
            dag_store::commands::list_events,
            dag_store::commands::respond_to_chat,
            dag_store::commands::get_chat_turns,
            // Phase 2 dag_store commands
            dag_store::commands::read_artifact_content,
            dag_store::commands::flag_knowledge_for_promotion,
            dag_store::commands::list_phases,
            // Phase 3 dag_store commands
            dag_store::commands::list_edges,
            dag_store::commands::update_node_sort_order,
            dag_store::commands::create_phase,
            dag_store::commands::activate_phase,
            dag_store::commands::advance_phase,
            dag_store::commands::revise_phase,
            dag_store::commands::rerun_phase,
            dag_store::commands::update_knowledge,
            dag_store::commands::promote_knowledge,
            dag_store::commands::get_node_ancestry,
            dag_store::commands::start_advisor_session,
            dag_store::commands::respond_to_advisor,
            dag_store::commands::get_advisor_turns,
            // orchestrator commands
            orchestrator::commands::advance_stage_gate,
            orchestrator::commands::set_concurrency_limit,
            // New orchestrator commands
            orchestrator::commands::cancel_task,
            orchestrator::commands::pause_stage,
            orchestrator::commands::abort_project,
            orchestrator::commands::revise_stage,
            orchestrator::commands::rerun_stage,
            // agent_lifecycle commands
            agent_lifecycle::commands::write_to_agent,
            agent_lifecycle::commands::interrupt_agent,
            agent_lifecycle::commands::list_agents,
            // agent handover commands
            agent::pty_handover::agent_handover_open,
            agent::pty_handover::agent_handover_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running poe2 application");
}
