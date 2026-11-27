//! The launcher: the action panel that opens at the cursor.
//!
//! ## What this widget is responsible for
//!
//! Drawing and the selection model, nothing else. Which commands exist, what the
//! recent notes are, and what a run *means* are the caller's -- the panel returns
//! an [`Action`] and the app layer decides whether that spawns a note or opens a
//! workbench tab. Keeping the split here is what lets the selection rules be
//! tested without a window (see the tests at the bottom).
//!
//! ## The three rules that are easy to get wrong
//!
//! * **The query is the only thing that can be typed.** Arrow keys move the
//!   selection over whatever the query matched, in one flat list of commands
//!   followed by notes; the two sections are drawn apart but the keyboard does
//!   not care, because "press down three times" should not have a dead zone at a
//!   section boundary.
//! * **Enter with nothing selected does nothing.** Not "runs the first card":
//!   the first card is 新建笔记, and an accidental Enter on a query that matched
//!   nothing must not create an empty note.
//! * **Every reveal starts clean.** The query and the selection are reset when
//!   the panel opens, so the second `Ctrl+Alt+Space` never shows the previous
//!   search -- that reads as "my typing is being remembered" and it would make
//!   the first Enter do something unexpected.

use egui::{Align2, CornerRadius, FontId, Key, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::manager::Tab;
use crate::theme::Palette;

/// Inner padding; must match `hub-core::layout::PanelSpec::padding`, because the
/// column maths counts it.
pub const PANEL_PADDING: f32 = 16.0;
const CARD_GAP: f32 = 8.0;
const HEADER_HEIGHT: f32 = 46.0;
const SEARCH_HEIGHT: f32 = 44.0;
const CARD_HEIGHT: f32 = 76.0;
const ROW_HEIGHT: f32 = 34.0;
const SECTION_GAP: f32 = 12.0;
const FOOTER_HEIGHT: f32 = 28.0;

/// Recent notes shown before the list has to scroll. The panel is 860 logical
/// pixels tall; more than this and the grid scrolls out of reach of the cursor.
pub const RECENT_LIMIT: usize = 5;

/// What the user asked the panel to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nothing this frame.
    None,
    /// Close the panel and go back to whatever was focused before.
    Close,
    NewNote,
    OpenManager(Tab),
    OpenNote(String),
}

/// One row in the panel: a command card or a recent note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub title: String,
    /// The chord for a command, the timestamp for a note.
    pub hint: String,
    /// Lower-cased text the query is matched against. For a command this is the
    /// title plus the words a user might type instead of it ("new", "note"), for
    /// a note it is the title plus a preview of its body.
    haystack: String,
    pub action: Action,
    /// Notes are drawn as list rows; commands as grid cards.
    pub is_note: bool,
}

impl Item {
    pub fn command(title: &str, hint: &str, keywords: &str, action: Action) -> Self {
        Self {
            title: title.to_string(),
            hint: hint.to_string(),
            haystack: format!("{title} {keywords}").to_lowercase(),
            action,
            is_note: false,
        }
    }

