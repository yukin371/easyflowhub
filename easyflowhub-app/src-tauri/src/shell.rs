//! Handover from this process to the native shell (T-Gate 0).
//!
//! Phase 1 finished the tray, the hotkeys, the Launcher, the quick note and the
//! Manager pages that no longer need a webview, so this process stops creating
//! the surfaces that used to serve them. What is left is one Manager window,
//! created on demand and destroyed on close -- the exit condition written into
//! the transition table (plan `2026-11-24-runtime-redesign` §Phase 1).
//!
//! The handover is **on by default**. A flag that defaults to "keep doing the
//! old thing" is how a transition becomes permanent: both stacks would keep
//! working, nobody would notice which one answered a keystroke, and the memory
//! this whole migration exists to remove would stay. `--legacy-ui` (or
//! `EFH_LEGACY_UI=1`) brings the old shell back, for a latency baseline run or
//! for rolling back on a user machine without shipping a new build.
//!
//! The decision is logged once per process (`shell takeover active=<bool>
//! source=<why>`), because a gate that cannot tell "the webview is gone" from
//! "the whole launcher is gone" would pass on a broken build.

/// Command-line flag that asks for the pre-T-Gate behaviour.
pub const LEGACY_FLAG: &str = "--legacy-ui";

/// Environment variable that asks for the same thing without changing argv.
pub const LEGACY_ENV: &str = "EFH_LEGACY_UI";

/// Why the current process runs the old shell, or `None` when it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// This process serves only the Manager window.
    Takeover,
    /// The old shell is back, by request.
    Legacy(&'static str),
}

impl Decision {
    pub fn takeover(self) -> bool {
        matches!(self, Decision::Takeover)
    }

    /// The `source=` value in the log line. Stable strings -- the gate greps them.
    pub fn source(self) -> &'static str {
        match self {
            Decision::Takeover => "default",
            Decision::Legacy(why) => why,
        }
    }
}

/// Resolve the handover from argv and the environment.
///
/// Split out from the process-wide read so the precedence (flag wins over env,
/// env wins over the default) is testable without spawning anything.
pub fn decide(argv: &[String], legacy_env: Option<&str>) -> Decision {
    if argv.iter().any(|arg| arg == LEGACY_FLAG) {
        return Decision::Legacy("flag");
    }
    match legacy_env {
        Some(value) if !value.is_empty() && value != "0" && value != "false" => Decision::Legacy("env"),
        _ => Decision::Takeover,
    }
}

/// The decision for this process, resolved once.
pub fn current() -> Decision {
    static DECISION: std::sync::OnceLock<Decision> = std::sync::OnceLock::new();
    *DECISION.get_or_init(|| {
        let argv: Vec<String> = std::env::args().skip(1).collect();
        let env = std::env::var(LEGACY_ENV).ok();
        decide(&argv, env.as_deref())
    })
}

/// Log the decision so a run can be judged from its log alone.
pub fn announce(decision: Decision) {
    crate::logging::log(
        "info",
        "shell",
        &format!(
            "shell takeover active={} source={}",
            decision.takeover(),
            decision.source()
        ),
    );
}

/// One line for a command that T-Gate 0 moved to the native shell.
///
/// Returned as an error instead of silently doing nothing: the Manager's UI
/// still offers a few of these buttons, and "nothing happened" is the one
/// answer a user cannot act on.
pub fn moved_to_native(what: &str) -> String {
    crate::logging::log(
        "info",
        "shell",
        &format!("refused {what}: served by the native shell since T-Gate 0"),
    );
    format!("{what}已由原生壳接管（T-Gate 0）。请使用原生窗口或托盘图标。")
}

/// One global shortcut this process is responsible for after the handover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewNote,
    Manager,
    CloseAllNotes,
    ToggleNotes,
}

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Action::NewNote => "new_note",
            Action::Manager => "manager",
            Action::CloseAllNotes => "close_all_notes",
            Action::ToggleNotes => "toggle_notes",
        }
    }
}

/// A shortcut the process would register, and the chord it would use.
#[derive(Debug, Clone, Copy)]
pub struct Planned {
    pub action: Action,
    pub chord: &'static str,
}

