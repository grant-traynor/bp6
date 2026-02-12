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

pub fn find_beads_file() -> Option<PathBuf> {
    let mut curr = std::env::current_dir().ok()?;
    loop {
        // First check if there's a sync-branch worktree (remote/sync mode)
        if let Some(sync_branch) = get_sync_branch_name(&curr) {
            let worktree_path = curr
                .join(".git")
                .join("beads-worktrees")
                .join(&sync_branch)
                .join(".beads")
                .join("issues.jsonl");

            if worktree_path.exists() {
                return Some(worktree_path);
            }
        }

        // Fall back to working tree (local mode)
        let test_path = curr.join(".beads/issues.jsonl");
        if test_path.exists() {
            return Some(test_path);
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

#[tauri::command]
pub fn get_beads() -> Result<Vec<Bead>, String> {
    let path = find_beads_file().ok_or_else(|| "Could not locate .beads/issues.jsonl in any parent directory".to_string())?;

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

#[tauri::command]
pub fn update_bead(updated_bead: Bead, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("update")
        .arg(&updated_bead.id)
        .arg("--title").arg(&updated_bead.title)
        .arg("--status").arg(&updated_bead.status)
        .arg("--priority").arg(updated_bead.priority.to_string())
        .arg("--type").arg(&updated_bead.issue_type);

    if let Some(desc) = &updated_bead.description {
        cmd.arg("--description").arg(desc);
    }
    if let Some(est) = updated_bead.estimate {
        cmd.arg("--estimate").arg(est.to_string());
    }
    if let Some(owner) = &updated_bead.owner {
        cmd.arg("--assignee").arg(owner);
    }
    if let Some(labels) = &updated_bead.labels {
        if !labels.is_empty() {
            cmd.arg("--set-labels").arg(labels.join(","));
        }
    }
    if let Some(ac) = &updated_bead.acceptance_criteria {
        if !ac.is_empty() {
            cmd.arg("--acceptance").arg(ac.join("\n"));
        }
    }
    if let Some(parent) = &updated_bead.parent {
        cmd.arg("--parent").arg(parent);
    }
    if let Some(ext_ref) = &updated_bead.external_reference {
        cmd.arg("--external-ref").arg(ext_ref);
    }
    if let Some(design) = &updated_bead.design {
        cmd.arg("--design").arg(design);
    }
    if let Some(notes) = &updated_bead.notes {
        cmd.arg("--notes").arg(notes);
    }

    let metadata_json = serde_json::to_string(&updated_bead).map_err(|e| e.to_string())?;
    cmd.arg("--metadata").arg(metadata_json);

    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
pub fn close_bead(bead_id: String, reason: Option<String>, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("close").arg(&bead_id);

    if let Some(r) = reason {
        cmd.arg("--reason").arg(r);
    }

    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
pub fn reopen_bead(bead_id: String, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("reopen").arg(&bead_id);

    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
pub fn claim_bead(bead_id: String, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("update")
        .arg(&bead_id)
        .arg("--status")
        .arg("in_progress");

    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
pub fn create_bead(new_bead: Bead, app_handle: AppHandle) -> Result<String, String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = Command::new("bd");
    cmd.arg("create")
        .arg(&new_bead.title)
        .arg("--priority").arg(new_bead.priority.to_string())
        .arg("--type").arg(&new_bead.issue_type)
        .arg("--silent");

    if let Some(desc) = &new_bead.description {
        cmd.arg("--description").arg(desc);
    }
    if let Some(est) = new_bead.estimate {
        cmd.arg("--estimate").arg(est.to_string());
    }
    if let Some(owner) = &new_bead.owner {
        cmd.arg("--assignee").arg(owner);
    }
    if let Some(labels) = &new_bead.labels {
        if !labels.is_empty() {
            cmd.arg("--labels").arg(labels.join(","));
        }
    }
    if let Some(ac) = &new_bead.acceptance_criteria {
        if !ac.is_empty() {
            cmd.arg("--acceptance").arg(ac.join("\n"));
        }
    }
    if let Some(parent) = &new_bead.parent {
        cmd.arg("--parent").arg(parent);
    }
    if let Some(ext_ref) = &new_bead.external_reference {
        cmd.arg("--external-ref").arg(ext_ref);
    }
    if let Some(design) = &new_bead.design {
        cmd.arg("--design").arg(design);
    }
    if let Some(notes) = &new_bead.notes {
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
        .arg("--status").arg(&new_bead.status);

    let metadata_json = serde_json::to_string(&new_bead).map_err(|e| e.to_string())?;
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

    let _ = app_handle.emit("beads-updated", ());
    Ok(new_id)
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
