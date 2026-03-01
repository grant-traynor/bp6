/// Architect persona implementation
use crate::agent::persona::{PersonaContext, PersonaPlugin, PersonaType};
use std::fs;
use std::path::Path;

/// Architect persona for high-level system design and epic establishment
pub struct ArchitectPersona;

impl ArchitectPersona {
    pub fn new() -> Self {
        ArchitectPersona
    }

    /// Load project tech stack context from common configuration files
    ///
    /// Searches for and reads:
    /// - Cargo.toml (Rust dependencies)
    /// - package.json (Node/JS dependencies)
    /// - pubspec.yaml (Flutter/Dart dependencies)
    ///
    /// Returns a formatted string with the tech stack information
    pub fn load_tech_stack_context(&self) -> String {
        let mut context = String::from("\n## Tech Stack Context\n\n");
        let mut found_any = false;

        // Try to find project root by looking for common files
        let project_roots = vec![
            std::env::current_dir().ok(),
            std::env::var("CARGO_MANIFEST_DIR").ok().map(|p| Path::new(&p).to_path_buf()),
        ];

        for root_option in project_roots {
            if let Some(root) = root_option {
                // Check for Cargo.toml
                let cargo_path = root.join("Cargo.toml");
                if cargo_path.exists() {
                    if let Ok(content) = fs::read_to_string(&cargo_path) {
                        context.push_str("### Rust (Cargo.toml)\n");
                        context.push_str("```toml\n");
                        context.push_str(&Self::extract_dependencies_section(&content, "dependencies"));
                        context.push_str("```\n\n");
                        found_any = true;
                    }
                }

                // Check for package.json
                let package_path = root.join("package.json");
                if package_path.exists() {
                    if let Ok(content) = fs::read_to_string(&package_path) {
                        context.push_str("### Node.js (package.json)\n");
                        context.push_str("```json\n");
                        context.push_str(&Self::extract_json_section(&content, "dependencies"));
                        context.push_str("```\n\n");
                        found_any = true;
                    }
                }

                // Check for pubspec.yaml
                let pubspec_path = root.join("pubspec.yaml");
                if pubspec_path.exists() {
                    if let Ok(content) = fs::read_to_string(&pubspec_path) {
                        context.push_str("### Flutter/Dart (pubspec.yaml)\n");
                        context.push_str("```yaml\n");
                        context.push_str(&Self::extract_dependencies_section(&content, "dependencies"));
                        context.push_str("```\n\n");
                        found_any = true;
                    }
                }

                if found_any {
                    break;
                }
            }
        }

        if !found_any {
            context.push_str("*No tech stack configuration files found (Cargo.toml, package.json, pubspec.yaml)*\n\n");
        }

        context
    }

    /// Extract dependencies section from TOML or YAML content
    fn extract_dependencies_section(content: &str, section: &str) -> String {
        let mut result = String::new();
        let mut in_section = false;
        let section_header = format!("[{}]", section);

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed == section_header || trimmed.starts_with(&format!("{}:", section)) {
                in_section = true;
                result.push_str(line);
                result.push('\n');
            } else if in_section {
                if trimmed.starts_with('[') || (trimmed.ends_with(':') && !trimmed.starts_with(' ')) {
                    // New section started
                    break;
                }
                result.push_str(line);
                result.push('\n');
            }
        }

        if result.is_empty() {
            format!("# No {} section found\n", section)
        } else {
            result
        }
    }

    /// Extract JSON section (simplified, just shows dependencies)
    fn extract_json_section(content: &str, _section: &str) -> String {
        // For JSON, we'll just include relevant parts
        // A proper implementation would use a JSON parser
        content.to_string()
    }
}

impl PersonaPlugin for ArchitectPersona {
    fn persona_type(&self) -> PersonaType {
        PersonaType::Architect
    }

    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String> {
        let task = context.task.as_deref().unwrap_or("");

        // Select template based on task
        let template_name = match task {
            "establish" | "establish-epic" => "establish-epic",
            "chat" => "chat",
            _ if !task.is_empty() => {
                // For unknown tasks, try to map to known templates
                match task {
                    "establish" => "establish-epic",
                    _ => "chat", // Fallback to chat for interactive mode
                }
            }
            _ => "chat", // Default to chat
        };

        Ok(template_name.to_string())
    }

    fn build_prompt(
        &self,
        template_content: String,
        context: &PersonaContext,
        bead_json: Option<String>,
    ) -> String {
        let mut prompt = String::new();

        // Add tech stack context for architectural decisions
        prompt.push_str(&self.load_tech_stack_context());
        prompt.push_str("\n---\n\n");

        // Add the template content
        prompt.push_str(&template_content);

        // Substitute {{feature_id}} with actual bead_id
        if let Some(bead_id) = &context.bead_id {
            prompt = prompt.replace("{{feature_id}}", bead_id);
        }

        // Append bead JSON context if provided
        if let Some(json) = bead_json {
            prompt.push_str("\n\n## Context JSON\n```json\n");
            prompt.push_str(&json);
            prompt.push_str("\n```\n");
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_establish_epic_template() {
        let persona = ArchitectPersona::new();
        let context = PersonaContext {
            task: Some("establish".to_string()),
            issue_type: Some("epic".to_string()),
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "establish-epic");
    }

    #[test]
    fn test_chat_template() {
        let persona = ArchitectPersona::new();
        let context = PersonaContext {
            task: Some("chat".to_string()),
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "chat");
    }

    #[test]
    fn test_default_fallback_to_chat() {
        let persona = ArchitectPersona::new();
        let context = PersonaContext {
            task: None,
            issue_type: None,
            bead_id: Some("bp6-123".to_string()),
            role: None,
        };

        let template_name = persona.get_template_name(&context).unwrap();
        assert_eq!(template_name, "chat");
    }

    #[test]
    fn test_extract_dependencies_section() {
        let toml_content = r#"
[package]
name = "test"

[dependencies]
serde = "1.0"
tokio = "1.0"

[dev-dependencies]
test = "1.0"
"#;

        let result = ArchitectPersona::extract_dependencies_section(toml_content, "dependencies");
        assert!(result.contains("serde"));
        assert!(result.contains("tokio"));
        assert!(!result.contains("test = \"1.0\""));
    }
}
