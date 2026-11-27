//! The translation card: what is running, what came back, and what to do with it.
//!
//! ## Why it is not a list row
//!
//! Everything else in the panel is Enter-able, and Enter always means "run the row
//! under the cursor". A translation is not a row: it arrives asynchronously and it
//! carries two different follow-ups (write it back, or copy it), neither of which is
//! "run this". Putting it in the list would make Enter ambiguous at exactly the
//! moment the user is holding a result they want to use.
//!
//! So the card is chrome with two buttons, drawn above the list, and the list keeps
//! its one meaning. Enter still opens things; the result needs a click.
//!
//! ## What each state has to say
//!
//! | state | the question the user has | what is drawn |
//! |---|---|---|
//! | running | did it take? | 正在翻译… and the source length |
//! | done | what does it say? | the text, truncated to four rows |
//! | failed | what do I do? | the fixed sentence from `hub-core` |
//!
//! The failed sentence is passed in as a string rather than as a `Problem`: this
//! crate draws, it does not decide what went wrong, and the mapping from status
//! code to Chinese lives with the code that made the request.

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::{Action, State, Translate};

/// Padding inside the card.
const PAD: f32 = 10.0;
/// The label row at the top (label on the left, buttons on the right).
const HEADER: f32 = 18.0;
/// Gap between the label row and the text.
const GAP: f32 = 6.0;
/// Rows of translation shown before it is cut. The panel is 860 tall and the list
/// below still has to be reachable; a long paragraph is a reason to copy it out,
/// not a reason to push every command off the screen.
const MAX_ROWS: usize = 4;

const BUTTON_HEIGHT: f32 = 20.0;
const BUTTON_GAP: f32 = 6.0;

/// Draw the card and return whatever a button asked for.
pub(super) fn card(ui: &mut egui::Ui, state: &mut State) -> Action {
    let Some(translate) = state.translate.clone() else {
        return Action::None;
    };
    let palette = state.palette;
    let width = ui.available_width();

    // Measured before allocating, because the card's height is the text's height:
    // a fixed height would either clip a two-line translation or leave a hole
    // under a one-line one.
    let (label, body, danger) = match &translate {
        Translate::Running => ("正在翻译", "正在翻译…".to_string(), false),
        Translate::Done { text } => ("译文", text.clone(), false),
        Translate::Failed { message } => ("翻译失败", message.clone(), true),
    };
    let body_colour = if danger { palette.ink_soft } else { palette.ink };
    let mut job =
        egui::text::LayoutJob::simple(body, FontId::proportional(12.5), body_colour, width - PAD * 2.0);
    job.wrap = egui::text::TextWrapping {
        max_width: width - PAD * 2.0,
        max_rows: MAX_ROWS,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.painter().layout_job(job);
    let height = PAD * 2.0 + HEADER + GAP + galley.size().y;

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    state.layout.translation = Some(rect);

    let radius = CornerRadius::same(palette.radius.card);
    let (fill, outline) = if danger {
        (palette.danger_soft, palette.danger)
    } else {
        (palette.surface_soft, palette.border)
    };
    ui.painter().rect_filled(rect, radius, fill);
    ui.painter()
        .rect_stroke(rect, radius, Stroke::new(1.0, outline), StrokeKind::Inside);

    let header = Rect::from_min_size(
        rect.left_top() + Vec2::new(PAD, PAD),
        Vec2::new(width - PAD * 2.0, HEADER),
    );
    ui.painter().text(
        header.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        if danger { palette.danger } else { palette.ink_subtle },
    );

    ui.painter().galley(
        rect.left_top() + Vec2::new(PAD, PAD + HEADER + GAP),
        galley,
        body_colour,
    );

    // Only a finished translation can be used, and 替换选区 only makes sense while
    // there is still a selection to replace: after a successful replace the shell
    // clears it, and a button that would paste over whatever the user has since
    // clicked into must not stay on screen.
    if !matches!(translate, Translate::Done { .. }) {
        return Action::None;
    }
    let mut action = Action::None;
    let mut right = header.right();
    // Right to left: 复制 is the safe action, so it sits furthest from the cursor's
    // path into the panel and 替换选区 -- the one that overwrites something -- is
    // the one the eye reaches first.
    let buttons: [(Action, &str, bool); 2] = [
        (Action::CopyTranslation, "复制", false),
        (Action::ReplaceSelection, "替换选区", true),
    ];
    for (each, text, primary) in buttons {
        if primary && state.selection.is_none() {
            continue;
        }
        let galley = ui.painter().layout_no_wrap(
            text.to_string(),
            FontId::proportional(11.0),
            if primary { palette.on_accent } else { palette.ink_soft },
        );
        let size = Vec2::new(galley.size().x + 14.0, BUTTON_HEIGHT);
        let button = Rect::from_min_size(
            egui::pos2(right - size.x, header.center().y - size.y / 2.0),
            size,
        );
        right = button.left() - BUTTON_GAP;

        let response = ui.interact(button, ui.id().with(("translate", text)), Sense::click());
        if primary {
            // Recorded only for the action a gate has to click: the copy button has
            // no visible side effect to sample, and the clipboard is not this
            // crate's business.
            state.layout.replace = Some(button);
        }
        let chip_radius = CornerRadius::same(palette.radius.chip);
        // Hover is a stroke width, not a colour: the theme module holds no
        // coefficients, and a "slightly darker accent" computed here would be the
        // one place in this crate that invents a colour.
        let (bg, stroke, ink, width_px) = if primary {
            (palette.accent, palette.accent, palette.on_accent, 1.0)
        } else if response.hovered() {
            (palette.surface_hover, palette.border_strong, palette.ink_soft, 1.0)
        } else {
            (palette.surface, palette.border, palette.ink_soft, 1.0)
        };
        let width_px = if response.hovered() { width_px + 0.5 } else { width_px };
        ui.painter().rect_filled(button, chip_radius, bg);
        ui.painter().rect_stroke(
            button,
            chip_radius,
            Stroke::new(width_px, stroke),
            StrokeKind::Inside,
        );
        ui.painter().galley(
            egui::pos2(
                button.center().x - galley.size().x / 2.0,
                button.center().y - galley.size().y / 2.0,
            ),
            galley,
            ink,
        );
        if response.clicked() {
            action = each;
        }
    }
    action
}
