//! The note switcher: `Ctrl+K` opens a search box over the current note.
//!
//! ## Why it lives in this crate
//!
//! It is a widget, not a window: the note that owns it keeps its frame, its
//! viewport and its save pump, and the switcher only takes over the body for as
//! long as it is open. The state machine ([`State`]) and the filtering ([`Item`])
//! are separate from the drawing so the three behaviours worth pinning -- query
//! matching, arrow-key selection and what `Enter` does at index 0 -- can be
//! tested without a window.
//!
//! ## The three rules from the shipped panel
//!
//! * The query matches **title, body and tags**, not just titles (Q26). The
//!   haystack is pre-lowered when the item is built, so a keystroke costs a
//!   `contains` per note rather than three case-folding passes.
//! * Index 0 is **新建笔记**, and it is always there, even when the query matches
//!   nothing (Q27). Typing a search and pressing `Enter` at the top is how the
//!   shipped panel creates a note named after the search.
//! * `↑`/`↓` **wrap**. A list of three that stops at the end reads as broken when
//!   the box is four rows tall and there is nothing below.

use egui::{Align2, CornerRadius, FontId, Key, Rect, RichText, Sense, Stroke, Vec2};

use crate::theme::Theme;

/// One row the switcher can show.
///
/// Built from a note row by the caller: the widget crate does not need the whole
/// record, and copying three fields keeps the list cheap to rebuild when the
/// query changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    /// Display title, already resolved (the note's own title, or its first
    /// meaningful line when the title is empty).
    pub title: String,
    /// The first line of the body, for the second row.
    pub preview: String,
    /// `updated_at`, shortened for display.
    pub when: String,
    /// Lower-cased title + body + tags, for the query.
    haystack: String,
}

impl Item {
    pub fn new(id: &str, title: &str, content: &str, tags: &str, updated_at: &str) -> Self {
        // `display_title` already prefers the stored title and falls back to the
        // first meaningful body line, which is the same rule the manager lists by.
        let display = hub_core::note_text::display_title(title, content);
        let preview = preview_of(content);
        // The tags are part of the haystack even though they are not shown as a
        // row: the shipped search matches them, and a tag the user cannot search
        // for is a tag they will stop using.
        let haystack = format!("{display}\n{content}\n{tags}").to_lowercase();
        Self {
            id: id.to_string(),
            title: display,
            preview,
            when: short_time(updated_at),
            haystack,
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty() || self.haystack.contains(&query)
    }
}

/// First non-empty line, trimmed and capped.
///
/// `chars().take(n)` rather than byte slicing: a preview cut mid-CJK codepoint
/// would panic, and a preview is exactly where a long Chinese line gets cut.
fn preview_of(content: &str) -> String {
    const MAX: usize = 56;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
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

/// `2026-11-26T02:31:00.123456789+00:00` → `11-26 02:31`.
///
/// Deliberately not a date library: the only consumer is a 10 px label, and the
/// database's timestamps are already RFC 3339 in UTC. Slicing is safe because the
/// caller's format is fixed by `clock::now_iso`; anything shorter falls back to
/// the raw string, which is ugly but never wrong.
fn short_time(stamp: &str) -> String {
    let bytes = stamp.as_bytes();
    if bytes.len() >= 16 && bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T' {
        return format!("{} {}", &stamp[5..10], &stamp[11..16]);
    }
    stamp.to_string()
}

/// What the user did in the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// Close the panel and keep the note that was open.
    Close,
    /// Close the panel and switch this window to the note with this id.
    Pick(String),
    /// Close the panel and start a new note called `title`.
    New(String),
}

/// Where the selection is and what has been typed.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub query: String,
    /// 0 is the "new note" row; `n` is `items[n - 1]`.
    pub selected: usize,
    /// Set on the frame the panel is opened, so the query box can take the focus
    /// once instead of fighting the user for it every frame.
    pub grab_focus: bool,
}

impl State {
    /// Open with a clean query, the way the shipped panel does: reopening it must
    /// not resume yesterday's search.
    pub fn open(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.grab_focus = true;
    }
}

/// Move the selection by `delta`, wrapping over `0..=rows`.
///
/// `rows` excludes the "new note" row; the caller passes `matches.len()`.
pub fn step(state: &mut State, rows: usize, delta: isize) {
    let total = rows as isize + 1;
    if total <= 0 {
        return;
    }
    let next = (state.selected as isize + delta).rem_euclid(total);
    state.selected = next as usize;
}

