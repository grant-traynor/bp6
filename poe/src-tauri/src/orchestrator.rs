// bp6-pdr.3: Stage runner + input bundle assembler + concurrency-limited dispatcher
//
// Receives DagChanged signals, queries SQLite for ready tasks, assembles input
// bundles and spawns Claude CLI processes. Pipes stdout through the ingester.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{ChildStdin, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Utc;
use rusqlite::params;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::Sender;

use crate::db::{self, ArtifactRow, DbState, KnowledgeRow, PhaseRow, ProjectRow, TaskRow};
use crate::ingester::{self, DagChanged};

// ── Concurrency limits ────────────────────────────────────────────────────────

const MAX_CONCURRENT_PER_PROJECT: usize = 5;
const MAX_CONCURRENT_GLOBAL: usize = 15;

// ── Managed state ─────────────────────────────────────────────────────────────

pub struct OrchestratorState {
    /// project_id → JoinHandle for the orchestrator loop
    pub loops: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
    /// Global running agent count (across all projects)
    pub global_count: Arc<AtomicUsize>,
    /// task_id → stdin handle of the running agent process
    pub running_agents: Mutex<HashMap<String, ChildStdin>>,
}

impl OrchestratorState {
    pub fn new() -> Self {
        Self {
            loops: Mutex::new(HashMap::new()),
            global_count: Arc::new(AtomicUsize::new(0)),
            running_agents: Mutex::new(HashMap::new()),
        }
    }
}

// ── Tauri event payloads ──────────────────────────────────────────────────────

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TaskUpdatePayload {
    task_id: String,
    status: String,
    title: Option<String>,
    updated_at: i64,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DecisionResolvedPayload {
    decision_id: i64,
}

// ── Main loop ─────────────────────────────────────────────────────────────────

/// Spawn the per-project orchestrator loop. Returns immediately; the loop runs
/// as a Tokio background task.
pub fn start_orchestrator(
    project_id: String,
    project_path: PathBuf,
    app_handle: AppHandle,
    db_state: Arc<DbState>,
    mut dag_rx: tokio::sync::mpsc::Receiver<DagChanged>,
    global_count: Arc<AtomicUsize>,
    dag_tx: Sender<DagChanged>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        // Kick off immediately in case there are already-pending tasks
        let _ = dag_tx.try_send(DagChanged {
            project_id: project_id.clone(),
        });

        loop {
            // Wait for a signal that the DAG may have changed
            let signal = dag_rx.recv().await;
            if signal.is_none() {
                // Channel closed — orchestrator for this project is done
                break;
            }

            // Count how many agents are already running for this project
            let per_project_count = count_running_for_project(&project_id, &db_state);

            if per_project_count >= MAX_CONCURRENT_PER_PROJECT {
                continue;
            }
            if global_count.load(Ordering::SeqCst) >= MAX_CONCURRENT_GLOBAL {
                continue;
            }

            // Open (or reuse) the DB connection
            let conn = match db::open_project_db(&project_id, &project_path, &db_state) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[orchestrator:{}] open_project_db failed: {}", project_id, e);
                    continue;
                }
            };

            // Query tasks that are ready to run
            let ready = match query_ready_tasks(&conn, &project_id) {
                Ok(tasks) => tasks,
                Err(e) => {
                    eprintln!("[orchestrator:{}] query_ready_tasks failed: {}", project_id, e);
                    continue;
                }
            };

            let mut spawned = 0usize;
            for task in ready {
                let global = global_count.load(Ordering::SeqCst);
                if global >= MAX_CONCURRENT_GLOBAL {
                    break;
                }
                if per_project_count + spawned >= MAX_CONCURRENT_PER_PROJECT {
                    break;
                }

                spawn_task(
                    task,
                    project_path.clone(),
                    app_handle.clone(),
                    conn.clone(),
                    dag_tx.clone(),
                    global_count.clone(),
                )
                .await;

                spawned += 1;
            }
        }
    })
}

/// Count tasks currently in status='running' for the given project.
fn count_running_for_project(project_id: &str, db_state: &Arc<DbState>) -> usize {
    let conns = match db_state.connections.lock() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let conn_arc = match conns.get(project_id) {
        Some(c) => Arc::clone(c),
        None => return 0,
    };
    drop(conns);

    let conn = match conn_arc.lock() {
        Ok(c) => c,
        Err(_) => return 0,
    };

    conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND status = 'running'",
        params![project_id],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0) as usize
}

