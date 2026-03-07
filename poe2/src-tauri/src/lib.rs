pub mod agent_lifecycle;
pub mod dag_store;
pub mod event_ingester;
pub mod orchestrator;
pub mod skills;

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
            app.manage(dag_store::new_registry());
            app.manage(agent_lifecycle::new_agent_map());
            app.manage(orchestrator::ConcurrencyLimits::new());
            app.manage(dag_tx);

            // Start orchestrator loop with the receiver
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                orchestrator::start(app_handle, dag_rx).await;
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
            // orchestrator commands
            orchestrator::commands::advance_stage_gate,
            orchestrator::commands::set_concurrency_limit,
            // agent_lifecycle commands
            agent_lifecycle::commands::write_to_agent,
            agent_lifecycle::commands::interrupt_agent,
            agent_lifecycle::commands::list_agents,
        ])
        .run(tauri::generate_context!())
        .expect("error while running poe2 application");
}