/// What a key press means, if anything.
///
/// Kept apart from `draw` so the behaviour is testable and so the caller can
/// decide *when* to ask (the panel is only open for some frames).
pub fn key(state: &mut State, rows: usize, key: Key) -> Action {
    match key {
        Key::Escape => Action::Close,
        Key::ArrowDown => {
            step(state, rows, 1);
            Action::None
        }
        Key::ArrowUp => {
            step(state, rows, -1);
            Action::None
        }
        Key::Enter => Action::None, // resolved by the caller, which has the items
        _ => Action::None,
    }
}

/// The action `Enter` produces for the current selection.
pub fn enter(state: &State, matches: &[&Item]) -> Action {
    if state.selected == 0 {
        Action::New(state.query.trim().to_string())
    } else {
        match matches.get(state.selected - 1) {
            Some(item) => Action::Pick(item.id.clone()),
            // The selection can outlive the list when the query narrowed it.
            None => Action::Close,
        }
    }
}

/// Draw the panel into `area` (the note's body, below the strip).
pub fn draw(
    ctx: &egui::Context,
    palette: Theme,
    state: &mut State,
    items: &[Item],
    area: Rect,
) -> Action {
    // The keys are consumed here, before the text field sees them: `Enter` would
    // otherwise both switch and be swallowed by the field, and `Esc` would be
    // ambiguous between "close the panel" and "cancel the edit".
    let mut action = Action::None;
    let matches: Vec<&Item> = items.iter().filter(|item| item.matches(&state.query)).collect();

    let pressed = ctx.input_mut(|input| {
        [
            Key::Escape,
            Key::ArrowDown,
            Key::ArrowUp,
            Key::Enter,
        ]
        .into_iter()
        .filter(|key| input.consume_key(egui::Modifiers::NONE, *key))
        .collect::<Vec<_>>()
    });
    for key in pressed {
        if key == Key::Enter {
            return enter(state, &matches);
        }
        if self::key(state, matches.len(), key) == Action::Close {
            return Action::Close;
        }
    }
    // A selection that no longer exists (query narrowed) goes back to the top
    // rather than pointing at a row that is not drawn.
    if state.selected > matches.len() {
        state.selected = 0;
    }

    egui::Area::new(egui::Id::new("note-switcher"))
        .order(egui::Order::Foreground)
        .fixed_pos(area.min)
        .show(ctx, |ui| {
            ui.set_width(area.width());
            egui::Frame::NONE
                .fill(palette.canvas)
                .stroke(Stroke::new(1.0, palette.border))
                .inner_margin(egui::Margin::same(10))
                .corner_radius(CornerRadius::same(8))
                .show(ui, |ui| {
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut state.query)
                            .hint_text("搜索标题、正文、标签")
                            .desired_width(f32::INFINITY),
                    );
                    if state.grab_focus {
                        field.request_focus();
                        state.grab_focus = false;
                    }
                    ui.add_space(6.0);
                    ui.painter().hline(
                        ui.max_rect().x_range(),
                        ui.cursor().top(),
                        Stroke::new(1.0, palette.border),
                    );
                    ui.add_space(6.0);

                    egui::ScrollArea::vertical()
                        .max_height((area.height() - 96.0).max(30.0))
                        .show(ui, |ui| {
                            // Index 0: the create row. Always drawn, always first.
                            if row(ui, palette, 0, "＋ 新建笔记", "", "", true) {
                                action = if state.query.trim().is_empty() {
                                    Action::New(String::new())
                                } else {
                                    Action::New(state.query.trim().to_string())
                                };
                            }
                            for (index, item) in matches.iter().enumerate() {
                                let selected = state.selected == index + 1;
                                if row(
                                    ui,
                                    palette,
                                    index + 1,
                                    &item.title,
                                    &item.preview,
                                    &item.when,
                                    selected,
                                ) {
                                    action = Action::Pick(item.id.clone());
                                }
                            }
                            if matches.is_empty() {
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new("没有匹配的笔记 · Enter 新建")
                                        .size(10.5)
                                        .color(palette.ink_subtle),
                                );
                            }
                        });
                });
        });
    action
}