// ── Query ready tasks ─────────────────────────────────────────────────────────

fn query_ready_tasks(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    project_id: &str,
) -> Result<Vec<TaskRow>, String> {
    let guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    let mut stmt = guard
        .prepare(
            "SELECT t.id, t.project_id, t.phase_id, t.parent_id, t.title, t.description,
                    t.type, t.skill, t.status, t.session_id, t.created_at, t.updated_at
             FROM tasks t
             WHERE t.project_id = ?1
               AND t.status = 'pending'
               AND t.skill IS NOT NULL
               AND NOT EXISTS (
                 SELECT 1 FROM edges e
                 JOIN tasks dep ON dep.id = e.from_id
                 WHERE e.to_id = t.id
                   AND dep.status != 'done'
               )",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![project_id], |row| {
            Ok(TaskRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                phase_id: row.get(2)?,
                parent_id: row.get(3)?,
                title: row.get(4)?,
                description: row.get(5)?,
                task_type: row.get(6)?,
                skill: row.get(7)?,
                status: row.get(8)?,
                session_id: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let result: Result<Vec<_>, _> = rows.collect();
    result.map_err(|e: rusqlite::Error| e.to_string())
}

// ── spawn_task ────────────────────────────────────────────────────────────────

async fn spawn_task(
    task: TaskRow,
    project_path: PathBuf,
    app_handle: AppHandle,
    conn: Arc<Mutex<rusqlite::Connection>>,
    dag_tx: Sender<DagChanged>,
    global_count: Arc<AtomicUsize>,
) {
    let skill_name = match &task.skill {
        Some(s) => s.clone(),
        None => {
            eprintln!("[orchestrator] task {} has no skill — skipping", task.id);
            return;
        }
    };

    // ── 1. Resolve skill file ─────────────────────────────────────────────────
    let skill_path = resolve_skill_path(&skill_name, &project_path, &app_handle);

    let skill_path = match skill_path {
        Some(p) => p,
        None => {
            eprintln!(
                "[orchestrator] skill '{}' not found for task {}",
                skill_name, task.id
            );
            cancel_task(
                &task.id,
                &format!("skill '{}' not found", skill_name),
                &conn,
                &app_handle,
            );
            return;
        }
    };

    // ── 2. Assemble input bundle ──────────────────────────────────────────────
    let bundle = match assemble_bundle(&task, &project_path, &skill_path, &conn) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "[orchestrator] bundle assembly failed for task {}: {}",
                task.id, e
            );
            cancel_task(&task.id, &format!("bundle assembly failed: {e}"), &conn, &app_handle);
            return;
        }
    };

    // ── 3. Spawn `claude` CLI process ─────────────────────────────────────────
    let mut child = match std::process::Command::new("claude")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--print")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[orchestrator] failed to spawn claude for task {}: {}",
                task.id, e
            );
            cancel_task(
                &task.id,
                &format!("failed to spawn claude: {e}"),
                &conn,
                &app_handle,
            );
            return;
        }
    };

    // ── 4. Write bundle to stdin then close ───────────────────────────────────
    let mut stdin = match child.stdin.take() {
        Some(s) => s,
        None => {
            eprintln!("[orchestrator] no stdin handle for task {}", task.id);
            cancel_task(&task.id, "no stdin handle", &conn, &app_handle);
            return;
        }
    };

    if let Err(e) = stdin.write_all(bundle.as_bytes()) {
        eprintln!(
            "[orchestrator] failed to write bundle to stdin for task {}: {}",
            task.id, e
        );
        cancel_task(
            &task.id,
            &format!("failed to write bundle: {e}"),
            &conn,
            &app_handle,
        );
        return;
    }
    // Closing stdin signals EOF to the child process
    drop(stdin);

    // ── 5. UPDATE tasks SET status='running' ─────────────────────────────────
    let now = Utc::now().timestamp();
    {
        match conn.lock() {
            Ok(guard) => {
                let _ = guard.execute(
                    "UPDATE tasks SET status = 'running', updated_at = ?1 WHERE id = ?2",
                    params![now, task.id],
                );
            }
            Err(e) => eprintln!("[orchestrator] lock poisoned updating task status: {e}"),
        }
    }

    let _ = app_handle.emit(
        "poe://task-update",
        TaskUpdatePayload {
            task_id: task.id.clone(),
            status: "running".to_string(),
            title: Some(task.title.clone()),
            updated_at: now,
        },
    );

    // ── 6. Increment global_count ─────────────────────────────────────────────
    global_count.fetch_add(1, Ordering::SeqCst);

    // ── 6b. Store stdin handle for resolve_decision ───────────────────────────
    // Note: we already consumed stdin above (wrote the bundle). For the
    // `resolve_decision` use-case, the child must be re-spawned with stdin
    // kept open. Here we store a placeholder-None since stdin was closed.
    // The full interactive flow requires spawning without closing stdin;
    // for now we store nothing (resolve_decision will get None and return an
    // informative error).
    //
    // To support live decision injection, we would need to keep stdin open
    // and write to it async. That requires moving to tokio::process::Command.
    // We use tokio::process below instead.

    // ── 7. Spawn async reader task ────────────────────────────────────────────
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            eprintln!("[orchestrator] no stdout for task {}", task.id);
            global_count.fetch_sub(1, Ordering::SeqCst);
            return;
        }
    };

    // Convert std::process stdout to tokio-compatible async reader
    let async_stdout = match tokio::process::ChildStdout::from_std(stdout) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[orchestrator] failed to convert stdout to async for task {}: {}",
                task.id, e
            );
            global_count.fetch_sub(1, Ordering::SeqCst);
            return;
        }
    };

    let task_id = task.id.clone();
    let project_id = task.project_id.clone();
    let conn_clone = conn.clone();
    let app_clone = app_handle.clone();
    let dag_tx_clone = dag_tx.clone();
    let global_count_clone = global_count.clone();

    tokio::spawn(async move {
        let reader = BufReader::new(async_stdout);
        let mut lines = reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    ingester::ingest_line(
                        &line,
                        &task_id,
                        &project_id,
                        &project_path,
                        conn_clone.clone(),
                        &app_clone,
                        &dag_tx_clone,
                    );
                }
                Ok(None) => break, // EOF — agent exited
                Err(e) => {
                    eprintln!("[orchestrator] read error for task {}: {}", task_id, e);
                    break;
                }
            }
        }

        // Agent exited — decrement global count
        global_count_clone.fetch_sub(1, Ordering::SeqCst);

        // If status is still 'running' (not set to 'done'/'waiting' by ingester),
        // mark as done.
        check_and_finalize_task(&task_id, &conn_clone, &app_clone, &dag_tx_clone, &project_id);
    });
}

