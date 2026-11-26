//! The workbench: the note list, search, the todo inbox, the trash, and keys.
//!
//! ## Why a window and not a list in the strip
//!
//! The quick note is a scratch pad: it has to appear where the cursor is and get
//! out of the way. Everything that needs *deciding* -- which note, what to search,
//! what to restore, which key does what -- belongs somewhere with room, and in the
//! shipped build that is the manager panel. `Ctrl+Alt+M` has been registered and
//! pointed at nothing since 1b.3; this is the thing it opens.
//!
//! ## What this module does not do
//!
//! It does not open the database, save settings, or register hotkeys. It renders
//! what it is given and returns an [`Action`]; the controller owns the store and
//! the side effects. That split is what lets every behaviour worth pinning --
//! filtering, tab counts, which row a click means -- be tested without a window,
//! and it keeps the "one connection for the process" rule in one place.
//!
//! ## Rebind capture
//!
//! Clicking a key chip puts that command into capture mode; the next key press
//! becomes the new binding. The chord is read here rather than in the controller
//! because this is where the intent is ("this row is capturing"), and the mapping
//! from a frame key to a chord already exists in [`crate::keys`] -- the same one
//! the note uses, so a captured key and a pressed key cannot disagree.

use egui::{Align2, CornerRadius, FontId, RichText, Sense, Stroke, Vec2};
use hub_core::{Chord, Command, Scope, Table};

use crate::hint;
use crate::theme::Palette;

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
            preview: preview_of(content),
            title: display.clone(),
            when: short_time(updated_at),
            pinned,
            deleted_at,
            haystack: format!("{display}\n{content}\n{tags}").to_lowercase(),
        }
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty() || self.haystack.contains(&query)
    }
}

fn preview_of(content: &str) -> String {
    const MAX: usize = 64;
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

fn short_time(stamp: &str) -> String {
    let bytes = stamp.as_bytes();
    if bytes.len() >= 16 && bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T' {
        return format!("{} {}", &stamp[5..10], &stamp[11..16]);
    }
    stamp.to_string()
}

/// The four sections the sidebar offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Notes,
    Todos,
    Trash,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Notes, Tab::Todos, Tab::Trash, Tab::Settings];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Notes => "笔记",
            Tab::Todos => "待办",
            Tab::Trash => "回收站",
            Tab::Settings => "设置",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Tab::Notes => "notes",
            Tab::Todos => "todos",
            Tab::Trash => "trash",
            Tab::Settings => "settings",
        }
    }
}

/// A checkbox line out of the todo note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
}

/// Parse the Markdown checkboxes the todo note is made of.
///
/// Todos are **not a table**: the product keeps them as `- [ ]` lines inside a
/// note titled 待办箱. Parsing here rather than in `hub-core` is deliberate -- the
/// store already exposes `count_todo_lines` for the counts, and this is the view's
/// list, which only has to be good enough to show.
pub fn todo_items(content: &str) -> Vec<TodoItem> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        let (done, rest) = if let Some(rest) = trimmed
            .strip_prefix("- [ ]")
            .or_else(|| trimmed.strip_prefix("* [ ]"))
        {
            (false, rest)
        } else if let Some(rest) = trimmed
            .strip_prefix("- [x]")
            .or_else(|| trimmed.strip_prefix("- [X]"))
            .or_else(|| trimmed.strip_prefix("* [x]"))
            .or_else(|| trimmed.strip_prefix("* [X]"))
        {
            (true, rest)
        } else {
            continue;
        };
        out.push(TodoItem {
            text: rest.trim().to_string(),
            done,
        });
    }
    out
}

/// What the user asked the workbench to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Close,
    Open(String),
    Pin { id: String, pinned: bool },
    Trash(String),
    Restore(String),
    Purge(String),
    SetTab(Tab),
    BeginCapture(Command),
    CancelCapture,
    Rebind(Command, Chord),
}