    /// A note row. `body` is the whole note text and only feeds the match, so a
    /// word from the middle of a note is found here the way it is in the switcher
    /// (whose haystack is also the full content): a launcher that searches only
    /// titles would make the panel's search look broken next to Ctrl+K's.
    pub fn note(id: &str, title: &str, when: &str, preview: &str, body: &str) -> Self {
        Self {
            title: title.to_string(),
            hint: when.to_string(),
            haystack: format!("{title} {preview} {body}").to_lowercase(),
            action: Action::OpenNote(id.to_string()),
            is_note: true,
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty() || self.haystack.contains(&query)
    }
}

#[derive(Clone)]
pub struct State {
    pub query: String,
    /// Index into the *visible* list, not into `items`: the query reorders what
    /// the arrows walk, so an index into `items` would have to be translated on
    /// every keystroke and would skip rows when the list shrinks.
    pub cursor: usize,
    pub columns: u8,
    pub palette: Palette,
    pub lite: bool,
    /// The chord that opens the panel, shown top-right. An empty string hides it.
    pub open_hint: String,
    /// Bumped by the caller on every reveal; drawn as a footer counter only when
    /// it is non-zero, which is what the spike evidence used.
    pub reveal_count: u64,
    pub items: Vec<Item>,
    /// Take the keyboard on the next draw. Set by [`State::on_open`], cleared by
    /// the draw that uses it -- requesting focus every frame would fight the
    /// user's own clicks inside the query box.
    pub take_focus: bool,
    /// Total rows drawn last pass, exposed for the benches: a panel that paints
    /// twelve cards and paints zero look identical from the outside.
    pub drawn_rows: usize,
    /// Whether the query box owned the keyboard on the frame just drawn.
    ///
    /// A reveal paints before the text box can take focus, so a keystroke that
    /// arrives in between is consumed by egui with nowhere to go. The shell logs
    /// the frame this flips to true: that is the first moment typing lands, and a
    /// gate that types earlier is measuring its own impatience.
    pub keyboard: bool,
}

impl State {
    pub fn new(columns: u8, lite: bool) -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            columns: columns.clamp(1, 6),
            palette: if lite {
                Palette::lite()
            } else {
                Palette::standard()
            },
            lite,
            open_hint: String::new(),
            reveal_count: 0,
            items: Vec::new(),
            take_focus: false,
            drawn_rows: 0,
            keyboard: false,
        }
    }

    /// A fresh panel: no query, cursor on the first row, keyboard wanted.
    pub fn on_open(&mut self) {
        self.query.clear();
        self.cursor = 0;
        self.take_focus = true;
    }

    /// Replace the rows and keep the selection valid.
    ///
    /// Called on every reveal, because a rebind changes the hint and a note
    /// created in the meantime changes the recent list -- but a *running* panel
    /// is not refreshed under the user's hands.
    pub fn set_items(&mut self, items: Vec<Item>) {
        self.items = items;
        self.clamp_cursor();
    }

    /// Indexes of the items the query matches, in drawing order.
    pub fn visible(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.matches(&self.query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn selected(&self) -> Option<&Item> {
        let visible = self.visible();
        visible.get(self.cursor).map(|index| &self.items[*index])
    }

    /// Move over the visible rows, wrapping at both ends.
    ///
    /// Wrapping rather than clamping because the list is short and the panel is
    /// the only thing on screen: a cursor stuck at the bottom reads as "there is
    /// nothing below", which is true only when the list really ends.
    pub fn move_cursor(&mut self, delta: isize) {
        let len = self.visible().len();
        if len == 0 {
            self.cursor = 0;
            return;
        }
        let len = len as isize;
        let next = (self.cursor as isize + delta).rem_euclid(len);
        self.cursor = next as usize;
    }

    /// What Enter means right now.
    pub fn run_selected(&self) -> Action {
        match self.selected() {
            Some(item) => item.action.clone(),
            None => Action::None,
        }
    }

    fn clamp_cursor(&mut self) {
        let len = self.visible().len();
        if len == 0 {
            self.cursor = 0;
        } else if self.cursor >= len {
            self.cursor = len - 1;
        }
    }
}

/// Text that must be rasterised before the first visible frame.
///
/// The fixed chrome plus every command label. Recent notes are *not* here on
/// purpose: their titles come from the database, and §5.8 caps the pre-rasterised
/// set at the characters the first screen always shows.
pub fn first_screen_text(items: &[Item]) -> Vec<String> {
    let mut out = vec![
        "easyflowhub".to_string(),
        "动作面板".to_string(),
        "搜索动作、笔记".to_string(),
        "动作".to_string(),
        "最近笔记".to_string(),
        "Enter 打开".to_string(),
        "Esc 关闭".to_string(),
        "方向键 选择".to_string(),
        "没有匹配的结果".to_string(),
    ];
    for item in items.iter().filter(|item| !item.is_note) {
        out.push(item.title.clone());
        out.push(item.hint.clone());
    }
    out
}

/// Draw one frame and return what the user asked for.
pub fn draw(ctx: &egui::Context, state: &mut State) -> Action {
    let palette = state.palette;
    let mut action = key_action(ctx, state);

    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(palette.panel)
                .inner_margin(egui::Margin::same(PANEL_PADDING as i8)),
        )
        .show(ctx, |ui| {
            ui.set_clip_rect(ctx.screen_rect());
            draw_header(ui, state);
            ui.add_space(6.0);
            draw_search(ui, state);
            ui.add_space(SECTION_GAP);

            let visible = state.visible();
            state.clamp_cursor();
            if visible.is_empty() {
                draw_empty(ui, state);
                state.drawn_rows = 0;
            } else {
                let commands: Vec<usize> = visible
                    .iter()
                    .copied()
                    .filter(|index| !state.items[*index].is_note)
                    .collect();
                let notes: Vec<usize> = visible
                    .iter()
                    .copied()
                    .filter(|index| state.items[*index].is_note)
                    .collect();
                state.drawn_rows = commands.len() + notes.len();

                if !commands.is_empty() {
                    section_label(ui, state, "动作");
                    ui.add_space(4.0);
                    let clicked = draw_grid(ui, state, &commands);
                    if !matches!(clicked, Action::None) {
                        action = clicked;
                    }
                    ui.add_space(SECTION_GAP);
                }
                if !notes.is_empty() {
                    section_label(
                        ui,
                        state,
                        if state.query.trim().is_empty() {
                            "最近笔记"
                        } else {
                            "匹配的笔记"
                        },
                    );
                    ui.add_space(4.0);
                    let clicked = draw_notes(ui, state, &notes);
                    if !matches!(clicked, Action::None) {
                        action = clicked;
                    }
                }
            }

            draw_footer(ui, state);
        });

    action
}

