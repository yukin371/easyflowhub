//! The desktop todo card: a small always-on-top window over the task list.
//!
//! ## What this file owns, and what it does not
//!
//! Drawing, the grouped model and the hit rectangles. The *rules* about what a
//! checkbox line is live in `hub_core::todo`, and the store lives in `app`. That is
//! the same split as the workbench: this crate never sees SQL and the app never
//! draws, so the grouping can be tested without a window and a write can be tested
//! without a painter.
//!
//! ## Why the toolbar is a pure function
//!
//! The card has five targets in 32 physical-points of height: a count, `+`, `↻`,
//! the pin, and `×`. A layout that is only expressed as consecutive `ui` calls
//! cannot be checked for overlap, and "the pin moved two pixels under the refresh
//! button" is exactly the kind of bug a screenshot review argues about instead of
//! measuring. [`strip_layout`] is the layout; the painting consumes it; the gate
//! clicks the rectangles it reports.
//!
//! ## Editing
//!
//! One row is editable at a time, and it is addressed by *text* when the write
//! happens (`hub_core::todo::locate`), not by the row number: another window may
//! have inserted a line above it since this list was read.

use egui::{Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Vec2};

use hub_core::todo::{self, Line};

use crate::theme::Theme;

/// Logical size of a new card, from the shipped window (`inner_size(320, 400)`).
pub const WIDTH: f32 = 320.0;
pub const HEIGHT: f32 = 400.0;
/// Smallest window the strip still lays out without the count meeting the buttons.
/// The card is resizable; below this the drag area would collapse and the window
/// would become impossible to move, so the window enforces it instead.
pub const MIN_WIDTH: f32 = 220.0;
pub const MIN_HEIGHT: f32 = 160.0;
/// The title strip, same 32 px as a note's.
pub const STRIP: f32 = 32.0;

/// One row of the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The line in the note body, as of the read that produced this list.
    pub line: usize,
    pub text: String,
    pub done: bool,
}

/// One note's todos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub note_id: String,
    pub title: String,
    pub items: Vec<Item>,
}

impl Group {
    pub fn pending(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|item| !item.done)
    }

    pub fn done(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|item| item.done)
    }
}

/// What the card reads out of one note.
pub struct Note<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub content: &'a str,
}

/// Group a snapshot into the card's model.
///
/// Two rules, both from the shipped card:
///
/// * groups with nothing to show are dropped, so an ordinary note does not add an
///   empty heading;
/// * the inbox comes first, then the notes in the order the manager listed them.
///   The order is stable rather than alphabetical: a card that reorders itself
///   between reads is a card the user has to re-read every time.
pub fn groups(notes: &[Note<'_>]) -> Vec<Group> {
    let mut out: Vec<Group> = notes
        .iter()
        .filter_map(|note| {
            let mut items: Vec<Item> = todo::lines(note.content)
                .into_iter()
                .map(|line: Line| Item {
                    line: line.index,
                    text: line.text,
                    done: line.done,
                })
                .collect();
            if items.is_empty() {
                return None;
            }
            // Open first inside a group; the line order is the user's own order and
            // is kept on both sides of that split.
            items.sort_by_key(|item| item.done);
            let title = if note.title.trim().is_empty() {
                "无标题".to_string()
            } else {
                note.title.trim().to_string()
            };
            Some(Group {
                note_id: note.id.to_string(),
                title,
                items,
            })
        })
        .collect();
    // Stable sort: the inbox is the one group that has to be reachable without
    // looking, everything else keeps the list's order.
    out.sort_by_key(|group| !is_inbox(&group.title));
    out
}

fn is_inbox(title: &str) -> bool {
    title == hub_core::store::TODO_INBOX_TITLE
}

/// Where every target in the strip is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strip {
    pub count: Rect,
    pub add: Rect,
    pub refresh: Rect,
    pub pin: Rect,
    pub close: Rect,
    /// The part of the strip that is not a button: dragging it moves the window.
    pub drag: Rect,
}

const BUTTON: f32 = 22.0;
const GAP: f32 = 4.0;
const EDGE: f32 = 8.0;
const COUNT: f32 = 40.0;

