use std::path::PathBuf;
use std::process::Command;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use crate::Bead;

/// Returns the path to .beads/last-touched — updated by the daemon on every
/// mutation, making it the most reliable trigger for external changes.
pub fn find_last_touched() -> Option<PathBuf> {
    let repo_root = find_repo_root()?;
    let path = repo_root.join(".beads").join("last-touched");
    if path.exists() { Some(path) } else { None }
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

/// Resolve the absolute path to a CLI binary by name.
///
/// Bundled macOS apps inherit a minimal PATH (/usr/bin:/bin:/usr/sbin:/sbin),
/// so `Command::new("foo")` fails even when `foo` is in the user's shell PATH.
/// We check common install locations first, then fall back to asking the user's
/// login shell via `which`.
pub fn resolve_cli_path(name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();

    // Check well-known static paths first — fast and reliable for tools installed
    // via cargo, homebrew, npm globals, etc.
    let candidates = [
        format!("{}/.cargo/bin/{}", home, name),
        format!("/opt/homebrew/bin/{}", name),
        format!("/usr/local/bin/{}", name),
        format!("{}/.local/bin/{}", home, name),
        format!("/usr/bin/{}", name),
        format!("{}/.npm-global/bin/{}", home, name),
        format!("/usr/local/lib/node_modules/.bin/{}", name),
    ];
    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            eprintln!("🔧 resolve_cli_path({}): static → {}", name, p.display());
            return Some(p);
        }
    }

    // Fall back to the login shell. Take only the last non-empty line of stdout
    // because .zshrc/.zprofile often print banners/welcome messages before the path.
    let which_cmd = format!("which {}", name);
    for shell in &["/bin/zsh", "/bin/bash"] {
        if let Ok(output) = Command::new(shell)
            .args(["-l", "-c", &which_cmd])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let path_str = stdout
                .lines()
                .filter(|l| !l.trim().is_empty())
                .last()
                .map(|l| l.trim())
                .unwrap_or("")
                .to_string();
            if !path_str.is_empty() {
                let p = PathBuf::from(&path_str);
                if p.exists() {
                    eprintln!("🔧 resolve_cli_path({}): shell which → {}", name, p.display());
                    return Some(p);
                }
            }
        }
    }

    // Last resort: nvm default alias, following alias chains.
    let nvm_dir = format!("{}/.nvm", home);
    let nvm_alias_path = format!("{}/alias/default", nvm_dir);
    if let Ok(raw) = std::fs::read_to_string(&nvm_alias_path) {
        let mut alias = raw.trim().to_string();
        for _ in 0..3 {
            if alias.starts_with('v') {
                let nvm_bin = format!("{}/versions/node/{}/bin/{}", nvm_dir, alias, name);
                let p = PathBuf::from(&nvm_bin);
                if p.exists() {
                    eprintln!("🔧 resolve_cli_path({}): nvm → {}", name, p.display());
                    return Some(p);
                }
                break;
            }
            let next_path = format!("{}/alias/{}", nvm_dir, alias);
            match std::fs::read_to_string(&next_path) {
                Ok(next) => alias = next.trim().to_string(),
                Err(_) => break,
            }
        }
    }

    eprintln!("⚠️  resolve_cli_path({}): not found", name);
    None
}

/// Returns a `Command` pointed at the `bd` binary, or an error if it cannot be found.
fn bd_command() -> Result<Command, String> {
    match resolve_cli_path("bd") {
        Some(path) => Ok(Command::new(path)),
        None => Err("The 'bd' CLI is not found. Please ensure it is installed (e.g. cargo install beads) and available in your PATH.".to_string()),
    }
}

pub fn check_bd_available() -> Result<(), String> {
    bd_command()?.arg("--version").output()
        .map_err(|e| format!("Failed to run 'bd --version': {}", e))?;
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
                            // bd export ≥0.62 includes memories ({_type, key, value}) and
                            // other non-issue records that have no `id` field. Skip them
                            // silently rather than treating them as parse errors.
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                                if v.get("id").is_none() || v.get("_type").is_some() {
                                    continue;
                                }
                            }
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
        if let Ok(mut cmd) = bd_command() {
            let _ = cmd
                .arg("export")
                .arg("-o")
                .arg(&dump_path)
                .current_dir(repo_path)
                .output();
        }
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn update_bead(updatedBead: Bead, app_handle: AppHandle) -> Result<(), String> {
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;

    let mut cmd = bd_command()?;
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

    let mut cmd = bd_command()?;
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

    let mut cmd = bd_command()?;
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

    let mut cmd = bd_command()?;
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

    let mut cmd = bd_command()?;
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

    let mut update_cmd = bd_command()?;
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
pub fn sync_project(project_path: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    check_bd_available()?;
    let path = std::path::Path::new(&project_path);
    let output = bd_command()?
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
    let _ = app_handle.emit("beads-updated", ());
    Ok(stdout)
}
