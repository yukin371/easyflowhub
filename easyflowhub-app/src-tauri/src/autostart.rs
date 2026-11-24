//! 开机自启动：持久化用户意图 + 启动时与系统注册表对账。
//!
//! 为什么需要这一层：`tauri-plugin-autostart` 只读写系统启动项，不保存任何
//! 意图。启动项被安装器、清理软件或任务管理器删掉之后，`is_enabled()` 只会
//! 返回 `false`，用户看到的是「开关自己关了」，而且此后再也不会自动恢复
//! （plan §2.4 记录的实测现象）。
//!
//! 这里把用户意图单独存进 `settings` 表（键 `autostart`），并且**不放进**
//! `AppSettings`：设置面板的全量 `update_settings` 写 8 个域，如果 autostart
//! 也在里面，面板拿着旧快照保存就会把这里刚写的结果覆盖回去。
//!
//! 启动时 `reconcile()` 对账一次：意图为开而系统缺失就补回来。这是 G1 门禁
//! 「删除启动项后重启能自愈」的实现。

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::settings;

const SETTINGS_KEY: &str = "autostart";

/// Persisted user intent. Kept as a struct (not a bare bool) so later fields
/// (for example "launch minimized") can be added without a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
struct AutostartIntent {
    enabled: bool,
}

/// Snapshot handed to the UI and returned by the startup reconciliation.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct AutostartState {
    /// 系统启动项当前是否真的存在。UI 开关显示的就是它。
    pub system_enabled: bool,
    /// 上次通过本应用保存的意图；`None` 表示升级上来的老用户还没记录过。
    pub user_enabled: Option<bool>,
    /// 本次是否把被外部移除的启动项补了回来。
    pub repaired: bool,
    /// 系统启动项能否读写。`false` 时 UI 应显示「不可用」而不是「已关闭」，
    /// 否则组策略/权限受限的机器上会一直显示成用户自己关掉的。
    pub available: bool,
}

fn system_enabled(app: &AppHandle) -> Option<bool> {
    app.autolaunch().is_enabled().ok()
}

fn set_system(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|e| e.to_string())
}

fn read_intent(app: &AppHandle) -> Option<bool> {
    settings::read_setting_json::<AutostartIntent>(app, SETTINGS_KEY).map(|intent| intent.enabled)
}

fn write_intent(app: &AppHandle, enabled: bool) -> Result<(), String> {
    settings::write_setting_json(app, SETTINGS_KEY, &AutostartIntent { enabled })
}

/// Current state without changing anything.
pub fn snapshot(app: &AppHandle) -> AutostartState {
    let system = system_enabled(app);
    AutostartState {
        system_enabled: system.unwrap_or(false),
        user_enabled: read_intent(app),
        repaired: false,
        available: system.is_some(),
    }
}

/// Apply a user toggle: write the system entry, verify it, then persist the
/// intent. Verification matters because a "successful" write can still be
/// swallowed by policy or a locked registry key.
pub fn apply(app: &AppHandle, enabled: bool) -> Result<AutostartState, String> {
    set_system(app, enabled)?;

    let observed = system_enabled(app);
    if observed != Some(enabled) {
        return Err(format!(
            "系统启动项写入未生效（期望 {}，实际 {}）",
            enabled,
            observed.map(|value| value.to_string()).unwrap_or_else(|| "无法读取".into())
        ));
    }

    if let Err(error) = write_intent(app, enabled) {
        // Keep the registry and the persisted intent consistent: if the intent
        // cannot be stored the toggle must not silently half-apply.
        let _ = set_system(app, !enabled);
        return Err(error);
    }

    Ok(AutostartState {
        system_enabled: enabled,
        user_enabled: Some(enabled),
        repaired: false,
        available: true,
    })
}

/// Startup reconciliation. Cheap (one registry read, one row read) and only
/// writes when the two disagree.
///
/// Cases:
/// - no stored intent (first run after upgrade): adopt whatever the registry
///   already says, without writing the registry;
/// - intent `true` but entry missing: re-create it (self-heal);
/// - intent `false` but entry present: leave it alone. The entry may have been
///   re-added by a user or an installer, and force-disabling it on every launch
///   would fight the user. The settings UI surfaces the mismatch instead.
pub fn reconcile(app: &AppHandle) -> AutostartState {
    let system = system_enabled(app);
    let intent = read_intent(app);
    let mut repaired = false;

    match (intent, system) {
        (None, Some(current)) => {
            if let Err(error) = write_intent(app, current) {
                crate::logging::log("warn", "autostart", &format!("迁移启动项意图失败: {error}"));
            }
        }
        (Some(true), Some(false)) => match set_system(app, true) {
            Ok(()) if system_enabled(app) == Some(true) => {
                repaired = true;
                crate::logging::log("info", "autostart", "系统启动项缺失，已按持久化意图补回");
            }
            Ok(()) => crate::logging::log("warn", "autostart", "补回启动项后回读仍为关闭"),
            Err(error) => {
                crate::logging::log("warn", "autostart", &format!("补回启动项失败: {error}"))
            }
        },
        _ => {}
    }

    AutostartState {
        system_enabled: system_enabled(app).unwrap_or(false),
        user_enabled: read_intent(app),
        repaired,
        available: system.is_some(),
    }
}

#[tauri::command]
pub fn get_autostart_state(app: AppHandle) -> AutostartState {
    snapshot(&app)
}

#[tauri::command]
pub fn set_autostart_state(app: AppHandle, enabled: bool) -> Result<AutostartState, String> {
    apply(&app, enabled)
}
