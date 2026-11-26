//! The autosave draft on disk, and the question it asks when it is newer than the
//! row it belongs to (parity Q24).
//!
//! ## Why a file and not `localStorage`
//!
//! The shipped build writes its draft to the WebView's `localStorage` under
//! `easyflowhub_note_autosave_{id}`, with `{title, content, tags, timestamp}` and
//! `timestamp` in `Date.now()` milliseconds. A native window has no origin, no
//! `localStorage` and no access to the WebView's profile, so the same record goes
//! to `{store_root}/autosave/{id}.json`, **field for field**. The shape is worth
//! copying even though the two files never meet: a gate can hand-write one, and
//! the day the drafts move into a table they will already agree on the columns.
//!
//! The two sides therefore cannot recover each other's drafts. That is a real
//! limitation, not an oversight -- see the README's "known shortcuts".
//!
//! ## What gets asked, and what gets deleted
//!
//! From `useBackupRestore.ts`, which is the behaviour Q24 names: a draft is only
//! shown when it differs from the row *and* is not older than it. A draft that is
//! identical to the row is a leftover from the last successful save, and one that
//! is older than `updated_at` was written before an edit that already reached the
//! database -- asking about either would train the user to dismiss the prompt
//! without reading it.
//!
//! ## Which button does what
//!
//! * 保留当前 -- delete the draft, leave the row alone.
//! * 恢复备份 -- the draft becomes the document; the next save writes it.
//! * 合并 -- the two bodies joined by a `--- 备份内容 ---` line, tags unioned,
//!   title from the current row. Same text the shipped `onMergeDraft` builds.
//!
//! "保留当前" deletes the file, which the shipped build does *not* do (it only
//! closes the dialog, so the same question comes back on the next open). Deleting
//! is what the button says, and a prompt that reappears after "keep current" is a
//! bug the user pays for on every open.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use hub_core::note_text;
use hub_core::{clock, NoteRow};

/// Directory under the store root. Named after the shipped key's prefix.
pub const DIR: &str = "autosave";

const SUFFIX: &str = ".json";

/// One unsaved draft, in the shipped `BackupDraft` shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub title: String,
    pub content: String,
    pub tags: String,
    /// Unix milliseconds, `Date.now()` on the other side.
    pub timestamp: u64,
}

impl Draft {
    /// Read the editor's text into the three columns the row uses.
    pub fn of(text: &str, timestamp: u64) -> Self {
        let parsed = note_text::parse(&note_text::normalize_newlines(text));
        Self {
            title: parsed.title,
            content: parsed.content,
            tags: parsed.tags,
            timestamp,
        }
    }

    /// The text that shows this draft in the editor.
    ///
    /// `format_for_display` and not a hand-built `title\ncontent\n# tags`: content
    /// already carries its first line, and rebuilding it from `title` would lose
    /// the difference between a note whose first line was a heading and one whose
    /// stored title was set separately.
    pub fn display(&self) -> String {
        note_text::format_for_display(&self.content, &note_text::normalize_tags(&self.tags))
    }
}

/// What [`inspect`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing stored, or something unreadable: no prompt either way.
    Quiet,
    /// A draft that cannot apply to this row, deleted. The reason is for the log.
    Removed(&'static str),
    /// Newer than the row and different from it: ask.
    Ask(Draft),
}

/// The directory that holds drafts for a store rooted at `root`.
pub fn dir(root: &Path) -> PathBuf {
    root.join(DIR)
}

/// How long ago the draft was written, in words.
///
/// Relative rather than a clock time: the recorded value is UTC milliseconds and
/// printing it as a date would show a time the user never saw on their own clock
/// unless this function grew a timezone database. "3 分钟前" is true regardless.
pub fn ago(now_ms: u64, timestamp: u64) -> String {
    if timestamp == 0 {
        return "时间未知".to_string();
    }
    let diff = now_ms.saturating_sub(timestamp);
    let minute = 60_000;
    let hour = 60 * minute;
    let day = 24 * hour;
    if diff < minute {
        "刚刚".to_string()
    } else if diff < hour {
        format!("{} 分钟前", diff / minute)
    } else if diff < day {
        format!("{} 小时前", diff / hour)
    } else {
        format!("{} 天前", diff / day)
    }
}

fn file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}{SUFFIX}"))
}

