//! The launcher panel: header, search row, action grid, footer hints.
//!
//! Everything is drawn by hand instead of via `egui::Grid` so the cell geometry
//! in physical pixels is exactly what `hub-core::layout` computed. The column
//! count is an *input*, never re-derived here — the whole point of caching the two
//! layouts at startup is that the first frame does no measuring.

use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
};

use crate::theme::Palette;

/// Inner padding of the panel; must match `hub-core::layout::PanelSpec::padding`
/// (the column maths depends on it).
pub const PANEL_PADDING: f32 = 16.0;
const CARD_GAP: f32 = 8.0;
const HEADER_HEIGHT: f32 = 52.0;
const SEARCH_HEIGHT: f32 = 44.0;
const FOOTER_HEIGHT: f32 = 34.0;

/// One action card. Content matches the shipped launcher closely enough that the
/// first frame has to lay out the same glyphs and measure the same strings.
struct Action {
    title: &'static str,
    hint: &'static str,
}

const ACTIONS: [Action; 8] = [
    Action { title: "新建笔记", hint: "Ctrl+N" },
    Action { title: "待办清单", hint: "Ctrl+T" },
    Action { title: "脚本面板", hint: "Ctrl+P" },
    Action { title: "剪贴板", hint: "Ctrl+B" },
    Action { title: "截图标注", hint: "Ctrl+Alt+A" },
    Action { title: "下载任务", hint: "Ctrl+D" },
    Action { title: "搜索笔记", hint: "Ctrl+F" },
    Action { title: "偏好设置", hint: "Ctrl+," },
];

pub struct PanelState {
    pub query: String,
    pub columns: u8,
    pub palette: Palette,
    pub lite: bool,
    pub reveal_count: u64,
}

impl PanelState {
    pub fn new(columns: u8, lite: bool) -> Self {
        Self {
            query: String::new(),
            columns,
            palette: if lite {
                Palette::lite()
            } else {
                Palette::standard()
            },
            lite,
            reveal_count: 0,
        }
    }
}

/// Text that must be rasterised before the first visible frame.
///
/// Deduplicated by `prewarm`; the set is the fixed chrome plus every visible card
/// label (§5.8: 300–800 characters in the product, far fewer here).
pub fn first_screen_text() -> Vec<String> {
    let mut out = vec![
        "easyflowhub".to_string(),
        "动作面板".to_string(),
        "搜索动作、脚本、笔记".to_string(),
        "最近使用".to_string(),
        "Enter 打开".to_string(),
        "Esc 关闭".to_string(),
        "Ctrl+K 命令面板".to_string(),
    ];
    for action in ACTIONS {
        out.push(action.title.to_string());
        out.push(action.hint.to_string());
    }
    out
}

/// Force glyphs into the atlas without showing anything.
///
/// `layout_no_wrap` resolves the glyphs (and grows the atlas) but the upload still
/// happens on the next tessellation, which the hidden warm frame provides. Called
/// once at startup, never on the reveal path.
pub fn prewarm(ctx: &egui::Context) {
    let sizes = [18.0_f32, 14.0, 12.0, 11.0];
    for text in first_screen_text() {
        for size in sizes {
            let _ = ctx.fonts(|fonts| {
                fonts.layout_no_wrap(
                    text.clone(),
                    FontId::proportional(size),
                    Color32::WHITE,
                )
            });
        }
    }
}

/// Draw one frame of the panel.
pub fn draw(ctx: &egui::Context, state: &mut PanelState) {
    let palette = state.palette;
    let full = ctx.screen_rect();

    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(palette.panel)
                .inner_margin(egui::Margin::same(PANEL_PADDING as i8)),
        )
        .show(ctx, |ui| {
            ui.set_clip_rect(full);
            draw_header(ui, state);
            ui.add_space(6.0);
            draw_search(ui, state);
            ui.add_space(10.0);
            draw_grid(ui, state);
            draw_footer(ui, state);
        });
}

