use std::path::PathBuf;
use std::process::Command;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use crate::Bead;

pub fn get_sync_branch_name(repo_path: &std::path::Path) -> Option<String> {
    // Try to read sync.branch from bd config
    let output = Command::new("bd")
        .arg("config")
        .arg("get")
        .arg("sync.branch")
        .current_dir(repo_path)
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }
    None
}

pub fn find_beads_db() -> Option<PathBuf> {
    let mut curr = std::env::current_dir().ok()?;
    loop {
        // Check local .beads/beads.db (primary store — always up to date)
        let test_path = curr.join(".beads/beads.db");
        if test_path.exists() {
            return Some(test_path);
        }

        // Sync-branch worktree may have its own DB
        if let Some(sync_branch) = get_sync_branch_name(&curr) {
            let worktree_path = curr
                .join(".git")
                .join("beads-worktrees")
                .join(&sync_branch)
                .join(".beads")
                .join("beads.db");

            if worktree_path.exists() {
                return Some(worktree_path);
            }
        }

        if !curr.pop() {
            break;
        }
    }
    None
}

pub fn find_repo_root() -> Option<PathBuf> {
    let curr_dir = std::env::current_dir().ok()?;
    let mut curr = curr_dir.clone();
    loop {
        let beads_path = curr.join(".beads");
        if beads_path.exists() {
            return Some(curr);
        }
        if !curr.pop() {
            break;
        }
    }
    None
}

pub fn check_bd_available() -> Result<(), String> {
    if Command::new("bd").arg("--version").output().is_err() {
        return Err("The 'bd' CLI is not found in the PATH. Please ensure it is installed and available.".to_string());
    }
    Ok(())
}

/// Returns (creating if necessary) the path to our own issue dump:
/// <repo_root>/.bp6/issue_dump.jsonl
/// This file is written exclusively by `bd export -o <path>` and is
/// never touched by the beads daemon — we own it.
pub fn find_dump_file() -> Option<PathBuf> {
    let repo_root = find_repo_root()?;
    let bp6_dir = repo_root.join(".bp6");
    std::fs::create_dir_all(&bp6_dir).ok()?;
    Some(bp6_dir.join("issue_dump.jsonl"))
}

#[tauri::command]
pub fn get_beads() -> Result<Vec<Bead>, String> {
    let path = find_dump_file()
        .ok_or_else(|| "Could not locate or create .bp6/issue_dump.jsonl".to_string())?;

    // Retry opening and reading the file to handle transient locks and partial writes
    let mut last_error = String::new();
    for i in 0..5 {
        match File::open(&path) {
            Ok(file) => {
                let metadata = file.metadata().map_err(|e| e.to_string())?;

                if metadata.len() == 0 && i < 4 {
                    std::thread::sleep(Duration::from_millis(100 * (i + 1)));
                    continue;
                }

                if metadata.len() == 0 {
                    return Ok(Vec::new());
                }

                let reader = BufReader::new(file);
                let mut beads = Vec::new();
                let mut had_parse_error = false;

                for (index, line) in reader.lines().enumerate() {
                    let line = match line {
                        Ok(l) => l,
                        Err(e) => {
                            if i < 4 {
                                had_parse_error = true;
                                last_error = format!("IO error reading line {}: {}", index + 1, e);
                                break;
                            } else {
                                return Err(format!("Error reading line {}: {}", index + 1, e));
                            }
                        }
                    };

                    if line.trim().is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<Bead>(&line) {
                        Ok(bead) => beads.push(bead),
                        Err(e) => {
                            if i < 4 {
                                had_parse_error = true;
                                last_error = format!("Failed to parse bead at line {}: {}", index + 1, e);
                                break;
                            } else {
                                return Err(format!("Failed to parse bead at line {}: {}", index + 1, e));
                            }
                        }
                    }
                }

                if !had_parse_error {
                    return Ok(beads);
                }

                std::thread::sleep(Duration::from_millis(100 * (i + 1)));
            }
            Err(e) => {
                if i == 4 {
                    return Err(format!("Failed to open issues.jsonl after retries: {}", e));
                }
                std::thread::sleep(Duration::from_millis(100 * (i + 1)));
            }
        }
    }

    Err(format!("Failed to read beads after retries. Last error: {}", last_error))
}

pub fn get_bead_by_id(id: &str) -> Result<Bead, String> {
    let beads = get_beads()?;
    beads.into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| format!("Bead with ID {} not found", id))
}

