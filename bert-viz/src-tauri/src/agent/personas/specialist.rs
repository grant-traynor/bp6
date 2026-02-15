/// Specialist persona implementation
use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// Specialist persona for domain-specific implementations
/// (web, flutter, rust, supabase-db, supabase-edge, etc.)
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
        // Use role from context to determine template, fallback to chat
        let template_name = match context.role.as_deref() {
            Some("web") => "web",
            Some("flutter") => "flutter",
            Some("supabase-db") => "supabase-db",
            Some("supabase-edge") => "supabase-edge",
            Some("rust") | Some("rust-tauri") => "rust-tauri",
            Some(role) => return Err(format!("Unknown specialist role: {}", role)),
            None => "chat", // Fallback to interactive chat mode
        };

        Ok(template_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specialist_web_template() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("web".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "web");
    }

    #[test]
    fn test_specialist_flutter_template() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("flutter".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "flutter");
    }

    #[test]
    fn test_specialist_rust_tauri_template() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("rust-tauri".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "rust-tauri");
    }

    #[test]
    fn test_specialist_rust_template_maps_to_rust_tauri() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: Some("rust".to_string()),
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "rust-tauri");
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
        assert_eq!(template_name, "supabase-edge");
    }

    #[test]
    fn test_specialist_missing_role_fallback_to_chat() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "chat");
    }
}