pub struct State {
    pub tab: Tab,
    pub query: String,
    pub selected: Option<String>,
    pub capturing: Option<Command>,
    /// One line of feedback under the header: "已移到回收站" and so on.
    pub status: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::Notes,
            query: String::new(),
            selected: None,
            capturing: None,
            status: String::new(),
        }
    }
}

/// Draw the whole workbench and return what the user asked for.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ctx: &egui::Context,
    palette: Palette,
    state: &mut State,
    rows: &[Row],
    todo: Option<&str>,
    table: &Table,
) -> Action {
    let mut action = Action::None;

    // Capture mode owns the keyboard: a click on a chip is a request to press the
    // next key, and nothing else should happen while that is pending.
    if let Some(command) = state.capturing {
        if let Some(chord) = crate::keys::chord_from_egui(ctx) {
            state.capturing = None;
            action = if chord.vk == 0x1B {
                Action::CancelCapture
            } else {
                Action::Rebind(command, chord)
            };
        }
    }

    egui::TopBottomPanel::top("manager-head")
        .frame(egui::Frame::NONE.fill(palette.panel_strong).inner_margin(egui::Margin::symmetric(14, 10)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("工作台").size(14.0).color(palette.ink));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&state.status)
                        .size(10.5)
                        .color(palette.ink_subtle),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close = ui.add(
                        egui::Button::new(RichText::new("关闭").size(11.0).color(palette.ink_soft))
                            .fill(palette.panel)
                            .stroke(Stroke::new(1.0, palette.border)),
                    );
                    if hint::hint(close, palette, "关闭工作台", "Esc").clicked() {
                        action = Action::Close;
                    }
                });
            });
        });

    egui::SidePanel::left("manager-tabs")
        .exact_width(132.0)
        .frame(egui::Frame::NONE.fill(palette.panel).inner_margin(egui::Margin::same(8)))
        .show(ctx, |ui| {
            for tab in Tab::ALL {
                let selected = state.tab == tab;
                let label = if tab == Tab::Trash {
                    format!("{} ({})", tab.label(), rows.iter().filter(|row| row.deleted_at.is_some()).count())
                } else if tab == Tab::Notes {
                    format!("{} ({})", tab.label(), rows.iter().filter(|row| row.deleted_at.is_none()).count())
                } else {
                    tab.label().to_string()
                };
                let button = ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(
                        RichText::new(label)
                            .size(11.5)
                            .color(if selected { palette.ink } else { palette.ink_soft }),
                    )
                    .fill(if selected { palette.panel_strong } else { palette.panel })
                    .stroke(Stroke::new(0.0, palette.border)),
                );
                if button.clicked() {
                    state.tab = tab;
                    state.query.clear();
                    state.status.clear();
                    action = Action::SetTab(tab);
                }
            }
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(palette.panel).inner_margin(egui::Margin::same(12)))
        .show(ctx, |ui| match state.tab {
            Tab::Notes | Tab::Trash => {
                let trash = state.tab == Tab::Trash;
                let field = ui.add(
                    egui::TextEdit::singleline(&mut state.query)
                        .hint_text("搜索标题、正文、标签")
                        .desired_width(f32::INFINITY),
                );
                let _ = field;
                ui.add_space(6.0);
                let visible: Vec<&Row> = rows
                    .iter()
                    .filter(|row| row.deleted_at.is_some() == trash && row.matches(&state.query))
                    .collect();
                if visible.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(if trash {
                            "回收站是空的"
                        } else {
                            "没有笔记 · 按 Ctrl+Alt+N 新建"
                        })
                        .size(11.0)
                        .color(palette.ink_subtle),
                    );
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for row in visible {
                        if let Some(clicked) = note_row(ui, palette, row, state.selected.as_deref()) {
                            state.selected = Some(row.id.clone());
                            action = clicked;
                        }
                    }
                });
            }
            Tab::Todos => {
                let content = todo.unwrap_or("");
                let items = todo_items(content);
                let open = items.iter().filter(|item| !item.done).count();
                ui.label(
                    RichText::new(format!("待办箱 · 未完成 {open} / 共 {}", items.len()))
                        .size(11.0)
                        .color(palette.ink_soft),
                );
                ui.add_space(6.0);
                if items.is_empty() {
                    ui.label(
                        RichText::new("还没有待办。在名为「待办箱」的笔记里写 - [ ] 事项")
                            .size(11.0)
                            .color(palette.ink_subtle),
                    );
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for item in &items {
                        let (mark, ink) = if item.done {
                            ("✓", palette.ink_subtle)
                        } else {
                            ("○", palette.ink)
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(mark).size(11.0).color(ink));
                            ui.label(
                                RichText::new(&item.text)
                                    .size(11.5)
                                    .color(if item.done { palette.ink_subtle } else { ink }),
                            );
                        });
                    }
                });
            }
            Tab::Settings => {
                ui.label(
                    RichText::new("快捷键 · 点按键位后按新键，Esc 取消")
                        .size(11.0)
                        .color(palette.ink_soft),
                );
                ui.add_space(6.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for scope in [Scope::Global, Scope::Window] {
                        ui.label(
                            RichText::new(match scope {
                                Scope::Global => "全局（其它窗口有焦点时也生效）",
                                Scope::Window => "笔记窗口内",
                            })
                            .size(10.0)
                            .color(palette.ink_subtle),
                        );
                        ui.add_space(2.0);
                        for command in Command::ALL
                            .into_iter()
                            .filter(|command| command.scope() == scope)
                        {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(command.label())
                                        .size(11.5)
                                        .color(palette.ink_soft),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if state.capturing == Some(command) {
                                            ui.label(
                                                RichText::new("按下新键…（Esc 取消）")
                                                    .size(10.5)
                                                    .color(palette.accent),
                                            );
                                            return;
                                        }
                                        let chip = chip_button(
                                            ui,
                                            palette,
                                            &table.hint(command),
                                            table.shadowed().contains(&command),
                                        );
                                        if hint::hint(chip, palette, "改键", "").clicked() {
                                            state.capturing = Some(command);
                                            action = Action::BeginCapture(command);
                                        }
                                    },
                                );
                            });
                        }
                        ui.add_space(8.0);
                    }
                });
            }
        });

    action
}