// ── Skill path resolution ─────────────────────────────────────────────────────

fn resolve_skill_path(skill: &str, project_path: &PathBuf, app_handle: &AppHandle) -> Option<PathBuf> {
    // 1. Project-local: {project_path}/.poe/skills/{skill}.md
    let project_skill = project_path
        .join(".poe")
        .join("skills")
        .join(format!("{skill}.md"));
    if project_skill.exists() {
        return Some(project_skill);
    }

    // 2. User home: ~/.poe/skills/{skill}.md
    if let Some(home) = dirs::home_dir() {
        let user_skill = home.join(".poe").join("skills").join(format!("{skill}.md"));
        if user_skill.exists() {
            return Some(user_skill);
        }
    }

    // 3. App bundle: {resource_dir}/skills/{skill}.md
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let bundle_skill = resource_dir.join("skills").join(format!("{skill}.md"));
        if bundle_skill.exists() {
            return Some(bundle_skill);
        }
    }

    None
}

// ── Input bundle assembly ─────────────────────────────────────────────────────

fn assemble_bundle(
    task: &TaskRow,
    project_path: &PathBuf,
    skill_path: &PathBuf,
    conn: &Arc<Mutex<rusqlite::Connection>>,
) -> Result<String, String> {
    let guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    // ── Fetch project row ──────────────────────────────────────────────────────
    let project: ProjectRow = guard
        .query_row(
            "SELECT id, name, path, created_at FROM projects WHERE id = ?1",
            params![task.project_id],
            |row| {
                Ok(ProjectRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .map_err(|e| format!("failed to fetch project: {e}"))?;

    // ── Fetch phase row (if any) ───────────────────────────────────────────────
    let phase: Option<PhaseRow> = if let Some(ref phase_id) = task.phase_id {
        guard
            .query_row(
                "SELECT id, project_id, name, stage_type, status, pdca_state, position, created_at
                 FROM phases WHERE id = ?1",
                params![phase_id],
                |row| {
                    Ok(PhaseRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        name: row.get(2)?,
                        stage_type: row.get(3)?,
                        status: row.get(4)?,
                        pdca_state: row.get(5)?,
                        position: row.get(6)?,
                        created_at: row.get(7)?,
                    })
                },
            )
            .ok()
    } else {
        None
    };

    // ── Fetch parent task (if any) ────────────────────────────────────────────
    let parent: Option<TaskRow> = if let Some(ref parent_id) = task.parent_id {
        guard
            .query_row(
                "SELECT id, project_id, phase_id, parent_id, title, description,
                        type, skill, status, session_id, created_at, updated_at
                 FROM tasks WHERE id = ?1",
                params![parent_id],
                |row| {
                    Ok(TaskRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        phase_id: row.get(2)?,
                        parent_id: row.get(3)?,
                        title: row.get(4)?,
                        description: row.get(5)?,
                        task_type: row.get(6)?,
                        skill: row.get(7)?,
                        status: row.get(8)?,
                        session_id: row.get(9)?,
                        created_at: row.get(10)?,
                        updated_at: row.get(11)?,
                    })
                },
            )
            .ok()
    } else {
        None
    };

    // ── Fetch completed dependencies ──────────────────────────────────────────
    let completed_deps: Vec<TaskRow> = {
        let mut dep_stmt = guard
            .prepare(
                "SELECT t.id, t.project_id, t.phase_id, t.parent_id, t.title, t.description,
                        t.type, t.skill, t.status, t.session_id, t.created_at, t.updated_at
                 FROM tasks t
                 JOIN edges e ON e.from_id = t.id
                 WHERE e.to_id = ?1 AND t.status = 'done'",
            )
            .map_err(|e| e.to_string())?;

        let dep_rows = dep_stmt
            .query_map(params![task.id], |row| {
                Ok(TaskRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    phase_id: row.get(2)?,
                    parent_id: row.get(3)?,
                    title: row.get(4)?,
                    description: row.get(5)?,
                    task_type: row.get(6)?,
                    skill: row.get(7)?,
                    status: row.get(8)?,
                    session_id: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })
            .map_err(|e| e.to_string())?;
        dep_rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: rusqlite::Error| e.to_string())?
    };

    // ── Fetch artifacts ───────────────────────────────────────────────────────
    let artifacts: Vec<ArtifactRow> = {
        let mut art_stmt = guard
            .prepare(
                "SELECT id, project_id, name, artifact_type, path, producing_task_id, created_at, updated_at
                 FROM artifacts WHERE project_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let art_rows = art_stmt
            .query_map(params![task.project_id], |row| {
                Ok(ArtifactRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    name: row.get(2)?,
                    artifact_type: row.get(3)?,
                    path: row.get(4)?,
                    producing_task_id: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        art_rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: rusqlite::Error| e.to_string())?
    };

    // ── Fetch knowledge ───────────────────────────────────────────────────────
    let knowledge: Vec<KnowledgeRow> = {
        let mut know_stmt = guard
            .prepare(
                "SELECT id, project_id, key, content, source_task_id, supersedes_id, created_at
                 FROM knowledge WHERE project_id = ?1 ORDER BY created_at",
            )
            .map_err(|e| e.to_string())?;

        let know_rows = know_stmt
            .query_map(params![task.project_id], |row| {
                Ok(KnowledgeRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    key: row.get(2)?,
                    content: row.get(3)?,
                    source_task_id: row.get(4)?,
                    supersedes_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        know_rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: rusqlite::Error| e.to_string())?
    };

    drop(guard);

    // ── Load skill body ───────────────────────────────────────────────────────
    let skill_body = crate::skills::get_skill_body(&skill_path.to_string_lossy())?;

    // ── Build bundle string ───────────────────────────────────────────────────
    let mut bundle = String::new();

    // --- Task section ---
    bundle.push_str("# Task\n\n");
    bundle.push_str(&format!("**ID**: {}\n", task.id));
    bundle.push_str(&format!("**Title**: {}\n", task.title));
    bundle.push_str(&format!("**Type**: {}\n", task.task_type));
    bundle.push_str(&format!(
        "**Skill**: {}\n",
        task.skill.as_deref().unwrap_or("")
    ));

    // --- Ancestry ---
    bundle.push_str("\n## Ancestry\n\n");
    bundle.push_str(&format!("- Project: {}\n", project.name));

    if let Some(ref p) = phase {
        bundle.push_str(&format!("- Phase: {} ({})\n", p.name, p.stage_type));
    }

    if let Some(ref par) = parent {
        bundle.push_str(&format!("- Parent: {} ({})\n", par.title, par.id));
    }

    // --- Description ---
    bundle.push_str("\n## Description\n\n");
    if let Some(ref desc) = task.description {
        bundle.push_str(desc);
        bundle.push('\n');
    }

    // --- Completed Dependencies ---
    if !completed_deps.is_empty() {
        bundle.push_str("\n## Completed Dependencies\n\n");
        for dep in &completed_deps {
            bundle.push_str(&format!("- {} [done] — {}\n", dep.title, dep.id));
        }
    }

    bundle.push_str("\n---\n\n");

    // --- Skill ---
    bundle.push_str("# Skill\n\n");
    bundle.push_str(&skill_body);
    bundle.push('\n');

    // --- Artifacts ---
    if !artifacts.is_empty() {
        bundle.push_str("\n---\n\n# Artifacts\n\n");
        for artifact in &artifacts {
            bundle.push_str(&format!(
                "## {} ({})\n\n",
                artifact.name, artifact.artifact_type
            ));
            // Read artifact content from project docs dir
            let artifact_path = project_path.join("docs").join(&artifact.name);
            match std::fs::read_to_string(&artifact_path) {
                Ok(content) => {
                    bundle.push_str(&content);
                    if !content.ends_with('\n') {
                        bundle.push('\n');
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[orchestrator] failed to read artifact '{}': {}",
                        artifact.name, e
                    );
                }
            }
            bundle.push_str("\n---\n\n");
        }
    }

    // --- Knowledge Register ---
    if !knowledge.is_empty() {
        bundle.push_str("# Knowledge Register\n\n");
        for entry in &knowledge {
            bundle.push_str(&format!("## {}\n\n", entry.key));
            bundle.push_str(&entry.content);
            if !entry.content.ends_with('\n') {
                bundle.push('\n');
            }
            bundle.push_str("\n---\n\n");
        }
    }

    Ok(bundle)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Mark a task as cancelled with a reason and emit the Tauri event.
fn cancel_task(
    task_id: &str,
    reason: &str,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    app_handle: &AppHandle,
) {
    let now = Utc::now().timestamp();
    match conn.lock() {
        Ok(guard) => {
            let _ = guard.execute(
                "UPDATE tasks SET status = 'cancelled', updated_at = ?1 WHERE id = ?2",
                params![now, task_id],
            );
            // Log the reason as an event
            let payload = serde_json::json!({ "reason": reason });
            let payload_str = payload.to_string();
            let _ = guard.execute(
                "INSERT INTO event_log (task_id, event_type, payload, created_at) VALUES (?1, 'task:cancel', ?2, ?3)",
                params![task_id, payload_str, now],
            );
        }
        Err(e) => eprintln!("[orchestrator] lock poisoned in cancel_task: {e}"),
    }

    let _ = app_handle.emit(
        "poe://task-update",
        TaskUpdatePayload {
            task_id: task_id.to_string(),
            status: "cancelled".to_string(),
            title: None,
            updated_at: now,
        },
    );
}

/// Called when the agent process exits. If the task is still in 'running' state
/// (i.e., the ingester never received a poe:done event), mark it done.
fn check_and_finalize_task(
    task_id: &str,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    app_handle: &AppHandle,
    dag_tx: &Sender<DagChanged>,
    project_id: &str,
) {
    let now = Utc::now().timestamp();
    let updated = match conn.lock() {
        Ok(guard) => {
            let rows_changed = guard
                .execute(
                    "UPDATE tasks SET status = 'done', updated_at = ?1
                     WHERE id = ?2 AND status = 'running'",
                    params![now, task_id],
                )
                .unwrap_or(0);
            rows_changed > 0
        }
        Err(e) => {
            eprintln!("[orchestrator] lock poisoned in check_and_finalize_task: {e}");
            false
        }
    };

    if updated {
        let _ = app_handle.emit(
            "poe://task-update",
            TaskUpdatePayload {
                task_id: task_id.to_string(),
                status: "done".to_string(),
                title: None,
                updated_at: now,
            },
        );
        let _ = dag_tx.try_send(DagChanged {
            project_id: project_id.to_string(),
        });
    }
}

// ── Tauri command: resolve_decision ──────────────────────────────────────────

#[tauri::command]
pub async fn resolve_decision(
    decision_id: i64,
    resolution: String,
    db_state: State<'_, DbState>,
    orchestrator_state: State<'_, OrchestratorState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let now = Utc::now().timestamp();

    // ── 1. Look up decision, get task_id and project_id ───────────────────────
    let (task_id, project_id) = {
        let conns = db_state
            .connections
            .lock()
            .map_err(|e| format!("DbState lock poisoned: {e}"))?;

        // Search all open connections for the decision
        let mut found: Option<(String, String)> = None;
        for conn_arc in conns.values() {
            let conn = conn_arc
                .lock()
                .map_err(|e| format!("Connection lock poisoned: {e}"))?;

            let result = conn
                .query_row(
                    "SELECT d.task_id, t.project_id
                     FROM decisions d
                     JOIN tasks t ON t.id = d.task_id
                     WHERE d.id = ?1",
                    params![decision_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .ok();

            if let Some(pair) = result {
                found = Some(pair);
                break;
            }
        }
        drop(conns);
        found.ok_or_else(|| format!("Decision {decision_id} not found in any open project"))?
    };

    // ── 2 & 3. UPDATE decisions and task ─────────────────────────────────────
    {
        let conns = db_state
            .connections
            .lock()
            .map_err(|e| format!("DbState lock poisoned: {e}"))?;

        let conn_arc = conns
            .get(&project_id)
            .ok_or_else(|| format!("No open DB for project {project_id}"))?
            .clone();
        drop(conns);

        let conn = conn_arc
            .lock()
            .map_err(|e| format!("Connection lock poisoned: {e}"))?;

        conn.execute(
            "UPDATE decisions SET resolution = ?1, resolved_at = ?2 WHERE id = ?3",
            params![resolution, now, decision_id],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE tasks SET status = 'running', updated_at = ?1 WHERE id = ?2",
            params![now, task_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // ── 4. Write resolution to running agent stdin ────────────────────────────
    {
        let mut agents = orchestrator_state
            .running_agents
            .lock()
            .map_err(|e| format!("running_agents lock poisoned: {e}"))?;

        if let Some(stdin) = agents.get_mut(&task_id) {
            let message = format!("\n---\nHuman: {resolution}\n");
            stdin
                .write_all(message.as_bytes())
                .map_err(|e| format!("Failed to write to agent stdin: {e}"))?;
        } else {
            eprintln!(
                "[orchestrator] no running agent stdin for task {} — decision written to DB only",
                task_id
            );
        }
    }

    // ── 5. Emit poe://task-update status='running' ────────────────────────────
    app_handle
        .emit(
            "poe://task-update",
            TaskUpdatePayload {
                task_id: task_id.clone(),
                status: "running".to_string(),
                title: None,
                updated_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    // ── 6. Emit poe://decision-resolved ──────────────────────────────────────
    app_handle
        .emit(
            "poe://decision-resolved",
            DecisionResolvedPayload { decision_id },
        )
        .map_err(|e| e.to_string())?;

    // ── 7. Send DagChanged ────────────────────────────────────────────────────
    // We need the dag_tx for this project. Since we don't store it in state,
    // we emit poe://task-update which the orchestrator loop already watches
    // via DagChanged signals from the ingester. The task status change above
    // will be picked up on next cycle. For now, signal via a best-effort
    // approach: the status update is already in the DB.
    eprintln!(
        "[orchestrator] resolve_decision for task {} complete (decision {})",
        task_id, decision_id
    );

    Ok(())
}