/// The shortcut plan for a given handover state.
///
/// This is the single source of truth for two readers: the registration loop in
/// `run()`, and `--shell-report`. A gate that reads a *second* list would prove
/// only that two lists agree; deriving both from this function means "the
/// report says three chords are gone" is the same statement as "the process
/// registers three fewer chords".
///
/// Under handover the plan is **empty**, not "just the Manager". That is not a
/// simplification: the first run of this transition kept `Ctrl+Alt+M` here and
/// the native shell reported `RegisterHotKey failed 1409` for the same chord on
/// the same machine — two stacks cannot own one combination, and the one that
/// loses depends on who started first. The Manager is still reachable from the
/// tray; a hotkey is not worth a race.
pub fn shortcut_plan(takeover: bool) -> Vec<Planned> {
    if takeover {
        return Vec::new();
    }
    vec![
        Planned { action: Action::Manager, chord: "Ctrl+Alt+M" },
        Planned { action: Action::NewNote, chord: "Ctrl+Alt+N" },
        Planned { action: Action::CloseAllNotes, chord: "Ctrl+Alt+D" },
        Planned { action: Action::ToggleNotes, chord: "Ctrl+Alt+H" },
    ]
}

/// What the tray offers: creating a quick note is a webview this process no
/// longer builds, so the entry goes away with the window.
pub fn tray_plan(takeover: bool) -> Vec<&'static str> {
    if takeover {
        vec!["show_manager", "quit"]
    } else {
        vec!["show_manager", "new_note", "quit"]
    }
}

/// The launcher window label, when this process still declares one.
///
/// Read from the config file rather than restated here: the whole failure mode
/// this guards against is a config that quietly keeps the window while the code
/// believes the window is gone.
pub fn declared_windows() -> Vec<String> {
    let raw = include_str!("../tauri.conf.json");
    let parsed: serde_json::Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    parsed["app"]["windows"]
        .as_array()
        .map(|windows| {
            windows
                .iter()
                .filter_map(|window| window["label"].as_str().map(|label| label.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Print the handover as JSON and let the caller exit.
///
/// Runs before any Tauri builder touches the disk or the single-instance mutex,
/// which is what makes it usable on a machine where the real app is already
/// resident: a gate can ask "what would this build do" without fighting the
/// running instance for the hotkey or for focus.
pub fn report() -> String {
    let decision = current();
    let takeover = decision.takeover();
    let shortcuts: Vec<serde_json::Value> = shortcut_plan(takeover)
        .into_iter()
        .map(|planned| serde_json::json!({ "action": planned.action.id(), "chord": planned.chord }))
        .collect();
    let report = serde_json::json!({
        "takeover": takeover,
        "source": decision.source(),
        "declaredWindows": declared_windows(),
        "shortcuts": shortcuts,
        "tray": tray_plan(takeover),
        "launcher": if takeover { "native" } else { "webview" },
        "quickNote": if takeover { "native" } else { "webview" },
        "todoCard": if takeover { "native" } else { "webview" },
    });
    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
}

/// Run the action behind a registered chord.
///
/// Kept next to the plan so the registration loop in `run()` stays a loop: the
/// shortcut table decides *what* is registered, this decides *what happens*.
pub fn run_shortcut(app: &tauri::AppHandle, action: Action) {
    match action {
        Action::Manager => crate::app_windows::toggle_manager_window(app),
        Action::NewNote => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::app_windows::create_note_window(app, None).await;
            });
        }
        Action::CloseAllNotes => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::app_windows::close_all_note_windows(app).await;
            });
        }
        Action::ToggleNotes => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::app_windows::toggle_note_windows_visibility(app).await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn the_default_is_handover_not_legacy() {
        assert_eq!(decide(&argv(&[]), None), Decision::Takeover);
        assert!(decide(&argv(&[]), None).takeover());
    }

    #[test]
    fn the_flag_beats_the_environment() {
        assert_eq!(decide(&argv(&["--legacy-ui"]), Some("1")), Decision::Legacy("flag"));
    }

    #[test]
    fn the_environment_accepts_the_values_a_shell_script_would_set() {
        for truthy in ["1", "true", "yes", "on"] {
            assert_eq!(decide(&argv(&[]), Some(truthy)), Decision::Legacy("env"), "{truthy}");
        }
    }

    #[test]
    fn a_disabled_or_empty_environment_leaves_the_handover_in_place() {
        for falsy in ["", "0", "false"] {
            assert_eq!(decide(&argv(&[]), Some(falsy)), Decision::Takeover, "{falsy:?}");
        }
    }

    #[test]
    fn unrelated_arguments_do_not_switch_the_shell_back() {
        assert_eq!(decide(&argv(&["--autostart"]), None), Decision::Takeover);
        assert_eq!(decide(&argv(&["--legacy"]), None), Decision::Takeover);
    }
}