/// The strip's layout for a window `width` points wide.
///
/// The buttons are laid out from the right edge inwards, in the order they are
/// drawn: `+`, `↻`, pin, `×`. Below the minimum width the count and the buttons
/// could meet; the count is clamped instead of letting the drag strip invert,
/// because a negative-width rect silently swallows the press it was meant to
/// exclude.
pub fn strip_layout(width: f32) -> Strip {
    let width = width.max(0.0);
    let mut right = width - EDGE;
    let next = |right: &mut f32| {
        let rect = Rect::from_min_size(
            egui::pos2(*right - BUTTON, (STRIP - BUTTON) / 2.0),
            Vec2::splat(BUTTON),
        );
        *right -= BUTTON + GAP;
        rect
    };
    let close = next(&mut right);
    let pin = next(&mut right);
    let refresh = next(&mut right);
    let add = next(&mut right);
    let count = Rect::from_min_size(
        egui::pos2(EDGE, 0.0),
        Vec2::new(COUNT.min(add.min.x - EDGE - GAP).max(0.0), STRIP),
    );
    let left = (count.max.x + GAP).min(add.min.x);
    let right = (add.min.x - GAP).max(left);
    let drag = Rect::from_min_max(egui::pos2(left, 0.0), egui::pos2(right, STRIP));
    Strip {
        count,
        add,
        refresh,
        pin,
        close,
        drag,
    }
}

/// A row's two targets, in logical points, relative to the window.
///
/// Reported for the same reason the strip is a function: the gate has to click the
/// checkbox a user would click, and deriving that from text metrics would be a
/// second implementation of the layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub note: String,
    pub line: usize,
    pub checkbox: Rect,
    pub text: Rect,
}

/// What the user did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Close,
    Refresh,
    TogglePin,
    BeginDrag,
    Toggle {
        note: String,
        line: usize,
        /// The text this row showed. The write locates the line by it, because the
        /// row number may be stale by the time the click is handled.
        text: String,
    },
    Commit {
        note: String,
        line: usize,
        from: String,
        text: String,
    },
    Create(String),
    OpenSource(String),
}

/// The card's own mutable state.
#[derive(Debug, Default)]
pub struct State {
    /// `(note id, line)` of the row being edited, and its draft.
    pub editing: Option<(String, usize)>,
    pub draft: String,
    pub composing: bool,
    pub new_text: String,
    /// One line of feedback: a refused write, a conflict, a failed save.
    pub error: String,
    /// The hits of the last frame.
    pub hits: Vec<Hit>,
    /// Set when a row starts being edited, so the field takes the keyboard on the
    /// frame it appears rather than on the next keystroke.
    focus: bool,
}

impl State {
    /// Cancel the edit in progress. Returns whether there was one.
    pub fn cancel_edit(&mut self) -> bool {
        if self.editing.take().is_some() {
            self.draft.clear();
            return true;
        }
        false
    }

    /// Close the composer. Returns whether it was open.
    pub fn cancel_compose(&mut self) -> bool {
        if self.composing {
            self.composing = false;
            self.new_text.clear();
            return true;
        }
        false
    }

    pub fn begin_edit(&mut self, note: &str, line: usize, text: &str) {
        self.composing = false;
        self.editing = Some((note.to_string(), line));
        self.draft = text.to_string();
        self.focus = true;
    }
}

