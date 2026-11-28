//! One note, as the list needs it: the four strings a row draws plus the search
//! haystack.
//!
//! Split from `manager.rs` when the parent crossed the 800-line ceiling. The row
//! model is the natural piece to move: it is the only thing in the workbench that
//! is data rather than layout, and `list.rs` already owns the drawing of it.

/// One note, as the list needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
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
            id: id.to_string(),
            preview: preview_of(content, &display),
            title: display.clone(),
            when: short_time(updated_at),
            pinned,
            deleted_at,
            haystack: format!("{display}\n{content}\n{tags}").to_lowercase(),
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