fn draw_header(ui: &mut egui::Ui, state: &mut PanelState) {
    let palette = state.palette;
    ui.allocate_ui(Vec2::new(ui.available_width(), HEADER_HEIGHT), |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("easyflowhub")
                    .size(18.0)
                    .color(palette.ink)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(RichText::new("动作面板").size(12.0).color(palette.ink_subtle));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("第 {} 次唤起", state.reveal_count))
                        .size(11.0)
                        .color(palette.ink_subtle),
                );
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

fn draw_search(ui: &mut egui::Ui, state: &mut PanelState) {
    let palette = state.palette;
    let width = ui.available_width();
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(width, SEARCH_HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(10), palette.panel_strong);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0, palette.border),
        StrokeKind::Inside,
    );

    let text = if state.query.is_empty() {
        "搜索动作、脚本、笔记".to_string()
    } else {
        state.query.clone()
    };
    let color = if state.query.is_empty() {
        palette.ink_subtle
    } else {
        palette.ink
    };
    let anchor = Align2::LEFT_CENTER;
    let pos = rect.left_center() + Vec2::new(14.0, 0.0);
    ui.painter()
        .text(pos, anchor, text, FontId::proportional(14.0), color);
}

fn draw_grid(ui: &mut egui::Ui, state: &mut PanelState) {
    let palette = state.palette;
    let columns = state.columns.max(1) as usize;
    let available = ui.available_width();
    let cell_width = (available - CARD_GAP * (columns as f32 - 1.0)) / columns as f32;
    let cell_height = 82.0;

    let rows = (ACTIONS.len() + columns - 1) / columns;
    let grid_height = rows as f32 * cell_height + (rows as f32 - 1.0) * CARD_GAP;

    // Section label first: painting it after the grid put "最近使用" underneath the
    // cards, which read as a stray caption in the screenshot evidence.
    ui.label(
        RichText::new("最近使用")
            .size(11.0)
            .color(palette.ink_subtle),
    );
    ui.add_space(4.0);

    let (area, _) = ui.allocate_exact_size(Vec2::new(available, grid_height), Sense::hover());
    let origin = area.min;
    for (index, action) in ACTIONS.iter().enumerate() {
        let row = index / columns;
        let column = index % columns;
        let min = origin
            + Vec2::new(
                column as f32 * (cell_width + CARD_GAP),
                row as f32 * (cell_height + CARD_GAP),
            );
        let rect = Rect::from_min_size(min, Vec2::new(cell_width, cell_height));

        let response = ui.interact(rect, ui.id().with(("action", index)), Sense::click());
        let fill = if response.hovered() {
            palette.panel_strong
        } else {
            palette.panel_strong.gamma_multiply(0.72)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(12),
            Stroke::new(1.0, palette.border),
            StrokeKind::Inside,
        );

        ui.painter().text(
            rect.left_top() + Vec2::new(12.0, 14.0),
            Align2::LEFT_TOP,
            action.title,
            FontId::proportional(14.0),
            palette.ink,
        );
        ui.painter().text(
            rect.left_bottom() + Vec2::new(12.0, -14.0),
            Align2::LEFT_BOTTOM,
            action.hint,
            FontId::proportional(11.0),
            palette.ink_subtle,
        );

        if index == 0 {
            ui.painter().circle_filled(
                rect.right_top() + Vec2::new(-16.0, 16.0),
                3.0,
                palette.accent,
            );
        }
    }
}

fn draw_footer(ui: &mut egui::Ui, state: &mut PanelState) {
    let palette = state.palette;
    ui.add_space(10.0);
    ui.allocate_ui(Vec2::new(ui.available_width(), FOOTER_HEIGHT), |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Enter 打开").size(11.0).color(palette.ink_subtle));
            ui.add_space(10.0);
            ui.label(RichText::new("Esc 关闭").size(11.0).color(palette.ink_subtle));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("Ctrl+K 命令面板")
                        .size(11.0)
                        .color(palette.ink_subtle),
                );
            });
        });
    });
}
