use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use notify::{Watcher, RecursiveMode, Config};
use tauri::{Emitter, Manager};

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
fn update_bead(updated_bead: Bead) -> Result<(), String> {
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

    Ok(())
}

#[tauri::command]
fn create_bead(new_bead: Bead) -> Result<(), String> {
    let path = find_beads_file().ok_or_else(|| "Could not locate .beads/issues.jsonl".to_string())?;
    
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;

    let line = serde_json::to_string(&new_bead).map_err(|e| e.to_string())?;
    writeln!(file, "{}", line).map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Project {
    pub name: String,
    pub path: String,
}

fn get_favorites_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".bert-viz");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).unwrap();
    }
    dir.join("favorites.jsonl")
}

#[tauri::command]
fn get_favorite_projects() -> Result<Vec<Project>, String> {
    let path = get_favorites_path();
    if !path.exists() { return Ok(Vec::new()); }
    
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut projects = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() { continue; }
        let project: Project = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        projects.push(project);
    }
    Ok(projects)
}

#[tauri::command]
fn add_favorite_project(project: Project) -> Result<(), String> {
    let path = get_favorites_path();
    let mut file = OpenOptions::new().append(true).create(true).open(path).map_err(|e| e.to_string())?;
    let line = serde_json::to_string(&project).map_err(|e| e.to_string())?;
    writeln!(file, "{}", line).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn remove_favorite_project(name: String) -> Result<(), String> {
    let path = get_favorites_path();
    let projects = get_favorite_projects()?;
    let filtered: Vec<Project> = projects.into_iter().filter(|p| p.name != name).collect();
    
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    for p in filtered {
        let line = serde_json::to_string(&p).map_err(|e| e.to_string())?;
        writeln!(file, "{}", line).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_project(path: String) -> Result<(), String> {
    std::env::set_current_dir(&path).map_err(|e| e.to_string())?;
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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_beads, update_bead, create_bead, 
            get_favorite_projects, add_favorite_project, remove_favorite_project, open_project,
            get_current_dir
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let fav_handle = app.handle().clone();
            
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

            // Watch favorites file
            let fav_path = get_favorites_path();
            let mut fav_watcher = notify::RecommendedWatcher::new(move |res| {
                match res {
                    Ok(_) => { let _ = fav_handle.emit("favorites-updated", ()); },
                    Err(e) => println!("watch error: {:?}", e),
                }
            }, Config::default()).unwrap();
            fav_watcher.watch(&fav_path.parent().unwrap(), RecursiveMode::Recursive).unwrap();
            std::mem::forget(fav_watcher);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
