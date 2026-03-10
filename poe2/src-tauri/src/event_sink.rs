use serde::Serialize;
use std::path::PathBuf;

/// Abstraction over Tauri's AppHandle::emit for testability.
/// Production: TauriEventSink wraps AppHandle.
/// Test: TestEventSink captures events to a Vec.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: &serde_json::Value);

    /// Returns the resource directory for loading bundled skill files.
    /// Production: Tauri's resource_dir. Test: current directory or temp dir.
    fn resource_dir(&self) -> PathBuf;
}

/// Production implementation — wraps tauri::AppHandle.
pub struct TauriEventSink(pub tauri::AppHandle);

impl EventSink for TauriEventSink {
    fn emit(&self, event: &str, payload: &serde_json::Value) {
        use tauri::Emitter;
        let _ = self.0.emit(event, payload.clone());
    }

    fn resource_dir(&self) -> PathBuf {
        use tauri::Manager;
        self.0
            .path()
            .resource_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Convenience helper — serialize payload and emit via a sink.
pub fn emit_event<P: Serialize>(sink: &dyn EventSink, event: &str, payload: &P) {
    if let Ok(value) = serde_json::to_value(payload) {
        sink.emit(event, &value);
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub mod test_helpers {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    pub struct TestEventSink {
        pub events: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
        pub resource_dir_path: Option<PathBuf>,
    }

    impl EventSink for TestEventSink {
        fn emit(&self, event: &str, payload: &serde_json::Value) {
            self.events
                .lock()
                .unwrap()
                .push((event.to_string(), payload.clone()));
        }

        fn resource_dir(&self) -> PathBuf {
            self.resource_dir_path
                .clone()
                .unwrap_or_else(|| PathBuf::from("."))
        }
    }

    impl TestEventSink {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_resource_dir(path: PathBuf) -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
                resource_dir_path: Some(path),
            }
        }

        pub fn emitted_types(&self) -> Vec<String> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .map(|(e, _)| e.clone())
                .collect()
        }

        pub fn find(&self, event: &str) -> Option<serde_json::Value> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .find(|(e, _)| e == event)
                .map(|(_, v)| v.clone())
        }

        pub fn find_all(&self, event: &str) -> Vec<serde_json::Value> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter(|(e, _)| e == event)
                .map(|(_, v)| v.clone())
                .collect()
        }
    }
}
