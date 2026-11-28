//! One row, as the list needs it: the four strings a row draws plus the search
//! haystack.
//!
//! Split from `manager.rs` when the parent crossed the 800-line ceiling. The row
//! model is the natural piece to move: it is the only thing in the workbench that
//! is data rather than layout, and `list.rs` already owns the drawing of it.
//!
//! ## One row type, three sources
//!
//! The trash holds notes, capture items and scripts, and they arrive from three
//! places: a table, another table, and a directory of manifests. Drawing three row
//! types would triple the densest code in the window for no visible difference --
//! the sub-tab already says which kind you are looking at, so a row does not label
//! itself. What the *controller* needs is the difference, because restoring a note
//! and restoring a script are not the same write: that is [`RowKind`], and it is
//! one field.

/// Which store a row came from.
///
/// Carried on the row rather than inferred from the sub-tab at the click: an action
/// travels to the controller, which does not read view state, and an id alone does
/// not say which restore to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowKind {
    #[default]
    Note,
    Inbox,
    Script,
}

impl RowKind {
    /// Stable id: it ends up in log lines (`manager_restore kind=scripts`).
    pub fn id(self) -> &'static str {
        match self {
            RowKind::Note => "notes",
            RowKind::Inbox => "inbox",
            RowKind::Script => "scripts",
        }
    }
}

/// One row of the list: a note, a capture item, or a deleted script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub id: String,
    pub title: String,
    pub preview: String,
    pub when: String,
    pub pinned: bool,
    /// `Some` when the row is in the trash.
    pub deleted_at: Option<String>,
    haystack: String,
}

impl Row {
    /// A note.
    pub fn new(
        id: &str,
        title: &str,
        content: &str,
        tags: &str,
        updated_at: &str,
        pinned: bool,
        deleted_at: Option<String>,
    ) -> Self {
        let display = hub_core::note_text::display_title(title, content);
        Self {
            kind: RowKind::Note,
            id: id.to_string(),
            preview: preview_of(content, &display),
            title: display.clone(),
            when: short_time(updated_at),
            pinned,
            deleted_at,
            haystack: format!("{display}\n{content}\n{tags}").to_lowercase(),
        }
    }

    /// The word for a capture's `kind` column.
    ///
    /// Here rather than in the controller because it is display text, and because
    /// [`Row::inbox`] is the only caller -- a label that lives away from the
    /// constructor that needs it drifts the first time the schema gains a kind.
    /// An unknown kind is shown as it is written: inventing "未知" would hide a
    /// newer capture from a newer build.
    pub fn inbox_label(kind: &str) -> &str {
        match kind {
            "text" => "文字",
            "link" => "链接",
            "image" => "图片",
            "file" => "文件",
            "audio" => "录音",
            other => other,
        }
    }

    /// A captured item in the Inbox's trash.
    ///
    /// `kind_label` is the item's kind in words (文字 / 链接 / 图片 / 文件 / 录音).
    /// It is the title when the capture has none -- the shipped panel does the same,
    /// because a row that reads "" is a row you cannot pick out of a list.
    pub fn inbox(id: &str, title: &str, content: &str, kind_label: &str, deleted_at: &str) -> Self {
        let title = title.trim();
        let display = if title.is_empty() {
            kind_label.to_string()
        } else {
            title.to_string()
        };
        Self {
            kind: RowKind::Inbox,
            id: id.to_string(),
            // No `display_title` here: an Inbox item's `title` column is already the
            // title. `display_title` exists for notes, whose title lives in the body.
            preview: preview_of(content, &display),
            title: display.clone(),
            when: short_time(deleted_at),
            pinned: false,
            deleted_at: Some(deleted_at.to_string()),
            haystack: format!("{display}\n{content}\n{kind_label}").to_lowercase(),
        }
    }

    /// A deleted script, as its manifest describes it.
    ///
    /// The second line is the original path, because that is the question a script
    /// in the trash raises: "where will this go back to". The size the manifest
    /// carries is deliberately not shown -- a row has one line for it and the path
    /// is the more useful one.
    pub fn script(id: &str, name: &str, path: &str, deleted_at: &str) -> Self {
        let display = if name.trim().is_empty() {
            id.to_string()
        } else {
            name.to_string()
        };
        Self {
            kind: RowKind::Script,
            id: id.to_string(),
            title: display.clone(),
            preview: path.to_string(),
            when: short_time(deleted_at),
            pinned: false,
            deleted_at: Some(deleted_at.to_string()),
            haystack: format!("{display}\n{path}").to_lowercase(),
        }
    }

    /// Whether this row answers `query`. `pub(super)` because the tests live in
    /// `manager::tests`, a sibling of this module rather than a child of it.
    pub(super) fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty() || self.haystack.contains(&query)
    }
}

/// The first line of body that is not blank, not a heading, and not the title
/// itself, cut at 64 characters.
///
/// Skipping the title is not cosmetic: a note written as "标题\n正文" is the
/// common shape in this product, and without the check the row reads
/// "周报 / 周报" -- the preview says nothing and costs a line to say it.
fn preview_of(content: &str, title: &str) -> String {
    const MAX: usize = 64;
    let title = title.trim();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line == title {
            continue;
        }
        let mut out: String = line.chars().take(MAX).collect();
        if line.chars().count() > MAX {
            out.push('…');
        }
        return out;
    }
    String::new()
}

/// `2026-11-26T02:31:00…` becomes `11-26 02:31`. A timestamp is read at a glance,
/// and the year is almost never the interesting part.
fn short_time(stamp: &str) -> String {
    let bytes = stamp.as_bytes();
    if bytes.len() >= 16 && bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T' {
        return format!("{} {}", &stamp[5..10], &stamp[11..16]);
    }
    stamp.to_string()
}
