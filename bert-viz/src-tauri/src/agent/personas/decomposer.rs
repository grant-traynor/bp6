/// Decomposer persona implementation

use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// Decomposer persona for breaking down features into tasks
pub struct DecomposerPersona;

impl DecomposerPersona {
    pub fn new() -> Self {
        DecomposerPersona
    }
}

impl PersonaPlugin for DecomposerPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::Decomposer
    }

    fn get_template_name(&self, _context: &PersonaContext) -> Result<String, String> {
        Ok("decompose".to_string())
    }
}