/// Paint the card and return what was asked for.
pub fn draw(
    ctx: &egui::Context,
    palette: Theme,
    state: &mut State,
    groups: &[Group],
    pinned: bool,
    scale: f32,
) -> Action {
    let mut action = Action::None;
    state.hits.clear();

    let strip = strip_layout(ctx.screen_rect().width());
    egui::TopBottomPanel::top("todocard-strip")
        .exact_height(STRIP)
        .frame(
            egui::Frame::NONE
                .fill(palette.surface)
                .inner_margin(egui::Margin::ZERO),
        )
        .show(ctx, |ui| {
            let drag = ui.interact(strip.drag, ui.id().with("drag"), Sense::click_and_drag());
            if drag.drag_started() {
                action = Action::BeginDrag;
            }
            let open = groups.iter().map(|group| group.pending().count()).sum::<usize>();
            ui.painter().text(
                strip.count.left_center() + Vec2::new(8.0, 0.0),
                Align2::LEFT_CENTER,
                open.to_string(),
                FontId::proportional(10.5 * scale.min(1.4)),
                palette.ink_subtle,
            );
            if button(ui, &palette, strip.add, "+", state.composing, "新增待办") {
                state.cancel_edit();
                state.composing = !state.composing;
                state.error.clear();
            }
            if button(ui, &palette, strip.refresh, "↻", false, "刷新") {
                action = Action::Refresh;
            }
            if pin_button(ui, &palette, strip.pin, pinned) {
                action = Action::TogglePin;
            }
            if button(ui, &palette, strip.close, "×", false, "关闭") {
                action = Action::Close;
            }
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(palette.canvas)
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ctx, |ui| {
            if state.composing {
                composer(ui, palette, state, &mut action);
            }
            if !state.error.is_empty() {
                error_row(ui, palette, state);
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if groups.is_empty() {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new("还没有待办")
                                .size(12.0)
                                .color(palette.ink_subtle),
                        );
                        return;
                    }
                    pending_pass(ui, palette, state, groups, &mut action);
                    done_pass(ui, palette, state, groups, &mut action);
                });
        });

    action
}

fn pending_pass(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    groups: &[Group],
    action: &mut Action,
) {
    let mut first = true;
    for group in groups {
        let rows: Vec<&Item> = group.pending().collect();
        if rows.is_empty() {
            continue;
        }
        if !first {
            separator(ui, palette);
        }
        first = false;
        header(ui, palette, group, action);
        for item in rows {
            row(ui, palette, state, group, item, action);
        }
    }
}

fn done_pass(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    groups: &[Group],
    action: &mut Action,
) {
    if groups.iter().all(|group| group.done().count() == 0) {
        return;
    }
    ui.add_space(6.0);
    separator(ui, palette);
    let mut first = true;
    for group in groups {
        let rows: Vec<&Item> = group.done().collect();
        if rows.is_empty() {
            continue;
        }
        if !first {
            separator(ui, palette);
        }
        first = false;
        header(ui, palette, group, action);
        for item in rows {
            row(ui, palette, state, group, item, action);
        }
    }
}

fn separator(ui: &mut egui::Ui, palette: Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    let y = rect.center().y;
    let inset = 6.0;
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + inset, y),
            egui::pos2(rect.right() - inset, y),
        ],
        Stroke::new(1.0, palette.border),
    );
}

fn header(ui: &mut egui::Ui, palette: Theme, group: &Group, action: &mut Action) {
    ui.horizontal(|ui| {
        let label = ui.add(
            egui::Label::new(
                RichText::new(&group.title)
                    .size(9.5)
                    .color(palette.ink_subtle),
            )
            .truncate(),
        );
        if !is_inbox(&group.title) {
            let open = ui
                .add(
                    egui::Button::new(RichText::new("↗").size(10.0).color(palette.ink_subtle))
                        .fill(palette.canvas)
                        .stroke(Stroke::NONE)
                        .min_size(Vec2::splat(16.0)),
                )
                .on_hover_text("打开原笔记");
            if open.clicked() {
                *action = Action::OpenSource(group.note_id.clone());
            }
        } else {
            let _ = label.on_hover_text("待办箱");
        }
    });
}