/// One note row: title, preview, timestamp, and two buttons.
fn note_row(
    ui: &mut egui::Ui,
    palette: Palette,
    row: &Row,
    selected: Option<&str>,
) -> Option<Action> {
    let mut action = None;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    let chosen = selected == Some(row.id.as_str());
    if chosen || response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(4),
            if chosen {
                palette.panel_strong
            } else {
                palette.panel_strong.gamma_multiply(0.55)
            },
        );
    }
    if let Some(marker) = if row.deleted_at.is_some() {
        Some("回收站")
    } else if row.pinned {
        Some("置顶")
    } else {
        None
    } {
        ui.painter().text(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            marker,
            FontId::proportional(9.0),
            palette.accent,
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 58.0, rect.center().y),
        Align2::LEFT_CENTER,
        &row.title,
        FontId::proportional(12.0),
        palette.ink,
    );
    if !row.preview.is_empty() {
        let start = rect.left() + 58.0 + 160.0;
        if start < rect.right() - 120.0 {
            ui.painter().text(
                egui::pos2(start, rect.center().y),
                Align2::LEFT_CENTER,
                &row.preview,
                FontId::proportional(10.5),
                palette.ink_subtle,
            );
        }
    }
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        &row.when,
        FontId::monospace(9.5),
        palette.ink_subtle,
    );

    if response.clicked() {
        action = if row.deleted_at.is_some() {
            None
        } else {
            Some(Action::Open(row.id.clone()))
        };
    }
    // Actions live on the selected row's header line rather than inside every row:
    // three 16 px buttons per row is a wall of glyphs, and the list is the part
    // that has to stay readable.
    if chosen {
        ui.horizontal(|ui| {
            if row.deleted_at.is_some() {
                if ui.button(RichText::new("恢复").size(10.5)).clicked() {
                    action = Some(Action::Restore(row.id.clone()));
                }
                if hint::hint(
                    ui.button(RichText::new("彻底删除").size(10.5)),
                    palette,
                    "从数据库删除这一行",
                    "",
                )
                .clicked()
                {
                    action = Some(Action::Purge(row.id.clone()));
                }
            } else {
                if ui.button(RichText::new("打开").size(10.5)).clicked() {
                    action = Some(Action::Open(row.id.clone()));
                }
                let pin_label = if row.pinned { "取消置顶" } else { "置顶" };
                if ui.button(RichText::new(pin_label).size(10.5)).clicked() {
                    action = Some(Action::Pin {
                        id: row.id.clone(),
                        pinned: !row.pinned,
                    });
                }
                if ui.button(RichText::new("移到回收站").size(10.5)).clicked() {
                    action = Some(Action::Trash(row.id.clone()));
                }
            }
        });
    }
    action
}

