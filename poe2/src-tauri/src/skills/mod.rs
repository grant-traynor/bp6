use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The resolved skill prompt ready for injection into an agent's input bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSkill {
    pub skill_id: String,
    /// Full markdown content (YAML frontmatter + body), merged from priority chain.
    pub prompt: String,
    /// Source path where the winning file was loaded from.
    pub source: String,
}

/// Load a skill by ID using the 3-level priority chain:
///   1. App bundle defaults  (resources/skills/<skill-id>.md)
///   2. User-level overrides (~/.poe/skills/<skill-id>.md)
///   3. Project-level overrides ({project}/.poe/skills/<skill-id>.md)
///
/// Highest priority wins. If a higher-priority file exists it completely replaces
/// the lower-priority version (no merging of individual fields).
pub fn load_skill(skill_id: &str, project_path: &Path, resource_dir: &Path) -> Result<ResolvedSkill> {
    let filename = format!("{}.md", skill_id);

    // Build candidates in ascending priority order; last match wins.
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. App bundle defaults
    let bundle_path = resource_dir.join("skills").join(&filename);
    if bundle_path.exists() {
        candidates.push(bundle_path);
    }

    // 2. User-level overrides
    if let Some(home) = dirs::home_dir() {
        let user_path = home.join(".poe").join("skills").join(&filename);
        if user_path.exists() {
            candidates.push(user_path);
        }
    }

    // 3. Project-level overrides
    let project_path_skill = project_path.join(".poe").join("skills").join(&filename);
    if project_path_skill.exists() {
        candidates.push(project_path_skill);
    }

    if candidates.is_empty() {
        return Err(anyhow::anyhow!(
            "Skill '{}' not found in any of: bundle defaults, ~/.poe/skills/, or {{project}}/.poe/skills/",
            skill_id
        ));
    }

    // Use highest-priority (last) candidate
    let winning_path = candidates.last().unwrap();
    let prompt = std::fs::read_to_string(winning_path)
        .with_context(|| format!("Failed to read skill file: {:?}", winning_path))?;

    Ok(ResolvedSkill {
        skill_id: skill_id.to_owned(),
        prompt,
        source: winning_path.to_string_lossy().to_string(),
    })
}

/// Assemble the full input bundle string for an agent.
///
/// Layout:
/// The output format is:
/// - Skill prompt
/// - Task Context (WBS ancestry)
/// - Task title and description
/// - Knowledge Register entries
/// - Relevant Artifacts list
pub fn assemble_input_bundle(
    skill: &ResolvedSkill,
    task_title: &str,
    task_description: Option<&str>,
    wbs_ancestry: &[(&str, &str)], // [(node_type, title)]
    knowledge: &[(&str, &str)],    // [(key, value)]
    artifacts: &[(&str, &str)],    // [(artifact_type, filename)]
) -> String {
    let mut bundle = String::new();

    // Skill prompt
    bundle.push_str(&skill.prompt);
    bundle.push_str("\n\n---\n");

    // WBS ancestry (root → parent → task, reversed so root is first)
    if !wbs_ancestry.is_empty() {
        bundle.push_str("\n# Task Context\n\n");
        for (node_type, title) in wbs_ancestry.iter().rev() {
            bundle.push_str(&format!("**{}**: {}\n", node_type, title));
        }
    }

    // Task
    bundle.push_str("\n# Task\n\n");
    bundle.push_str(&format!("**{}**\n", task_title));
    if let Some(desc) = task_description {
        if !desc.is_empty() {
            bundle.push('\n');
            bundle.push_str(desc);
            bundle.push('\n');
        }
    }

    // Knowledge register
    if !knowledge.is_empty() {
        bundle.push_str("\n# Knowledge Register\n\n");
        for (key, value) in knowledge {
            bundle.push_str(&format!("**{}**: {}\n\n", key, value));
        }
    }

    // Artifacts
    if !artifacts.is_empty() {
        bundle.push_str("\n# Relevant Artifacts\n\n");
        for (artifact_type, filename) in artifacts {
            bundle.push_str(&format!("- `{}` ({})\n", filename, artifact_type));
        }
        bundle.push('\n');
    }

    bundle
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_skill(dir: &Path, filename: &str, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(filename), content).unwrap();
    }

    #[test]
    fn test_load_skill_bundle_only() {
        let tmp = TempDir::new().unwrap();
        let bundle = tmp.path().join("skills");
        write_skill(&bundle, "planner.md", "# Planner\nYou are a planner.");

        let project = TempDir::new().unwrap();
        let skill = load_skill("planner", project.path(), tmp.path()).unwrap();
        assert_eq!(skill.skill_id, "planner");
        assert!(skill.prompt.contains("You are a planner."));
    }

    #[test]
    fn test_project_override_wins() {
        let tmp = TempDir::new().unwrap();
        let bundle = tmp.path().join("skills");
        write_skill(&bundle, "planner.md", "# Planner\nGeneric planner.");

        let project = TempDir::new().unwrap();
        let project_skills = project.path().join(".poe").join("skills");
        write_skill(&project_skills, "planner.md", "# Planner\nProject-specific planner.");

        let skill = load_skill("planner", project.path(), tmp.path()).unwrap();
        assert!(skill.prompt.contains("Project-specific planner."));
    }

    #[test]
    fn test_missing_skill_errors() {
        let tmp = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();
        let result = load_skill("nonexistent", project.path(), tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_assemble_input_bundle() {
        let skill = ResolvedSkill {
            skill_id: "implementer".to_owned(),
            prompt: "You are an implementer.".to_owned(),
            source: "bundle".to_owned(),
        };
        let bundle = assemble_input_bundle(
            &skill,
            "Implement login",
            Some("Add JWT-based login to the API"),
            &[("epic", "Auth"), ("feature", "Login Flow")],
            &[("jwt-secret", "Use HS256 with 24h expiry")],
            &[("design", "auth-design.md")],
        );
        assert!(bundle.contains("You are an implementer."));
        assert!(bundle.contains("Implement login"));
        assert!(bundle.contains("Auth"));
        assert!(bundle.contains("jwt-secret"));
        assert!(bundle.contains("auth-design.md"));
    }
}
