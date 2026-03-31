// EasyFlowHub MCP Integration
//
// How AI agents connect to EasyFlowHub's MCP server:
//
// OPTION 1: Direct scriptmgr MCP (Recommended for v1.0)
// ----------------------------------------------------------------
// Agents connect directly to scriptmgr.exe's built-in MCP server.
//
// Claude Code configuration (~/.claude/settings.json):
// {
//   "mcpServers": {
//     "scriptmgr": {
//       "command": "E:/path/to/scriptmgr.exe",
//       "args": ["mcp"]
//     }
//   }
// }
//
// Benefits:
// - Simple, works out of the box
// - No additional server to maintain
// - scriptmgr already has full MCP implementation
//
// OPTION 2: Through EasyFlowHub (Future enhancement)
// ----------------------------------------------------------------
// When EasyFlowHub runs, agents can access scriptmgr tools via
// the EasyFlowHub Tauri commands (already implemented in scriptmgr.rs).
//
// This is useful when:
// - Agent is already connected to EasyFlowHub UI
// - Need unified access to notes + scripts
//

use tauri::{AppHandle, Manager};

const MCP_DEFAULT_PORT: u16 = 8766;

/// Get information about the MCP server connection
#[tauri::command]
pub fn get_mcp_info(app: AppHandle) -> McpInfo {
    // Try to find scriptmgr.exe
    let scriptmgr_path = resolve_scriptmgr_path(&app);

    McpInfo {
        mode: "direct".to_string(),
        scriptmgr_path,
        mcp_port: Some(MCP_DEFAULT_PORT),
        connection_note: "Configure Claude Code to connect directly to scriptmgr.exe: scriptmgr mcp".to_string(),
    }
}

fn resolve_scriptmgr_path(app: &AppHandle) -> Option<String> {
    // Get the directory containing the app executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled_path = exe_dir.join("scriptmgr.exe");
            if bundled_path.exists() {
                return Some(bundled_path.to_string_lossy().to_string());
            }
        }
    }

    // Try development path
    if let Ok(resource_dir) = app.path().resource_dir() {
        if let Some(parent1) = resource_dir.parent() {
            if let Some(parent2) = parent1.parent() {
                let dev_path = parent2.join("scriptmgr-go").join("scriptmgr.exe");
                if dev_path.exists() {
                    return Some(dev_path.to_string_lossy().to_string());
                }
            }
        }
    }

    None
}

#[derive(serde::Serialize)]
pub struct McpInfo {
    pub mode: String,
    pub scriptmgr_path: Option<String>,
    pub mcp_port: Option<u16>,
    pub connection_note: String,
}
