use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use notify::{Watcher, RecursiveMode, Config};
use tauri::{Emitter, AppHandle, Manager};

// Global cache for beads file path (avoid expensive subprocess calls)
// Use Mutex<Option> instead of OnceLock so we can clear it when switching projects
static BEADS_FILE_PATH_CACHE: Mutex<Option<PathBuf>> = Mutex::new(None);

// Shared state for the file watcher
struct BeadsWatcher {
    watcher: notify::RecommendedWatcher,
    current_path: Option<PathBuf>,
    #[allow(dead_code)] // Used in watcher closure
    last_checksum: Arc<Mutex<u64>>,
    #[allow(dead_code)] // Used in watcher closure
    last_emit: Arc<Mutex<Instant>>,
}

impl BeadsWatcher {
    fn new(handle: AppHandle) -> Result<Self, String> {
        let last_checksum = Arc::new(Mutex::new(0u64));
        let last_emit = Arc::new(Mutex::new(Instant::now()));

        let checksum_clone = Arc::clone(&last_checksum);
        let emit_clone = Arc::clone(&last_emit);

        let watcher = notify::RecommendedWatcher::new(
            move |res: std::result::Result<notify::Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        eprintln!("📁 Event: {:?}", event.kind);

                        // Get first path from event
                        if let Some(path) = event.paths.first() {
                            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                                // Handle file deletion (beads daemon deletes and recreates the file)
                                if matches!(event.kind, notify::EventKind::Remove(_)) {
                                    eprintln!("  🗑️  File removed, clearing caches");
                                    let mut last_hash = checksum_clone.lock().unwrap();
                                    *last_hash = 0; // Reset checksum so next create triggers update

                                    // Clear the beads file path cache so get_processed_data reads the new file
                                    let mut cache = BEADS_FILE_PATH_CACHE.lock().unwrap();
                                    *cache = None;
                                    eprintln!("  🗑️  Cleared beads file path cache");
                                    return;
                                }

                                // Handle file creation and modification
                                // (beads daemon creates new file after deletion)
                                if matches!(event.kind, notify::EventKind::Create(_) | notify::EventKind::Modify(_)) {
                                    // Add a small delay for file creation to complete
                                    if matches!(event.kind, notify::EventKind::Create(_)) {
                                        std::thread::sleep(Duration::from_millis(50));
                                    }

                                    if let Ok(bytes) = std::fs::read(path) {
                                        let mut hasher = DefaultHasher::new();
                                        bytes.hash(&mut hasher);
                                        let new_checksum = hasher.finish();

                                        let mut last_hash = checksum_clone.lock().unwrap();

                                        if *last_hash != new_checksum {
                                            *last_hash = new_checksum;

                                            let mut last = emit_clone.lock().unwrap();
                                            let now = Instant::now();
                                            if now.duration_since(*last) >= Duration::from_millis(250) {
                                                *last = now;
                                                match handle.emit("beads-updated", ()) {
                                                    Ok(_) => eprintln!("  ✅ Emitted beads-updated ({})",
                                                        if matches!(event.kind, notify::EventKind::Create(_)) { "create" } else { "modify" }),
                                                    Err(e) => eprintln!("  ❌ Failed to emit beads-updated: {:?}", e),
                                                }
                                            }
                                        }
                                    } else {
                                        eprintln!("  ⚠️  Failed to read file, might be mid-write");
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => eprintln!("Watch error: {:?}", e),
                }
            },
            Config::default(),
        ).map_err(|e| e.to_string())?;

        Ok(BeadsWatcher {
            watcher,
            current_path: None,
            last_checksum,
            last_emit,
        })
    }

    fn watch_beads_file(&mut self, path: PathBuf) -> Result<(), String> {
        // Unwatch old path if exists
        if let Some(old_path) = &self.current_path {
            if let Some(old_parent) = old_path.parent() {
                let _ = self.watcher.unwatch(old_parent);
                eprintln!("🔓 Unwatched: {}", old_parent.display());
            }
        }

        // Watch new path's parent directory
        if let Some(parent) = path.parent() {
            self.watcher.watch(parent, RecursiveMode::Recursive)
                .map_err(|e| format!("Failed to watch {}: {}", parent.display(), e))?;
            eprintln!("🔍 Now watching: {}", parent.display());
            self.current_path = Some(path);
        }

        Ok(())
    }
}

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
    pub parent: Option<String>,
    pub external_reference: Option<String>,
    // Unified field naming (matches JSONL and BeadNode)
    #[serde(alias = "design_notes")]
    pub design: Option<String>,
    #[serde(alias = "working_notes")]
    pub notes: Option<String>,
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

fn find_repo_root() -> Option<PathBuf> {
    let curr_dir = std::env::current_dir().ok()?;
    eprintln!("🔍 find_repo_root: Starting from current_dir = {}", curr_dir.display());

    let mut curr = curr_dir.clone();
    loop {
        let beads_path = curr.join(".beads");
        eprintln!("🔍 find_repo_root: Checking {}", beads_path.display());
        if beads_path.exists() {
            eprintln!("✅ find_repo_root: Found repo root at {}", curr.display());
            return Some(curr);
        }
        if !curr.pop() {
            break;
        }
    }
    eprintln!("❌ find_repo_root: No .beads found in any parent directory");
    None
}

fn check_bd_available() -> Result<(), String> {
    if std::process::Command::new("bd").arg("--version").output().is_err() {
        return Err("The 'bd' CLI is not found in the PATH. Please ensure it is installed and available.".to_string());
    }
    Ok(())
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

// ============================================================================
// Main Tauri Command for Processed Data (bp6-07y.5.2)
// ============================================================================

#[tauri::command]
fn get_processed_data(params: FilterParams) -> Result<ProcessedData, String> {
    let start_time = std::time::Instant::now();

    // 1. Load beads from file (use cached path to avoid expensive subprocess)
    let beads_path = {
        let mut cache = BEADS_FILE_PATH_CACHE.lock().unwrap();
        if cache.is_none() {
            *cache = find_beads_file();
        }
        cache.clone().ok_or_else(|| "Could not locate .beads/issues.jsonl in any parent directory".to_string())?
    };

    eprintln!("📖 get_processed_data: Reading from {}", beads_path.display());
    let load_start = std::time::Instant::now();

    let beads = load_beads_from_file(&beads_path)?;

    eprintln!("⏱️  File load: {:.2}ms ({} beads)", load_start.elapsed().as_secs_f64() * 1000.0, beads.len());

    // 2. Apply filters
    let mut filtered = beads.clone();

    // Apply status and time filters
    filtered = filter_by_status_and_time(&filtered, params.hide_closed, &params.closed_time_filter);

    // Apply text search
    filtered = filter_by_text(&filtered, &params.filter_text);

    // Include hierarchy if needed
    if !params.filter_text.is_empty() && params.include_hierarchy {
        filtered = include_hierarchy(filtered, &beads, &params.filter_text, params.include_hierarchy);
    }

    let tree_start = std::time::Instant::now();

    // 3. Build dependency graph
    let graph = build_dependency_graph(&filtered);

    // 4. Build WBS tree
    let mut tree = build_wbs_tree(&filtered);

    // 5. Sort siblings (by dependencies or explicit sort)
    tree = sort_wbs_tree_siblings(tree, &graph, &params.sort_by, &params.sort_order);

    eprintln!("⏱️  Tree building: {:.2}ms", tree_start.elapsed().as_secs_f64() * 1000.0);
    let layout_start = std::time::Instant::now();

    // Apply collapsed state to tree
    fn apply_collapsed_state(nodes: &mut [WBSNode], collapsed_ids: &[String]) {
        for node in nodes {
            if collapsed_ids.contains(&node.bead.id) {
                node.is_expanded = false;
            }
            if !node.children.is_empty() {
                apply_collapsed_state(&mut node.children, collapsed_ids);
            }
        }
    }
    apply_collapsed_state(&mut tree, &params.collapsed_ids);

    // 6. Build blocks and successors maps for Gantt layout
    let mut blocks_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut successors_map: HashMap<String, Vec<String>> = HashMap::new();

    for bead in &filtered {
        for dep in &bead.dependencies {
            if dep.r#type == "blocks" {
                // dep.depends_on_id blocks bead.id
                blocks_map
                    .entry(bead.id.clone())
                    .or_insert_with(Vec::new)
                    .push(dep.depends_on_id.clone());

                successors_map
                    .entry(dep.depends_on_id.clone())
                    .or_insert_with(Vec::new)
                    .push(bead.id.clone());
            }
        }
    }

    // 7. Calculate earliest start times (X positions)
    let x_map = calculate_earliest_start_times(&filtered, &blocks_map);

    // 8. Calculate node ranges (position and width)
    let mut range_cache: HashMap<String, NodeRange> = HashMap::new();
    calculate_node_ranges(&tree, &x_map, &mut range_cache);

    // 9. Find critical path
    let critical_path = find_critical_path(&filtered, &successors_map);

    // 10. Mark critical nodes in tree
    fn mark_critical_nodes(nodes: &mut [WBSNode], critical_path: &HashSet<String>) {
        for node in nodes {
            if critical_path.contains(&node.bead.id) {
                node.is_critical = true;
            }
            mark_critical_nodes(&mut node.children, critical_path);
        }
    }
    mark_critical_nodes(&mut tree, &critical_path);

    // 11. Generate Gantt layout (items and connectors)
    let layout = generate_gantt_layout(
        &filtered,
        &tree,
        &x_map,
        &range_cache,
        &critical_path,
        params.zoom,
    );

    eprintln!("⏱️  Layout calculation: {:.2}ms", layout_start.elapsed().as_secs_f64() * 1000.0);

    // 12. Calculate state distributions from tree
    // Convert tree to temporary BeadNode tree for distribution calculation
    fn wbs_to_temp_bead_nodes(
        nodes: &[WBSNode],
        x_map: &HashMap<String, usize>,
        range_cache: &HashMap<String, NodeRange>,
    ) -> Vec<BeadNode> {
        nodes
            .iter()
            .map(|node| {
                let node_range = range_cache.get(&node.bead.id);
                let (cell_offset, cell_count) = if let Some(range) = node_range {
                    let offset = (range.x / 10.0).round() as usize;
                    let count = (range.width / 10.0).ceil().max(1.0) as usize;
                    (offset, count)
                } else {
                    let offset = x_map.get(&node.bead.id).copied().unwrap_or(0);
                    (offset, 1)
                };

                BeadNode {
                    id: node.bead.id.clone(),
                    title: node.bead.title.clone(),
                    description: node.bead.description.clone(),
                    status: node.bead.status.clone(),
                    priority: node.bead.priority,
                    issue_type: node.bead.issue_type.clone(),
                    estimate: node.bead.estimate,
                    dependencies: node.bead.dependencies.clone(),
                    owner: node.bead.owner.clone(),
                    created_at: node.bead.created_at.clone(),
                    created_by: node.bead.created_by.clone(),
                    updated_at: node.bead.updated_at.clone(),
                    labels: node.bead.labels.clone(),
                    acceptance_criteria: node.bead.acceptance_criteria.clone(),
                    closed_at: node.bead.closed_at.clone(),
                    close_reason: node.bead.close_reason.clone(),
                    is_favorite: node.bead.is_favorite,
                    parent: node.bead.parent.clone(),
                    external_reference: node.bead.external_reference.clone(),
                    design: node.bead.design.clone(),
                    notes: node.bead.notes.clone(),
                    children: wbs_to_temp_bead_nodes(&node.children, x_map, range_cache),
                    is_blocked: node.is_blocked,
                    is_critical: node.is_critical,
                    blocking_ids: vec![],
                    depth: 0,
                    cell_offset,
                    cell_count,
                    is_expanded: node.is_expanded,
                    is_visible: true,
                    extra_metadata: node.bead.extra_metadata.clone(),
                }
            })
            .collect()
    }

    let temp_tree = wbs_to_temp_bead_nodes(&tree, &x_map, &range_cache);
    let distributions = calculate_state_distribution_from_tree(&temp_tree);

    // 13. Return ProcessedData
    let total_time = start_time.elapsed();
    eprintln!("⏱️  Total processing time: {:.2}ms", total_time.as_secs_f64() * 1000.0);

    Ok(ProcessedData {
        tree,
        layout,
        distributions,
    })
}

// ============================================================================
// Unified View Layer (bp6-75y.2)
// ============================================================================

/// Convert a Bead to a BeadNode with computed properties.
/// Field names are now unified (design, notes) in both Bead and BeadNode.
fn bead_to_bead_node(
    bead: &Bead,
    children: Vec<BeadNode>,
    depth: usize,
    cell_offset: usize,
    cell_count: usize,
    is_blocked: bool,
    is_critical: bool,
    blocking_ids: Vec<String>,
    is_expanded: bool,
    is_visible: bool,
) -> BeadNode {
    BeadNode {
        // Core Bead Data
        id: bead.id.clone(),
        title: bead.title.clone(),
        description: bead.description.clone(),
        status: bead.status.clone(),
        priority: bead.priority,
        issue_type: bead.issue_type.clone(),
        estimate: bead.estimate,
        dependencies: bead.dependencies.clone(),

        // Metadata
        owner: bead.owner.clone(),
        created_at: bead.created_at.clone(),
        created_by: bead.created_by.clone(),
        updated_at: bead.updated_at.clone(),
        labels: bead.labels.clone(),
        acceptance_criteria: bead.acceptance_criteria.clone(),
        closed_at: bead.closed_at.clone(),
        close_reason: bead.close_reason.clone(),
        is_favorite: bead.is_favorite,
        parent: bead.parent.clone(),
        external_reference: bead.external_reference.clone(),

        // Unified Field Naming
        design: bead.design.clone(),
        notes: bead.notes.clone(),

        // Hierarchical Structure
        children,

        // Computed Properties
        is_blocked,
        is_critical,
        blocking_ids,

        // Logical Positioning
        depth,
        cell_offset,
        cell_count,

        // UI State
        is_expanded,
        is_visible,

        // Extra metadata
        extra_metadata: bead.extra_metadata.clone(),
    }
}

/// Convert WBSNode tree to BeadNode tree with computed properties.
fn convert_wbs_to_bead_nodes(
    nodes: &[WBSNode],
    depth: usize,
    x_map: &HashMap<String, usize>,
    range_cache: &HashMap<String, NodeRange>,
    critical_path: &HashSet<String>,
    collapsed_ids: &[String],
) -> Vec<BeadNode> {
    nodes.iter().map(|node| {
        // Get cell positioning
        // x_map contains cell offsets (0, 1, 2, 3...) - these ARE the cell positions
        // range cache contains duration in time units (10, 20, 30...)
        let cell_offset = x_map.get(&node.bead.id).copied().unwrap_or(0);

        let node_range = range_cache.get(&node.bead.id);
        let cell_count = if let Some(range) = node_range {
            // Convert time units to cell count (10 time units = 1 cell)
            (range.width / 10.0).ceil().max(1.0) as usize
        } else {
            // Fallback: 1 cell
            1
        };

        // Compute properties
        let is_blocked = node.is_blocked;
        let is_critical = critical_path.contains(&node.bead.id);
        let blocking_ids: Vec<String> = node.bead.dependencies
            .iter()
            .filter(|dep| dep.r#type == "blocks")
            .map(|dep| dep.depends_on_id.clone())
            .collect();

        // UI state
        let is_expanded = !collapsed_ids.contains(&node.bead.id);
        let is_visible = true; // Will be computed during tree traversal

        // Recursively convert children
        let children = if !node.children.is_empty() {
            convert_wbs_to_bead_nodes(
                &node.children,
                depth + 1,
                x_map,
                range_cache,
                critical_path,
                collapsed_ids,
            )
        } else {
            Vec::new()
        };

        bead_to_bead_node(
            &node.bead,
            children,
            depth,
            cell_offset,
            cell_count,
            is_blocked,
            is_critical,
            blocking_ids,
            is_expanded,
            is_visible,
        )
    }).collect()
}

/// Build ViewIndexes for fast lookups.
fn build_view_indexes(tree: &[BeadNode], critical_path: &HashSet<String>) -> ViewIndexes {
    let mut id_to_index = HashMap::new();
    let mut id_to_parent = HashMap::new();
    let mut index = 0;

    fn traverse(
        nodes: &[BeadNode],
        parent_id: Option<&str>,
        id_to_index: &mut HashMap<String, usize>,
        id_to_parent: &mut HashMap<String, String>,
        index: &mut usize,
    ) {
        for node in nodes {
            id_to_index.insert(node.id.clone(), *index);
            *index += 1;

            if let Some(parent) = parent_id {
                id_to_parent.insert(node.id.clone(), parent.to_string());
            }

            if !node.children.is_empty() {
                traverse(&node.children, Some(&node.id), id_to_index, id_to_parent, index);
            }
        }
    }

    traverse(tree, None, &mut id_to_index, &mut id_to_parent, &mut index);

    // Convert critical path HashSet to Vec
    let critical_path_vec: Vec<String> = critical_path.iter().cloned().collect();

    ViewIndexes {
        id_to_index,
        id_to_parent,
        critical_path: critical_path_vec,
    }
}

/// Calculate project metadata (aggregate statistics).
fn calculate_project_metadata(
    _tree: &[BeadNode],
    filtered_beads: &[Bead],
    distributions: Vec<BucketDistribution>,
    _critical_path: &HashSet<String>,
    x_map: &HashMap<String, usize>,
) -> ProjectMetadata {
    let mut open_count = 0;
    let mut in_progress_count = 0;
    let mut blocked_count = 0;
    let mut closed_count = 0;

    for bead in filtered_beads {
        match bead.status.as_str() {
            "open" | "pending" => open_count += 1,
            "in_progress" => in_progress_count += 1,
            "closed" | "done" => closed_count += 1,
            _ => {}
        }

        if bead.status != "closed" && bead.status != "done" {
            // Check if blocked
            let is_blocked = bead.dependencies.iter().any(|dep| {
                if dep.r#type == "blocks" {
                    // Check if the blocker is not closed
                    filtered_beads.iter().any(|b| {
                        b.id == dep.depends_on_id && b.status != "closed" && b.status != "done"
                    })
                } else {
                    false
                }
            });

            if is_blocked {
                blocked_count += 1;
            }
        }
    }

    // Calculate total duration (critical path length)
    let total_duration = x_map.values().copied().map(|v| v as f64).fold(0.0f64, f64::max);

    ProjectMetadata {
        total_beads: filtered_beads.len(),
        open_count,
        in_progress_count,
        blocked_count,
        closed_count,
        total_duration,
        distributions,
    }
}

/// Get ProjectViewModel - the unified view model for all UI components.
/// This function does all CPU-intensive computation: filtering, sorting, dependency
/// graph building, critical path calculation, and tree construction.
#[tauri::command]
fn get_project_view_model(params: FilterParams) -> Result<ProjectViewModel, String> {
    let start_time = std::time::Instant::now();

    // 1. Load beads from file (reuse logic from get_processed_data)
    let beads_path = {
        let mut cache = BEADS_FILE_PATH_CACHE.lock().unwrap();
        if cache.is_none() {
            *cache = find_beads_file();
        }
        cache.clone().ok_or_else(|| "Could not locate .beads/issues.jsonl in any parent directory".to_string())?
    };

    eprintln!("📖 get_project_view_model: Reading from {}", beads_path.display());
    let load_start = std::time::Instant::now();

    let beads = load_beads_from_file(&beads_path)?;

    eprintln!("⏱️  File load: {:.2}ms ({} beads)", load_start.elapsed().as_secs_f64() * 1000.0, beads.len());

    // 2. Apply filters
    let mut filtered = beads.clone();
    filtered = filter_by_status_and_time(&filtered, params.hide_closed, &params.closed_time_filter);
    filtered = filter_by_text(&filtered, &params.filter_text);

    if !params.filter_text.is_empty() && params.include_hierarchy {
        filtered = include_hierarchy(filtered, &beads, &params.filter_text, params.include_hierarchy);
    }

    let tree_start = std::time::Instant::now();

    // 3. Build dependency graph
    let graph = build_dependency_graph(&filtered);

    // 4. Build WBS tree
    let mut tree = build_wbs_tree(&filtered);

    // 5. Sort siblings (by dependencies or explicit sort)
    tree = sort_wbs_tree_siblings(tree, &graph, &params.sort_by, &params.sort_order);

    // Apply collapsed state
    fn apply_collapsed_state(nodes: &mut [WBSNode], collapsed_ids: &[String]) {
        for node in nodes {
            if collapsed_ids.contains(&node.bead.id) {
                node.is_expanded = false;
            }
            if !node.children.is_empty() {
                apply_collapsed_state(&mut node.children, collapsed_ids);
            }
        }
    }
    apply_collapsed_state(&mut tree, &params.collapsed_ids);

    eprintln!("⏱️  Tree building: {:.2}ms", tree_start.elapsed().as_secs_f64() * 1000.0);
    let compute_start = std::time::Instant::now();

    // 6. Build blocks and successors maps
    let mut blocks_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut successors_map: HashMap<String, Vec<String>> = HashMap::new();

    for bead in &filtered {
        for dep in &bead.dependencies {
            if dep.r#type == "blocks" {
                blocks_map
                    .entry(bead.id.clone())
                    .or_insert_with(Vec::new)
                    .push(dep.depends_on_id.clone());

                successors_map
                    .entry(dep.depends_on_id.clone())
                    .or_insert_with(Vec::new)
                    .push(bead.id.clone());
            }
        }
    }