fn row(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    group: &Group,
    item: &Item,
    action: &mut Action,
) {
    let editing = state
        .editing
        .as_ref()
        .is_some_and(|(note, line)| note == &group.note_id && *line == item.line);

    ui.horizontal(|ui| {
        let (box_rect, response) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
        let box_rect = box_rect.translate(Vec2::new(0.0, 2.0));
        ui.painter().circle(
            box_rect.center(),
            7.0,
            if item.done {
                palette.accent_soft
            } else {
                palette.surface
            },
            Stroke::new(1.0, palette.border),
        );
        if item.done {
            ui.painter().text(
                box_rect.center(),
                Align2::CENTER_CENTER,
                "✓",
                FontId::proportional(9.0),
                palette.accent,
            );
        }
        if response.clicked() && !editing {
            *action = Action::Toggle {
                note: group.note_id.clone(),
                line: item.line,
                text: item.text.clone(),
            };
        }

        if editing {
            let field = ui.add(
                egui::TextEdit::singleline(&mut state.draft)
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(12.0))
                    .text_color(palette.ink),
            );
            if std::mem::take(&mut state.focus) {
                field.request_focus();
            }
            let escaped = ui.input(|input| input.key_pressed(egui::Key::Escape));
            let entered = ui.input(|input| input.key_pressed(egui::Key::Enter));
            if escaped {
                state.cancel_edit();
            } else if entered || (field.lost_focus() && !state.draft.trim().is_empty()) {
                *action = Action::Commit {
                    note: group.note_id.clone(),
                    line: item.line,
                    from: item.text.clone(),
                    text: state.draft.clone(),
                };
            }
            state.hits.push(Hit {
                note: group.note_id.clone(),
                line: item.line,
                checkbox: box_rect,
                text: field.rect,
            });
        } else {
            let text = ui
                .add(
                    egui::Label::new(
                        RichText::new(&item.text)
                            .size(12.0)
                            .color(if item.done {
                                palette.ink_disabled
                            } else {
                                palette.ink
                            }),
                    )
                    .sense(Sense::click())
                    .truncate(),
                )
                // A card this narrow truncates long tasks; the hint is the only way
                // to read the rest without resizing the window.
                .on_hover_text(&item.text);
            if text.clicked() {
                state.begin_edit(&group.note_id, item.line, &item.text);
            }
            state.hits.push(Hit {
                note: group.note_id.clone(),
                line: item.line,
                checkbox: box_rect,
                text: text.rect,
            });
        }
    });
}

fn composer(ui: &mut egui::Ui, palette: Theme, state: &mut State, action: &mut Action) {
    egui::Frame::NONE
        .fill(palette.surface_soft)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(palette.radius.card))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut state.new_text)
                    .hint_text("新待办")
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(12.0))
                    .text_color(palette.ink),
            );
            if std::mem::take(&mut state.focus) {
                field.request_focus();
            }
            let escaped = ui.input(|input| input.key_pressed(egui::Key::Escape));
            let entered = ui.input(|input| input.key_pressed(egui::Key::Enter));
            if escaped {
                state.cancel_compose();
            } else if entered || field.lost_focus() {
                let text = state.new_text.trim().to_string();
                if text.is_empty() {
                    state.cancel_compose();
                } else {
                    *action = Action::Create(text);
                }
            }
        });
}

fn error_row(ui: &mut egui::Ui, palette: Theme, state: &mut State) {
    egui::Frame::NONE
        .fill(palette.danger_soft)
        .stroke(Stroke::new(1.0, palette.danger))
        .corner_radius(CornerRadius::same(palette.radius.chip))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&state.error).size(11.0).color(palette.ink));
                if ui
                    .add(
                        egui::Button::new(RichText::new("×").size(11.0).color(palette.ink_soft))
                            .fill(palette.danger_soft)
                            .stroke(Stroke::NONE),
                    )
                    .clicked()
                {
                    state.error.clear();
                }
            });
        });
}

/// One strip button: painted glyph, hover hint, click answer.
fn button(
    ui: &mut egui::Ui,
    palette: &Theme,
    rect: Rect,
    glyph: &str,
    active: bool,
    hint: &str,
) -> bool {    let response = ui.interact(rect, ui.id().with(glyph), Sense::click());
    let ink = if active {
        palette.accent
    } else if response.hovered() {
        palette.ink
    } else {
        palette.ink_subtle
    };
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), palette.surface_hover);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(12.0),
        ink,
    );
    response.on_hover_text(hint).clicked()
}

