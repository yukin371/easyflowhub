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
//! The fixed chrome (header, query field, empty state, footer) lives in
//! [`chrome`]; this file owns the list, because that is where the cursor is.
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
//!
//! ## What the panel looks like, and why
//!
//! Every row has three states, and they are distinguishable without measuring:
//!
//! | state | fill | outline | extra |
//! |---|---|---|---|
//! | at rest | `surface_soft` | `border` | nothing |
//! | hovered | `surface_hover` | `border_strong` | nothing |
//! | selected | `accent_soft` | `accent` | 2 px accent bar on the left edge |
//!
//! The previous version had "outline", "outline with a slightly lighter fill" and
//! "accent outline" -- a selected row and a hovered row were the same picture
//! unless you went looking for a half-pixel stroke difference, which is not
//! something a user does while typing. The bar exists because a tinted fill alone
//! still reads as "this card is special" rather than "this is where Enter goes".
//!
//! Colours come from [`crate::theme::Theme`] and nowhere else: no call site in
//! this file multiplies a colour by a coefficient (see that module's comment for
//! why the coefficients had to go).

use egui::{Align2, CornerRadius, FontId, Key, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::manager::Tab;
use crate::theme::Theme;

mod chrome;
mod translate;

/// Inner padding; must match `hub-core::layout::PanelSpec::padding`, because the
/// column maths counts it.
pub const PANEL_PADDING: f32 = 16.0;
const CARD_GAP: f32 = 8.0;
const CARD_HEIGHT: f32 = 64.0;
const ROW_HEIGHT: f32 = 36.0;
const SECTION_GAP: f32 = 14.0;

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
    /// Translate the text the panel captured when it opened.
    Translate,
    /// Write the finished translation over that text, in the application it came
    /// from. The one action here that changes somebody else's window.
    ReplaceSelection,
    /// Put the translation on the clipboard. The fallback for a replace that has
    /// nowhere to go -- a target window that has closed, or a selection that was
    /// never there.
    CopyTranslation,
}

/// What the panel knows about the translation it asked for.
///
/// No `Problem` here on purpose: the failure arrives as the sentence the user
/// reads, mapped from the status code by the layer that made the request. This
/// crate draws; it does not decide what went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Translate {
    /// Sent; no answer yet.
    Running,
    Done { text: String },
    Failed { message: String },
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

/// Where the last frame put things.
///
/// The theme gate samples pixels inside these rectangles, so it never has to
/// re-derive this file's spacing constants. A gate that recomputed the layout
/// would go on passing while the panel drew somewhere else -- exactly the bug it
/// is supposed to catch. Empty slots stay `None`: an empty result has no cards,
/// and a gate looking for "the unselected card" must not sample the void.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Layout {
    /// The content rect, inside the panel padding. Its top-left corner is page.
    pub canvas: Option<Rect>,
    pub search: Option<Rect>,
    /// The captured-selection row, present only on a reveal that took text. Its
    /// presence is what a gate samples: the row's fill is a different colour from
    /// the page behind it, so "the capture reached the panel" is a pixel rather
    /// than an inference from a log line.
    pub selection: Option<Rect>,
    /// The translation card, when one is running or has arrived.
    pub translation: Option<Rect>,
    /// The 替换选区 button inside it, which is the one a gate has to click.
    pub replace: Option<Rect>,
    pub selected_card: Option<Rect>,
    pub rest_card: Option<Rect>,
    pub selected_note: Option<Rect>,
    pub rest_note: Option<Rect>,
}