/// Write the draft for `id`, replacing whatever was there.
///
/// Write-then-rename: a crash halfway through leaves the previous draft intact
/// instead of a truncated JSON that reads as "no backup" -- which is the one
/// outcome the file exists to prevent.
pub fn write(dir: &Path, id: &str, draft: &Draft) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let json = serde_json::json!({
        "title": draft.title,
        "content": draft.content,
        "tags": draft.tags,
        "timestamp": draft.timestamp,
    })
    .to_string();
    let path = file(dir, id);
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, json)?;
    fs::rename(&temp, &path)
}

/// The draft for `id`, or `None` when there is none or it cannot be parsed.
pub fn read(dir: &Path, id: &str) -> Option<Draft> {
    let text = fs::read_to_string(file(dir, id)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    // Missing fields become empty rather than the row's value. The shipped code
    // falls back to the row, which reads a partial record as "no change"; here a
    // partial record reads as "changed" and is shown to the user. Wrong in the
    // safer direction: the draft is written by `write` above and always complete,
    // so this only affects hand-edited files, where asking beats deleting.
    let field = |name: &str| {
        value
            .get(name)
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string()
    };
    Some(Draft {
        title: field("title"),
        content: field("content"),
        tags: field("tags"),
        timestamp: value
            .get("timestamp")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
    })
}

/// Drop the draft for `id`. Missing is success -- the caller wanted it gone.
pub fn clear(dir: &Path, id: &str) {
    let _ = fs::remove_file(file(dir, id));
}

/// Decide what to do with the draft for `row`.
///
/// Reads, and deletes the two cases that can never apply. Deleting here rather
/// than in the caller keeps "the file is gone" and "the verdict says so" one
/// action, so a gate cannot observe a stale draft and a prompt for it at once.
pub fn inspect(dir: &Path, id: &str, row: &NoteRow) -> Verdict {
    let Some(draft) = read(dir, id) else {
        return Verdict::Quiet;
    };

    let changed = draft.title != row.title
        || draft.content != row.content
        || note_text::normalize_tags(&draft.tags) != note_text::normalize_tags(&row.tags);
    if !changed {
        // The last save wrote this row and the deletion did not happen (a crash
        // between the two). There is nothing to recover.
        clear(dir, id);
        return Verdict::Removed("identical");
    }

    match clock::unix_ms(&row.updated_at) {
        Some(ms) if (draft.timestamp as i64) < ms => {
            // Written before an edit that already reached the database: the row is
            // the newer text, and the draft would take the note backwards.
            clear(dir, id);
            Verdict::Removed("older")
        }
        // An unreadable `updated_at` keeps the draft: see `clock::unix_ms`.
        _ => Verdict::Ask(draft),
    }
}

/// Current text with the backup body appended, tags unioned, title unchanged.
///
/// `onMergeDraft` in `useNotesArchive.ts` builds exactly this: the two bodies
/// joined by a `--- 备份内容 ---` line, both tag lists merged without duplicates.
/// The separator is a literal the shipped build writes into the note, so it is
/// user-visible text, not a comment.
pub fn merge(current: &str, draft: &Draft) -> String {
    let parsed = note_text::parse(&note_text::normalize_newlines(current));
    let body = parsed.content.trim_end_matches('\n');
    let backup = draft.content.trim_end_matches('\n');

    let mut parts: Vec<&str> = Vec::new();
    if !body.trim().is_empty() {
        parts.push(body);
    }
    parts.push("--- 备份内容 ---");
    if !backup.trim().is_empty() {
        parts.push(backup);
    }

    let tags = note_text::normalize_tags(&format!("{} {}", parsed.tags, draft.tags));
    note_text::format_for_display(&parts.join("\n"), &tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "hub-backup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn row(title: &str, content: &str, tags: &str, updated_at: &str) -> NoteRow {
        NoteRow {
            id: "note-1".to_string(),
            title: title.to_string(),
            content: content.to_string(),
            tags: tags.to_string(),
            created_at: "2026-11-26T00:00:00.000000000+00:00".to_string(),
            updated_at: updated_at.to_string(),
            is_pinned: false,
            deleted_at: None,
        }
    }

    const NEWER: &str = "2026-11-26T10:00:00.000000000+00:00";

    #[test]
    fn a_written_draft_reads_back_with_all_four_fields() {
        let dir = temp_dir();
        let draft = Draft::of("买菜\n第二行\n# 生活 重要", 1_764_000_000_000);
        write(&dir, "note-1", &draft).expect("write");
        let back = read(&dir, "note-1").expect("read");
        assert_eq!(back, draft);
        assert_eq!(back.title, "买菜");
        assert_eq!(back.tags, "生活 重要");
    }

    #[test]
    fn the_display_text_round_trips_so_recovery_is_not_a_rewrite() {
        for text in ["标题", "标题\n正文", "标题\n正文\n# a b", "  两边有空格  "] {
            let draft = Draft::of(text, 0);
            assert_eq!(draft.display(), text, "{text:?} did not survive the trip");
        }
    }

    #[test]
    fn a_draft_that_matches_the_row_is_deleted_without_a_prompt() {
        // The crash-between-save-and-delete case. Asking here would show a diff of
        // nothing, and the user learns to click through the dialog.
        let dir = temp_dir();
        let draft = Draft::of("标题\n正文\n# 生活", 1_764_000_000_000);
        write(&dir, "note-1", &draft).expect("write");

        let verdict = inspect(&dir, "note-1", &row("标题", "标题\n正文", "生活", NEWER));
        assert_eq!(verdict, Verdict::Removed("identical"));
        assert!(read(&dir, "note-1").is_none(), "the identical draft survived");
    }

    #[test]
    fn a_draft_older_than_the_row_is_deleted_without_a_prompt() {
        // `updated_at` is 10:00; the draft was written at 09:59 the same day.
        let dir = temp_dir();
        let ms = clock::unix_ms("2026-11-26T10:00:00.000000000+00:00").unwrap() as u64;
        write(&dir, "note-1", &Draft::of("标题\n备份", ms - 60_000)).expect("write");

        assert_eq!(
            inspect(&dir, "note-1", &row("标题", "标题\n当前", "", NEWER)),
            Verdict::Removed("older")
        );
        assert!(read(&dir, "note-1").is_none());
    }

    #[test]
    fn a_newer_draft_that_differs_is_offered() {
        let dir = temp_dir();
        let ms = clock::unix_ms(NEWER).unwrap() as u64;
        let draft = Draft::of("标题\n备份正文", ms + 1_000);
        write(&dir, "note-1", &draft).expect("write");

        match inspect(&dir, "note-1", &row("标题", "标题\n当前正文", "", NEWER)) {
            Verdict::Ask(offered) => assert_eq!(offered.content, "标题\n备份正文"),
            other => panic!("expected Ask, got {other:?}"),
        }
        assert!(read(&dir, "note-1").is_some(), "asking must not delete");
    }

    #[test]
    fn a_tag_only_change_still_counts_as_a_change() {
        // The row's tags are column-separated spaces; a draft written from a body
        // with `# a  b` would compare unequal as text but equal as tags.
        let dir = temp_dir();
        let draft = Draft::of("标题\n正文\n# 生活  工作", 0);
        write(&dir, "note-1", &draft).expect("write");
        assert_eq!(
            inspect(&dir, "note-1", &row("标题", "标题\n正文", "生活 工作", "")),
            Verdict::Removed("identical")
        );

        let draft = Draft::of("标题\n正文\n# 生活", 0);
        write(&dir, "note-1", &draft).expect("write");
        assert!(matches!(
            inspect(&dir, "note-1", &row("标题", "标题\n正文", "工作", "")),
            Verdict::Ask(_)
        ));
    }

    #[test]
    fn an_unreadable_file_is_left_alone() {
        // Half-written JSON is the only case where the content might still be
        // salvageable by hand; deleting it would destroy the user's only copy.
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(file(&dir, "note-1"), "{\"title\": \"没写完").unwrap();
        assert_eq!(inspect(&dir, "note-1", &row("标题", "正文", "", NEWER)), Verdict::Quiet);
        assert!(file(&dir, "note-1").exists(), "an unreadable draft was deleted");
    }

    #[test]
    fn merge_joins_the_bodies_and_unions_the_tags() {
        let draft = Draft::of("备份标题\n备份正文\n# 工作 重要", 0);
        let merged = merge("当前标题\n当前正文\n# 生活 重要", &draft);
        assert_eq!(
            merged,
            "当前标题\n当前正文\n--- 备份内容 ---\n备份标题\n备份正文\n# 生活 重要 工作"
        );
    }

    #[test]
    fn merge_on_an_empty_current_body_starts_with_the_separator() {
        let draft = Draft::of("备份正文", 0);
        assert_eq!(merge("", &draft), "--- 备份内容 ---\n备份正文");
    }
}
