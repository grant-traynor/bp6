/// QC Engineer persona implementation
use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};

/// QC Engineer persona for quality metrics collection and reporting
pub struct QcEngineerPersona;

impl QcEngineerPersona {
    pub fn new() -> Self {
        QcEngineerPersona
    }
}

impl PersonaPlugin for QcEngineerPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::QcEngineer
    }

    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String> {
        let task = context.task.as_deref();

        // Select template based on task, default to chat for interactive mode
        let template_name = match task {
            Some("collect-metrics") | Some("collect_metrics") => "collect-metrics",
            Some("generate-report") | Some("generate_report") => "generate-report",
            Some(_) | None => "chat", // Default to interactive chat mode
        };

        Ok(template_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_metrics() {
        let persona = QcEngineerPersona::new();
        let context = PersonaContext {
            task: Some("collect-metrics".to_string()),
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "collect-metrics");
    }

    #[test]
    fn test_generate_report() {
        let persona = QcEngineerPersona::new();
        let context = PersonaContext {
            task: Some("generate-report".to_string()),
            issue_type: None,
            bead_id: None,
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "generate-report");
    }

    #[test]
    fn test_default_template_is_chat() {
        let persona = QcEngineerPersona::new();
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