impl Layout {
    /// One line a gate can parse: `x,y,w,h` in **logical points**, `-` for a slot
    /// the frame did not draw.
    ///
    /// Logical, not physical, because that is the unit everything above this file
    /// thinks in. The caller multiplies by the window scale, which it already
    /// knows from bringing the window up -- a gate that divided by a scale it had
    /// guessed would be measuring its own arithmetic.
    pub fn describe(&self) -> String {
        let slot = |r: Option<Rect>| match r {
            Some(r) => format!(
                "{:.0},{:.0},{:.0},{:.0}",
                r.left(),
                r.top(),
                r.width(),
                r.height()
            ),
            None => "-".to_string(),
        };
        format!(
            "canvas={} search={} selection={} translation={} replace={} selected_card={} rest_card={} selected_note={} rest_note={}",
            slot(self.canvas),
            slot(self.search),
            slot(self.selection),
            slot(self.translation),
            slot(self.replace),
            slot(self.selected_card),
            slot(self.rest_card),
            slot(self.selected_note),
            slot(self.rest_note),
        )
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
    pub palette: Theme,
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
    /// Text the panel took from the application underneath when it was opened,
    /// if any. Shown above the list so the user can see the capture landed before
    /// asking an action to use it -- a translate that fails because nothing was
    /// selected should be visible as "nothing was selected", not as an error.
    pub selection: Option<String>,
    /// The translation in flight or on screen, if any. Cleared by [`State::on_open`]
    /// for the same reason the selection is: a result belongs to the reveal that
    /// asked for it, and a stale one under a fresh selection reads as "this is the
    /// translation of what I have selected now".
    pub translate: Option<Translate>,
    /// Geometry of the frame just drawn (see [`Layout`]).
    pub layout: Layout,
}

impl State {
    pub fn new(columns: u8, lite: bool) -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            columns: columns.clamp(1, 6),
            palette: if lite {
                Theme::lite()
            } else {
                Theme::standard()
            },
            lite,
            open_hint: String::new(),
            reveal_count: 0,
            items: Vec::new(),
            take_focus: false,
            drawn_rows: 0,
            keyboard: false,
            selection: None,
            translate: None,
            layout: Layout::default(),
        }
    }

    /// A fresh panel: no query, cursor on the first row, keyboard wanted.
    ///
    /// The selection is cleared here rather than by the caller: a panel that keeps
    /// the previous capture would offer to translate text from an application the
    /// user has since left.
    pub fn on_open(&mut self) {
        self.query.clear();
        self.cursor = 0;
        self.take_focus = true;
        self.selection = None;
        self.translate = None;
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
        "Enter".to_string(),
        "打开".to_string(),
        "Esc".to_string(),
        "关闭".to_string(),
        "方向键 选择".to_string(),
        "没有匹配的结果".to_string(),
        "试试 笔记 / 工作台 / 新建".to_string(),
        // The selection row's label only. Its preview is arbitrary text from
        // another application and cannot be pre-rasterised; the label is what
        // decides whether the row reads as a sentence or as tofu.
        "已选中".to_string(),
        "字".to_string(),
        // The translation card's fixed words. The translation itself is arbitrary
        // and arrives later, so only the labels are here.
        "正在翻译".to_string(),
        "正在翻译…".to_string(),
        "译文".to_string(),
        "翻译失败".to_string(),
        "替换选区".to_string(),
        "复制".to_string(),
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
    state.layout = Layout::default();
    let mut action = key_action(ctx, state);

    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(palette.canvas)
                .inner_margin(egui::Margin::same(PANEL_PADDING as i8)),
        )
        .show(ctx, |ui| {
            ui.set_clip_rect(ctx.screen_rect());
            state.layout.canvas = Some(ui.max_rect());
            chrome::header(ui, state);
            ui.add_space(6.0);
            chrome::search(ui, state);
            if state.selection.is_some() {
                ui.add_space(6.0);
                let row = chrome::selection(ui, state);
                state.layout.selection = row;
            }
            if state.translate.is_some() {
                ui.add_space(6.0);
                let acted = translate::card(ui, state);
                if !matches!(acted, Action::None) {
                    action = acted;
                }
            }
            ui.add_space(SECTION_GAP);

            let visible = state.visible();
            state.clamp_cursor();
            if visible.is_empty() {
                chrome::empty(ui, state);
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
                    chrome::section_label(ui, state, "动作", commands.len());
                    ui.add_space(6.0);
                    let clicked = draw_grid(ui, state, &commands);
                    if !matches!(clicked, Action::None) {
                        action = clicked;
                    }
                    ui.add_space(SECTION_GAP);
                }
                if !notes.is_empty() {
                    chrome::section_label(
                        ui,
                        state,
                        if state.query.trim().is_empty() {
                            "最近笔记"
                        } else {
                            "匹配的笔记"
                        },
                        notes.len(),
                    );
                    ui.add_space(6.0);
                    let clicked = draw_notes(ui, state, &notes);
                    if !matches!(clicked, Action::None) {
                        action = clicked;
                    }
                }
            }

            chrome::footer(ui, state);
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
        if selected {
            state.layout.selected_card.get_or_insert(rect);
        } else if state.layout.rest_card.is_none() {
            state.layout.rest_card = Some(rect);
        }

        let hovered = response.hovered();
        let (fill, outline, width) = if selected {
            (palette.accent_soft, palette.accent, 1.5)
        } else if hovered {
            (palette.surface_hover, palette.border_strong, 1.0)
        } else {
            (palette.surface_soft, palette.border, 1.0)
        };
        let radius = CornerRadius::same(palette.radius.card);
        ui.painter().rect_filled(rect, radius, fill);
        ui.painter()
            .rect_stroke(rect, radius, Stroke::new(width, outline), StrokeKind::Inside);
        if selected {
            chrome::accent_bar(ui.painter(), palette, rect);
        }

        let item = &state.items[*item_index];
        ui.painter().text(
            rect.left_top() + Vec2::new(13.0, 12.0),
            Align2::LEFT_TOP,
            &item.title,
            FontId::proportional(13.5),
            palette.ink,
        );
        // The chord in monospace: `Ctrl+Alt+N` reads as one token only when the
        // glyphs share a width; proportional makes `l` and `N` disagree.
        ui.painter().text(
            rect.left_bottom() + Vec2::new(13.0, -12.0),
            Align2::LEFT_BOTTOM,
            &item.hint,
            FontId::monospace(10.0),
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
        if selected {
            state.layout.selected_note.get_or_insert(rect);
        } else if state.layout.rest_note.is_none() {
            state.layout.rest_note = Some(rect);
        }

        let radius = CornerRadius::same(palette.radius.row);
        let body = rect.shrink2(Vec2::new(0.0, 2.0));
        if selected {
            ui.painter().rect_filled(body, radius, palette.accent_soft);
            ui.painter().rect_stroke(
                body,
                radius,
                Stroke::new(1.0, palette.accent),
                StrokeKind::Inside,
            );
            chrome::accent_bar(ui.painter(), palette, body);
        } else if response.hovered() {
            ui.painter().rect_filled(body, radius, palette.surface_hover);
        }

        let item = &state.items[*item_index];
        let title = elide(&item.title, 34);
        ui.painter().text(
            rect.left_center() + Vec2::new(12.0, 0.0),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(13.0),
            palette.ink,
        );
        ui.painter().text(
            rect.right_center() + Vec2::new(-12.0, 0.0),
            Align2::RIGHT_CENTER,
            &item.hint,
            FontId::monospace(10.0),
            palette.ink_subtle,
        );
        if response.clicked() {
            state.cursor = slot;
            action = item.action.clone();
        }
    }
    action
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
        assert!(state.selection.is_none(), "a capture belongs to one reveal");
        assert!(
            state.translate.is_none(),
            "a translation from the previous reveal would be shown against this one's selection"
        );
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