    // 7. Calculate earliest start times (logical units, not pixels)
    let x_map = calculate_earliest_start_times(&filtered, &blocks_map);
    eprintln!("⏱️  x_map has {} entries", x_map.len());
    if !x_map.is_empty() {
        let first_entry = x_map.iter().next().unwrap();
        eprintln!("⏱️  First x_map entry: {} -> {}", first_entry.0, first_entry.1);
    }

    // 8. Calculate node ranges
    let mut range_cache: HashMap<String, NodeRange> = HashMap::new();
    calculate_node_ranges(&tree, &x_map, &mut range_cache);
    eprintln!("⏱️  range_cache has {} entries", range_cache.len());

    // 9. Find critical path
    let critical_path = find_critical_path(&filtered, &successors_map);

    // 10. Mark critical nodes in tree
    fn mark_critical_nodes(nodes: &mut [WBSNode], critical_path: &HashSet<String>) {
        for node in nodes {
            if critical_path.contains(&node.bead.id) {
                node.is_critical = true;
            }
            mark_critical_nodes(&mut node.children, critical_path);
        }
    }
    mark_critical_nodes(&mut tree, &critical_path);

    // 11. Convert WBS tree to BeadNode tree
    let bead_node_tree = convert_wbs_to_bead_nodes(
        &tree,
        0, // root depth
        &x_map,
        &range_cache,
        &critical_path,
        &params.collapsed_ids,
    );

