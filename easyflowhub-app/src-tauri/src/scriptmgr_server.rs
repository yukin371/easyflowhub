//! Native owner of the scriptmgr HTTP sidecar lifecycle.
//!
//! The manager panels (scripts / extensions / relay / MCP) talk to the Go
//! sidecar over `http://127.0.0.1:8765`. Nobody else starts that server: the
//! frontend used to fetch the hard-coded URL directly, which meant every panel
//! failed with "Failed to fetch" whenever `scriptmgr serve` was not already
//! running. This module owns the sidecar process: it spawns it on demand,
//! waits until the HTTP server answers, and stops it when the app exits.

use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Manager;

/// Default loopback port of the scriptmgr HTTP server.
pub const DEFAULT_PORT: u16 = 8765;

/// How long to wait for a freshly spawned sidecar to answer HTTP.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);

/// Poll interval while waiting for the sidecar to become healthy.
const POLL_INTERVAL: Duration = Duration::from_millis(120);

/// Per-request timeout used for probe HTTP calls.
const PROBE_TIMEOUT: Duration = Duration::from_millis(800);

/// Windows console-suppression flag (CREATE_NO_WINDOW) so the sidecar does not
/// flash a console window on launch.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// App-global sidecar state. `starting` gates concurrent `ensure_scriptmgr`
/// callers so only one spawn happens; `child` keeps the process handle for
/// shutdown.
#[derive(Default)]
pub struct ScriptmgrState {
    starting: Mutex<bool>,
    child: Mutex<Option<Child>>,
}

impl ScriptmgrState {
    /// Stop the sidecar if this app instance started it. Safe to call twice.
    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
                crate::logging::log("info", "scriptmgr", "sidecar stopped");
            }
        }
    }
}

/// Loopback base URL of the scriptmgr HTTP server.
pub fn base_url() -> String {
    format!("http://127.0.0.1:{DEFAULT_PORT}")
}

/// Resolve the port from a base URL string (`http://127.0.0.1:8765` -> 8765).
#[cfg(test)]
fn url_port(url: &str) -> Option<u16> {
    url::Url::parse(url).ok()?.port_or_known_default()
}

/// True when something accepts TCP connections on the URL's port.
fn port_open(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or("127.0.0.1");
    let Some(port) = parsed.port_or_known_default() else {
        return false;
    };
    match (host, port).to_socket_addrs() {
        Ok(mut addrs) => addrs.any(|addr| {
            TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
        }),
        Err(_) => false,
    }
}

/// True when a scriptmgr HTTP server answers `GET /api/scripts` on the URL.
fn http_ok(url: &str) -> bool {
    let client = match reqwest::blocking::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    match client
        .get(format!("{}/api/scripts?search=", url.trim_end_matches('/')))
        .send()
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

/// A scriptmgr server is considered healthy when its port accepts connections
/// AND the scripts endpoint answers. A foreign listener on the same port is
/// not good enough.
pub fn server_responding(url: &str) -> bool {
    port_open(url) && http_ok(url)
}

/// Spawn `scriptmgr serve` as a hidden child process.
fn spawn_sidecar(app: &tauri::AppHandle) -> Result<Child, String> {
    let scriptmgr_path = crate::scriptmgr::resolve_scriptmgr_path(app)?;
    let mut command = Command::new(&scriptmgr_path);
    command
        .arg("serve")
        .arg("--port")
        .arg(DEFAULT_PORT.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 scriptmgr serve 失败: {error}（路径: {scriptmgr_path}）"))?;

    // Forward sidecar stderr into the bounded log ring so startup failures are
    // diagnosable from the developer panel.
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                crate::logging::log("info", "scriptmgr", &line);
            }
        });
    }

    crate::logging::log("info", "scriptmgr", &format!("spawned {scriptmgr_path} serve --port {DEFAULT_PORT}"));
    Ok(child)
}

/// True when the spawned child already exited on its own.
fn child_exited(child: &Mutex<Option<Child>>) -> bool {
    if let Ok(mut guard) = child.lock() {
        if let Some(process) = guard.as_mut() {
            return matches!(process.try_wait(), Ok(Some(_status)));
        }
    }
    false
}

/// Reap an exited child process so no zombie is left behind.
fn reap_child(child: &Mutex<Option<Child>>) -> Result<(), String> {
    let mut guard = child.lock().map_err(|_| "child lock poisoned".to_string())?;
    if let Some(process) = guard.as_mut() {
        process.wait().map_err(|error| format!("reap failed: {error}"))?;
    }
    Ok(())
}

