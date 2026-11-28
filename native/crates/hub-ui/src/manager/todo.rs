//! The todo pane: a filter, one group per note, and a checkbox that writes.
//!
//! ## Why this pane is not just `todo::lines` in a loop
//!
//! The first version of this tab was read-only, and the reason was structural: the
//! workbench only knew how to *draw* a todo, and the write lived in the desktop
//! card. A checkbox that cannot be checked is a picture of a checkbox, so the pane
//! was really a viewer of the card's data. Everything that makes it operable is
//! here -- which row is a target, what a click asks for -- and the write itself is
//! still the card's: `hub_core::todo::locate` then `toggle`, addressed by text.
//!
//! ## Why the box and the text are two targets
//!
//! The shipped panel splits them: the box toggles, the text navigates to the note
//! the task lives in. That split is what makes the pane usable as a place to
//! *review* -- you can read where something came from without changing it -- and a
//! single click target cannot express both. It also gives the gate two named
//! rectangles instead of one, so "the tick works" and "the jump works" are separate
//! claims.

use egui::{Align2, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Vec2};

use hub_core::todo::{self, Line};

use crate::theme::Theme;

use super::{report, Action, State};

/// Height of one todo line.
///
/// Tighter than a note row (48) on purpose: a note row carries two lines of text and
/// an action strip, a todo is one line, and a list of fifteen has to fit without
/// scrolling past two screens.
const ROW: f32 = 30.0;
/// Group heading. Between "a section" and "air": the title is 9.5 px, so a full row
/// height would read as a gap rather than a heading.
const GROUP_HEAD: f32 = 24.0;
/// The column the checkbox lives in -- the whole column is the click target, not the
/// 14 px square: a 14 px target is a puzzle, not a control.
const BOX_COLUMN: f32 = 30.0;
/// Side of the drawn square.
const BOX: f32 = 14.0;
/// Centre of the square inside its column.
const BOX_LEAD: f32 = 16.0;

/// Which todos the pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// The default: what is still open. This is a to-do list, not an archive.
    #[default]
    Open,
    All,
}

impl Filter {
    /// Stable id: it ends up in the log and in the reported hit names.
    pub fn id(self) -> &'static str {
        match self {
            Filter::Open => "open",
            Filter::All => "all",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Filter::Open => "未完成",
            Filter::All => "全部",
        }
    }

    fn shows(self, item: &Line) -> bool {
        !item.done || self == Filter::All
    }
}

/// One note's checkbox lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoGroup {
    /// The note the lines live in: what a text click opens, and what a tick writes
    /// back to.
    pub note_id: String,
    pub title: String,
    pub items: Vec<Line>,
}

impl TodoGroup {
    /// `None` when the note has no checkbox lines at all.
    ///
    /// Most notes in a store are prose, and a heading with nothing under it is
    /// noise that pushes the real groups off the pane. A note that has only
    /// *completed* todos is still a group: hiding it would make the 全部 filter
    /// disagree with the counts in the header.
    pub fn from_note(note_id: &str, title: &str, content: &str) -> Option<Self> {
        let items = todo::lines(content);
        if items.is_empty() {
            return None;
        }
        Some(Self {
            note_id: note_id.to_string(),
            title: hub_core::note_text::display_title(title, content),
            items,
        })
    }
}

/// `(open, total)` across every group. Reported in the header, so it must not be
/// computed from what the filter happens to show.
pub fn counts(groups: &[TodoGroup]) -> (usize, usize) {
    let total = groups.iter().map(|group| group.items.len()).sum();
    let open = groups
        .iter()
        .flat_map(|group| group.items.iter())
        .filter(|item| !item.done)
        .count();
    (open, total)
}

/// Draw the filter row and every visible group; return what was clicked.
pub(super) fn pane(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    groups: &[TodoGroup],
) -> Action {
    let mut action = Action::None;
    let (open, total) = counts(groups);

    ui.horizontal(|ui| {
        for filter in [Filter::Open, Filter::All] {
            let count = if filter == Filter::Open { open } else { total };
            let label = format!("{} {count}", filter.label());
            let chosen = state.filter == filter;
            let chip = filter_chip(ui, palette, &label, chosen);
            report(state, &format!("todo-filter:{}", filter.id()), chip.rect);
            if chip.clicked() && !chosen {
                state.filter = filter;
                action = Action::SetTodoFilter(filter);
                // The chip's fill was decided before the click was known, and this
                // frame has already painted the old selection. Same reason the note
                // rail asks for a repaint.
                ui.ctx().request_repaint();
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!(
                    "{} 条笔记",
                    groups
                        .iter()
                        .filter(|group| group.items.iter().any(|item| state.filter.shows(item)))
                        .count()
                ))
                .size(9.0)
                .color(palette.ink_disabled),
            );
        });
    });
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut shown = 0;
            for group in groups {
                let visible: Vec<&Line> = group
                    .items
                    .iter()
                    .filter(|item| state.filter.shows(item))
                    .collect();
                if visible.is_empty() {
                    continue;
                }
                shown += 1;
                group_head(ui, palette, group, &visible);
                for item in visible {
                    if let Some(clicked) = row(ui, palette, state, group, item) {
                        action = clicked;
                    }
                }
            }
            if shown == 0 {
                // The empty state answers the filter that is on: "都做完了" and
                // "还没有待办" are different situations and the same sentence would
                // be wrong in one of them.
                ui.add_space(10.0);
                ui.label(
                    RichText::new(match state.filter {
                        Filter::Open => "都做完了。切到「全部」可以看到已完成的。",
                        Filter::All => "还没有待办。在任意笔记里写 - [ ] 事项，就会出现在这里。",
                    })
                    .size(11.0)
                    .color(palette.ink_subtle),
                );
            }
        });

    action
}

