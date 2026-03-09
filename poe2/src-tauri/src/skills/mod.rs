use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Execution mode for an agent spawn — determines the protocol block injected
/// at the top of the input bundle.
#[derive(Debug, Clone, PartialEq)]
pub enum SpawnMode {
    Autonomous,
    Interactive,
}

/// The resolved skill prompt ready for injection into an agent's input bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSkill {
    pub skill_id: String,
    /// Full markdown content (YAML frontmatter + body), merged from priority chain.
    pub prompt: String,
    /// Source path where the winning file was loaded from.
    pub source: String,
    /// Modes declared in skill YAML frontmatter. Defaults to `["autonomous"]` if absent.
    pub modes: Vec<String>,
    /// Model override from skill YAML frontmatter (`model:` key). When `Some`, the
    /// orchestrator passes `--model <value>` to the claude spawn. When `None`, the
    /// claude binary uses its configured default.
    pub model: Option<String>,
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

    // 1a. Dev-mode fallback: look next to the source tree (CARGO_MANIFEST_DIR = src-tauri/).
    //     In production the bundle resources take over; this is a no-op in release builds.
    #[cfg(debug_assertions)]
    {
        let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("skills")
            .join(&filename);
        if dev_path.exists() {
            candidates.push(dev_path);
        }
    }

    // 1b. App bundle defaults (production: resources/skills/<skill-id>.md)
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

    let modes = parse_modes_from_frontmatter(&prompt);
    let model = parse_model_from_frontmatter(&prompt);

    Ok(ResolvedSkill {
        skill_id: skill_id.to_owned(),
        prompt,
        source: winning_path.to_string_lossy().to_string(),
        modes,
        model,
    })
}