    // 12. Generate Gantt layout for distributions (reuse existing logic)
    // 12. Calculate state distributions from tree (before building layout)
    let distributions = calculate_state_distribution_from_tree(&bead_node_tree);

    // 13. Build indexes
    let indexes = build_view_indexes(&bead_node_tree, &critical_path);

    // 14. Calculate metadata
    let metadata = calculate_project_metadata(
        &bead_node_tree,
        &filtered,
        distributions,
        &critical_path,
        &x_map,
    );

    eprintln!("⏱️  Compute properties: {:.2}ms", compute_start.elapsed().as_secs_f64() * 1000.0);

    let total_time = start_time.elapsed();
    eprintln!("⏱️  Total view model time: {:.2}ms", total_time.as_secs_f64() * 1000.0);

    Ok(ProjectViewModel {
        tree: bead_node_tree,
        metadata,
        indexes,
    })
}

/// Robustly load beads from a jsonl file with retries to handle partial writes or locks.
fn load_beads_from_file(path: &std::path::Path) -> Result<Vec<Bead>, String> {
    let mut last_error = String::new();
    let mut last_size = 0;

    for i in 0..10 {
        // 1. Check metadata and stability
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                if i < 9 {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                return Err(format!("Failed to get metadata: {}", e));
            }
        };

        let current_size = metadata.len();
        if current_size == 0 {
            if i < 9 {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            return Ok(Vec::new());
        }

        // If file size is still changing, wait for it to settle
        if current_size != last_size && i < 9 {
            last_size = current_size;
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // 2. Try to open and verify completeness
        match File::open(path) {
            Ok(mut file) => {
                // Check if last byte is newline - beads JSONL always ends with \n
                use std::io::{Seek, SeekFrom, Read};
                if current_size > 0 {
                    if let Ok(_) = file.seek(SeekFrom::End(-1)) {
                        let mut last_byte = [0u8; 1];
                        if let Ok(_) = file.read_exact(&mut last_byte) {
                            if last_byte[0] != b'\n' && i < 9 {
                                std::thread::sleep(Duration::from_millis(100));
                                continue;
                            }
                        }
                    }
                    // Reset to start for reading
                    let _ = file.seek(SeekFrom::Start(0));
                }

                let reader = BufReader::new(file);
                let mut beads = Vec::new();
                let mut had_parse_error = false;

                for (index, line) in reader.lines().enumerate() {
                    let line = match line {
                        Ok(l) => l,
                        Err(e) => {
                            if i < 9 {
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
                            if i < 9 {
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

                std::thread::sleep(Duration::from_millis(100 * (i + 1)));
            }
            Err(e) => {
                if i == 9 {
                    return Err(format!("Failed to open issues.jsonl after retries: {}", e));
                }
                std::thread::sleep(Duration::from_millis(100 * (i + 1)));
            }
        }
    }

    Err(format!("Failed to read beads after retries. Last error: {}", last_error))
}

#[tauri::command]
fn get_beads() -> Result<Vec<Bead>, String> {
    let path = find_beads_file().ok_or_else(|| "Could not locate .beads/issues.jsonl in any parent directory".to_string())?;
    eprintln!("📖 get_beads: Reading from {}", path.display());
    load_beads_from_file(&path)
}

#[tauri::command]
fn update_bead(updated_bead: Bead, app_handle: AppHandle) -> Result<(), String> {
    eprintln!("📝 update_bead: Called for bead '{}' ({})", updated_bead.title, updated_bead.id);
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;
    eprintln!("📝 update_bead: Using repo_path = {}", repo_path.display());

    let mut cmd = std::process::Command::new("bd");
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

    // Pass everything also to --metadata to ensure extra_metadata is preserved
    // and fields that don't have explicit flags are updated.
    let metadata_json = serde_json::to_string(&updated_bead).map_err(|e| e.to_string())?;
    cmd.arg("--metadata").arg(metadata_json);

    eprintln!("📝 update_bead: Executing bd command in directory: {}", repo_path.display());
    eprintln!("📝 update_bead: Command: {:?}", cmd);
    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
fn close_bead(bead_id: String, reason: Option<String>, app_handle: AppHandle) -> Result<(), String> {
    eprintln!("🔒 close_bead: Called for bead '{}'", bead_id);
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;
    eprintln!("🔒 close_bead: Using repo_path = {}", repo_path.display());

    let mut cmd = std::process::Command::new("bd");
    cmd.arg("close").arg(&bead_id);

    if let Some(r) = reason {
        cmd.arg("--reason").arg(r);
    }

    eprintln!("🔒 close_bead: Executing bd command in directory: {}", repo_path.display());
    eprintln!("🔒 close_bead: Command: {:?}", cmd);
    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
fn reopen_bead(bead_id: String, app_handle: AppHandle) -> Result<(), String> {
    eprintln!("🔓 reopen_bead: Called for bead '{}'", bead_id);
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;
    eprintln!("🔓 reopen_bead: Using repo_path = {}", repo_path.display());

    let mut cmd = std::process::Command::new("bd");
    cmd.arg("reopen").arg(&bead_id);

    eprintln!("🔓 reopen_bead: Executing bd command in directory: {}", repo_path.display());
    eprintln!("🔓 reopen_bead: Command: {:?}", cmd);
    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
fn claim_bead(bead_id: String, app_handle: AppHandle) -> Result<(), String> {
    eprintln!("👤 claim_bead: Called for bead '{}'", bead_id);
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;
    eprintln!("👤 claim_bead: Using repo_path = {}", repo_path.display());

    let mut cmd = std::process::Command::new("bd");
    cmd.arg("update")
        .arg(&bead_id)
        .arg("--status")
        .arg("in_progress");

    eprintln!("👤 claim_bead: Executing bd command in directory: {}", repo_path.display());
    eprintln!("👤 claim_bead: Command: {:?}", cmd);
    let output = cmd.current_dir(repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let _ = app_handle.emit("beads-updated", ());
    Ok(())
}

#[tauri::command]
fn create_bead(new_bead: Bead, app_handle: AppHandle) -> Result<String, String> {
    eprintln!("🆕 create_bead: Called for bead '{}'", new_bead.title);
    check_bd_available()?;
    let repo_path = find_repo_root().ok_or_else(|| "Could not locate .beads directory in any parent".to_string())?;
    eprintln!("🆕 create_bead: Using repo_path = {}", repo_path.display());

    // 1. Create with minimal flags to get ID
    let mut cmd = std::process::Command::new("bd");
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

    eprintln!("🆕 create_bead: Executing bd command in directory: {}", repo_path.display());
    eprintln!("🆕 create_bead: Command: {:?}", cmd);
    let output = cmd.current_dir(&repo_path).output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("CLI Create Error: {}", stderr));
    }

    // Capture the generated ID
    let new_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if new_id.is_empty() {
        return Err("Create command succeeded but returned no ID".to_string());
    }
    eprintln!("🆕 create_bead: Created bead with ID: {}", new_id);

    // 2. Immediately update to set fields that create doesn't support (like status and metadata)
    let mut update_cmd = std::process::Command::new("bd");
    update_cmd.arg("update")
        .arg(&new_id)
        .arg("--status").arg(&new_bead.status);

    let metadata_json = serde_json::to_string(&new_bead).map_err(|e| e.to_string())?;
    update_cmd.arg("--metadata").arg(metadata_json);

    eprintln!("🆕 create_bead: Executing follow-up bd update in directory: {}", repo_path.display());
    eprintln!("🆕 create_bead: Update command: {:?}", update_cmd);
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

// ============================================================================
// Data Structures for Processed Output (bp6-07y.1)
// ============================================================================

/// WBSNode represents a node in the Work Breakdown Structure tree.
/// Extends Bead with tree-specific metadata for hierarchical display.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WBSNode {
    // Flatten all Bead fields
    #[serde(flatten)]
    pub bead: Bead,

    // Tree structure
    pub children: Vec<WBSNode>,

    // UI state flags (use camelCase for TypeScript compatibility)
    #[serde(rename = "isExpanded")]
    pub is_expanded: bool,
    #[serde(rename = "isBlocked")]
    pub is_blocked: bool,
    #[serde(rename = "isCritical")]
    pub is_critical: bool,
}

/// Point represents an (x, y) coordinate for Gantt chart rendering.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// GanttItem represents a single bead's position in the Gantt chart.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GanttItem {
    pub bead: Bead,
    pub x: f64,
    pub width: f64,
    pub row: usize,
    pub depth: usize,
    #[serde(rename = "isCritical")]
    pub is_critical: bool,
    #[serde(rename = "isBlocked")]
    pub is_blocked: bool,
}

/// GanttConnector represents a dependency line between two beads in the Gantt chart.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GanttConnector {
    pub from: Point,
    pub to: Point,
    #[serde(rename = "isCritical")]
    pub is_critical: bool,
}

/// GanttLayout contains all computed layout data for Gantt chart rendering.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GanttLayout {
    pub items: Vec<GanttItem>,
    pub connectors: Vec<GanttConnector>,
    #[serde(rename = "rowCount")]
    pub row_count: usize,
    #[serde(rename = "rowDepths")]
    pub row_depths: Vec<usize>,
}

/// BucketDistribution represents status counts for a time bucket in the Gantt header.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BucketDistribution {
    pub open: usize,
    #[serde(rename = "inProgress")]
    pub in_progress: usize,
    pub blocked: usize,
    pub closed: usize,
}

/// ProcessedData is the top-level response structure containing all processed data.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessedData {
    pub tree: Vec<WBSNode>,
    pub layout: GanttLayout,
    pub distributions: Vec<BucketDistribution>,
}

// ============================================================================
// Unified View Model (bp6-75y.1)
// ============================================================================

/// BeadNode is the unified node structure in the view model tree.
/// It contains all bead data, computed properties, hierarchical structure,
/// and logical positioning (NOT pixel coordinates - those are computed by frontend).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BeadNode {
    // ===== Core Bead Data =====
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: u32,
    #[serde(rename = "issueType")]
    pub issue_type: String,
    pub estimate: Option<u32>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,

    // ===== Metadata Fields =====
    pub owner: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "createdBy")]
    pub created_by: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    pub labels: Option<Vec<String>>,
    #[serde(default, rename = "acceptanceCriteria")]
    pub acceptance_criteria: Option<Vec<String>>,
    #[serde(rename = "closedAt")]
    pub closed_at: Option<String>,
    #[serde(rename = "closeReason")]
    pub close_reason: Option<String>,
    #[serde(rename = "isFavorite")]
    pub is_favorite: Option<bool>,
    pub parent: Option<String>,
    #[serde(rename = "externalReference")]
    pub external_reference: Option<String>,

    // ===== Unified Field Naming =====
    // Note: JSONL uses 'design' and 'notes', NOT 'design_notes'/'working_notes'
    pub design: Option<String>,
    pub notes: Option<String>,

    // ===== Hierarchical Structure =====
    pub children: Vec<BeadNode>,

    // ===== Computed Properties (Backend calculates these) =====
    #[serde(rename = "isBlocked")]
    pub is_blocked: bool,
    #[serde(rename = "isCritical")]
    pub is_critical: bool,
    #[serde(rename = "blockingIds")]
    pub blocking_ids: Vec<String>,

    // ===== Logical Positioning (NOT pixels - frontend converts to pixels) =====
    /// Tree depth (0 = root, 1 = child, 2 = grandchild, etc.)
    pub depth: usize,
    /// Cell offset: which grid cell column this task starts in (0, 1, 2, ...)
    #[serde(rename = "cellOffset")]
    pub cell_offset: usize,
    /// Cell count: how many grid cells wide this task is (1, 2, 3, ...)
    #[serde(rename = "cellCount")]
    pub cell_count: usize,

    // ===== UI State =====
    #[serde(rename = "isExpanded")]
    pub is_expanded: bool,
    #[serde(rename = "isVisible")]
    pub is_visible: bool,

    // Extra metadata (preserves any unknown fields from JSONL)
    #[serde(flatten)]
    pub extra_metadata: serde_json::Map<String, serde_json::Value>,
}

