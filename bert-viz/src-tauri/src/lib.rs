use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
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

fn find_beads_file() -> Option<PathBuf> {
    let mut curr = std::env::current_dir().ok()?;
    loop {
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

#[tauri::command]
fn get_beads() -> Result<Vec<Bead>, String> {
    let path = find_beads_file().ok_or_else(|| "Could not locate .beads/issues.jsonl in any parent directory".to_string())?;
    
    let file = File::open(path).map_err(|e| format!("Failed to open issues.jsonl: {}", e))?;
    let reader = BufReader::new(file);
    let mut beads = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Error reading line: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }
        let bead: Bead = serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse bead: {} in line: {}", e, line))?;
        beads.push(bead);
    }

    Ok(beads)
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

    let mut file = File::create(path).map_err(|e| e.to_string())?;
    for line in lines {
        writeln!(file, "{}", line).map_err(|e| e.to_string())?;
    }

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
            
            // Watch beads file
            if let Some(path) = find_beads_file() {
                let parent_path = path.parent().unwrap().to_path_buf();
                let mut watcher = notify::RecommendedWatcher::new(move |res| {
                    match res {
                        Ok(_) => { let _ = handle.emit("beads-updated", ()); },
                        Err(e) => println!("watch error: {:?}", e),
                    }
                }, Config::default()).unwrap();
                watcher.watch(&parent_path, RecursiveMode::Recursive).unwrap();
                std::mem::forget(watcher);
            }

            // Watch projects file
            if let Ok(proj_path) = get_projects_path() {
                let mut proj_watcher = notify::RecommendedWatcher::new(move |res| {
                    match res {
                        Ok(_) => { let _ = proj_handle.emit("projects-updated", ()); },
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