/// Parse the `modes:` field from YAML frontmatter at the top of a skill file.
/// Returns `["autonomous"]` if the file has no frontmatter or no modes field.
fn parse_modes_from_frontmatter(content: &str) -> Vec<String> {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return vec!["autonomous".to_string()];
    }
    // Find end of frontmatter block.
    let after_start = if content.starts_with("---\r\n") { 5 } else { 4 };
    let end_marker = if content[after_start..].contains("\n---\n") {
        "\n---\n"
    } else if content[after_start..].contains("\n---\r\n") {
        "\n---\r\n"
    } else {
        return vec!["autonomous".to_string()];
    };
    let end_idx = content[after_start..].find(end_marker).unwrap_or(0);
    let frontmatter = &content[after_start..after_start + end_idx];

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("modes:") {
            let rest = rest.trim();
            // Handle: modes: [autonomous, interactive]
            if rest.starts_with('[') && rest.ends_with(']') {
                let inner = &rest[1..rest.len() - 1];
                return inner
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    vec!["autonomous".to_string()]
}

/// Parse the `model:` field from YAML frontmatter at the top of a skill file.
/// Returns `None` if the file has no frontmatter or no model field.
fn parse_model_from_frontmatter(content: &str) -> Option<String> {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return None;
    }
    let after_start = if content.starts_with("---\r\n") { 5 } else { 4 };
    let end_marker = if content[after_start..].contains("\n---\n") {
        "\n---\n"
    } else if content[after_start..].contains("\n---\r\n") {
        "\n---\r\n"
    } else {
        return None;
    };
    let end_idx = content[after_start..].find(end_marker).unwrap_or(0);
    let frontmatter = &content[after_start..after_start + end_idx];

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("model:") {
            let value = rest.trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// Check that a skill supports the requested spawn mode.
/// Returns `Err` if the skill's declared modes do not include the requested mode.
pub fn check_mode_compatibility(skill: &ResolvedSkill, mode: &SpawnMode) -> Result<()> {
    let mode_str = match mode {
        SpawnMode::Autonomous => "autonomous",
        SpawnMode::Interactive => "interactive",
    };
    if !skill.modes.contains(&mode_str.to_string()) {
        return Err(anyhow::anyhow!(
            "Skill '{}' does not support {} mode (supports: {}). \
             Add 'modes: [{}]' to the skill's YAML frontmatter to enable it.",
            skill.skill_id,
            mode_str,
            skill.modes.join(", "),
            mode_str,
        ));
    }
    Ok(())
}

/// Assemble the full input bundle string for an agent.
///
/// Layout:
/// The output format is:
/// - Mode protocol block (Protocol.md §3)
/// - Skill prompt
/// - Task Context (WBS ancestry)
/// - Task title and description
/// - Knowledge Register entries
/// - Relevant Artifacts list
pub fn assemble_input_bundle(
    mode: &SpawnMode,
    skill: &ResolvedSkill,
    task_title: &str,
    task_description: Option<&str>,
    wbs_ancestry: &[(&str, &str)], // [(node_type, title)]
    knowledge: &[(&str, &str)],    // [(key, value)]
    artifacts: &[(&str, &str)],    // [(artifact_type, filename)]
) -> String {
    let mut bundle = String::new();

    // Mode protocol block (Protocol.md §3) — prepended before skill content.
    let mode_block = match mode {
        SpawnMode::Autonomous => concat!(
            "## Execution Protocol\n\n",
            "You are running in autonomous mode — there is no human at the keyboard.\n\n",
            "- Begin by emitting `poe:brief` describing your understanding of the task.\n",
            "- Work from the context in this bundle. Do not ask questions.\n",
            "- Raise genuine blockers via `poe:decision`, then continue with your best judgement.\n",
            "- Emit `poe:done` as your final output. The process exits after this.\n",
        ),
        SpawnMode::Interactive => concat!(
            "## Interactive Mode\n\n",
            "You are running in interactive mode — a human is present at the keyboard.\n\n",
            "- Use `poe:chat` + `poe:yield` to ask questions and wait for the human's answer.\n",
            "- The orchestrator will resume your run with the answer as `Human: {response}`.\n",
            "- Follow the Interactive Mode instructions in your skill prompt exactly.\n",
        ),
    };
    bundle.push_str(mode_block);
    bundle.push_str("\n---\n\n");

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
        assert_eq!(skill.modes, vec!["autonomous".to_string()]);
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
        assert_eq!(skill.modes, vec!["autonomous".to_string()]);
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
            modes: vec!["autonomous".to_string()],
            model: None,
        };
        let bundle = assemble_input_bundle(
            &SpawnMode::Autonomous,
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

    #[test]
    fn assemble_bundle_autonomous_mode_block() {
        let skill = ResolvedSkill {
            skill_id: "implementer".to_owned(),
            prompt: "You are an implementer.".to_owned(),
            source: "bundle".to_owned(),
            modes: vec!["autonomous".to_string()],
            model: None,
        };
        let bundle = assemble_input_bundle(
            &SpawnMode::Autonomous,
            &skill,
            "Build feature",
            None,
            &[],
            &[],
            &[],
        );
        assert!(bundle.contains("autonomous mode"));
        assert!(bundle.contains("poe:brief"));
        assert!(bundle.contains("poe:done"));
        assert!(bundle.contains("You are an implementer."));
        // Mode block must come before skill prompt
        let mode_pos = bundle.find("autonomous mode").unwrap();
        let skill_pos = bundle.find("You are an implementer.").unwrap();
        assert!(mode_pos < skill_pos, "mode block must precede skill prompt");
    }

    #[test]
    fn assemble_bundle_interactive_mode_block() {
        let skill = ResolvedSkill {
            skill_id: "analyst".to_owned(),
            prompt: "You are an analyst.".to_owned(),
            source: "bundle".to_owned(),
            modes: vec!["autonomous".to_string(), "interactive".to_string()],
            model: None,
        };
        let bundle = assemble_input_bundle(
            &SpawnMode::Interactive,
            &skill,
            "Discuss requirements",
            None,
            &[],
            &[],
            &[],
        );
        assert!(bundle.contains("## Interactive Mode"));
        assert!(bundle.contains("interactive mode"));
        // The interactive block must not contain the autonomous mandate.
        assert!(
            !bundle.contains("Emit `poe:done` as your final output"),
            "interactive block must not contain the autonomous poe:done mandate"
        );
    }

    #[test]
    fn mode_guard_autonomous_ok() {
        let skill = ResolvedSkill {
            skill_id: "pm".to_owned(),
            prompt: "".to_owned(),
            source: "".to_owned(),
            modes: vec!["autonomous".to_string()],
            model: None,
        };
        assert!(check_mode_compatibility(&skill, &SpawnMode::Autonomous).is_ok());
    }

    #[test]
    fn mode_guard_blocks_interactive_when_not_declared() {
        let skill = ResolvedSkill {
            skill_id: "pm".to_owned(),
            prompt: "".to_owned(),
            source: "".to_owned(),
            modes: vec!["autonomous".to_string()],
            model: None,
        };
        assert!(check_mode_compatibility(&skill, &SpawnMode::Interactive).is_err());
        let err = check_mode_compatibility(&skill, &SpawnMode::Interactive).unwrap_err();
        assert!(err.to_string().contains("does not support interactive mode"));
    }

    #[test]
    fn parse_modes_from_frontmatter_works() {
        let content = "---\nid: test\nmodes: [autonomous, interactive]\n---\n# Skill\nContent.";
        let skill_dir = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        let bundle_dir = skill_dir.path().join("skills");
        write_skill(&bundle_dir, "test.md", content);
        let skill = load_skill("test", proj_dir.path(), skill_dir.path()).unwrap();
        assert_eq!(skill.modes, vec!["autonomous", "interactive"]);
    }

    #[test]
    fn parse_modes_defaults_to_autonomous() {
        let content = "# Skill\nNo frontmatter here.";
        let skill_dir = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        let bundle_dir = skill_dir.path().join("skills");
        write_skill(&bundle_dir, "nofront.md", content);
        let skill = load_skill("nofront", proj_dir.path(), skill_dir.path()).unwrap();
        assert_eq!(skill.modes, vec!["autonomous"]);
    }

    #[test]
    fn parse_model_from_frontmatter_works() {
        let content = "---\nid: analyst\nmodes: [autonomous]\nmodel: claude-opus-4-6\n---\n# Skill\nContent.";
        let skill_dir = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        let bundle_dir = skill_dir.path().join("skills");
        write_skill(&bundle_dir, "analyst.md", content);
        let skill = load_skill("analyst", proj_dir.path(), skill_dir.path()).unwrap();
        assert_eq!(skill.model, Some("claude-opus-4-6".to_string()));
    }

    #[test]
    fn parse_model_defaults_to_none_when_absent() {
        let content = "---\nid: planner\nmodes: [autonomous]\n---\n# Skill\nContent.";
        let skill_dir = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        let bundle_dir = skill_dir.path().join("skills");
        write_skill(&bundle_dir, "planner.md", content);
        let skill = load_skill("planner", proj_dir.path(), skill_dir.path()).unwrap();
        assert_eq!(skill.model, None);
    }

    #[test]
    fn parse_model_defaults_to_none_when_no_frontmatter() {
        let content = "# Skill\nNo frontmatter here.";
        let skill_dir = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        let bundle_dir = skill_dir.path().join("skills");
        write_skill(&bundle_dir, "nofm.md", content);
        let skill = load_skill("nofm", proj_dir.path(), skill_dir.path()).unwrap();
        assert_eq!(skill.model, None);
    }
}
