/// Backend plugin registry for dynamic backend lookup and management
use crate::agent::backends::{ClaudeCodeBackend, GeminiBackend};
use crate::agent::plugin::{BackendId, CliBackendPlugin};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Thread-safe registry for CLI backend plugins
///
/// The registry maintains a collection of backend implementations and provides
/// type-safe lookup by BackendId. It uses RwLock for efficient concurrent access
/// (many readers, rare writers).
///
/// # Example
///
/// ```ignore
/// let registry = BackendRegistry::new();
/// registry.register_defaults();
///
/// let gemini = registry.get(BackendId::Gemini).unwrap();
/// let args = gemini.build_args("Hello", false);
/// ```
pub struct BackendRegistry {
    backends: RwLock<HashMap<BackendId, Arc<dyn CliBackendPlugin>>>,
}

impl BackendRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        BackendRegistry {
            backends: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new registry with all built-in backends registered
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register all built-in backend implementations
    ///
    /// Currently registers:
    /// - Gemini (Google Gemini CLI)
    /// - ClaudeCode (Anthropic Claude Code CLI)
    pub fn register_defaults(&self) {
        self.register(BackendId::Gemini, Arc::new(GeminiBackend::new()));
        self.register(BackendId::ClaudeCode, Arc::new(ClaudeCodeBackend::new()));
    }

    /// Register a backend plugin
    ///
    /// # Arguments
    ///
    /// * `id` - The BackendId to associate with this plugin
    /// * `backend` - The backend implementation (wrapped in Arc for sharing)
    ///
    /// # Example
    ///
    /// ```ignore
    /// registry.register(BackendId::Gemini, Arc::new(GeminiBackend::new()));
    /// ```
    pub fn register(&self, id: BackendId, backend: Arc<dyn CliBackendPlugin>) {
        let mut backends = self.backends.write().unwrap();
        backends.insert(id, backend);
    }

    /// Retrieve a backend plugin by ID
    ///
    /// # Arguments
    ///
    /// * `id` - The BackendId to look up
    ///
    /// # Returns
    ///
    /// * `Some(Arc<dyn CliBackendPlugin>)` if the backend is registered
    /// * `None` if no backend is registered for this ID
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(backend) = registry.get(BackendId::Gemini) {
    ///     let cmd = backend.command_name();
    /// }
    /// ```
    pub fn get(&self, id: BackendId) -> Option<Arc<dyn CliBackendPlugin>> {
        let backends = self.backends.read().unwrap();
        backends.get(&id).cloned()
    }

    /// Get a list of all registered backend IDs
    ///
    /// Useful for UI enumeration or debugging.
    #[allow(dead_code)]
    pub fn list_backends(&self) -> Vec<BackendId> {
        let backends = self.backends.read().unwrap();
        backends.keys().copied().collect()
    }

    /// Check if a specific backend is registered
    #[allow(dead_code)]
    pub fn has_backend(&self, id: BackendId) -> bool {
        let backends = self.backends.read().unwrap();
        backends.contains_key(&id)
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_is_empty() {
        let registry = BackendRegistry::new();
        assert_eq!(registry.list_backends().len(), 0);
    }

    #[test]
    fn test_with_defaults_registers_backends() {
        let registry = BackendRegistry::with_defaults();
        let backends = registry.list_backends();

        assert_eq!(backends.len(), 2);
        assert!(backends.contains(&BackendId::Gemini));
        assert!(backends.contains(&BackendId::ClaudeCode));
    }

    #[test]
    fn test_register_and_get() {
        let registry = BackendRegistry::new();
        registry.register(BackendId::Gemini, Arc::new(GeminiBackend::new()));

        let backend = registry.get(BackendId::Gemini).unwrap();
        assert_eq!(backend.command_name(), "gemini");
    }

    #[test]
    fn test_get_nonexistent() {
        let registry = BackendRegistry::new();
        assert!(registry.get(BackendId::Gemini).is_none());
    }

    #[test]
    fn test_has_backend() {
        let registry = BackendRegistry::new();
        assert!(!registry.has_backend(BackendId::Gemini));

        registry.register(BackendId::Gemini, Arc::new(GeminiBackend::new()));
        assert!(registry.has_backend(BackendId::Gemini));
    }

    #[test]
    fn test_list_backends() {
        let registry = BackendRegistry::new();
        registry.register(BackendId::Gemini, Arc::new(GeminiBackend::new()));
        registry.register(BackendId::ClaudeCode, Arc::new(ClaudeCodeBackend::new()));

        let backends = registry.list_backends();
        assert_eq!(backends.len(), 2);
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let registry = Arc::new(BackendRegistry::with_defaults());
        let mut handles = vec![];

        // Spawn multiple readers
        for _ in 0..10 {
            let reg = Arc::clone(&registry);
            let handle = thread::spawn(move || {
                let backend = reg.get(BackendId::Gemini).unwrap();
                assert_eq!(backend.command_name(), "gemini");
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