fn filter_chip(ui: &mut egui::Ui, palette: Theme, label: &str, chosen: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(10.0)
                .color(if chosen { palette.ink } else { palette.ink_soft }),
        )
        .fill(if chosen {
            palette.accent_soft
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(Stroke::new(
            1.0,
            if chosen {
                palette.accent
            } else {
                palette.border
            },
        ))
        .corner_radius(CornerRadius::same(palette.radius.chip))
        .min_size(Vec2::new(0.0, 24.0)),
    )
}

/// The note a group's lines came from, plus how much of it is still open.
fn group_head(ui: &mut egui::Ui, palette: Theme, group: &TodoGroup, visible: &[&Line]) {
    ui.add_space(6.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, GROUP_HEAD), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        &group.title,
        FontId::proportional(9.5),
        palette.ink_subtle,
    );
    let open = visible.iter().filter(|item| !item.done).count();
    ui.painter().text(
        egui::pos2(rect.right() - 2.0, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("{open}/{}", visible.len()),
        FontId::monospace(9.0),
        palette.ink_disabled,
    );
}

/// One todo line: the box toggles, the text opens the note it came from.
fn row(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    group: &TodoGroup,
    item: &Line,
) -> Option<Action> {
    let mut action = None;
    let width = ui.available_width();
    // Painted before the widgets so the fill sits under the glyphs: the painter is
    // a single stream, and hovering a row must not paint over its own text.
    let rect = egui::Rect::from_min_size(ui.available_rect_before_wrap().min, Vec2::new(width, ROW));
    if ui.rect_contains_pointer(rect) {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(palette.radius.row),
            palette.surface_soft,
        );
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let (box_area, tick) = ui.allocate_exact_size(Vec2::new(BOX_COLUMN, ROW), Sense::click());
        report(
            state,
            &format!("todo-tick:{}:{}", group.note_id, item.index),
            box_area,
        );
        let boxed = egui::Rect::from_center_size(
            egui::pos2(box_area.left() + BOX_LEAD, box_area.center().y),
            Vec2::splat(BOX),
        );
        let radius = CornerRadius::same(4);
        if item.done {
            // Solid accent with a drawn tick, the shipped panel's filled box. A wash
            // plus a `✓` glyph read as a smudge at this size: the character comes
            // from whatever face covers it, and at 9 px its strokes are one pixel.
            ui.painter().rect_filled(boxed, radius, palette.accent);
            let (cx, cy) = (boxed.center().x, boxed.center().y);
            let pen = Stroke::new(1.6, palette.on_accent);
            ui.painter()
                .line_segment([egui::pos2(cx - 3.5, cy), egui::pos2(cx - 1.0, cy + 2.5)], pen);
            ui.painter()
                .line_segment([egui::pos2(cx - 1.0, cy + 2.5), egui::pos2(cx + 4.0, cy - 3.0)], pen);
        } else {
            // The outline brightens under the pointer: the square is the only part of
            // the row that looks like a control, and a control that does not react to
            // the pointer reads as decoration.
            let line = if tick.hovered() {
                palette.accent
            } else {
                palette.border_strong
            };
            ui.painter().rect_stroke(
                boxed,
                radius,
                Stroke::new(1.0, line),
                StrokeKind::Inside,
            );
        }

        let mut text = RichText::new(&item.text)
            .size(11.5)
            .color(if item.done {
                palette.ink_subtle
            } else {
                palette.ink
            });
        if item.done {
            // Struck through, not just dimmed: at 11.5 px a colour one step off the
            // page is a suggestion, and "which of these is left" is the question the
            // list exists to answer.
            text = text.strikethrough();
        }
        let label = ui
            .add(egui::Label::new(text).truncate().sense(Sense::click()))
            .on_hover_text(&item.text);
        report(
            state,
            &format!("todo-open:{}:{}", group.note_id, item.index),
            label.rect,
        );

        if tick.clicked() {
            action = Some(Action::ToggleTodo {
                note: group.note_id.clone(),
                line: item.index,
                text: item.text.clone(),
            });
        } else if label.clicked() {
            action = Some(Action::Open(group.note_id.clone()));
        }
    });
    action
}
