// bp6: Agent skill system
//
// Skills are markdown files with YAML frontmatter defining metadata.
// Load order (highest priority last wins on id collision):
//   1. App resource dir (global defaults, shipped with the app)
//   2. ~/.poe/skills/   (user globals)
//   3. <project>/.poe/skills/ (project-specific overrides)

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::project::ProjectState;

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub applies_to: Vec<String>,
    /// "global" | "user" | "project"
    pub source: String,
    /// Absolute path — used internally to load content
    pub path: String,
}

// ── Frontmatter parser ────────────────────────────────────────────────────────

/// Parse a YAML frontmatter block from a markdown file.
/// Returns (metadata fields as key→value, body_start_byte_offset).
fn parse_frontmatter(content: &str) -> Option<(std::collections::HashMap<String, String>, usize)> {
    let content = content.trim_start_matches('\u{feff}'); // strip BOM
    if !content.starts_with("---") {
        return None;
    }
    // Find closing ---
    let rest = &content[3..];
    let close = rest.find("\n---")?;
    let fm_text = &rest[..close];
    let body_start = 3 + close + 4; // skip opening "---", the fm text, and closing "\n---"

    let mut map = std::collections::HashMap::new();
    for line in fm_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim().to_string();
            let val = line[colon + 1..].trim().to_string();
            map.insert(key, val);
        }
    }
    Some((map, body_start))
}

/// Parse a YAML inline array value like `[a, b, c]` into a Vec<String>.
fn parse_yaml_array(val: &str) -> Vec<String> {
    let val = val.trim();
    if val.starts_with('[') && val.ends_with(']') {
        val[1..val.len() - 1]
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        // Single value
        let v = val.trim_matches('"').trim_matches('\'').to_string();
        if v.is_empty() { vec![] } else { vec![v] }
    }
}

// ── File scanning ─────────────────────────────────────────────────────────────

fn load_skills_from_dir(dir: &Path, source: &str) -> Vec<SkillMeta> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some((fm, _body_start)) = parse_frontmatter(&content) else {
            eprintln!("[skills] No frontmatter in {:?} — skipping", path);
            continue;
        };

        let id = fm.get("id").cloned().unwrap_or_default();
        if id.is_empty() {
            eprintln!("[skills] Missing 'id' in {:?} — skipping", path);
            continue;
        }

        skills.push(SkillMeta {
            id,
            name: fm.get("name").cloned().unwrap_or_default(),
            description: fm.get("description").cloned().unwrap_or_default(),
            tags: fm.get("tags").map(|v| parse_yaml_array(v)).unwrap_or_default(),
            applies_to: fm.get("applies_to").map(|v| parse_yaml_array(v)).unwrap_or_default(),
            source: source.to_string(),
            path: path.to_string_lossy().to_string(),
        });
    }
    skills
}

/// Collect skills from all sources. Higher-priority sources override lower ones by id.
pub fn collect_skills(app: &AppHandle, project_dir: Option<&Path>) -> Vec<SkillMeta> {
    let mut by_id: std::collections::HashMap<String, SkillMeta> = std::collections::HashMap::new();

    let mut add = |skills: Vec<SkillMeta>| {
        for s in skills {
            by_id.insert(s.id.clone(), s);
        }
    };

    // 1. Global defaults (app resource dir)
    let global_dir = resolve_global_skills_dir(app);
    add(load_skills_from_dir(&global_dir, "global"));

    // 2. User globals (~/.poe/skills/)
    if let Some(home) = dirs::home_dir() {
        add(load_skills_from_dir(&home.join(".poe").join("skills"), "user"));
    }

    // 3. Project-specific (<project>/.poe/skills/)
    if let Some(dir) = project_dir {
        add(load_skills_from_dir(&dir.join(".poe").join("skills"), "project"));
    }

    let mut skills: Vec<SkillMeta> = by_id.into_values().collect();
    skills.sort_by(|a, b| a.id.cmp(&b.id));
    skills
}

fn resolve_global_skills_dir(app: &AppHandle) -> PathBuf {
    // Production: resources are bundled next to the binary
    if let Ok(resource_dir) = app.path().resource_dir() {
        let p = resource_dir.join("skills");
        if p.exists() {
            return p;
        }
    }
    // Dev fallback: skills/ next to Cargo.toml
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        return PathBuf::from(manifest_dir).join("skills");
    }
    PathBuf::new()
}

/// Return the body of a skill file (everything after the frontmatter).
pub fn get_skill_body(path: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read skill '{}': {}", path, e))?;
    if let Some((_fm, body_start)) = parse_frontmatter(&content) {
        Ok(content[body_start..].trim_start_matches('\n').to_string())
    } else {
        Ok(content)
    }
}

/// Build the skills context block to prepend to an agent prompt.
/// Returns an empty string if no valid skills are found.
pub fn build_skills_prompt(skill_ids: &[String], app: &AppHandle, project_dir: Option<&Path>) -> String {
    let skills = collect_skills(app, project_dir);
    let mut parts = Vec::new();

    for id in skill_ids {
        if let Some(meta) = skills.iter().find(|s| &s.id == id) {
            match get_skill_body(&meta.path) {
                Ok(body) if !body.is_empty() => {
                    parts.push(format!("[SKILL: {}]\n{}\n[/SKILL]", meta.name, body));
                }
                Ok(_) => {}
                Err(e) => eprintln!("[skills] {}", e),
            }
        } else {
            eprintln!("[skills] Skill '{}' not found — skipping", id);
        }
    }

    parts.join("\n\n")
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_skills(
    app: AppHandle,
    project_state: State<'_, ProjectState>,
) -> Vec<SkillMeta> {
    let project_dir = project_state
        .active_dir_str()
        .map(PathBuf::from);
    collect_skills(&app, project_dir.as_deref())
}

#[tauri::command]
pub fn get_skill_content(id: String, app: AppHandle, project_state: State<'_, ProjectState>) -> Result<String, String> {
    let project_dir = project_state
        .active_dir_str()
        .map(PathBuf::from);
    let skills = collect_skills(&app, project_dir.as_deref());
    let meta = skills
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("Skill '{}' not found", id))?;
    get_skill_body(&meta.path)
}
