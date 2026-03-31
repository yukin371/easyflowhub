use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Manager;

// ============================================================================
// Types matching scriptmgr-go output
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub script_type: String,
    #[serde(default)]
    pub source_root: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub label: String,
    #[serde(default)]
    pub default: String,
    pub required: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptDetail {
    pub id: String,
    pub name: String,
    pub path: String,
    pub script_type: String,
    #[serde(default)]
    pub source_root: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub parameters: Vec<ScriptParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputMeta {
    pub truncated: bool,
    pub preview: String,
    pub total_length: i64,
    pub line_count: i64,
    #[serde(default)]
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub script_id: String,
    pub script_name: String,
    #[serde(default)]
    pub command: Vec<String>,
    pub exit_code: i32,
    pub status: String,
    pub succeeded: bool,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub output_meta: Option<OutputMeta>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
    pub ok: bool,
    pub count: i32,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub roots: Vec<String>,
    pub scripts: Vec<ScriptSummary>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeResponse {
    pub ok: bool,
    pub script: ScriptDetail,
    pub generated_at: String,
}

// ============================================================================
// Sidecar path resolution
// ============================================================================

/// Resolve the path to scriptmgr.exe sidecar
/// Order of resolution:
/// 1. Development: ../../../scriptmgr-go/scriptmgr.exe (relative to src-tauri)
/// 2. Production: bundled in resources directory next to the app
fn resolve_scriptmgr_path(app: &tauri::AppHandle) -> Result<String, String> {
    // Get the directory containing the app executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled_path = exe_dir.join("scriptmgr.exe");
            if bundled_path.exists() {
                return Ok(bundled_path.to_string_lossy().to_string());
            }
        }
    }

    // Try to use Tauri resource dir (for packaged app)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled_path = resource_dir.join("scriptmgr.exe");
        if bundled_path.exists() {
            return Ok(bundled_path.to_string_lossy().to_string());
        }
    }

    // Fallback: try development path relative to repo root
    // This works when running from easyflowhub-app/src-tauri/target/debug/
    if let Ok(resource_dir) = app.path().resource_dir() {
        // Go up: target/debug/ -> src-tauri/ -> easyflowhub-app/ -> repo root
        if let Some(parent1) = resource_dir.parent() {
            if let Some(parent2) = parent1.parent() {
                let dev_path = parent2
                    .join("scriptmgr-go")
                    .join("scriptmgr.exe");
                if dev_path.exists() {
                    return Ok(dev_path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Last resort: assume scriptmgr is in PATH
    Ok("scriptmgr.exe".to_string())
}

// ============================================================================
// Commands
// ============================================================================

/// List all available scripts
#[tauri::command]
pub async fn list_scripts(
    app: tauri::AppHandle,
    search: Option<String>,
) -> Result<ListResponse, String> {
    let scriptmgr_path = resolve_scriptmgr_path(&app)?;

    let mut cmd = Command::new(&scriptmgr_path);
    cmd.arg("list").arg("--json");

    if let Some(ref s) = search {
        if !s.is_empty() {
            cmd.arg("--search").arg(s);
        }
    }

    let output = cmd.output().map_err(|e| {
        format!(
            "Failed to execute scriptmgr list: {} (path: {})",
            e, scriptmgr_path
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("scriptmgr list failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: ListResponse =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse list response: {}", e))?;

    Ok(response)
}

/// Get detailed information about a specific script
#[tauri::command]
pub async fn describe_script(
    app: tauri::AppHandle,
    script_id: String,
) -> Result<DescribeResponse, String> {
    let scriptmgr_path = resolve_scriptmgr_path(&app)?;

    let output = Command::new(&scriptmgr_path)
        .arg("describe")
        .arg(&script_id)
        .arg("--json")
        .output()
        .map_err(|e| format!("Failed to execute scriptmgr describe: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("scriptmgr describe failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: DescribeResponse =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse describe response: {}", e))?;

    Ok(response)
}

/// Run a script with optional arguments
#[tauri::command]
pub async fn run_script(
    app: tauri::AppHandle,
    script_id: String,
    args: Option<Vec<String>>,
    dry_run: Option<bool>,
) -> Result<RunResult, String> {
    let scriptmgr_path = resolve_scriptmgr_path(&app)?;

    let mut cmd = Command::new(&scriptmgr_path);
    cmd.arg("run").arg(&script_id).arg("--json").arg("--capture-output");

    if dry_run.unwrap_or(false) {
        cmd.arg("--dry-run");
    }

    if let Some(ref script_args) = args {
        for arg in script_args {
            cmd.arg(arg);
        }
    }

    let output = cmd.output().map_err(|e| format!("Failed to execute scriptmgr run: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the result even if exit code is non-zero (script might have failed)
    let result: RunResult =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse run response: {}", e))?;

    Ok(result)
}