/// Poll the health endpoint until the sidecar answers, the spawned child exits
/// early, or `timeout` elapses.
fn wait_until_healthy(
    url: &str,
    child: &Mutex<Option<Child>>,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if server_responding(url) {
            return Ok(());
        }
        if child_exited(child) {
            // The sidecar exited on its own (e.g. the port is taken by a
            // foreign application). Reap it and report clearly.
            let _ = reap_child(child);
            return Err(format!(
                "scriptmgr 服务启动后退出（端口 {DEFAULT_PORT} 可能被其他程序占用）"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "scriptmgr 服务启动超时（{} 秒内无响应）",
                timeout.as_secs()
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Blocking implementation behind `ensure_scriptmgr`.
fn ensure_blocking(app: &tauri::AppHandle) -> Result<String, String> {
    let url = base_url();
    let state = app.state::<ScriptmgrState>();

    // Gate: the first caller brings the sidecar up (spawn + health wait) while
    // holding the lock; later callers return immediately once the gate opens.
    let mut gate = state
        .starting
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if *gate {
        return Ok(url);
    }

    let spawned = if server_responding(&url) {
        // An instance is already healthy (external start or previous orphan).
        crate::logging::log("info", "scriptmgr", "reusing an already healthy instance");
        None
    } else {
        Some(spawn_sidecar(app)?)
    };
    if let Some(child) = spawned {
        if let Ok(mut guard) = state.child.lock() {
            *guard = Some(child);
        }
    }

    let wait_result = wait_until_healthy(&url, &state.child, STARTUP_TIMEOUT);
    if let Err(error) = wait_result {
        crate::logging::log("error", "scriptmgr", &error);
        state.shutdown();
        return Err(error);
    }

    *gate = true;
    crate::logging::log("info", "scriptmgr", "sidecar ready");
    Ok(url)
}

/// Ensure the scriptmgr HTTP sidecar is running and return its base URL.
/// Idempotent: concurrent callers share one spawn attempt, and later calls
/// are a no-op once the sidecar is up.
#[tauri::command]
pub async fn ensure_scriptmgr(app: tauri::AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || ensure_blocking(&app))
        .await
        .map_err(|error| format!("scriptmgr 启动任务失败: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_points_at_default_loopback_port() {
        assert_eq!(base_url(), "http://127.0.0.1:8765");
        assert_eq!(url_port("http://127.0.0.1:8765"), Some(8765));
        assert_eq!(url_port("http://127.0.0.1"), Some(80));
        assert_eq!(url_port("not a url"), None);
    }

    #[test]
    fn port_open_detects_live_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind probe listener");
        let port = listener.local_addr().expect("listener addr").port();
        let url = format!("http://127.0.0.1:{port}");
        assert!(port_open(&url));
        // A bare TCP listener is not a scriptmgr server: no HTTP answer.
        assert!(!server_responding(&url));
    }

    #[test]
    fn port_open_false_when_nothing_listens() {
        // Bind and immediately drop to get a very likely free port.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind free port")
            .local_addr()
            .expect("addr")
            .port();
        let url = format!("http://127.0.0.1:{port}");
        assert!(!port_open(&url));
        assert!(!server_responding(&url));
    }

    #[test]
    fn wait_until_healthy_times_out_when_only_tcp_listens() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind probe listener");
        let url = format!("http://127.0.0.1:{}", listener.local_addr().expect("addr").port());
        let child = Mutex::new(None);
        let result = wait_until_healthy(&url, &child, Duration::from_millis(250));
        assert!(matches!(result, Err(message) if message.contains("启动超时")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn wait_until_healthy_reports_early_child_exit() {
        let mut exited = std::process::Command::new("cmd")
            .args(["/C", "exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn short-lived helper");
        // Give the helper a moment to exit so try_wait observes it.
        let _ = exited.wait();
        let child = Mutex::new(Some(exited));
        // Use a free port instead of the real default port: a running app may
        // own 8765 and would make the probe succeed.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind free port")
            .local_addr()
            .expect("addr")
            .port();
        let url = format!("http://127.0.0.1:{port}");
        let result = wait_until_healthy(&url, &child, Duration::from_millis(250));
        assert!(matches!(result, Err(message) if message.contains("启动后退出")));
    }

    #[test]
    fn scriptmgr_state_shutdown_is_safe_without_child() {
        let state = ScriptmgrState::default();
        state.shutdown();
        state.shutdown();
    }
}