/// Arrow keys and Enter, read before the panel is laid out so a selection move
/// and the paint that shows it happen in the same frame.
fn key_action(ctx: &egui::Context, state: &mut State) -> Action {
    if ctx.input(|input| input.key_pressed(Key::Escape)) {
        return Action::Close;
    }
    if ctx.input(|input| input.key_pressed(Key::ArrowDown)) {
        state.move_cursor(1);
    }
    if ctx.input(|input| input.key_pressed(Key::ArrowUp)) {
        state.move_cursor(-1);
    }
    if ctx.input(|input| input.key_pressed(Key::Enter)) {
        let action = state.run_selected();
        if matches!(action, Action::None) {
            // egui hands the keyboard back when Enter is pressed in a single-line
            // editor, on the assumption that the text was submitted somewhere. With
            // nothing selected there is nowhere to hand it to, and without this the
            // panel stays on screen but ignores every key after it -- which reads as
            // a frozen window, not as "that query matched nothing".
            state.take_focus = true;
        }
        return action;
    }
    Action::None
}

fn draw_header(ui: &mut egui::Ui, state: &State) {
    let palette = state.palette;
    ui.allocate_ui(Vec2::new(ui.available_width(), HEADER_HEIGHT), |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("easyflowhub")
                    .size(17.0)
                    .color(palette.ink)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(RichText::new("动作面板").size(12.0).color(palette.ink_subtle));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !state.open_hint.is_empty() {
                    ui.label(
                        RichText::new(&state.open_hint)
                            .size(11.0)
                            .color(palette.ink_subtle),
                    );
                }
            });
        });
    });
    let sep_y = ui.cursor().top();
    ui.painter().hline(
        ui.max_rect().x_range(),
        sep_y,
        Stroke::new(1.0, palette.border),
    );
}

fn draw_search(ui: &mut egui::Ui, state: &mut State) {
    let palette = state.palette;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, SEARCH_HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(10), palette.panel_strong);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0, palette.border),
        StrokeKind::Inside,
    );

    let edit_rect = rect.shrink2(Vec2::new(12.0, 6.0));
    let edit = egui::TextEdit::singleline(&mut state.query)
        .frame(false)
        .hint_text("搜索动作、笔记")
        .font(FontId::proportional(14.0))
        .text_color(palette.ink)
        .margin(egui::Margin::symmetric(2, 8))
        .id_source("launcher_query");
    let response = ui.put(edit_rect, edit);
    if state.take_focus {
        response.request_focus();
        state.take_focus = false;
    }
    state.keyboard = response.has_focus();
}

fn section_label(ui: &mut egui::Ui, state: &State, text: &str) {
    ui.label(
        RichText::new(text)
            .size(11.0)
            .color(state.palette.ink_subtle),
    );
}