/// The clickable key chip.
fn chip_button(ui: &mut egui::Ui, palette: Palette, chord: &str, conflict: bool) -> egui::Response {
    let color = if conflict { palette.accent } else { palette.ink };
    ui.add(
        egui::Button::new(RichText::new(chord).monospace().size(10.5).color(color))
            .fill(palette.panel_strong)
            .stroke(Stroke::new(1.0, palette.border)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, title: &str, content: &str, tags: &str, deleted: bool) -> Row {
        Row::new(
            id,
            title,
            content,
            tags,
            "2026-11-26T02:31:00.123456789+00:00",
            false,
            deleted.then(|| "2026-11-26T03:00:00+00:00".to_string()),
        )
    }

    #[test]
    fn search_covers_title_body_and_tags() {
        let rows = vec![
            row("a", "周末计划", "买螺丝刀", "", false),
            row("b", "会议", "本周进度", "工作 重要", false),
        ];
        assert_eq!(
            rows.iter().filter(|r| r.matches("螺丝刀")).count(),
            1,
            "the body was not searched"
        );
        assert_eq!(rows.iter().filter(|r| r.matches("重要")).count(), 1);
        assert_eq!(rows.iter().filter(|r| r.matches("")).count(), 2);
    }

    #[test]
    fn the_trash_tab_sees_only_deleted_rows() {
        let rows = vec![row("a", "留下", "", "", false), row("b", "删掉", "", "", true)];
        let live = rows.iter().filter(|row| row.deleted_at.is_none()).count();
        let trash = rows.iter().filter(|row| row.deleted_at.is_some()).count();
        assert_eq!((live, trash), (1, 1));
    }

    #[test]
    fn todo_lines_are_parsed_in_both_marks() {
        let items = todo_items("- [ ] 买菜\n* [x] 洗衣\n- [X] 回邮件\nnot a todo\n");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], TodoItem { text: "买菜".to_string(), done: false });
        assert!(items[1].done);
        assert!(items[2].done);
    }

    #[test]
    fn an_empty_todo_note_has_no_items_rather_than_a_blank_one() {
        assert!(todo_items("").is_empty());
        assert!(todo_items("普通段落\n- 列表但不是待办").is_empty());
    }

    #[test]
    fn a_row_title_falls_back_to_the_first_line_like_the_note_does() {
        // The list and the note must agree on what a note is called, or the same
        // note has two names depending on where you look at it.
        let row = row("a", "", "\n买菜清单\n鸡蛋", "", false);
        assert_eq!(row.title, "买菜清单");
    }

    #[test]
    fn tabs_are_stable_for_the_settings_file_and_the_log() {
        // The ids end up in log lines; a rename would break a saved expectation.
        assert_eq!(Tab::Notes.id(), "notes");
        assert_eq!(Tab::Trash.id(), "trash");
        assert_eq!(Tab::ALL.len(), 4);
    }
}
