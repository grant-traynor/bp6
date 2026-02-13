/// QA Engineer persona implementation

use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// QA Engineer persona for testing, validation, and dependency management
pub struct QaEngineerPersona;

impl QaEngineerPersona {
    pub fn new() -> Self {
        QaEngineerPersona
    }
}

impl PersonaPlugin for QaEngineerPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::QaEngineer
    }

    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String> {
        let task = context.task.as_deref().unwrap_or("");

        // QA Engineer currently only has fix_dependencies template
        let template_name = match task {
            "fix_dependencies" => "fix-dependencies",
            _ => "fix-dependencies", // Default for qa-engineer
        };

        Ok(template_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_dependencies() {
        let persona = QaEngineerPersona::new();
        let context = PersonaContext {
            task: Some("fix_dependencies".to_string()),
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "fix-dependencies");
    }

    #[test]
    fn test_default_template() {
        let persona = QaEngineerPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "fix-dependencies");
    }
}
