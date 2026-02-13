/// Specialist persona implementation

use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// Specialist persona for domain-specific implementations
/// (web, flutter, rust, supabase-db, etc.)
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
        // Use role from context to determine template
        let role = context
            .role
            .as_ref()
            .ok_or_else(|| "Specialist persona requires a role".to_string())?;

        // Map role to template file name
        let template_name = match role.as_str() {
            "web" => "web",
            "flutter" => "flutter",
            "supabase-db" => "supabase-db",
            "rust" | "rust-tauri" => "rust-tauri",
            _ => return Err(format!("Unknown specialist role: {}", role)),
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
    fn test_specialist_missing_role() {
        let persona = SpecialistPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        assert!(persona.get_template_name(&context).is_err());
    }
}
