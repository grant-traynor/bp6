/// Orchestrator persona implementation

use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// Orchestrator persona for high-level coordination and delegation
pub struct OrchestratorPersona;

impl OrchestratorPersona {
    pub fn new() -> Self {
        OrchestratorPersona
    }
}

impl PersonaPlugin for OrchestratorPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::Orchestrator
    }

    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String> {
        let task = context.task.as_deref();

        // Select template based on task, default to chat for interactive mode
        let template_name = match task {
            Some("coordinate") => "coordinate",
            Some(_) | None => "chat", // Default to interactive chat mode
        };

        Ok(template_name.to_string())
    }
}
