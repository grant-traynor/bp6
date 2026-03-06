use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ── Ports ─────────────────────────────────────────────────────────────────────

pub const RESTATE_ADMIN_PORT: u16 = 9070;
pub const RESTATE_SERVICES_PORT: u16 = 9080;

const HEALTH_TIMEOUT_SECS: u64 = 15;
const HEALTH_POLL_MS: u64 = 200;

// ── Managed state ─────────────────────────────────────────────────────────────

/// Tauri-managed state holding the Restate child process handle.
pub struct RestateState {
    pub child: Mutex<Option<Child>>,
}

impl RestateState {
    pub fn new() -> Self {
        RestateState {
            child: Mutex::new(None),
        }
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

/// Resolve the path to the restate-server binary.
/// Looks for it next to the executable, then in PATH.
fn restate_binary_path() -> Result<std::path::PathBuf, String> {
    // First: look alongside our own executable (bundled)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("restate-server");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // Second: fall back to PATH
    which_restate().ok_or_else(|| {
        "restate-server not found. Place the binary alongside the POE executable or in PATH.".to_string()
    })
}

fn which_restate() -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join("restate-server");
            if candidate.exists() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

/// Spawn the Restate server. Returns the child process.
pub fn spawn_restate(data_dir: &std::path::Path) -> Result<Child, String> {
    let binary = restate_binary_path()?;
    let base_dir = data_dir.join("restate");

    std::fs::create_dir_all(&base_dir)
        .map_err(|e| format!("Failed to create restate data dir: {}", e))?;

    let child = Command::new(&binary)
        .args([
            "--bind-address",
            &format!("0.0.0.0:{}", RESTATE_SERVICES_PORT),
            "--admin-bind-address",
            &format!("0.0.0.0:{}", RESTATE_ADMIN_PORT),
            "--base-dir",
            base_dir.to_str().unwrap_or("."),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn restate-server at {}: {}", binary.display(), e))?;

    eprintln!("🚀 Restate server spawned (pid {})", child.id());
    Ok(child)
}

/// Poll /health until ready or timeout.
pub fn wait_for_restate_healthy() -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(HEALTH_TIMEOUT_SECS);

    loop {
        match ureq_get_health() {
            Ok(true) => {
                eprintln!("✅ Restate is healthy");
                return Ok(());
            }
            _ => {}
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "Restate did not become healthy within {}s",
                HEALTH_TIMEOUT_SECS
            ));
        }

        std::thread::sleep(Duration::from_millis(HEALTH_POLL_MS));
    }
}

fn ureq_get_health() -> Result<bool, ()> {
    // Use a simple TCP check since we don't want a heavy HTTP client in the health loop.
    use std::net::TcpStream;
    TcpStream::connect(format!("127.0.0.1:{}", RESTATE_ADMIN_PORT))
        .map(|_| true)
        .map_err(|_| ())
}

/// Gracefully stop the Restate child process.
pub fn stop_restate(child: &mut Child) {
    eprintln!("🛑 Stopping Restate server (pid {})…", child.id());

    // SIGTERM
    #[cfg(unix)]
    {
        let _ = unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    }

    // Give it 3 seconds to exit cleanly
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                eprintln!("✅ Restate exited with {}", status);
                return;
            }
            Ok(None) if Instant::now() >= deadline => break,
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // SIGKILL as last resort
    eprintln!("⚠️  Restate did not exit; sending SIGKILL");
    let _ = child.kill();
    let _ = child.wait();
}