fn draw_grid(ui: &mut egui::Ui, state: &mut State, rows: &[usize]) -> Action {
    let palette = state.palette;
    let columns = state.columns.max(1) as usize;
    let available = ui.available_width();
    let cell_width = (available - CARD_GAP * (columns as f32 - 1.0)) / columns as f32;
    let count = rows.len();
    let lines = count.div_ceil(columns);
    let grid_height = lines as f32 * CARD_HEIGHT + (lines as f32 - 1.0).max(0.0) * CARD_GAP;
    let (area, _) = ui.allocate_exact_size(Vec2::new(available, grid_height), Sense::hover());
    let origin = area.min;

    let mut action = Action::None;
    for (slot, item_index) in rows.iter().enumerate() {
        let row = slot / columns;
        let column = slot % columns;
        // A trailing row with fewer cards keeps the cell width: a card that
        // stretches to fill the gap reads as a different kind of card.
        let min = origin
            + Vec2::new(
                column as f32 * (cell_width + CARD_GAP),
                row as f32 * (CARD_HEIGHT + CARD_GAP),
            );
        let rect = Rect::from_min_size(min, Vec2::new(cell_width, CARD_HEIGHT));
        let selected = state.cursor == slot;

        let response = ui.interact(rect, ui.id().with(("card", item_index)), Sense::click());
        let fill = if selected || response.hovered() {
            palette.panel_strong
        } else {
            palette.panel_strong.gamma_multiply(0.72)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(11), fill);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(11),
            Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected { palette.accent } else { palette.border },
            ),
            StrokeKind::Inside,
        );

        let item = &state.items[*item_index];
        ui.painter().text(
            rect.left_top() + Vec2::new(11.0, 13.0),
            Align2::LEFT_TOP,
            &item.title,
            FontId::proportional(13.5),
            palette.ink,
        );
        ui.painter().text(
            rect.left_bottom() + Vec2::new(11.0, -12.0),
            Align2::LEFT_BOTTOM,
            &item.hint,
            FontId::proportional(10.5),
            palette.ink_subtle,
        );

        if response.clicked() {
            state.cursor = slot;
            action = item.action.clone();
        }
    }
    action
}

fn draw_notes(ui: &mut egui::Ui, state: &mut State, rows: &[usize]) -> Action {
    let palette = state.palette;
    let width = ui.available_width();
    let height = rows.len() as f32 * ROW_HEIGHT;
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let mut action = Action::None;
    // The cursor counts commands first, so a note's slot is offset by them.
    let offset = state
        .visible()
        .iter()
        .take_while(|index| !state.items[**index].is_note)
        .count();

    for (line, item_index) in rows.iter().enumerate() {
        let rect = Rect::from_min_size(
            area.min + Vec2::new(0.0, line as f32 * ROW_HEIGHT),
            Vec2::new(width, ROW_HEIGHT),
        );
        let slot = offset + line;
        let selected = state.cursor == slot;
        let response = ui.interact(rect, ui.id().with(("note", item_index)), Sense::click());
        if selected || response.hovered() {
            ui.painter().rect_filled(
                rect.shrink2(Vec2::new(0.0, 1.0)),
                CornerRadius::same(7),
                palette.panel_strong,
            );
        }
        if selected {
            ui.painter().rect_stroke(
                rect.shrink2(Vec2::new(0.0, 1.0)),
                CornerRadius::same(7),
                Stroke::new(1.0, palette.accent),
                StrokeKind::Inside,
            );
        }
        let item = &state.items[*item_index];
        let title = elide(&item.title, 34);
        ui.painter().text(
            rect.left_center() + Vec2::new(10.0, 0.0),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(13.0),
            palette.ink,
        );
        ui.painter().text(
            rect.right_center() + Vec2::new(-10.0, 0.0),
            Align2::RIGHT_CENTER,
            &item.hint,
            FontId::proportional(10.5),
            palette.ink_subtle,
        );
        if response.clicked() {
            state.cursor = slot;
            action = item.action.clone();
        }
    }
    action
}

fn draw_empty(ui: &mut egui::Ui, state: &State) {
    let palette = state.palette;
    let height = 90.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "没有匹配的结果",
        FontId::proportional(13.0),
        palette.ink_subtle,
    );
}

