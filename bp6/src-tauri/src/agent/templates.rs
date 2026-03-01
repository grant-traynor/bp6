/// Template loading and variable substitution for persona prompts
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Template loader for persona prompt templates
///
/// Loads markdown templates from the filesystem and performs variable substitution.
/// Templates are located in the `templates/personas/` directory organized by persona type.
pub struct TemplateLoader {
    template_root: PathBuf,
}

impl TemplateLoader {
    /// Create a new template loader with the default template directory
    ///
    /// Searches for `templates/personas/` by walking up from the binary location.
    pub fn new() -> Result<Self, String> {
        // Get the directory containing the binary
        let exe_path =
            std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

        // Walk up from the executable to find the templates directory
        let mut current_dir = exe_path.parent();
        while let Some(dir) = current_dir {
            // Check for templates/personas directly in this directory
            let template_path = dir.join("templates/personas");
            if template_path.is_dir() {
                return Ok(TemplateLoader {
                    template_root: template_path,
                });
            }

            // Also check for bert-viz/templates/personas (for the case where we're one level above bert-viz)
            let nested_path = dir.join("bert-viz/templates/personas");
            if nested_path.is_dir() {
                return Ok(TemplateLoader {
                    template_root: nested_path,
                });
            }

            current_dir = dir.parent();
        }

        Err("Failed to determine template root path: Could not find 'templates/personas' or 'bert-viz/templates/personas' in any parent directory of the executable".to_string())
    }

    /// Create a template loader with a custom template directory
    #[allow(dead_code)]
    pub fn with_root<P: AsRef<Path>>(root: P) -> Self {
        TemplateLoader {
            template_root: root.as_ref().to_path_buf(),
        }
    }

    /// Load a template by persona and template name
    ///
    /// # Arguments
    ///
    /// * `persona` - The persona type (e.g., "product-manager", "specialist")
    /// * `template_name` - The template file name without extension (e.g., "decompose-feature")
    ///
    /// # Returns
    ///
    /// The raw template content as a String
    ///
    /// # Example
    ///
    /// ```ignore
    /// let loader = TemplateLoader::new()?;
    /// let template = loader.load_template("product-manager", "decompose-feature")?;
    /// ```
    pub fn load_template(&self, persona: &str, template_name: &str) -> Result<String, String> {
        let template_path = self
            .template_root
            .join(persona)
            .join(format!("{}.md", template_name));

        fs::read_to_string(&template_path).map_err(|e| {
            format!(
                "Failed to load template '{}' for persona '{}': {}. Path: {}",
                template_name,
                persona,
                e,
                template_path.display()
            )
        })
    }

    /// Load a persona prompt using the two-file architecture
    ///
    /// This method loads a persona identity file (`persona.md`) and a task-specific file,
    /// then concatenates them. For backward compatibility, if `persona.md` doesn't exist,
    /// it falls back to loading only the task-specific file.
    ///
    /// # Arguments
    ///
    /// * `persona` - The persona type (e.g., "product-manager", "specialist")
    /// * `template_name` - The template file name without extension (e.g., "flutter/chat")
    ///
    /// # Returns
    ///
    /// The combined prompt content as a String
    ///
    /// # Example
    ///
    /// ```ignore
    /// let loader = TemplateLoader::new()?;
    /// // Loads specialist/flutter/persona.md + specialist/flutter/chat.md
    /// let prompt = loader.load_persona_prompt("specialist", "flutter/chat")?;
    /// ```
    pub fn load_persona_prompt(&self, persona: &str, template_name: &str) -> Result<String, String> {
        // Extract the directory from template_name (e.g., "flutter" from "flutter/chat")
        let persona_dir = if let Some(slash_pos) = template_name.find('/') {
            &template_name[..slash_pos]
        } else {
            // No slash means it's a simple template name, use persona root
            ""
        };

        // Construct path to persona.md
        let persona_md_path = if !persona_dir.is_empty() {
            self.template_root
                .join(persona)
                .join(persona_dir)
                .join("persona.md")
        } else {
            self.template_root.join(persona).join("persona.md")
        };

        // Try to load persona.md
        let persona_content = fs::read_to_string(&persona_md_path).ok();

        // Load the task-specific template
        let task_template = self.load_template(persona, template_name)?;

        // Combine if persona.md exists, otherwise return just the task template
        if let Some(persona_md) = persona_content {
            Ok(format!("{}\n\n---\n\n{}", persona_md, task_template))
        } else {
            // Fallback to single file for backward compatibility
            Ok(task_template)
        }
    }

    /// Load a template and substitute variables
    #[allow(dead_code)]
    ///
    /// # Arguments
    ///
    /// * `persona` - The persona type
    /// * `template_name` - The template file name without extension
    /// * `variables` - HashMap of variable names to values (e.g., "feature_id" -> "bp6-123")
    ///
    /// # Returns
    ///
    /// The template with all `{{variable}}` placeholders replaced
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut vars = HashMap::new();
    /// vars.insert("feature_id".to_string(), "bp6-123".to_string());
    /// let prompt = loader.load_with_vars("product-manager", "decompose-feature", &vars)?;
    /// ```
    pub fn load_with_vars(
        &self,
        persona: &str,
        template_name: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String, String> {
        let mut template = self.load_template(persona, template_name)?;

        // Substitute all {{variable}} placeholders
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            template = template.replace(&placeholder, value);
        }

        Ok(template)
    }

