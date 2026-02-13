
/// Registry for persona plugins
pub struct PersonaRegistry {
    personas: HashMap<PersonaType, Arc<dyn PersonaPlugin>>,
}

impl PersonaRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        PersonaRegistry {
            personas: HashMap::new(),
        }
    }

    /// Create a registry with all built-in personas
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register all built-in persona implementations
    pub fn register_defaults(&self) {
        use crate::agent::personas::{ProductManagerPersona, QaEngineerPersona, SpecialistPersona};

        // SAFETY: We're using interior mutability pattern similar to BackendRegistry
        // This is safe because registration only happens during initialization
        unsafe {
            let personas_ptr = &self.personas as *const HashMap<PersonaType, Arc<dyn PersonaPlugin>>
                as *mut HashMap<PersonaType, Arc<dyn PersonaPlugin>>;
            (*personas_ptr).insert(
                PersonaType::ProductManager,
                Arc::new(ProductManagerPersona::new()),
            );
            (*personas_ptr).insert(PersonaType::QaEngineer, Arc::new(QaEngineerPersona::new()));
            (*personas_ptr).insert(PersonaType::Specialist, Arc::new(SpecialistPersona::new()));
        }
    }

    /// Get a persona plugin by type
    pub fn get(&self, persona_type: PersonaType) -> Option<Arc<dyn PersonaPlugin>> {
        self.personas.get(&persona_type).cloned()
    }
}

impl Default for PersonaRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