fn draw_footer(ui: &mut egui::Ui, state: &State) {
    let palette = state.palette;
    // Pushed to the bottom regardless of how much the list used, so the hint does
    // not jump up and down as the query grows and shrinks the result set.
    let y = ui.max_rect().bottom() - FOOTER_HEIGHT;
    let rect = Rect::from_min_max(
        egui::pos2(ui.max_rect().left(), y),
        egui::pos2(ui.max_rect().right(), ui.max_rect().bottom()),
    );
    ui.painter().text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        "Enter 打开",
        FontId::proportional(11.0),
        palette.ink_subtle,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(82.0, 0.0),
        Align2::LEFT_CENTER,
        "Esc 关闭",
        FontId::proportional(11.0),
        palette.ink_subtle,
    );
    ui.painter().text(
        rect.right_center(),
        Align2::RIGHT_CENTER,
        "方向键 选择",
        FontId::proportional(11.0),
        palette.ink_subtle,
    );
}

/// Cut a title to `max` characters, adding an ellipsis.
///
/// `chars()` rather than byte slicing: a note title is Chinese more often than
/// not, and a byte slice through a CJK codepoint panics.
fn elide(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<Item> {
        vec![
            Item::command(
                "新建笔记",
                "Ctrl+Alt+N",
                "new note 笔记",
                Action::NewNote,
            ),
            Item::command(
                "工作台",
                "Ctrl+Alt+M",
                "manager 管理",
                Action::OpenManager(Tab::Notes),
            ),
            Item::note(
                "id-a",
                "周末计划",
                "11-26 02:31",
                "买螺丝刀和膨胀管",
                "周末计划\n买螺丝刀和膨胀管，顺手把窗台的合页也换了",
            ),
        ]
    }

    fn state() -> State {
        let mut state = State::new(4, false);
        state.set_items(items());
        state
    }

    #[test]
    fn the_query_matches_titles_keywords_and_note_bodies() {
        let state = state();
        assert_eq!(state.visible().len(), 3, "empty query shows everything");

        let mut by_title = state.clone();
        by_title.query = "笔记".to_string();
        assert_eq!(by_title.visible(), vec![0], "only the command title has the word");

        let mut by_keyword = state.clone();
        by_keyword.query = "manager".to_string();
        assert_eq!(by_keyword.visible(), vec![1]);

        let mut by_body = state.clone();
        by_body.query = "螺丝刀".to_string();
        assert_eq!(by_body.visible(), vec![2]);
    }

    #[test]
    fn filtering_is_case_insensitive_and_trims() {
        let mut state = state();
        state.query = "  NEW  ".to_string();
        assert_eq!(state.visible(), vec![0], "keywords are lower-cased once, at build");
        state.query = "Ctrl+Alt+N".to_string();
        assert!(
            state.visible().is_empty(),
            "the chord is a hint, not something to search by"
        );
    }

    #[test]
    fn the_cursor_wraps_and_clamps_to_the_visible_list() {
        let mut state = state();
        state.move_cursor(-1);
        assert_eq!(state.cursor, 2, "up from the top wraps to the last row");
        state.move_cursor(1);
        assert_eq!(state.cursor, 0);

        state.query = "管理器".to_string();
        assert!(state.visible().is_empty());
        state.move_cursor(1);
        assert_eq!(state.cursor, 0, "an empty list has no second row");
    }

    #[test]
    fn enter_on_an_empty_result_runs_nothing() {
        let mut state = state();
        state.query = "没有这个".to_string();
        assert_eq!(state.run_selected(), Action::None);
    }

    #[test]
    fn enter_runs_the_row_under_the_cursor() {
        let mut state = state();
        state.move_cursor(1);
        assert_eq!(state.run_selected(), Action::OpenManager(Tab::Notes));
        state.move_cursor(1);
        assert_eq!(state.run_selected(), Action::OpenNote("id-a".to_string()));
    }

    #[test]
    fn reopening_clears_the_query_and_the_selection() {
        let mut state = state();
        state.query = "螺丝刀".to_string();
        state.move_cursor(1);
        state.on_open();
        assert!(state.query.is_empty());
        assert_eq!(state.cursor, 0);
        assert!(state.take_focus);
    }

    #[test]
    fn replacing_the_items_keeps_the_cursor_inside_the_new_list() {
        let mut state = state();
        state.cursor = 2;
        state.set_items(vec![items()[0].clone()]);
        assert_eq!(state.cursor, 0, "one row means the cursor is row zero");
    }

    #[test]
    fn long_titles_are_cut_on_a_character_boundary() {
        let cut = elide("周末计划要买的东西很多", 4);
        assert_eq!(cut, "周末计划…");
        assert_eq!(elide("短", 4), "短");
    }

}
