/// Specialist persona implementation
use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// Specialist persona for domain-specific implementations
/// (flutter, tauri, supabase-db, supabase-edge)
pub struct SpecialistPersona;

impl SpecialistPersona {
    pub fn new() -> Self {
        SpecialistPersona
    }
}

impl PersonaPlugin for SpecialistPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::Specialist
    }

    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String> {
        let role = context.role.as_deref().unwrap_or("default");

        // Determine task: default to "chat" if not specified
        let task = context.task.as_deref().unwrap_or("chat");

        // Valid roles
        let valid_roles = ["flutter", "tauri", "supabase-db", "supabase-edge"];
        if !valid_roles.contains(&role) {
            return Err(format!("Unknown specialist role: {}", role));
        }

        // Valid tasks
        let valid_tasks = ["chat", "implement", "review"];
        if !valid_tasks.contains(&task) {
            return Err(format!("Unknown task for specialist: {}", task));
        }

        // Template path: role/task e.g., "tauri/implement"
        let template_name = format!("{}/{}", role, task);

        Ok(template_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specialist_flutter_chat() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("flutter".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "flutter/chat");
    }

    #[test]
    fn test_specialist_flutter_implement() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: Some("implement".to_string()),
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("flutter".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "flutter/implement");
    }

    #[test]
    fn test_specialist_tauri_chat() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("tauri".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "tauri/chat");
    }

    #[test]
    fn test_specialist_tauri_implement() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: Some("implement".to_string()),
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("tauri".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "tauri/implement");
    }

    #[test]
    fn test_specialist_supabase_edge_template() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("supabase-edge".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "supabase-edge/chat");
    }

    #[test]
    fn test_specialist_supabase_db_implement() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: Some("implement".to_string()),
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("supabase-db".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "supabase-db/implement");
    }

    #[test]
    fn test_specialist_missing_role_error() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let result = persona.get_template_name(&context);
        assert!(result.is_err());
    }

    #[test]
    fn test_specialist_unknown_role_error() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("unknown".to_string()),
        };

        let result = persona.get_template_name(&context);
        assert!(result.is_err());
    }

    #[test]
    fn test_specialist_unknown_task_error() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: Some("unknown_task".to_string()),
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("web".to_string()),
        };

        let result = persona.get_template_name(&context);
        assert!(result.is_err());
    }
}
