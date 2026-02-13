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
        let task = context.task.as_deref();

        // Select template based on task, default to chat for interactive mode
        let template_name = match task {
            Some("fix_dependencies") => "fix-dependencies",
            Some(_) | None => "chat", // Default to interactive chat mode
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
    fn test_default_template_is_chat() {
        let persona = QaEngineerPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "chat");
    }
}