/// The pin, drawn as a bookmark rather than a glyph.
///
/// `▮` was the first version and it reads as a window control at this size -- the
/// card has no frame for a maximise button to belong to, so the shape has to say
/// "keep this here" on its own. Filled means pinned, like the note's bookmark.
fn pin_button(ui: &mut egui::Ui, palette: &Theme, rect: Rect, on: bool) -> bool {
    let response = ui.interact(rect, ui.id().with("pin"), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), palette.surface_hover);
    }
    let ink = if on {
        palette.accent
    } else if response.hovered() {
        palette.ink
    } else {
        palette.ink_subtle
    };
    let bookmark = Rect::from_center_size(rect.center(), Vec2::new(9.0, 12.0));
    // A flag with a notch, not a rectangle: a 9x11 outline reads as "maximise" at
    // this size, and the card has no frame for that button to belong to.
    let notch = 3.5;
    let points = vec![
        bookmark.left_top(),
        bookmark.right_top(),
        bookmark.right_bottom(),
        egui::pos2(bookmark.center().x, bookmark.bottom() - notch),
        bookmark.left_bottom(),
    ];
    if on {
        ui.painter()
            .add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));
    } else {
        ui.painter()
            .add(egui::Shape::closed_line(points, Stroke::new(1.2, ink)));
    }
    response
        .on_hover_text(if on { "取消固定" } else { "固定到桌面" })
        .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note<'a>(id: &'a str, title: &'a str, content: &'a str) -> Note<'a> {
        Note { id, title, content }
    }

    #[test]
    fn only_notes_with_todos_become_groups() {
        let got = groups(&[
            note("a", "普通笔记", "正文\n- 列表但不是待办"),
            note("b", "待办箱", "- [ ] 买菜"),
        ]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].note_id, "b");
    }

    #[test]
    fn the_inbox_is_first_and_open_items_precede_done_ones() {
        let got = groups(&[
            note("a", "项目", "- [x] 已完成\n- [ ] 未完成"),
            note("b", "待办箱", "- [ ] 甲"),
        ]);
        assert_eq!(
            got.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(),
            vec!["待办箱", "项目"]
        );
        let project = &got[1];
        assert_eq!(
            project.items.iter().map(|i| (i.text.as_str(), i.done)).collect::<Vec<_>>(),
            vec![("未完成", false), ("已完成", true)]
        );
        assert_eq!(project.items[0].line, 1, "the line number is the body's, not the row's");
    }

    #[test]
    fn the_strip_never_overlaps_itself() {
        for width in [MIN_WIDTH, 320.0, 640.0] {
            let strip = strip_layout(width);
            let targets = [
                ("count", strip.count),
                ("add", strip.add),
                ("refresh", strip.refresh),
                ("pin", strip.pin),
                ("close", strip.close),
            ];
            for (index, (name, rect)) in targets.iter().enumerate() {
                assert!(
                    rect.min.x >= 0.0 && rect.max.x <= width + 0.01,
                    "{name} leaves the strip at {width}: {rect:?}"
                );
                for (other, rect2) in &targets[index + 1..] {
                    assert!(
                        !rect.intersects(*rect2),
                        "{name} and {other} overlap at {width}: {rect:?} / {rect2:?}"
                    );
                }
            }
            assert!(
                strip.drag.width() > 0.0 && strip.drag.max.x <= strip.add.min.x + 0.01,
                "the drag strip must be a real area left of the buttons: {width}"
            );
        }
    }

    #[test]
    fn the_buttons_are_in_the_order_they_are_drawn() {
        let strip = strip_layout(320.0);
        assert!(strip.add.min.x < strip.refresh.min.x);
        assert!(strip.refresh.min.x < strip.pin.min.x);
        assert!(strip.pin.min.x < strip.close.min.x);
        assert!(strip.count.max.x < strip.add.min.x);
    }

    #[test]
    fn escape_cancels_an_edit_before_it_cancels_the_card() {
        let mut state = State::default();
        assert!(!state.cancel_edit(), "nothing to cancel yet");
        state.begin_edit("b", 3, "买菜");
        assert!(state.cancel_edit());
        assert!(state.editing.is_none());
        assert!(state.draft.is_empty());
    }

    #[test]
    fn starting_an_edit_puts_the_composer_away() {
        let mut state = State {
            composing: true,
            ..State::default()
        };
        state.begin_edit("b", 0, "x");
        assert!(!state.composing);
        assert_eq!(state.editing, Some(("b".to_string(), 0)));
    }
}
