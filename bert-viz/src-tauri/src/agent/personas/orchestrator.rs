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

    fn get_template_name(&self, _context: &PersonaContext) -> Result<String, String> {
        // For now, default to coordinate
        Ok("coordinate".to_string())
    }
}