/// ViewIndexes provides fast lookups into the view model tree.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ViewIndexes {
    /// Map from bead ID to its index in a flattened tree traversal
    #[serde(rename = "idToIndex")]
    pub id_to_index: HashMap<String, usize>,

    /// Map from bead ID to its parent's ID
    #[serde(rename = "idToParent")]
    pub id_to_parent: HashMap<String, String>,

    /// List of all critical path bead IDs (in order)
    #[serde(rename = "criticalPath")]
    pub critical_path: Vec<String>,
}

/// ProjectMetadata contains aggregate statistics about the project.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    /// Total number of beads in the project
    #[serde(rename = "totalBeads")]
    pub total_beads: usize,

    /// Number of open beads
    #[serde(rename = "openCount")]
    pub open_count: usize,

    /// Number of blocked beads
    #[serde(rename = "blockedCount")]
    pub blocked_count: usize,

    /// Number of beads in progress
    #[serde(rename = "inProgressCount")]
    pub in_progress_count: usize,

    /// Number of closed beads
    #[serde(rename = "closedCount")]
    pub closed_count: usize,

    /// Total project duration (critical path length)
    #[serde(rename = "totalDuration")]
    pub total_duration: f64,

    /// State distributions by time bucket
    pub distributions: Vec<BucketDistribution>,
}

