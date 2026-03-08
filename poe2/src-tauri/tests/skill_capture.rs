//! Integration tests for the poe:skill event handler.
//!
//! Validates that `write_project_skill` writes the skill file to the correct
//! project-local path and that `load_skill` can subsequently load it via the
//! priority-3 project override chain (without needing a Tauri AppHandle).

use poe2_lib::event_ingester::write_project_skill;
use poe2_lib::skills::load_skill;
use tempfile::TempDir;

/// poe:skill handler writes {project}/.poe/skills/{name}.md and
/// load_skill() reads it back via the project-level priority chain.
#[test]
fn skill_write_then_load_roundtrip() {
    let project_dir = TempDir::new().expect("tempdir");
    let resource_dir = TempDir::new().expect("tempdir");

    let skill_id = "custom-analyst";
    let skill_content = concat!(
        "---\n",
        "id: custom-analyst\n",
        "modes: [autonomous]\n",
        "---\n",
        "# Custom Analyst\n\n",
        "You are a project-specific analyst captured from a prior run.\n",
    );

    // Write the skill via the same helper the handler uses.
    write_project_skill(project_dir.path(), skill_id, skill_content)
        .expect("write_project_skill failed");

    // Confirm the file exists at the expected path.
    let expected_path = project_dir.path().join(".poe").join("skills").join("custom-analyst.md");
    assert!(
        expected_path.exists(),
        "expected skill file at {:?}",
        expected_path
    );

    // load_skill must find and return the project-level override (priority 3).
    let resolved = load_skill(skill_id, project_dir.path(), resource_dir.path())
        .expect("load_skill failed");

    assert_eq!(resolved.skill_id, skill_id);
    assert!(
        resolved.prompt.contains("project-specific analyst"),
        "loaded prompt does not contain expected content: {}",
        resolved.prompt
    );
    assert_eq!(resolved.modes, vec!["autonomous"]);
    // Source path must point to the project-local skills dir.
    assert!(
        resolved.source.contains(".poe/skills"),
        "source should be project-local, got: {}",
        resolved.source
    );
}

/// write_project_skill creates the directory if it does not yet exist.
#[test]
fn skill_write_creates_directory() {
    let project_dir = TempDir::new().expect("tempdir");

    // Directory must not exist yet.
    let skills_dir = project_dir.path().join(".poe").join("skills");
    assert!(!skills_dir.exists());

    write_project_skill(project_dir.path(), "new-skill", "# New Skill\nContent.")
        .expect("write_project_skill failed");

    assert!(skills_dir.exists(), ".poe/skills/ was not created");
    assert!(skills_dir.join("new-skill.md").exists());
}

/// Project-level skill overrides a bundle-level default with the same skill id.
#[test]
fn project_skill_overrides_bundle_default() {
    let project_dir = TempDir::new().expect("tempdir");
    let resource_dir = TempDir::new().expect("tempdir");

    // Write a bundle-level default.
    let bundle_skills = resource_dir.path().join("skills");
    std::fs::create_dir_all(&bundle_skills).unwrap();
    std::fs::write(
        bundle_skills.join("generic-skill.md"),
        "# Generic Skill\nBundle default.",
    )
    .unwrap();

    // Write a project-level override via write_project_skill.
    write_project_skill(
        project_dir.path(),
        "generic-skill",
        "# Generic Skill\nProject override — wins over bundle default.",
    )
    .expect("write_project_skill failed");

    let resolved = load_skill("generic-skill", project_dir.path(), resource_dir.path())
        .expect("load_skill failed");

    assert!(
        resolved.prompt.contains("Project override"),
        "project override should win, got: {}",
        resolved.prompt
    );
}
