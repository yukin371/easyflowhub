//! What the panel lists: the five commands, then the notes the store last had.
//!
//! Split out of `launcher.rs` when that file passed the guard's 800-line cap. The
//! seam is the row model: everything here turns database rows into [`Item`]s, and
//! nothing here touches a window, a hotkey or a frame.
//!
//! ## Snapshot, not a query
//!
//! [`Shell::refresh_recent`] runs at startup and whenever a note is created or
//! closed -- never on the reveal path. The panel shows the list it already had:
//! reading the database in the same pass as the first frame is exactly what §5.8
//! forbids, and a reveal that waited on SQLite would spend the 60 ms budget on the
//! one thing the user is not looking at yet.

use hub_core::Command;
use hub_ui::launcher::{self, Item};
use hub_ui::manager::Tab;
use hub_ui::LauncherAction;

use super::Shell;

/// A note in the panel's recent list.
#[derive(Debug, Clone)]
pub(super) struct Recent {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) when: String,
    pub(super) preview: String,
    /// The whole note, kept only for the search (see [`Item::note`]).
    pub(super) body: String,
}

/// Reload the recent list from the store.
pub(super) fn refresh(shell: &mut Shell) {
    shell.recent.clear();
    let Some(store) = shell.store.as_ref() else {
        return;
    };
    let guard = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Ok(rows) = guard.notes(false) else {
        shell.log.mark(0, "launcher_recent", "error=query");
        return;
    };
    for row in rows.iter().take(launcher::RECENT_LIMIT) {
        let title = hub_core::note_text::display_title(&row.title, &row.content);
        shell.recent.push(Recent {
            id: row.id.clone(),
            preview: first_line(&row.content, &title),
            body: row.content.clone(),
            title,
            when: short_time(&row.updated_at),
        });
    }
    shell.log.mark(
        0,
        "launcher_recent",
        &format!("notes={}", shell.recent.len()),
    );
}

/// The rows, in drawing order: commands first, then recent notes.
pub(super) fn items(shell: &Shell) -> Vec<Item> {
    let mut items = vec![
        Item::command(
            "新建笔记",
            &shell.hint(Command::NewNote),
            "new note 笔记 新建",
            LauncherAction::NewNote,
        ),
        Item::command(
            "工作台",
            &shell.hint(Command::OpenManager),
            "manager workbench 工作台 管理 笔记",
            LauncherAction::OpenManager(Tab::Notes),
        ),
        Item::command(
            "待办清单",
            "",
            "todo 待办 清单 任务",
            LauncherAction::OpenManager(Tab::Todos),
        ),
        Item::command(
            "回收站",
            "",
            "trash 回收站 删除",
            LauncherAction::OpenManager(Tab::Trash),
        ),
        Item::command(
            "偏好设置",
            "",
            "settings 设置 偏好 快捷键",
            LauncherAction::OpenManager(Tab::Settings),
        ),
    ];
    for note in &shell.recent {
        items.push(Item::note(
            &note.id,
            &note.title,
            &note.when,
            &note.preview,
            &note.body,
        ));
    }
    items
}

/// First meaningful line of a note, capped for a list row.
///
/// The line the title already shows is skipped: the quick note's content begins
/// with its own title, and a row that repeats the title underneath itself reads as
/// a rendering mistake rather than as a preview.
fn first_line(content: &str, title: &str) -> String {
    let mut seen_title = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("- [") {
            continue;
        }
        if !seen_title && line == title {
            seen_title = true;
            continue;
        }
        return line.chars().take(40).collect();
    }
    String::new()
}

/// `2026-11-26T02:31:00.123456789+00:00` → `11-26 02:31`.
///
/// Same slice as `hub-ui::switcher`'s: the format is fixed by `clock::now_iso`,
/// and anything shorter falls back to the raw string, which is ugly but never
/// wrong. A date library for a 10 px label is not worth its build time.
fn short_time(stamp: &str) -> String {
    let bytes = stamp.as_bytes();
    if bytes.len() >= 16 && bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T' {
        return format!("{} {}", &stamp[5..10], &stamp[11..16]);
    }
    stamp.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_skips_headings_checkboxes_and_blank_lines() {
        assert_eq!(first_line("\n# 标题\n买了牛奶", "标题"), "买了牛奶");
        assert_eq!(first_line("- [ ] 待办\n正文", "待办"), "正文");
        assert_eq!(first_line("", ""), "");
    }

    #[test]
    fn the_first_line_does_not_repeat_the_title() {
        // The quick note stores its title as the first line of its own content.
        assert_eq!(
            first_line("周末计划\n买螺丝刀和膨胀管", "周末计划"),
            "买螺丝刀和膨胀管"
        );
        // A note whose body is only its title has no preview at all, rather than
        // the same word twice.
        assert_eq!(first_line("周末计划", "周末计划"), "");
    }

    #[test]
    fn the_first_line_is_cut_by_characters() {
        let long = "一".repeat(80);
        assert_eq!(first_line(&long, "").chars().count(), 40);
    }

    #[test]
    fn a_timestamp_is_shortened_but_never_mangled() {
        assert_eq!(short_time("2026-11-26T02:31:00.123+00:00"), "11-26 02:31");
        assert_eq!(short_time("short"), "short");
        assert_eq!(short_time(""), "");
    }
}