/// ProjectViewModel is the single source of truth for all UI components.
/// Backend computes this once per filter/view change, frontend reactively renders it.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectViewModel {
    /// Hierarchical tree of beads with all computed properties
    pub tree: Vec<BeadNode>,

    /// Project-level metadata and statistics
    pub metadata: ProjectMetadata,

    /// Fast lookup indexes
    pub indexes: ViewIndexes,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Project {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub is_favorite: bool,
    pub last_opened: Option<String>,
}

// ============================================================================
// WBS Tree Building Algorithms (bp6-07y.2)
// ============================================================================

/// DependencyGraph contains the graph representation of blocking dependencies.
/// Used for topological sorting and building the WBS tree.
#[derive(Debug, Clone)]
struct DependencyGraph {
    /// Map from bead ID to list of beads it blocks (successors)
    blocks: HashMap<String, Vec<String>>,
    /// Map from bead ID to list of beads that block it (predecessors)
    blocked_by: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    fn new() -> Self {
        DependencyGraph {
            blocks: HashMap::new(),
            blocked_by: HashMap::new(),
        }
    }
}

/// Construct a dependency graph from bead dependencies.
/// Separates parent-child relationships from blocking dependencies.
///
/// Returns a DependencyGraph with blocks and blocked_by maps populated.
fn build_dependency_graph(beads: &[Bead]) -> DependencyGraph {
    let mut graph = DependencyGraph::new();

    // Initialize empty vectors for all beads
    for bead in beads {
        graph.blocks.insert(bead.id.clone(), Vec::new());
        graph.blocked_by.insert(bead.id.clone(), Vec::new());
    }

    // Build the graph from dependencies
    for bead in beads {
        for dep in &bead.dependencies {
            if dep.r#type == "blocks" {
                // dep.depends_on_id blocks bead.id
                // So: depends_on_id -> bead.id (edge in graph)
                let blocker_id = dep.depends_on_id.clone();
                let blocked_id = bead.id.clone();

                // Add to blocks map (blocker blocks blocked_id)
                graph.blocks
                    .entry(blocker_id.clone())
                    .or_insert_with(Vec::new)
                    .push(blocked_id.clone());

                // Add to blocked_by map (blocked_id is blocked by blocker)
                graph.blocked_by
                    .entry(blocked_id)
                    .or_insert_with(Vec::new)
                    .push(blocker_id);
            }
        }
    }

    graph
}

/// Topologically sort nodes using Kahn's Algorithm.
///
/// This handles circular dependencies by appending remaining nodes at the end.
/// Secondary sort by priority for deterministic ordering.
///
/// # Arguments
/// * `nodes` - The WBSNodes to sort (typically siblings at the same tree level)
/// * `bead_ids` - Set of bead IDs that are part of this sibling group
/// * `graph` - The dependency graph for all beads
///
/// # Returns
/// A topologically sorted vector of WBSNodes.
fn topological_sort(
    nodes: Vec<WBSNode>,
    bead_ids: &HashSet<String>,
    graph: &DependencyGraph,
) -> Vec<WBSNode> {
    if nodes.len() <= 1 {
        return nodes;
    }

    // Build a map for quick lookup
    let node_map: HashMap<String, WBSNode> =
        nodes.into_iter().map(|n| (n.bead.id.clone(), n)).collect();

    // Calculate in-degree for nodes in this sibling group
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    for id in bead_ids {
        // Count how many blockers from THIS sibling group block this node
        let empty_vec = Vec::new();
        let blockers = graph.blocked_by.get(id).unwrap_or(&empty_vec);
        let count = blockers.iter().filter(|b| bead_ids.contains(*b)).count();
        in_degree.insert(id.clone(), count);
    }

    // Initialize queue with nodes that have in-degree 0
    let mut initial_nodes: Vec<&WBSNode> = node_map
        .values()
        .filter(|n| *in_degree.get(&n.bead.id).unwrap_or(&0) == 0)
        .collect();

    // Sort by priority for deterministic ordering (ascending: P0 < P1 < P2)
    // Secondary sort by ID for stability
    initial_nodes.sort_by(|a, b| {
        let ord = a.bead.priority.cmp(&b.bead.priority);
        if ord == std::cmp::Ordering::Equal {
            a.bead.id.cmp(&b.bead.id)
        } else {
            ord
        }
    });

    let mut queue: Vec<String> = initial_nodes.iter().map(|n| n.bead.id.clone()).collect();
    let mut result: Vec<WBSNode> = Vec::new();

    // Kahn's algorithm
    while let Some(u) = queue.pop() {
        if let Some(node) = node_map.get(&u) {
            result.push(node.clone());
        }

        // Process neighbors (nodes that u blocks)
        if let Some(neighbors) = graph.blocks.get(&u) {
            for v in neighbors {
                // Only process if v is in this sibling group
                if bead_ids.contains(v) {
                    if let Some(degree) = in_degree.get_mut(v) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push(v.clone());
                        }
                    }
                }
            }
        }
    }

    // Handle circular dependencies: append remaining nodes sorted by priority
    if result.len() != node_map.len() {
        let added_ids: HashSet<String> = result.iter().map(|n| n.bead.id.clone()).collect();
        let mut remaining: Vec<WBSNode> = node_map
            .into_iter()
            .filter(|(id, _)| !added_ids.contains(id))
            .map(|(_, node)| node)
            .collect();

        remaining.sort_by_key(|n| n.bead.priority);
        result.extend(remaining);
    }

    result
}

// ============================================================================
// Filtering and State Distribution (bp6-07y.4)
// ============================================================================

/// ClosedTimeFilter enum for filtering closed beads by time.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClosedTimeFilter {
    All,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "6h")]
    SixHours,
    #[serde(rename = "24h")]
    TwentyFourHours,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "older_than_6h")]
    OlderThan6h,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    Priority,
    Title,
    Type,
    Id,
    None,
}

