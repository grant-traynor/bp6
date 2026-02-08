use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use notify::{Watcher, RecursiveMode, Config};
use tauri::{Emitter, AppHandle};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dependency {
    pub issue_id: String,
    pub depends_on_id: String,
    pub r#type: String,
    #[serde(flatten)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Bead {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: u32,
    pub issue_type: String,
    pub estimate: Option<u32>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    // Metadata fields to preserve
    pub owner: Option<String>,
    pub created_at: Option<String>,
    pub created_by: Option<String>,
    pub updated_at: Option<String>,
    pub labels: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_acceptance_criteria")]
    pub acceptance_criteria: Option<Vec<String>>,
    pub closed_at: Option<String>,
    pub close_reason: Option<String>,
    pub is_favorite: Option<bool>,
    #[serde(flatten)]
    pub extra_metadata: serde_json::Map<String, serde_json::Value>,
}

fn deserialize_acceptance_criteria<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::String(s)) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()))
            }
        }
        Some(serde_json::Value::Array(arr)) => {
            let mut result = Vec::new();
            for val in arr {
                if let serde_json::Value::String(s) = val {
                    result.push(s);
                }
            }
            Ok(Some(result))
        }
        _ => Ok(None),
    }
}

fn get_sync_branch_name(repo_path: &std::path::Path) -> Option<String> {
    // Try to read sync.branch from bd config
    let output = std::process::Command::new("bd")
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

fn find_beads_file() -> Option<PathBuf> {
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
                println!("Using sync-branch worktree: {}", worktree_path.display());
                return Some(worktree_path);
            }
        }

        // Fall back to working tree (local mode)
        let test_path = curr.join(".beads/issues.jsonl");
        if test_path.exists() {
            println!("Using local working tree: {}", test_path.display());
            return Some(test_path);
        }

        if !curr.pop() {
            break;
        }
    }
    None
}

#[tauri::command]
fn get_beads() -> Result<Vec<Bead>, String> {
    let path = find_beads_file().ok_or_else(|| "Could not locate .beads/issues.jsonl in any parent directory".to_string())?;

    // Retry opening and reading the file to handle transient locks and partial writes
    let mut last_error = String::new();
    for i in 0..5 {
        match File::open(&path) {
            Ok(file) => {
                let metadata = file.metadata().map_err(|e| e.to_string())?;

                // If file size is 0, it might be mid-write, retry
                if metadata.len() == 0 && i < 4 {
                    std::thread::sleep(Duration::from_millis(100 * (i + 1)));
                    continue;
                }

                // If file size is 0 on last retry, return empty list (legitimate empty state)
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
                            // IO error reading line - file might be corrupted or mid-write
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
                            // Parse error - might be partial write, retry
                            if i < 4 {
                                had_parse_error = true;
                                last_error = format!("Failed to parse bead at line {}: {}", index + 1, e);
                                break;
                            } else {
                                return Err(format!("Failed to parse bead at line {}: {}. Line content: {}", index + 1, e, line));
                            }
                        }
                    }
                }

                if !had_parse_error {
                    return Ok(beads);
                }

                // Had parse error, retry after delay
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

#[tauri::command]
fn update_bead(updated_bead: Bead, app_handle: AppHandle) -> Result<(), String> {
    let path = find_beads_file().ok_or_else(|| "Could not locate .beads/issues.jsonl".to_string())?;
    
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut found = false;

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() { continue; }
        
        let mut bead: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if bead["id"] == updated_bead.id {
            bead = serde_json::to_value(&updated_bead).map_err(|e| e.to_string())?;
            found = true;
        }
        lines.push(serde_json::to_string(&bead).map_err(|e| e.to_string())?);
    }

    if !found {
        return Err(format!("Bead with id {} not found", updated_bead.id));
    }

    // Atomic write using a temporary file
    let tmp_path = path.with_extension("jsonl.tmp");
    {
        let mut file = File::create(&tmp_path).map_err(|e| e.to_string())?;
        for line in lines {
            writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        }
        file.sync_all().map_err(|e| e.to_string())?;
    }
    std::fs::rename(tmp_path, path).map_err(|e| e.to_string())?;

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
fn create_bead(new_bead: Bead, app_handle: AppHandle) -> Result<(), String> {
    let path = find_beads_file().ok_or_else(|| "Could not locate .beads/issues.jsonl".to_string())?;
    
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;

    let line = serde_json::to_string(&new_bead).map_err(|e| e.to_string())?;
    writeln!(file, "{}", line).map_err(|e| e.to_string())?;

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Project {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub is_favorite: bool,
    pub last_opened: Option<String>,
}

fn get_projects_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "Could not locate home directory (neither HOME nor USERPROFILE is set)".to_string())?;
    
    let dir = PathBuf::from(home).join(".bert-viz");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create directory {}: {}", dir.display(), e))?;
    }
    Ok(dir.join("projects.json"))
}

