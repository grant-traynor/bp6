pub mod dag;
pub mod project;
pub mod restate;

use tauri::Manager;

use project::{
    close_project, create_edge, create_node, create_queue_item, delete_edge, delete_node,
    get_snapshot, list_queue_items, open_project, resolve_queue_item, update_node, ProjectState,
};
use restate::{is_restate_healthy, spawn_restate, stop_restate, wait_for_restate_healthy, RestateState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ProjectState::new())
        .manage(RestateState::new())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running POE application");
}