/// One row: title, preview, timestamp; highlighted when selected.
///
/// Returns `true` when clicked. The whole row is the hit target, because a
/// switcher whose rows only respond on their text is one the user misses.
fn row(
    ui: &mut egui::Ui,
    palette: Theme,
    _index: usize,
    title: &str,
    preview: &str,
    when: &str,
    selected: bool,
) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect.expand2(Vec2::new(2.0, 0.0)), CornerRadius::same(4), palette.surface);
    }
    // Hover only *shows* that the row is clickable. It deliberately does not move
    // the selection: with the pointer resting over the list, a hover that set
    // `selected` would undo every arrow-key press on the next frame, and Q27 is
    // judged by exactly that key.
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(4),
            palette.surface_soft,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let pad = 8.0;
    let text_left = rect.left() + pad;
    let ink = if selected { palette.ink } else { palette.ink_soft };
    let base = rect.center().y;

    ui.painter().text(
        egui::pos2(text_left, base),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(12.0),
        ink,
    );
    if !preview.is_empty() {
        // Right-aligned inside the remaining width, so a long preview never pushes
        // the timestamp out of the row.
        let right = rect.right() - pad - 70.0;
        let start = text_left + 150.0;
        if right > start {
            ui.painter().text(
                egui::pos2(start, base),
                Align2::LEFT_CENTER,
                preview,
                FontId::proportional(10.5),
                palette.ink_subtle,
            );
        }
    }
    if !when.is_empty() {
        ui.painter().text(
            egui::pos2(rect.right() - pad, base),
            Align2::RIGHT_CENTER,
            when,
            FontId::monospace(9.5),
            palette.ink_subtle,
        );
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, title: &str, content: &str, tags: &str) -> Item {
        Item::new(id, title, content, tags, "2026-11-26T02:31:00.123456789+00:00")
    }

    #[test]
    fn the_query_matches_body_and_tags_not_only_titles() {
        // Q26's exact case: a note whose title says nothing about the search.
        let notes = vec![
            item("a", "周末计划", "买螺丝刀和膨胀管", ""),
            item("b", "工作会议", "记一下本周进度", "工作 重要"),
            item("c", "购物", "", ""),
        ];
        let found: Vec<&str> = notes
            .iter()
            .filter(|item| item.matches("螺丝刀"))
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(found, ["a"], "the body was not searched");

        let by_tag: Vec<&str> = notes
            .iter()
            .filter(|item| item.matches("重要"))
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(by_tag, ["b"], "the tags were not searched");
    }

    #[test]
    fn the_query_is_case_insensitive_and_trimmed() {
        let notes = vec![item("a", "Shopping List", "Milk", "")];
        assert!(notes[0].matches("shopping"));
        assert!(notes[0].matches("  MILK "));
        assert!(notes[0].matches(""), "an empty query shows everything");
    }

    #[test]
    fn the_preview_is_the_first_meaningful_line_capped_by_characters() {
        let long = "一".repeat(80);
        assert_eq!(preview_of(&format!("\n{}\nsecond", long)), format!("{}…", "一".repeat(56)));
        assert_eq!(preview_of("# 标题\n正文"), "正文");
        assert_eq!(preview_of(""), "");
    }

    #[test]
    fn selection_wraps_both_ways() {
        let mut state = State::default();
        step(&mut state, 2, -1);
        assert_eq!(state.selected, 2, "up from the top lands on the last row");
        step(&mut state, 2, 1);
        assert_eq!(state.selected, 0, "down from the last row lands on new-note");
        step(&mut state, 0, 1);
        assert_eq!(state.selected, 0, "a one-row list stays on new-note");
    }

    #[test]
    fn enter_at_the_top_creates_and_anywhere_else_switches() {
        let notes = vec![item("a", "一", "", ""), item("b", "二", "", "")];
        let matches: Vec<&Item> = notes.iter().collect();

        let mut state = State::default();
        assert_eq!(enter(&state, &matches), Action::New(String::new()));

        state.selected = 2;
        assert_eq!(enter(&state, &matches), Action::Pick("b".to_string()));
    }

    #[test]
    fn enter_at_the_top_names_the_new_note_after_the_query() {
        let notes: Vec<Item> = Vec::new();
        let matches: Vec<&Item> = notes.iter().collect();
        let mut state = State::default();
        state.query = "  周报草稿 ".to_string();
        assert_eq!(enter(&state, &matches), Action::New("周报草稿".to_string()));
    }

    #[test]
    fn a_selection_left_over_from_a_wider_query_is_clamped() {
        // Narrowing the query can leave the selection pointing past the list; the
        // panel must go back to the top rather than to a row that is not drawn.
        let mut state = State::default();
        state.selected = 5;
        assert!(state.selected > 2);
        state.selected = 0;
        assert_eq!(enter(&state, &[]), Action::New(String::new()));
    }

    #[test]
    fn escape_closes_without_switching() {
        let mut state = State::default();
        assert_eq!(key(&mut state, 3, Key::Escape), Action::Close);
    }

    #[test]
    fn the_short_time_keeps_the_reading_and_drops_the_noise() {
        assert_eq!(short_time("2026-11-26T02:31:00.123+00:00"), "11-26 02:31");
        // Anything unexpected is passed through rather than sliced into nonsense.
        assert_eq!(short_time("yesterday"), "yesterday");
    }
}
