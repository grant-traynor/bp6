use super::*;
use tauri::State;

pub type Registry = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<ProjectDb>>>>;

#[tauri::command]
pub async fn open_project(
    path: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Project, String> {
    let project_path = std::path::Path::new(&path);
    let (project, conn) = open_project_db(project_path).map_err(|e| e.to_string())?;

    let db = std::sync::Arc::new(ProjectDb {
        project: project.clone(),
        conn: Mutex::new(conn),
    });

    registry.lock().unwrap().insert(project.path.clone(), db);
    Ok(project)
}

#[tauri::command]
pub async fn close_project(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    let mut reg = registry.lock().unwrap();
    reg.retain(|_, db| db.project.id != project_id);
    Ok(())
}

#[tauri::command]
pub async fn list_projects(
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Project>, String> {
    let reg = registry.lock().unwrap();
    Ok(reg.values().map(|db| db.project.clone()).collect())
}

fn with_conn<F, T>(registry: &State<'_, ProjectRegistry>, project_id: &str, f: F) -> Result<T, String>
where
    F: FnOnce(&Connection) -> anyhow::Result<T>,
{
    let reg = registry.lock().unwrap();
    let db = reg
        .values()
        .find(|db| db.project.id == project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?
        .clone();
    drop(reg);
    let conn = db.conn.lock().unwrap();
    f(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_node(
    input: CreateNodeInput,
    registry: State<'_, ProjectRegistry>,
) -> Result<Node, String> {
    let project_id = input.project_id.clone();
    with_conn(&registry, &project_id, |conn| db_create_node(conn, &input))
}

#[tauri::command]
pub async fn update_node(
    node_id: String,
    project_id: String,
    input: UpdateNodeInput,
    registry: State<'_, ProjectRegistry>,
) -> Result<Node, String> {
    with_conn(&registry, &project_id, |conn| db_update_node(conn, &node_id, &input))
}

#[tauri::command]
pub async fn cancel_node(
    node_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Node, String> {
    with_conn(&registry, &project_id, |conn| db_cancel_node(conn, &node_id))
}

#[tauri::command]
pub async fn get_node(
    node_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Node, String> {
    with_conn(&registry, &project_id, |conn| db_get_node(conn, &node_id))
}

#[tauri::command]
pub async fn list_nodes(
    project_id: String,
    phase_id: Option<String>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Node>, String> {
    with_conn(&registry, &project_id, |conn| {
        db_list_nodes(conn, &project_id, phase_id.as_deref())
    })
}

#[tauri::command]
pub async fn create_edge(
    from_id: String,
    to_id: String,
    project_id: String,
    edge_type: Option<String>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Edge, String> {
    let et = edge_type
        .as_deref()
        .unwrap_or("depends_on")
        .parse::<EdgeType>()
        .map_err(|e| e.to_string())?;
    with_conn(&registry, &project_id, |conn| db_create_edge(conn, &from_id, &to_id, et))
}

#[tauri::command]
pub async fn remove_edge(
    from_id: String,
    to_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    with_conn(&registry, &project_id, |conn| db_remove_edge(conn, &from_id, &to_id))
}

#[tauri::command]
pub async fn create_artifact(
    input: CreateArtifactInput,
    registry: State<'_, ProjectRegistry>,
) -> Result<Artifact, String> {
    let project_id = input.project_id.clone();
    with_conn(&registry, &project_id, |conn| db_upsert_artifact(conn, &input))
}

#[tauri::command]
pub async fn list_artifacts(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Artifact>, String> {
    with_conn(&registry, &project_id, |conn| db_list_artifacts(conn, &project_id))
}

#[tauri::command]
pub async fn create_knowledge(
    input: CreateKnowledgeInput,
    registry: State<'_, ProjectRegistry>,
) -> Result<KnowledgeEntry, String> {
    let project_id = input.project_id.clone();
    with_conn(&registry, &project_id, |conn| db_create_knowledge(conn, &input))
}

#[tauri::command]
pub async fn list_knowledge(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<KnowledgeEntry>, String> {
    with_conn(&registry, &project_id, |conn| db_list_knowledge(conn, &project_id))
}

#[tauri::command]
pub async fn list_queue_items(
    project_id: String,
    unresolved_only: Option<bool>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<QueueItem>, String> {
    let unresolved = unresolved_only.unwrap_or(false);
    with_conn(&registry, &project_id, |conn| {
        db_list_queue_items(conn, &project_id, unresolved)
    })
}

#[tauri::command]
pub async fn resolve_queue_item(
    item_id: String,
    project_id: String,
    resolution: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<QueueItem, String> {
    with_conn(&registry, &project_id, |conn| db_resolve_queue_item(conn, &item_id, &resolution))
}

#[tauri::command]
pub async fn list_events(
    project_id: String,
    since: Option<String>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<EventRecord>, String> {
    with_conn(&registry, &project_id, |conn| {
        db_list_events(conn, &project_id, since.as_deref())
    })
}