/// Force an immediate export from the beads daemon into our own dump file,
/// bypassing the daemon's 5s flush debounce. Call this after every mutation
/// so the frontend reads fresh data when it re-fetches on beads-updated.
fn flush_jsonl(repo_path: &std::path::Path) {
    if let Some(dump_path) = find_dump_file() {
        let _ = Command::new("bd")
            .arg("export")
            .arg("-o")
            .arg(&dump_path)
            .current_dir(repo_path)
            .output();
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn update_bead(updatedBead: Bead, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("update")
        .arg(&updatedBead.id)
        .arg("--title").arg(&updatedBead.title)
        .arg("--status").arg(&updatedBead.status)
        .arg("--priority").arg(updatedBead.priority.to_string())
        .arg("--type").arg(&updatedBead.issue_type);

    if let Some(desc) = &updatedBead.description {
        cmd.arg("--description").arg(desc);
    }
    if let Some(est) = updatedBead.estimate {
        cmd.arg("--estimate").arg(est.to_string());
    }
    if let Some(owner) = &updatedBead.owner {
        cmd.arg("--assignee").arg(owner);
    }
    if let Some(labels) = &updatedBead.labels {
        if !labels.is_empty() {
            cmd.arg("--set-labels").arg(labels.join(","));
        }
    }
    if let Some(ac) = &updatedBead.acceptance_criteria {
        if !ac.is_empty() {
            cmd.arg("--acceptance").arg(ac.join("\n"));
        }
    }
    if let Some(parent) = &updatedBead.parent {
        cmd.arg("--parent").arg(parent);
    }
    if let Some(ext_ref) = &updatedBead.external_reference {
        cmd.arg("--external-ref").arg(ext_ref);
    }
    if let Some(design) = &updatedBead.design {
        cmd.arg("--design").arg(design);
    }
    if let Some(notes) = &updatedBead.notes {
        cmd.arg("--notes").arg(notes);
    }

    let metadata_json = serde_json::to_string(&updatedBead).map_err(|e| e.to_string())?;
    cmd.arg("--metadata").arg(metadata_json);

    let output = cmd.current_dir(&repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    flush_jsonl(&repo_path);
    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn close_bead(beadId: String, reason: Option<String>, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("close").arg(&beadId);

    if let Some(r) = reason {
        cmd.arg("--reason").arg(r);
    }

    let output = cmd.current_dir(&repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    flush_jsonl(&repo_path);
    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn reopen_bead(beadId: String, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("reopen").arg(&beadId);

    let output = cmd.current_dir(&repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    flush_jsonl(&repo_path);
    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn claim_bead(beadId: String, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("update")
        .arg(&beadId)
        .arg("--status")
        .arg("in_progress");

    let output = cmd.current_dir(&repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    flush_jsonl(&repo_path);
    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn create_bead(newBead: Bead, app_handle: AppHandle) -> Result<String, String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("create")
        .arg(&newBead.title)
        .arg("--priority").arg(newBead.priority.to_string())
        .arg("--type").arg(&newBead.issue_type)
        .arg("--silent");

    if let Some(desc) = &newBead.description {
        cmd.arg("--description").arg(desc);
    }
    if let Some(est) = newBead.estimate {
        cmd.arg("--estimate").arg(est.to_string());
    }
    if let Some(owner) = &newBead.owner {
        cmd.arg("--assignee").arg(owner);
    }
    if let Some(labels) = &newBead.labels {
        if !labels.is_empty() {
            cmd.arg("--labels").arg(labels.join(","));
        }
    }
    if let Some(ac) = &newBead.acceptance_criteria {
        if !ac.is_empty() {
            cmd.arg("--acceptance").arg(ac.join("\n"));
        }
    }
    if let Some(parent) = &newBead.parent {
        cmd.arg("--parent").arg(parent);
    }
    if let Some(ext_ref) = &newBead.external_reference {
        cmd.arg("--external-ref").arg(ext_ref);
    }
    if let Some(design) = &newBead.design {
        cmd.arg("--design").arg(design);
    }
    if let Some(notes) = &newBead.notes {
        cmd.arg("--notes").arg(notes);
    }

    let output = cmd.current_dir(&repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("CLI Create Error: {}", stderr));
    }

    let new_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if new_id.is_empty() {
        return Err("Create command succeeded but returned no ID".to_string());
    }

    let mut update_cmd = Command::new("bd");
    update_cmd.arg("update")
        .arg(&new_id)
        .arg("--status").arg(&newBead.status);

    let metadata_json = serde_json::to_string(&newBead).map_err(|e| e.to_string())?;
    update_cmd.arg("--metadata").arg(metadata_json);

    let update_output = update_cmd.current_dir(&repo_path).output().map_err(|e| e.to_string())?;

    if !update_output.status.success() {
        let stderr = String::from_utf8_lossy(&update_output.stderr);
        return Err(format!(
            "Bead created as {} but initial update failed: {}",
            new_id,
            stderr
        ));
    }

    flush_jsonl(&repo_path);
    let _ = app_handle.emit("beads-updated", ());
    Ok(new_id)
}

#[tauri::command]
pub fn sync_project(project_path: String) -> Result<String, String> {
    check_bd_available()?;
    let path = std::path::Path::new(&project_path);
    let output = Command::new("bd")
        .arg("sync")
        .current_dir(path)
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }

    flush_jsonl(path);
    Ok(stdout)
}

pub fn execute_bd(args: Vec<String>) -> Result<String, String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let output = Command::new("bd")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
