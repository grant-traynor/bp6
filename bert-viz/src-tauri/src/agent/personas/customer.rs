/// Customer persona implementation for scope definition and end-user perspective
use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// Customer persona for exploratory conversations about project scope and requirements
/// from an end-user/stakeholder perspective.
///
/// Unlike other personas, Customer:
/// - Is project-scoped (no beadId required)
/// - Always uses 'chat' task (no decompose/implement variants)
/// - Has dual modes: Bootstrap (no epics) vs Refinement (with epics)
pub struct CustomerPersona;

impl CustomerPersona {
    pub fn new() -> Self {
        CustomerPersona
    }
}

impl PersonaPlugin for CustomerPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::Customer
    }

    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String> {
        // Customer persona only supports 'chat' task
        // Template selection depends on whether epics exist in the project

        // For now, we'll use a simple approach:
        // - If task is explicitly 'bootstrap' or 'refinement', use that
        // - Otherwise, default to 'chat' (the base template)
        // The epic detection logic will be added in task bp6-rpc4.2

        let task = context.task.as_deref().unwrap_or("chat");

        let template_name = match task {
            "bootstrap" => "bootstrap",
            "refinement" => "refinement",
            "chat" | _ => "chat",
        };

        Ok(template_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_type() {
        let persona = CustomerPersona::new();
        assert_eq!(persona.persona_type(), PersonaType::Customer);
    }

    #[test]
    fn test_chat_template() {
        let persona = CustomerPersona::new();
        let context = PersonaContext {
            task: Some("chat".to_string()),
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "chat");
    }

    #[test]
    fn test_bootstrap_template() {
        let persona = CustomerPersona::new();
        let context = PersonaContext {
            task: Some("bootstrap".to_string()),
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "bootstrap");
    }

    #[test]
    fn test_refinement_template() {
        let persona = CustomerPersona::new();
        let context = PersonaContext {
            task: Some("refinement".to_string()),
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "refinement");
    }

    #[test]
    fn test_default_template() {
        let persona = CustomerPersona::new();
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