    /// List all available templates for a persona
    #[allow(dead_code)]
    pub fn list_templates(&self, persona: &str) -> Result<Vec<String>, String> {
        let persona_dir = self.template_root.join(persona);

        let entries = fs::read_dir(&persona_dir).map_err(|e| {
            format!(
                "Failed to read persona directory '{}': {}",
                persona_dir.display(),
                e
            )
        })?;

        let mut templates = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        templates.push(stem.to_string());
                    }
                }
            }
        }

        templates.sort();
        Ok(templates)
    }
}

impl Default for TemplateLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create default TemplateLoader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_templates() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let personas_dir = temp_dir.path().join("personas");

        // Create product-manager templates
        let pm_dir = personas_dir.join("product-manager");
        fs::create_dir_all(&pm_dir).unwrap();
        fs::write(
            pm_dir.join("test-template.md"),
            "# Test Template\n\nFeature: {{feature_id}}",
        )
        .unwrap();

        // Create specialist templates
        let spec_dir = personas_dir.join("specialist");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(spec_dir.join("web.md"), "# Web Specialist").unwrap();

        temp_dir
    }

    #[test]
    fn test_load_template() {
        let temp_dir = create_test_templates();
        let loader = TemplateLoader::with_root(temp_dir.path().join("personas"));

        let template = loader
            .load_template("product-manager", "test-template")
            .unwrap();
        assert!(template.contains("Test Template"));
        assert!(template.contains("{{feature_id}}"));
    }

    #[test]
    fn test_load_with_vars() {
        let temp_dir = create_test_templates();
        let loader = TemplateLoader::with_root(temp_dir.path().join("personas"));

        let mut vars = HashMap::new();
        vars.insert("feature_id".to_string(), "bp6-123".to_string());

        let result = loader
            .load_with_vars("product-manager", "test-template", &vars)
            .unwrap();

        assert!(result.contains("Feature: bp6-123"));
        assert!(!result.contains("{{feature_id}}"));
    }

    #[test]
    fn test_load_nonexistent_template() {
        let temp_dir = create_test_templates();
        let loader = TemplateLoader::with_root(temp_dir.path().join("personas"));

        let result = loader.load_template("product-manager", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_templates() {
        let temp_dir = create_test_templates();
        let loader = TemplateLoader::with_root(temp_dir.path().join("personas"));

        let templates = loader.list_templates("product-manager").unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0], "test-template");
    }

    #[test]
    fn test_load_persona_prompt_with_persona_md() {
        let temp_dir = TempDir::new().unwrap();
        let personas_dir = temp_dir.path().join("personas");

        // Create specialist/flutter directory with persona.md and chat.md
        let flutter_dir = personas_dir.join("specialist").join("flutter");
        fs::create_dir_all(&flutter_dir).unwrap();
        fs::write(
            flutter_dir.join("persona.md"),
            "# Flutter Specialist Identity\n\nYou are a Flutter expert.",
        )
        .unwrap();
        fs::write(flutter_dir.join("chat.md"), "# Chat Task\n\nHelp the user.")
            .unwrap();

        let loader = TemplateLoader::with_root(personas_dir);

        let result = loader
            .load_persona_prompt("specialist", "flutter/chat")
            .unwrap();

        // Should contain both files with separator
        assert!(result.contains("Flutter Specialist Identity"));
        assert!(result.contains("---"));
        assert!(result.contains("Chat Task"));
    }

    #[test]
    fn test_load_persona_prompt_without_persona_md() {
        let temp_dir = TempDir::new().unwrap();
        let personas_dir = temp_dir.path().join("personas");

        // Create specialist/flutter directory with only chat.md (no persona.md)
        let flutter_dir = personas_dir.join("specialist").join("flutter");
        fs::create_dir_all(&flutter_dir).unwrap();
        fs::write(flutter_dir.join("chat.md"), "# Chat Task\n\nHelp the user.")
            .unwrap();

        let loader = TemplateLoader::with_root(personas_dir);

        let result = loader
            .load_persona_prompt("specialist", "flutter/chat")
            .unwrap();

        // Should only contain chat.md content (fallback behavior)
        assert!(result.contains("Chat Task"));
        assert!(!result.contains("---"));
    }

    #[test]
    fn test_load_persona_prompt_simple_template() {
        let temp_dir = TempDir::new().unwrap();
        let personas_dir = temp_dir.path().join("personas");

        // Create product-manager directory with persona.md and simple template
        let pm_dir = personas_dir.join("product-manager");
        fs::create_dir_all(&pm_dir).unwrap();
        fs::write(
            pm_dir.join("persona.md"),
            "# Product Manager Identity\n\nYou manage products.",
        )
        .unwrap();
        fs::write(
            pm_dir.join("decompose-epic.md"),
            "# Decompose Epic\n\nBreak down the epic.",
        )
        .unwrap();

        let loader = TemplateLoader::with_root(personas_dir);

        let result = loader
            .load_persona_prompt("product-manager", "decompose-epic")
            .unwrap();

        // Should contain both files with separator
        assert!(result.contains("Product Manager Identity"));
        assert!(result.contains("---"));
        assert!(result.contains("Decompose Epic"));
    }

    #[test]
    fn test_load_persona_prompt_nonexistent_task() {
        let temp_dir = create_test_templates();
        let loader = TemplateLoader::with_root(temp_dir.path().join("personas"));

        let result = loader.load_persona_prompt("product-manager", "nonexistent");
        assert!(result.is_err());
    }
}
