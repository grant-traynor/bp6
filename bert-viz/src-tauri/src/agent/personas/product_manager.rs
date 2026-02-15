/// Product Manager persona implementation
use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// Product Manager persona for decomposition, extension, and implementation
pub struct ProductManagerPersona;

impl ProductManagerPersona {
    pub fn new() -> Self {
        ProductManagerPersona
    }
}

impl PersonaPlugin for ProductManagerPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::ProductManager
    }

    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String> {
        let task = context.task.as_deref().unwrap_or("");
        let issue_type = context.issue_type.as_deref().unwrap_or("");

        // Select template based on task and issue type
        let template_name = match (task, issue_type) {
            // Decompose templates
            ("decompose", "epic") => "decompose-epic",
            ("decompose", _) => "decompose-feature",

            // Extend templates
            ("extend", "epic") => "extend-epic",
            ("extend", _) => "extend-feature",

            // Implement templates
            ("implement", "feature") => "implement-feature",
            ("implement", _) => "implement-task",

            // Chat template
            ("chat", _) => "chat",

            // Default fallback
            _ if !task.is_empty() => {
                // Try to map unknown tasks to known templates
                match task {
                    "decompose" => "decompose-feature",
                    "extend" => "extend-feature",
                    "implement" => "implement-task",
                    "chat" => "chat",
                    _ => "system-prompt", // Ultimate fallback
                }
            }
            _ => "system-prompt",
        };

        Ok(template_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompose_epic() {
        let persona = ProductManagerPersona::new();
        let context = PersonaContext {
            task: Some("decompose".to_string()),
            issue_type: Some("epic".to_string()),
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "decompose-epic");
    }

    #[test]
    fn test_decompose_feature() {
        let persona = ProductManagerPersona::new();
        let context = PersonaContext {
            task: Some("decompose".to_string()),
            issue_type: Some("feature".to_string()),
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "decompose-feature");
    }

    #[test]
    fn test_implement_feature() {
        let persona = ProductManagerPersona::new();
        let context = PersonaContext {
            task: Some("implement".to_string()),
            issue_type: Some("feature".to_string()),
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "implement-feature");
    }

    #[test]
    fn test_implement_task() {
        let persona = ProductManagerPersona::new();
        let context = PersonaContext {
            task: Some("implement".to_string()),
            issue_type: Some("task".to_string()),
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "implement-task");
    }

    #[test]
    fn test_chat() {
        let persona = ProductManagerPersona::new();
        let context = PersonaContext {
            task: Some("chat".to_string()),
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "chat");
    }
}
