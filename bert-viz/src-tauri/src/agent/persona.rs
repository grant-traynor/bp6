/// Persona plugin system for AI agent templates
///
/// This module defines the trait-based plugin system for different AI personas
/// (specialist, product-manager, qa-engineer, etc.)

use std::collections::HashMap;
use std::sync::Arc;

/// Represents different persona types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PersonaType {
    Specialist,
    ProductManager,
    QaEngineer,
}

impl PersonaType {
    pub fn as_str(&self) -> &str {
        match self {
            PersonaType::Specialist => "specialist",
            PersonaType::ProductManager => "product-manager",
            PersonaType::QaEngineer => "qa-engineer",
        }
    }
}

/// Context information for persona template selection
#[derive(Debug, Clone)]
pub struct PersonaContext {
    pub task: Option<String>,
    pub issue_type: Option<String>,
    pub bead_id: Option<String>,
    pub role: Option<String>,
}

/// Plugin trait for AI personas
///
/// Each persona type implements this trait to provide custom template selection
/// and prompt building logic.
pub trait PersonaPlugin: Send + Sync {
    /// Returns the persona type
    #[allow(dead_code)]
    fn persona_type(&self) -> PersonaType;

    /// Get the template name based on context
    ///
    /// # Arguments
    ///
    /// * `context` - Context information for template selection
    ///
    /// # Returns
    ///
    /// The template file name (without .md extension) to load
    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String>;

    /// Build the complete prompt from template and context
    ///
    /// # Arguments
    ///
    /// * `template_content` - The loaded template content
    /// * `context` - Context information for variable substitution
    /// * `bead_json` - Optional JSON representation of the bead
    ///
    /// # Returns
    ///
    /// The final prompt string ready to send to the agent
    fn build_prompt(
        &self,
        template_content: String,
        context: &PersonaContext,
        bead_json: Option<String>,
    ) -> String {
        let mut prompt = template_content;

        // Substitute {{feature_id}} with actual bead_id
        if let Some(bead_id) = &context.bead_id {
            prompt = prompt.replace("{{feature_id}}", bead_id);
        }

        // Append bead JSON context if provided
        if let Some(json) = bead_json {
            prompt.push_str("\nContext JSON:\n```json\n");
            prompt.push_str(&json);
            prompt.push_str("\n```\n");
        }

        prompt
    }

    /// Get variables for template substitution
    #[allow(dead_code)]
    fn get_variables(&self, context: &PersonaContext) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        if let Some(bead_id) = &context.bead_id {
            vars.insert("feature_id".to_string(), bead_id.clone());
        }
        vars
    }
}

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