/// Filter beads by text search across title, id, owner, and labels.
/// Case-insensitive matching.
fn filter_by_text(beads: &[Bead], filter_text: &str) -> Vec<Bead> {
    if filter_text.is_empty() {
        return beads.to_vec();
    }

    let search = filter_text.to_lowercase();

    beads
        .iter()
        .filter(|b| {
            b.title.to_lowercase().contains(&search)
                || b.id.to_lowercase().contains(&search)
                || b.owner
                    .as_ref()
                    .map(|o| o.to_lowercase().contains(&search))
                    .unwrap_or(false)
                || b.labels
                    .as_ref()
                    .map(|labels| labels.iter().any(|l| l.to_lowercase().contains(&search)))
                    .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// Check if a bead passes the closed time filter.
fn passes_closed_time_filter(bead: &Bead, filter: &ClosedTimeFilter) -> bool {
    // If not closed, always passes
    if bead.status != "closed" {
        return true;
    }

    // 'all' filter shows all closed tasks
    if *filter == ClosedTimeFilter::All {
        return true;
    }

    // If no closed_at timestamp, include it (benefit of the doubt)
    let closed_at = match &bead.closed_at {
        Some(s) if !s.is_empty() => s,
        _ => return true,
    };

    // Parse the timestamp (RFC3339 format expected)
    let closed_date = match chrono::DateTime::parse_from_rfc3339(closed_at) {
        Ok(dt) => dt,
        Err(_) => return true, // Invalid timestamp, include it
    };

    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(closed_date);
    let hours_ago = duration.num_hours() as f64 + (duration.num_minutes() % 60) as f64 / 60.0;

    match filter {
        ClosedTimeFilter::All => true,
        ClosedTimeFilter::OneHour => hours_ago <= 1.0,
        ClosedTimeFilter::SixHours => hours_ago <= 6.0,
        ClosedTimeFilter::TwentyFourHours => hours_ago <= 24.0,
        ClosedTimeFilter::SevenDays => hours_ago <= 24.0 * 7.0,
        ClosedTimeFilter::ThirtyDays => hours_ago <= 24.0 * 30.0,
        ClosedTimeFilter::OlderThan6h => hours_ago > 6.0,
    }
}

/// Filter beads by status (hide closed) and time-based filters.
fn filter_by_status_and_time(
    beads: &[Bead],
    hide_closed: bool,
    closed_time_filter: &ClosedTimeFilter,
) -> Vec<Bead> {
    beads
        .iter()
        .filter(|b| {
            // Apply hide_closed filter
            if hide_closed && b.status == "closed" {
                return false;
            }

            // Apply time-based filter for closed tasks
            passes_closed_time_filter(b, closed_time_filter)
        })
        .cloned()
        .collect()
}

/// Include ancestors of matched beads when text search is active and include_hierarchy is true.
/// Ensures tree context is preserved.
fn include_hierarchy(
    matched_beads: Vec<Bead>,
    all_beads: &[Bead],
    filter_text: &str,
    include_hierarchy_flag: bool,
) -> Vec<Bead> {
    if !include_hierarchy_flag || filter_text.is_empty() {
        return matched_beads;
    }

    // Build a map of all beads for quick lookup (not currently used but may be needed for optimization)
    let _bead_map: HashMap<String, &Bead> = all_beads.iter().map(|b| (b.id.clone(), b)).collect();

    // Build parent map from dependencies
    let mut parent_map: HashMap<String, String> = HashMap::new();
    for bead in all_beads {
        for dep in &bead.dependencies {
            if dep.r#type == "parent-child" {
                parent_map.insert(bead.id.clone(), dep.depends_on_id.clone());
            }
        }
    }

    let mut included_ids: HashSet<String> = HashSet::new();

    // Recursive function to add a bead and its ancestors
    fn add_with_ancestors(
        bead_id: &str,
        parent_map: &HashMap<String, String>,
        included_ids: &mut HashSet<String>,
    ) {
        if included_ids.contains(bead_id) {
            return;
        }

        included_ids.insert(bead_id.to_string());

        // Recursively add parent
        if let Some(parent_id) = parent_map.get(bead_id) {
            add_with_ancestors(parent_id, parent_map, included_ids);
        }
    }

    // Add matched beads and their ancestors
    for bead in &matched_beads {
        add_with_ancestors(&bead.id, &parent_map, &mut included_ids);
    }

    // Return all beads that are in included_ids
    all_beads
        .iter()
        .filter(|b| included_ids.contains(&b.id))
        .cloned()
        .collect()
}

/// Calculate state distribution (open/inProgress/blocked/closed counts) across grid cell buckets.
/// Used for Gantt header visualization. Each bucket = 1 grid cell.
fn calculate_state_distribution_from_tree(
    tree: &[BeadNode],
) -> Vec<BucketDistribution> {
    // Flatten tree to get all nodes
    fn flatten(nodes: &[BeadNode], acc: &mut Vec<BeadNode>) {
        for node in nodes {
            acc.push(node.clone());
            if !node.children.is_empty() {
                flatten(&node.children, acc);
            }
        }
    }

    let mut all_nodes = Vec::new();
    flatten(tree, &mut all_nodes);

    if all_nodes.is_empty() {
        return Vec::new();
    }

    // Find the maximum cell offset + count to determine number of buckets
    let max_cell = all_nodes
        .iter()
        .map(|node| node.cell_offset + node.cell_count)
        .max()
        .unwrap_or(1);

    let num_buckets = max_cell.max(1);

    let mut buckets: Vec<BucketDistribution> = (0..num_buckets)
        .map(|_| BucketDistribution {
            open: 0,
            in_progress: 0,
            blocked: 0,
            closed: 0,
        })
        .collect();

    // Count beads in each bucket by status
    // Exclude epics and features (tasks only)
    for node in &all_nodes {
        if node.issue_type == "epic" || node.issue_type == "feature" {
            continue;
        }

        let start_bucket = node.cell_offset;
        let end_bucket = node.cell_offset + node.cell_count - 1;

        // Handle bead overlap across buckets
        for bucket_idx in start_bucket..=end_bucket.min(num_buckets - 1) {
            match node.status.as_str() {
                "open" => buckets[bucket_idx].open += 1,
                "in_progress" => buckets[bucket_idx].in_progress += 1,
                "closed" => buckets[bucket_idx].closed += 1,
                _ => {}
            }

            // Count blocked beads
            if node.is_blocked {
                buckets[bucket_idx].blocked += 1;
            }
        }
    }

    buckets
}

// ============================================================================
// WBS Tree Building - Build Tree Structure (bp6-07y.2.3)
// ============================================================================

/// Build hierarchical WBS tree from flat bead list using parent-child dependencies.
/// Groups beads into root nodes and nested children recursively to support deep hierarchies.
fn build_wbs_tree(beads: &[Bead]) -> Vec<WBSNode> {
    let bead_map: HashMap<String, &Bead> = beads.iter().map(|b| (b.id.clone(), b)).collect();
    
    // Map of parent_id -> list of child_ids (ordered by appearance in beads list)
    let mut parent_to_children: HashMap<String, Vec<String>> = HashMap::new();
    let mut root_ids: Vec<String> = Vec::new();

    // 1. Identify roots and parent-child relationships
    for bead in beads {
        let parent_dep = bead.dependencies.iter().find(|d| d.r#type == "parent-child");
        if let Some(dep) = parent_dep {
            parent_to_children
                .entry(dep.depends_on_id.clone())
                .or_default()
                .push(bead.id.clone());
        } else {
            root_ids.push(bead.id.clone());
        }
    }

    // 2. Pre-calculate blocked status for efficiency
    let status_map: HashMap<String, String> = beads.iter().map(|b| (b.id.clone(), b.status.clone())).collect();
    let mut blocked_map: HashMap<String, bool> = HashMap::new();
    
    for bead in beads {
        let is_blocked = bead.dependencies.iter()
            .filter(|d| d.r#type == "blocks")
            .any(|d| {
                status_map.get(&d.depends_on_id)
                    .map(|status| status != "closed" && status != "done")
                    .unwrap_or(false)
            });
        blocked_map.insert(bead.id.clone(), is_blocked);
    }

    // 3. Recursive builder function
    fn build_node_recursive(
        id: &str,
        bead_map: &HashMap<String, &Bead>,
        parent_to_children: &HashMap<String, Vec<String>>,
        blocked_map: &HashMap<String, bool>,
    ) -> WBSNode {
        let bead = bead_map.get(id).expect("Bead ID missing from map");
        let mut children = Vec::new();
        
        if let Some(child_ids) = parent_to_children.get(id) {
            for child_id in child_ids {
                children.push(build_node_recursive(child_id, bead_map, parent_to_children, blocked_map));
            }
        }

        WBSNode {
            bead: (*bead).clone(),
            children,
            is_expanded: true,
            is_blocked: *blocked_map.get(id).unwrap_or(&false),
            is_critical: false,
        }
    }

    // 4. Build the tree starting from roots
    root_ids.into_iter()
        .map(|id| build_node_recursive(&id, &bead_map, &parent_to_children, &blocked_map))
        .collect()
}

// ============================================================================
// Gantt Layout Calculation - Earliest Start Times (bp6-07y.3.1)
// ============================================================================

/// Calculate earliest start time (X position) for each bead based on blocking dependencies.
/// Uses memoization to avoid recomputation.
fn calculate_earliest_start_times(
    beads: &[Bead],
    blocks_map: &HashMap<String, Vec<String>>,
) -> HashMap<String, usize> {
    let mut x_map: HashMap<String, usize> = HashMap::new();

    fn get_x(
        id: &str,
        blocks_map: &HashMap<String, Vec<String>>,
        x_map: &mut HashMap<String, usize>,
        visited: &mut HashSet<String>,
    ) -> usize {
        // Return memoized result if available
        if let Some(&x) = x_map.get(id) {
            return x;
        }

        // Detect circular dependencies
        if visited.contains(id) {
            return 0;
        }

        visited.insert(id.to_string());

        // Get predecessors (beads that block this one)
        let preds = blocks_map.get(id).cloned().unwrap_or_default();

        if preds.is_empty() {
            // No blockers, start at x=0
            x_map.insert(id.to_string(), 0);
            return 0;
        }

        // Calculate x as max(predecessor x values) + 1
        let max_pred_x = preds
            .iter()
            .map(|p| get_x(p, blocks_map, x_map, &mut visited.clone()))
            .max()
            .unwrap_or(0);

        let x = max_pred_x + 1;
        x_map.insert(id.to_string(), x);
        x
    }

    // Calculate x position for all beads
    for bead in beads {
        let mut visited = HashSet::new();
        get_x(&bead.id, blocks_map, &mut x_map, &mut visited);
    }

    x_map
}

// ============================================================================
// Filter Parameters (bp6-07y.5.1)
// ============================================================================

/// FilterParams contains all filter and display parameters passed from frontend.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilterParams {
    #[serde(default)]
    pub filter_text: String,

    #[serde(default)]
    pub hide_closed: bool,

    #[serde(default)]
    pub closed_time_filter: ClosedTimeFilter,

    #[serde(default = "default_true")]
    pub include_hierarchy: bool,

    #[serde(default = "default_zoom")]
    pub zoom: f64,

    #[serde(default)]
    pub collapsed_ids: Vec<String>,

    #[serde(default)]
    pub sort_by: SortBy,

    #[serde(default)]
    pub sort_order: SortOrder,
}

fn default_true() -> bool {
    true
}

fn default_zoom() -> f64 {
    1.0
}

impl Default for FilterParams {
    fn default() -> Self {
        FilterParams {
            filter_text: String::new(),
            hide_closed: false,
            closed_time_filter: ClosedTimeFilter::All,
            include_hierarchy: true,
            zoom: 1.0,
            collapsed_ids: Vec::new(),
            sort_by: SortBy::None,
            sort_order: SortOrder::None,
        }
    }
}

impl Default for ClosedTimeFilter {
    fn default() -> Self {
        ClosedTimeFilter::All
    }
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::None
    }
}

impl Default for SortBy {
    fn default() -> Self {
        SortBy::None
    }
}

// ============================================================================
// WBS Tree Building - Sort Siblings by Dependencies (bp6-07y.2.4)
// ============================================================================

/// Recursively sort sibling nodes using topological sort or explicit property sort.
fn sort_wbs_tree_siblings(
    mut tree: Vec<WBSNode>,
    graph: &DependencyGraph,
    sort_by: &SortBy,
    sort_order: &SortOrder,
) -> Vec<WBSNode> {
    // If explicit sort is requested, use it
    if *sort_by != SortBy::None && *sort_order != SortOrder::None {
        tree.sort_by(|a, b| {
            let ord = match sort_by {
                SortBy::Priority => a.bead.priority.cmp(&b.bead.priority),
                SortBy::Title => a.bead.title.to_lowercase().cmp(&b.bead.title.to_lowercase()),
                SortBy::Type => a.bead.issue_type.cmp(&b.bead.issue_type),
                SortBy::Id => a.bead.id.cmp(&b.bead.id),
                SortBy::None => std::cmp::Ordering::Equal,
            };

            // Use ID as tie-breaker for stable sorting across runs
            let ord = if ord == std::cmp::Ordering::Equal {
                a.bead.id.cmp(&b.bead.id)
            } else {
                ord
            };

            if *sort_order == SortOrder::Desc {
                ord.reverse()
            } else {
                ord
            }
        });
    } else {
        // Fallback to topological sort based on dependencies
        let sibling_ids: HashSet<String> = tree.iter().map(|n| n.bead.id.clone()).collect();

        // If only one node, no sorting needed
        if tree.len() > 1 {
            tree = topological_sort(tree, &sibling_ids, graph);
        }
    }

    // Recursively sort children
    for node in &mut tree {
        if !node.children.is_empty() {
            node.children = sort_wbs_tree_siblings(
                node.children.clone(),
                graph,
                sort_by,
                sort_order,
            );
        }
    }

    tree
}

// ============================================================================
// Gantt Layout - Calculate Node Ranges (bp6-07y.3.2)
// ============================================================================

/// NodeRange represents the calculated position and width of a node in the Gantt chart.
#[derive(Debug, Clone)]
struct NodeRange {
    x: f64,
    width: f64,
}

/// Calculate position and width for each node in the tree.
/// All values are in logical time units (NOT pixels).
/// Leaf nodes: start at earliestStart, duration = 1 grid cell (10 time units) or estimate-based.
/// Parent nodes: span from earliest child start to latest child end (rollup).
fn calculate_node_ranges(
    tree: &[WBSNode],
    x_map: &HashMap<String, usize>,
    range_cache: &mut HashMap<String, NodeRange>,
) {
    fn calc_range(
        node: &WBSNode,
        x_map: &HashMap<String, usize>,
        range_cache: &mut HashMap<String, NodeRange>,
    ) -> NodeRange {
        // Return cached result if available
        if let Some(range) = range_cache.get(&node.bead.id) {
            return range.clone();
        }

        let range = if node.children.is_empty() {
            // Leaf node: position in logical time units
            let earliest_start = x_map.get(&node.bead.id).copied().unwrap_or(0) as f64;

            // Duration: default to 10 time units (1 grid cell), or use estimate
            // If estimate exists and is > 0, map it to time units (assume minutes, 1 time unit = 60 min)
            let duration = if let Some(est) = node.bead.estimate {
                if est > 0 {
                    (est as f64 / 60.0).max(10.0)  // Convert minutes to time units, min 10 units (1 grid cell)
                } else {
                    10.0  // Zero estimate = milestone, but still give it width for now
                }
            } else {
                10.0  // No estimate = 1 grid cell (10 time units)
            };

            NodeRange { x: earliest_start, width: duration }
        } else {
            // Parent node: spans children's ranges (rollup behavior)
            let child_ranges: Vec<NodeRange> = node
                .children
                .iter()
                .map(|child| calc_range(child, x_map, range_cache))
                .collect();

            if child_ranges.is_empty() {
                // Fallback if somehow no children (shouldn't happen)
                let earliest_start = x_map.get(&node.bead.id).copied().unwrap_or(0) as f64;
                NodeRange { x: earliest_start, width: 10.0 }
            } else {
                // Start at earliest child's start, end at latest child's end
                let min_x = child_ranges.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
                let max_x = child_ranges
                    .iter()
                    .map(|r| r.x + r.width)
                    .fold(f64::NEG_INFINITY, f64::max);

                NodeRange {
                    x: min_x,
                    width: max_x - min_x,
                }
            }
        };

        range_cache.insert(node.bead.id.clone(), range.clone());
        range
    }

    // Calculate ranges for all root nodes
    for node in tree {
        calc_range(node, x_map, range_cache);
    }
}

// ============================================================================
// Gantt Layout - Find Critical Path (bp6-07y.3.3)
// ============================================================================

/// Find critical path using longest path algorithm.
/// Returns a set of node IDs that are on the critical path.
fn find_critical_path(
    beads: &[Bead],
    successors_map: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    if beads.is_empty() {
        return HashSet::new();
    }

    let mut max_dist_map: HashMap<String, usize> = HashMap::new();
    let mut next_in_path: HashMap<String, String> = HashMap::new();

    /// Recursively find maximum distance to furthest successor.
    fn find_max_dist(
        id: &str,
        successors_map: &HashMap<String, Vec<String>>,
        max_dist_map: &mut HashMap<String, usize>,
        next_in_path: &mut HashMap<String, String>,
    ) -> usize {
        // Return memoized result if available
        if let Some(&dist) = max_dist_map.get(id) {
            return dist;
        }

        let succs = successors_map.get(id).cloned().unwrap_or_default();

        if succs.is_empty() {
            // No successors, distance is 0
            max_dist_map.insert(id.to_string(), 0);
            return 0;
        }

        let mut max_val = 0;
        let mut best_succ = String::new();

        for s in &succs {
            let d = find_max_dist(s, successors_map, max_dist_map, next_in_path);
            if d > max_val {
                max_val = d;
                best_succ = s.clone();
            }
        }

        let dist = max_val + 1;
        max_dist_map.insert(id.to_string(), dist);
        if !best_succ.is_empty() {
            next_in_path.insert(id.to_string(), best_succ);
        }

        dist
    }

    // Find the global maximum distance (start of critical path)
    let mut global_max = 0;
    let mut start_node = String::new();

    for bead in beads {
        let d = find_max_dist(
            &bead.id,
            successors_map,
            &mut max_dist_map,
            &mut next_in_path,
        );
        if d > global_max {
            global_max = d;
            start_node = bead.id.clone();
        }
    }

    // Reconstruct critical path
    let mut critical_path_nodes: HashSet<String> = HashSet::new();
    let mut curr = Some(start_node);

    while let Some(node_id) = curr {
        critical_path_nodes.insert(node_id.clone());
        curr = next_in_path.get(&node_id).cloned();
    }

    critical_path_nodes
}

// ============================================================================
// Gantt Layout - Generate Gantt Items and Connectors (bp6-07y.3.4)
// ============================================================================

/// Generate GanttItems and GanttConnectors from the WBS tree and computed data.
fn generate_gantt_layout(
    beads: &[Bead],
    tree: &[WBSNode],
    x_map: &HashMap<String, usize>,
    range_cache: &HashMap<String, NodeRange>,
    critical_path: &HashSet<String>,
    zoom: f64,
) -> GanttLayout {
    let mut items: Vec<GanttItem> = Vec::new();
    let mut connectors: Vec<GanttConnector> = Vec::new();

    // Flatten tree to get visible rows and depths
    let mut visible_rows: Vec<String> = Vec::new();
    let mut row_depths: Vec<usize> = Vec::new();

    fn flatten_tree(
        nodes: &[WBSNode],
        depth: usize,
        visible_rows: &mut Vec<String>,
        row_depths: &mut Vec<usize>,
    ) {
        for node in nodes {
            visible_rows.push(node.bead.id.clone());
            row_depths.push(depth);
            if node.is_expanded {
                flatten_tree(&node.children, depth + 1, visible_rows, row_depths);
            }
        }
    }

    flatten_tree(tree, 0, &mut visible_rows, &mut row_depths);

    let row_count = visible_rows.len();

    // Create row map for quick lookup
    let row_map: HashMap<String, usize> = visible_rows
        .iter()
        .enumerate()
        .map(|(idx, id)| (id.clone(), idx))
        .collect();

    // Create depth map
    let depth_map: HashMap<String, usize> = visible_rows
        .iter()
        .zip(row_depths.iter())
        .map(|(id, &depth)| (id.clone(), depth))
        .collect();

    // Helper to check if a bead is blocked
    let is_blocked = |bead: &Bead| -> bool {
        bead.dependencies
            .iter()
            .filter(|d| d.r#type == "blocks")
            .any(|d| {
                beads
                    .iter()
                    .find(|b| b.id == d.depends_on_id)
                    .map(|pred| pred.status != "closed")
                    .unwrap_or(false)
            })
    };

    // Generate GanttItems
    for bead in beads {
        let row = match row_map.get(&bead.id) {
            Some(&r) => r,
            None => continue, // Bead not visible in tree
        };

        // Get range from cache or calculate fallback
        let range = range_cache.get(&bead.id).cloned().unwrap_or_else(|| {
            let earliest_start = x_map.get(&bead.id).copied().unwrap_or(0);
            let x = (earliest_start * 100 + 40) as f64;
            let estimate = bead.estimate.unwrap_or(600);
            let width = (estimate as f64 / 10.0).max(40.0);
            NodeRange { x, width }
        });

        // Apply zoom factor
        let x = range.x * zoom;
        let width = range.width * zoom;

        items.push(GanttItem {
            bead: bead.clone(),
            x,
            width,
            row,
            depth: *depth_map.get(&bead.id).unwrap_or(&0),
            is_critical: critical_path.contains(&bead.id),
            is_blocked: is_blocked(bead),
        });
    }

    // Generate GanttConnectors
    for bead in beads {
        let row = match row_map.get(&bead.id) {
            Some(&r) => r,
            None => continue,
        };

        let range = range_cache.get(&bead.id).cloned().unwrap_or_else(|| {
            let earliest_start = x_map.get(&bead.id).copied().unwrap_or(0);
            let x = (earliest_start * 100 + 40) as f64;
            let estimate = bead.estimate.unwrap_or(600);
            let _width = (estimate as f64 / 10.0).max(40.0);
            NodeRange { x, width: _width }
        });

        let x = range.x * zoom;
        let _width = range.width * zoom;

        // Create connectors for blocking dependencies
        for dep in &bead.dependencies {
            if dep.r#type != "blocks" {
                continue;
            }

            let pred_id = &dep.depends_on_id;
            let pred_row = match row_map.get(pred_id) {
                Some(&r) => r,
                None => continue,
            };

            let pred_range = range_cache.get(pred_id).cloned().unwrap_or_else(|| {
                let earliest_start = x_map.get(pred_id).copied().unwrap_or(0);
                let x = (earliest_start * 100 + 40) as f64;
                let pred_bead = beads.iter().find(|b| &b.id == pred_id);
                let estimate = pred_bead.and_then(|b| b.estimate).unwrap_or(600);
                let width = (estimate as f64 / 10.0).max(40.0);
                NodeRange { x, width }
            });

            let pred_x = pred_range.x * zoom;
            let pred_width = pred_range.width * zoom;

            connectors.push(GanttConnector {
                from: Point {
                    x: pred_x + pred_width,
                    y: (pred_row * 48 + 24) as f64,
                },
                to: Point {
                    x,
                    y: (row * 48 + 24) as f64,
                },
                is_critical: critical_path.contains(&bead.id) && critical_path.contains(pred_id),
            });
        }
    }

    GanttLayout {
        items,
        connectors,
        row_count,
        row_depths,
    }
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
    eprintln!("📂 open_project: Attempting to set current_dir to: {}", path);
    std::env::set_current_dir(&path).map_err(|e| format!("Failed to change directory to {}: {}", path, e))?;

    let new_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    eprintln!("✅ open_project: Successfully set current_dir to: {}", new_dir.display());

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

    // Clear cached beads file path when switching projects
    {
        let mut cache = BEADS_FILE_PATH_CACHE.lock().unwrap();
        *cache = None;
        eprintln!("🗑️  Cleared beads file path cache for new project");
    }

    // Update watcher to monitor new project's beads file
    if let Some(new_beads_path) = find_beads_file() {
        let watcher_state = app_handle.state::<Arc<Mutex<BeadsWatcher>>>();
        let mut watcher = watcher_state.lock().unwrap();
        watcher.watch_beads_file(new_beads_path)?;
    }

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
            get_beads, get_processed_data, get_project_view_model, update_bead, create_bead, close_bead, reopen_bead, claim_bead,
            get_projects, add_project, remove_project, open_project, toggle_favorite,
            get_current_dir
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let proj_handle = app.handle().clone();

            // Initialize file watcher (lazy - will watch when first project is opened)
            let beads_watcher = BeadsWatcher::new(handle.clone())
                .expect("Failed to create beads watcher");

            // Store watcher in managed state (but don't watch anything yet)
            app.manage(Arc::new(Mutex::new(beads_watcher)));

            // Watch projects file with debouncing
            if let Ok(proj_path) = get_projects_path() {
                let proj_last_emit = Arc::new(Mutex::new(Instant::now()));

                let mut proj_watcher = notify::RecommendedWatcher::new(move |res: std::result::Result<notify::Event, notify::Error>| {
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
