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
    pub acceptance_criteria: Option<Vec<String>>,
    pub closed_at: Option<String>,
    pub close_reason: Option<String>,
    pub is_favorite: Option<bool>,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_beads, update_bead, create_bead])
        .setup(|app| {
            let handle = app.handle().clone();
            let path = find_beads_file().expect("Failed to find beads file during setup");
            let parent_path = path.parent().unwrap().to_path_buf();

            let mut watcher = notify::RecommendedWatcher::new(move |res| {
                match res {
                    Ok(_) => {
                        let _ = handle.emit("beads-updated", ());
                    },
                    Err(e) => println!("watch error: {:?}", e),
                }
            }, Config::default()).unwrap();

            watcher.watch(&parent_path, RecursiveMode::Recursive).unwrap();
            
            // Keep watcher alive by moving it to app state or just leaking it for simplicity in this small app
            std::mem::forget(watcher);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
