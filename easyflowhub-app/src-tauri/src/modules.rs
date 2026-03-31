/**
 * 模块配置管理
 * 读写 ~/.config/easyflowhub/modules.json
 */
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesConfig {
    pub version: u32,
    pub modules: std::collections::HashMap<String, ModuleConfig>,
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self {
            version: 1,
            modules: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LoadModulesConfigResponse {
    pub ok: bool,
    pub config: Option<ModulesConfig>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SaveModulesConfigResponse {
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ToggleModuleConfigResponse {
    pub ok: bool,
    pub error: Option<String>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// 获取配置文件路径: ~/.config/easyflowhub/modules.json
fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    // 使用 app config 目录
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get app config dir: {}", e))?;

    // 确保目录存在
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    Ok(config_dir.join("modules.json"))
}

/// 读取配置文件
fn read_config_file(path: &PathBuf) -> Result<ModulesConfig, String> {
    if !path.exists() {
        return Ok(ModulesConfig::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let config: ModulesConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    Ok(config)
}

/// 写入配置文件
fn write_config_file(path: &PathBuf, config: &ModulesConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(path, content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

// ============================================================================
// Commands
// ============================================================================

/// 加载模块配置
#[tauri::command]
pub fn load_modules_config(app: tauri::AppHandle) -> LoadModulesConfigResponse {
    match config_path(&app) {
        Ok(path) => match read_config_file(&path) {
            Ok(config) => LoadModulesConfigResponse {
                ok: true,
                config: Some(config),
                error: None,
            },
            Err(e) => LoadModulesConfigResponse {
                ok: false,
                config: None,
                error: Some(e),
            },
        },
        Err(e) => LoadModulesConfigResponse {
            ok: false,
            config: None,
            error: Some(e),
        },
    }
}

/// 保存模块配置
#[tauri::command]
pub fn save_modules_config(
    app: tauri::AppHandle,
    config: ModulesConfig,
) -> SaveModulesConfigResponse {
    match config_path(&app) {
        Ok(path) => match write_config_file(&path, &config) {
            Ok(()) => SaveModulesConfigResponse {
                ok: true,
                error: None,
            },
            Err(e) => SaveModulesConfigResponse {
                ok: false,
                error: Some(e),
            },
        },
        Err(e) => SaveModulesConfigResponse {
            ok: false,
            error: Some(e),
        },
    }
}

/// 切换单个模块的启用状态
#[tauri::command]
pub fn toggle_module_config(
    app: tauri::AppHandle,
    module_id: String,
    enabled: bool,
) -> ToggleModuleConfigResponse {
    match config_path(&app) {
        Ok(path) => {
            // 读取现有配置
            let mut config = match read_config_file(&path) {
                Ok(c) => c,
                Err(e) => {
                    return ToggleModuleConfigResponse {
                        ok: false,
                        error: Some(e),
                    }
                }
            };

            // 更新模块状态
            config.modules.insert(module_id, ModuleConfig { enabled });

            // 保存配置
            match write_config_file(&path, &config) {
                Ok(()) => ToggleModuleConfigResponse {
                    ok: true,
                    error: None,
                },
                Err(e) => ToggleModuleConfigResponse {
                    ok: false,
                    error: Some(e),
                },
            }
        }
        Err(e) => ToggleModuleConfigResponse {
            ok: false,
            error: Some(e),
        },
    }
}