#[tauri::command]
fn get_projects() -> Result<Vec<Project>, String> {
    let path = get_projects_path()?;
    if !path.exists() { return Ok(Vec::new()); }
    
    let file = File::open(&path).map_err(|e| format!("Failed to open projects.json: {}", e))?;
    let reader = BufReader::new(file);
    
    // Attempt to parse projects, default to empty if file is empty or invalid
    let projects: Vec<Project> = serde_json::from_reader(reader)
        .unwrap_or_else(|_| Vec::new());
        
    Ok(projects)
}

#[tauri::command]
fn save_projects(projects: Vec<Project>) -> Result<(), String> {
    let path = get_projects_path()?;
    let file = File::create(path).map_err(|e| format!("Failed to create projects.json: {}", e))?;
    serde_json::to_writer_pretty(file, &projects).map_err(|e| format!("Failed to write projects: {}", e))?;
    Ok(())
}

#[tauri::command]
fn add_project(project: Project, app_handle: AppHandle) -> Result<(), String> {
    let mut projects = get_projects()?;
    if let Some(existing) = projects.iter_mut().find(|p| p.path == project.path) {
        existing.name = project.name;
    } else {
        projects.push(project);
    }
    save_projects(projects)?;
    let _ = app_handle.emit("projects-updated", ());
    Ok(())
}

#[tauri::command]
fn remove_project(path: String, app_handle: AppHandle) -> Result<(), String> {
    let projects = get_projects()?;
    let filtered: Vec<Project> = projects.into_iter().filter(|p| p.path != path).collect();
    save_projects(filtered)?;
    let _ = app_handle.emit("projects-updated", ());
    Ok(())
}

#[tauri::command]
fn open_project(path: String, app_handle: AppHandle) -> Result<(), String> {
    std::env::set_current_dir(&path).map_err(|e| format!("Failed to change directory to {}: {}", path, e))?;
    
    // Update last_opened
    let mut projects = get_projects()?;
    if let Some(project) = projects.iter_mut().find(|p| p.path == path) {
        project.last_opened = Some(chrono::Utc::now().to_rfc3339());
    } else {
        let parts = path.split(|c| c == '/' || c == '\\').collect::<Vec<_>>();
        let name = parts.last().unwrap_or(&"Project").to_string();
        projects.push(Project {
            name,
            path: path.clone(),
            is_favorite: false,
            last_opened: Some(chrono::Utc::now().to_rfc3339()),
        });
    }
    save_projects(projects)?;
    let _ = app_handle.emit("projects-updated", ());
    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
fn toggle_favorite(path: String, app_handle: AppHandle) -> Result<(), String> {
    let mut projects = get_projects()?;
    if let Some(project) = projects.iter_mut().find(|p| p.path == path) {
        project.is_favorite = !project.is_favorite;
    }
    save_projects(projects)?;
    let _ = app_handle.emit("projects-updated", ());
    Ok(())
}

#[tauri::command]
fn get_current_dir() -> Result<String, String> {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_beads, update_bead, create_bead, 
            get_projects, add_project, remove_project, open_project, toggle_favorite,
            get_current_dir
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let proj_handle = app.handle().clone();

            // Watch beads file with debouncing
            if let Some(path) = find_beads_file() {
                let parent_path = path.parent().unwrap().to_path_buf();
                let last_emit = Arc::new(Mutex::new(Instant::now()));

                let mut watcher = notify::RecommendedWatcher::new(move |res| {
                    match res {
                        Ok(_) => {
                            // Debounce: only emit if at least 200ms have passed since last emit
                            let mut last = last_emit.lock().unwrap();
                            let now = Instant::now();
                            if now.duration_since(*last) >= Duration::from_millis(200) {
                                *last = now;
                                let _ = handle.emit("beads-updated", ());
                            }
                        },
                        Err(e) => eprintln!("Beads file watch error: {:?}", e),
                    }
                }, Config::default()).unwrap();

                // Watch the parent directory (either .beads or beads-sync/.beads)
                if let Err(e) = watcher.watch(&parent_path, RecursiveMode::Recursive) {
                    eprintln!("Failed to watch beads directory {}: {:?}", parent_path.display(), e);
                } else {
                    println!("Watching beads file at: {}", path.display());
                }
                std::mem::forget(watcher);
            }

            // Watch projects file with debouncing
            if let Ok(proj_path) = get_projects_path() {
                let proj_last_emit = Arc::new(Mutex::new(Instant::now()));

                let mut proj_watcher = notify::RecommendedWatcher::new(move |res| {
                    match res {
                        Ok(_) => {
                            let mut last = proj_last_emit.lock().unwrap();
                            let now = Instant::now();
                            if now.duration_since(*last) >= Duration::from_millis(200) {
                                *last = now;
                                let _ = proj_handle.emit("projects-updated", ());
                            }
                        },
                        Err(e) => println!("watch error: {:?}", e),
                    }
                }, Config::default()).unwrap();
                let parent = proj_path.parent().unwrap();
                // Ensure parent exists (already done in get_projects_path but double check)
                if !parent.exists() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if parent.exists() {
                    let _ = proj_watcher.watch(parent, RecursiveMode::Recursive);
                    std::mem::forget(proj_watcher);
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
