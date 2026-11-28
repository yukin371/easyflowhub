//! Handing a request to the native shell.
//!
//! T-Gate 0 moved the quick note into `easyflowhub-native.exe`, but the Manager is
//! still this process -- the last webview. A button in the workbench therefore has to
//! reach a program it does not link against, and the contract is argv: exactly what a
//! Start-menu shortcut would pass.
//!
//! Two things make that a *handover* rather than a second copy of the app. The native
//! shell's single-instance mutex turns a launch against a running shell into a
//! forwarded request, so this is one path whether or not the shell is already up; and
//! `--note-id` travels through a small file beside the settings, because the ring that
//! wakes the owner cannot carry a payload. See `native/app/src/pending.rs` for why.
//!
//! What is deliberately *not* here: waiting for the child. This process is a window
//! provider, not a supervisor -- a note window outlives the Manager, and a Manager
//! that blocked on the shell would freeze its own webview for as long as the shell
//! takes to answer.

use std::path::PathBuf;
use std::process::Command;

/// `CREATE_NO_WINDOW`. The native build is a console-subsystem executable (it prints
/// its bench and probe output to stdout), so a plain `spawn` would flash a black
/// window for every note the Manager asks for.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Environment variable that points at a specific shell build.
///
/// The gates need it: `check-tauri-resident.py` drives a release build out of a
/// target directory, and an install directory that happens to hold an older shell
/// must not win over the one under test.
pub const EXE_ENV: &str = "EFH_NATIVE_EXE";

/// The argument vector for "open a quick note".
///
/// Split out so the shape can be asserted without spawning anything: a stray `--note`
/// missing here shows up as the panel appearing instead of a note, which is precisely
/// the bug the second doorbell exists to prevent.
pub fn argv(note_id: Option<&str>) -> Vec<String> {
    let mut args = vec!["--note".to_string()];
    if let Some(id) = note_id {
        args.push("--note-id".to_string());
        args.push(id.to_string());
    }
    args
}

/// The argument vector for "open the todo card".
///
/// One bare flag -- and it is *not* `--note`. The two entry points look
/// interchangeable from up here and are not: `--note` opens a note window, while
/// `--todo-card` opens the 320x400 card that reads the 待办箱 note. Sharing one
/// builder with a boolean would make that mistake writable and invisible; two
/// functions with two names make it a diff.
///
/// No id travels: the card is a singleton in the native shell, so there is
/// nothing to name it by.
pub fn todo_card_argv() -> Vec<String> {
    vec!["--todo-card".to_string()]
}

/// Choose the shell from the candidates, in the order a user would expect.
///
/// A function of three `Option`s rather than of the filesystem so the precedence is
/// testable: the interesting cases ("the install directory has an old shell") cannot
/// be produced on this machine without writing one.
pub fn pick(
    from_env: Option<PathBuf>,
    beside_this_exe: Option<PathBuf>,
    installed: Option<PathBuf>,
) -> Result<PathBuf, String> {
    for candidate in [from_env, beside_this_exe, installed].into_iter().flatten() {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "找不到原生壳（easyflowhub-native.exe）。设置 {EXE_ENV} 指向它，或把它放在本程序旁边。"
    ))
}

fn usr_local_appdata() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| {
        PathBuf::from(base)
            .join("EasyFlowHub")
            .join("easyflowhub-native.exe")
    })
}

/// The shell this build should hand work to.
pub fn exe() -> Result<PathBuf, String> {
    let from_env = std::env::var_os(EXE_ENV).map(PathBuf::from);
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("easyflowhub-native.exe")));
    pick(from_env, beside, usr_local_appdata())
}

/// Ask the native shell for a quick-note window.
///
/// Returns a token for the log rather than a `bool`: "the request left" and "a
/// window appeared" are different claims, and only the shell can make the second one.
pub fn open_note(note_id: Option<&str>) -> Result<String, String> {
    launch("note", &argv(note_id))
}

/// Ask the native shell for the todo card.
///
/// Same contract as `open_note`, down to the token: the card is a window in the
/// other process, and whether it appeared is the shell's answer to give. A second
/// tap focuses the card that is already up (`Request::TodoCard` is idempotent), so
/// this never piles up windows.
pub fn open_todo_card() -> Result<String, String> {
    launch("todo-card", &todo_card_argv())
}

/// Start the shell with `args`, and log what was asked for.
///
/// Shared by both entry points because the parts that are easy to get wrong are the
/// same in both: resolving the exe, suppressing the console window, and *not*
/// waiting. The `kind` is what makes the log answer "which door was used".
fn launch(kind: &str, args: &[String]) -> Result<String, String> {
    let exe = exe()?;

    let mut command = Command::new(&exe);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let child = command
        .spawn()
        .map_err(|error| format!("无法启动原生壳 {}：{error}", exe.display()))?;
    let token = format!("native:{}", child.id());
    crate::logging::log(
        "info",
        "shell",
        &format!(
            "handover={kind} target={} args={} pid={}",
            exe.display(),
            args.join(" "),
            child.id()
        ),
    );
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("efh-handover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join(name);
        std::fs::write(&path, b"stub").expect("touch");
        path
    }

    #[test]
    fn a_new_note_needs_no_id() {
        assert_eq!(argv(None), vec!["--note"]);
    }

    #[test]
    fn an_existing_note_carries_its_id_after_the_mode_flag() {
        // Order matters to the parser in `native/app/src/args.rs`: `--note-id` takes
        // the next token, and `--note` has to come first for the shell to be in note
        // mode at all.
        assert_eq!(argv(Some("note-42")), vec!["--note", "--note-id", "note-42"]);
    }

    #[test]
    fn the_todo_card_is_one_flag_and_it_is_not_the_note_one() {
        // The two doors must not become one: `--note` would answer with a blank note
        // window, which looks like success from this side and is the wrong window.
        assert_eq!(todo_card_argv(), vec!["--todo-card"]);
        assert!(
            !todo_card_argv().contains(&"--note".to_string()),
            "the card forward started a note"
        );
        assert_ne!(todo_card_argv(), argv(None));
    }

    #[test]
    fn the_environment_wins_over_a_shell_beside_this_exe() {
        let env = temp("from-env.exe");
        let beside = temp("beside.exe");
        assert_eq!(
            pick(Some(env.clone()), Some(beside), None).expect("env wins"),
            env
        );
    }

    #[test]
    fn a_shell_that_is_not_there_does_not_win() {
        let missing = std::env::temp_dir().join("efh-not-here.exe");
        let installed = temp("installed.exe");
        assert_eq!(
            pick(Some(missing), None, Some(installed.clone())).expect("falls through"),
            installed
        );
    }

    #[test]
    fn no_candidate_is_an_error_that_names_the_variable() {
        let missing = std::env::temp_dir().join("efh-nowhere.exe");
        let error = pick(Some(missing), None, None).expect_err("nothing to run");
        assert!(error.contains(EXE_ENV), "unexpected error: {error}");
    }
}
