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

// `manager.rs` plus a `manager/` directory: the classic non-`mod.rs` layout. Six
// jobs got big enough to be their own file -- the row (the densest drawing code
// here), the confirm dialog (its own state machine, tested on its own), the
// sidebar (a rail made of tiles and captions), the panel header (title,
// description, actions, and the hairline under them), the todo pane (the one list
// in this window that writes back to a note body) and the trash pane (sub-tabs and
// the batch bar).
mod check;
mod confirm;
mod head;
mod list;
mod nav;
mod row;
mod todo;
mod trash;

pub use confirm::Pending;
pub use row::Row;
pub use todo::{Filter, TodoGroup};
pub use trash::TrashView;

use std::collections::BTreeSet;

use egui::{RichText, Stroke};
use hub_core::{Chord, Command, Scope, Table};

use crate::hint;
use crate::theme::Theme;

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

    /// The single character in the sidebar's icon tile.
    ///
    /// A Chinese character, not an emoji: the CJK face installed by
    /// [`crate::theme`] covers these four, while `🗑` and `⚙` depend on emoji
    /// fonts whose presence on a stripped-down Windows install is not something
    /// the navigation should bet on. The tile is the only icon the rail has, so it
    /// must never come up as tofu.
    pub fn glyph(self) -> &'static str {
        match self {
            Tab::Notes => "笔",
            Tab::Todos => "办",
            Tab::Trash => "删",
            Tab::Settings => "设",
        }
    }

    /// The small-caps second line under the name, the same one the shipped
    /// sidebar shows: the section names are short enough that two lines of
    /// text would otherwise be vertically unbalanced.
    pub fn caption(self) -> &'static str {
        match self {
            Tab::Notes => "NOTES",
            Tab::Todos => "TODOS",
            Tab::Trash => "TRASH",
            Tab::Settings => "SETTINGS",
        }
    }
}

// The todo parser used to live here, with a comment saying it was "deliberate" that
// it did not live in `hub-core`. That was true when this list was the only reader.
// The desktop card now reads the same checkbox grammar (and rewrites it), so two
// copies would drift the first time someone typed a numbered list -- the view uses
// `hub_core::todo::lines`, one parser for the card, the list and the writer.

/// What the user asked the workbench to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Close,
    /// A blank note, exactly what `Ctrl+Alt+N` does. The window itself is opened by
    /// the controller -- a frame must not create viewports.
    NewNote,
    /// The desktop todo card. Same window the tray item opens; the workbench was the
    /// last place that could only *point at* it (frontend had a button, the native
    /// manager had none, so in takeover mode nobody could reach it).
    TodoCard,
    /// Tick or untick one checkbox line.
    ///
    /// Carries the text as well as the line number because the controller re-locates
    /// the task before writing (`hub_core::todo::locate`): a line index from this
    /// frame is a hint, and another window may have inserted a line above it since.
    ToggleTodo {
        note: String,
        line: usize,
        text: String,
    },
    /// View state, but it travels as an action so the log records it -- "why does
    /// the list look empty" has an answer in the file.
    SetTodoFilter(Filter),
    /// Which sub-tab of the trash is open. Also view state, also for the log: the
    /// same reason as the todo filter.
    SetTrashView(TrashView),
    /// Restore every ticked trash row.
    ///
    /// Carries the ids instead of saying "the checked ones": the controller never
    /// reads view state, and a log line that names the rows can be checked against
    /// the database afterwards.
    RestoreSelected(Vec<String>),
    /// 清空回收站: every trashed row, gone for good. Only the dialog's yes produces
    /// this, exactly like [`Action::Purge`].
    EmptyTrash,
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
    /// Every control drawn this frame, in client points.
    ///
    /// The row buttons used to be unclickable by any gate: the old note in this file
    /// said a script must not hardcode a coordinate inside a layout free to change,
    /// which is true, but the conclusion "so do not click them" was wrong. The todo
    /// card settled the better contract: the *product* reports the rectangle, and the
    /// gate clicks the rectangle. Names are `verb:id` for row buttons (`pin:note-1`)
    /// and bare for chrome (`new-note`, `todo-card`, `tab:trash`), so a click can be
    /// addressed without the script knowing the arrangement.
    pub hits: Vec<(String, egui::Rect)>,
    /// Which todos the todo tab lists. View state, like `selected`: the filter does
    /// not belong to the controller, and a reopen starts from 未完成 again.
    pub filter: Filter,
    /// Which trash sub-tab is open. Only [`TrashView::Notes`] has a list; the other
    /// two exist so the question "where did my deleted capture go" has an answer.
    pub trash_view: TrashView,
    /// The trash rows ticked for a batch restore.
    ///
    /// A `BTreeSet` rather than a `HashSet` for one small reason: the ids end up in
    /// an action and then in the log, and a set that iterates in a random order would
    /// log `ids=a,b` one run and `ids=b,a` the next, which is a diff nobody wants to
    /// read. Pruned every frame against the rows that still exist.
    pub checked: BTreeSet<String>,
    /// A destructive action waiting for its second yes, if any.
    ///
    /// This is view state on purpose: an open question is not a decision, and the
    /// controller should not have to know how to draw one. See [`Pending`].
    pub pending: Option<Pending>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::Notes,
            query: String::new(),
            selected: None,
            capturing: None,
            status: String::new(),
            hits: Vec::new(),
            filter: Filter::default(),
            trash_view: TrashView::default(),
            checked: BTreeSet::new(),
            pending: None,
        }
    }
}

impl State {
    /// Close the open confirmation, if there is one.
    ///
    /// The caller needs the answer, not just the side effect: in the window Esc
    /// means "close the workbench", so with a dialog open it has to mean "cancel
    /// the dialog" instead. Returns whether anything was open.
    pub fn dismiss(&mut self) -> bool {
        self.pending.take().is_some()
    }
}

/// Record where a control ended up, for the gate to click.
fn report(state: &mut State, name: &str, rect: egui::Rect) {
    state.hits.push((name.to_string(), rect));
}

/// Draw the whole workbench and return what the user asked for.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ctx: &egui::Context,
    palette: Theme,
    state: &mut State,
    rows: &[Row],
    todos: &[TodoGroup],
    table: &Table,
) -> Action {
    let mut action = Action::None;
    // Rects belong to one frame: a row that moved must not be clickable at its old
    // place, and a button that disappeared must stop being addressable.
    state.hits.clear();

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

    egui::SidePanel::left("manager-tabs")
        .exact_width(nav::WIDTH)
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(palette.surface)
                .inner_margin(egui::Margin {
                    left: 14,
                    right: 10,
                    top: 18,
                    bottom: 12,
                }),
        )
        .show(ctx, |ui| {
            let rail = nav::sidebar(
                ui,
                palette,
                state,
                rows.iter().filter(|row| row.deleted_at.is_none()).count(),
                rows.iter().filter(|row| row.deleted_at.is_some()).count(),
            );
            if rail != Action::None {
                action = rail;
            }
        });

    // The status line lives under the content, not in the header. A message that
    // comes and goes must not reflow the title above it -- and "已移到回收站"
    // arriving in the header would push the whole list down for a second.
    //
    // Declared before the central panel on purpose: a bottom panel takes its
    // height off whatever is left after the side panel, and the panel order is
    // exactly how egui decides that.
    egui::TopBottomPanel::bottom("manager-status")
        .frame(
            egui::Frame::NONE
                .fill(palette.canvas)
                .inner_margin(egui::Margin {
                    left: 22,
                    right: 22,
                    top: 4,
                    bottom: 8,
                }),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&state.status)
                        .size(9.5)
                        .color(palette.ink_subtle),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Esc 关闭")
                            .size(9.0)
                            .color(palette.ink_disabled),
                    );
                });
            });
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(palette.canvas)
                .inner_margin(egui::Margin {
                    left: 22,
                    right: 22,
                    top: 18,
                    bottom: 10,
                }),
        )
        .show(ctx, |ui| match state.tab {
            Tab::Notes | Tab::Trash => {
                let trash = state.tab == Tab::Trash;
                // The trash rows, in one place: the header's 清空 count, the sub-tab
                // count and the selection pruning all mean the same set.
                let trashed: Vec<&Row> = rows
                    .iter()
                    .filter(|row| row.deleted_at.is_some())
                    .collect();
                if trash {
                    // A row that left the trash (restored here, in a note window, or by
                    // the retention sweep) must not keep its tick: `checked` is an id
                    // list, and an id the list cannot show is a count in "已选 N 条"
                    // nobody can account for. Pruned against every trashed row rather
                    // than the search result -- a filtered-out row is still picked.
                    state
                        .checked
                        .retain(|id| trashed.iter().any(|row| &row.id == id));
                } else {
                    state.checked.clear();
                }
                let description = if trash {
                    format!(
                        "删除的笔记保留 {} 天；勾选多条可以一次恢复。",
                        hub_core::store::TRASH_RETENTION_DAYS
                    )
                } else {
                    "点一行选中它，操作按钮出现在该行下方。".to_string()
                };
                // Read before the header: 清空 empties *this* list, so it belongs to
                // the view that has one. On 收集箱 it would be a destructive button
                // over a page that says "the notes are elsewhere".
                let view = state.trash_view;
                let head_action = head::panel_head(
                    ui,
                    palette,
                    state,
                    if trash { "RECYCLE BIN" } else { "NOTES" },
                    if trash { "回收站" } else { "笔记" },
                    &description,
                    |ui, state| {
                        let mut asked = Action::None;
                        if head::ghost(ui, palette, "关闭").clicked() {
                            asked = Action::Close;
                        }
                        if trash {
                            // Only offered when there is something to empty, and only
                            // on the view that lists it. A destructive button that is
                            // always on screen gets pressed by accident.
                            if view.has_list() && !trashed.is_empty() {
                                let empty = head::warn(ui, palette, "清空回收站");
                                report(state, "empty-trash", empty.rect);
                                if empty.clicked() {
                                    // A question, not a decision: the dialog asks, and
                                    // only its yes becomes `Action::EmptyTrash`.
                                    state.pending =
                                        Some(Pending::EmptyTrash { count: trashed.len() });
                                    ui.ctx().request_repaint();
                                }
                            }
                        } else {
                            // The one action the workbench could only *describe*
                            // before: the empty list said "press Ctrl+Alt+N" because
                            // there was no button to point at. Same request either
                            // way, so the button is not a second path.
                            let new_note = head::primary(ui, palette, "新建笔记");
                            report(state, "new-note", new_note.rect);
                            if hint::hint(new_note, palette, "新建一条快速笔记", "Ctrl+Alt+N")
                                .clicked()
                            {
                                asked = Action::NewNote;
                            }
                        }
                        asked
                    },
                );
                if head_action != Action::None {
                    action = head_action;
                }

                if trash {
                    let count = trashed.len();
                    if let Some(asked) = trash::subnav(ui, palette, state, count) {
                        action = asked;
                    }
                    ui.add_space(10.0);
                }

                if trash && !view.has_list() {
                    // A sub-tab the native shell does not own. Its body is the
                    // explanation, not an empty list: "收集箱是空的" would be a claim
                    // about data this window cannot see.
                    trash::notice(ui, palette, state, view);
                } else {
                    // The field is a card, not a bare `TextEdit`: an input that floats
                    // on the page with a hairline is the one shape the shipped manager
                    // never uses, and the search box is the first thing the eye lands
                    // on in this panel.
                    egui::Frame::NONE
                        .fill(palette.surface)
                        .corner_radius(egui::CornerRadius::same(palette.radius.card))
                        .inner_margin(egui::Margin::symmetric(12, 7))
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut state.query)
                                    .hint_text("搜索标题、正文、标签")
                                    .desired_width(f32::INFINITY)
                                    .frame(false),
                            );
                        });
                    ui.add_space(10.0);
                    if trash {
                        if let Some(asked) = trash::selection_bar(ui, palette, state) {
                            action = asked;
                        }
                        ui.add_space(6.0);
                    }
                    let visible: Vec<&Row> = rows
                        .iter()
                        .filter(|row| row.deleted_at.is_some() == trash && row.matches(&state.query))
                        .collect();
                    if visible.is_empty() {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(if trash {
                                // The same number as the header's sentence, from the
                                // same constant: two copies of "30" is one of them
                                // being wrong after the setting exists.
                                format!(
                                    "回收站是空的 · 删除的笔记会在这里保留 {} 天",
                                    hub_core::store::TRASH_RETENTION_DAYS
                                )
                            } else {
                                "没有笔记 · 点「新建笔记」或按 Ctrl+Alt+N".to_string()
                            })
                            .size(11.0)
                            .color(palette.ink_subtle),
                        );
                    }
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for row in visible {
                                if let Some(clicked) = list::note_row(ui, palette, state, row) {
                                    action = clicked;
                                }
                            }
                        });
                }
            }
            Tab::Todos => {
                // The counts come from every group, not from what the filter shows:
                // "未完成 2 / 共 5" is a fact about the store, and a header that
                // changed its meaning with the filter would be two different numbers
                // under one sentence.
                let (open, total) = todo::counts(todos);
                let description = format!("未完成 {open} · 共 {total} 条，按笔记分组。");
                let head_action = head::panel_head(
                    ui,
                    palette,
                    state,
                    "TODO",
                    "待办",
                    &description,
                    |ui, state| {
                        let mut asked = Action::None;
                        if head::ghost(ui, palette, "关闭").clicked() {
                            asked = Action::Close;
                        }
                        // The frontend panel has had this button since the card
                        // shipped; in takeover mode the frontend panel is not on
                        // screen, so the card was reachable only from the tray.
                        // Same request either way (`Request::TodoCard`), so there is
                        // still one window, not two.
                        let card = head::quiet(ui, palette, "悬浮卡片");
                        report(state, "todo-card", card.rect);
                        if hint::hint(card, palette, "在桌面显示待办卡片", "").clicked() {
                            asked = Action::TodoCard;
                        }
                        asked
                    },
                );
                if head_action != Action::None {
                    action = head_action;
                }

                // The pane owns the filter row, the groups and the two targets per
                // line. It writes nothing itself: a tick comes back as
                // `Action::ToggleTodo` and the controller does the locating.
                let asked = todo::pane(ui, palette, state, todos);
                if asked != Action::None {
                    action = asked;
                }
            }
            Tab::Settings => {
                let head_action = head::panel_head(
                    ui,
                    palette,
                    state,
                    "SETTINGS",
                    "设置",
                    "点按键位后按新键，Esc 取消。",
                    |ui, _state| {
                        if head::ghost(ui, palette, "关闭").clicked() {
                            Action::Close
                        } else {
                            Action::None
                        }
                    },
                );
                if head_action != Action::None {
                    action = head_action;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for scope in [Scope::Global, Scope::Window] {
                            ui.label(
                                RichText::new(match scope {
                                    Scope::Global => "全局（其它窗口有焦点时也生效）",
                                    Scope::Window => "笔记窗口内",
                                })
                                .size(8.5)
                                .color(palette.ink_subtle)
                                .extra_letter_spacing(2.2),
                            );
                            ui.add_space(6.0);
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
                                            // A command with no chord still needs a
                                            // chip: an empty button is invisible, and
                                            // the only way to bind one is to click the
                                            // thing that is not there.
                                            let hint = table.hint(command);
                                            let label = if hint.trim().is_empty() {
                                                "未绑定".to_string()
                                            } else {
                                                hint
                                            };
                                            let chip = chip_button(
                                                ui,
                                                palette,
                                                &label,
                                                table.shadowed().contains(&command),
                                            );
                                            if hint::hint(chip, palette, "改键", "").clicked() {
                                                state.capturing = Some(command);
                                                action = Action::BeginCapture(command);
                                            }
                                        },
                                    );
                                });
                                ui.add_space(6.0);
                            }
                            ui.add_space(14.0);
                        }
                    });
            }
        });

    // Drawn last so it sits above the panels, with a backdrop that swallows clicks:
    // a modal that lets a stray click land on a row is not modal. The answer
    // replaces the frame's action -- the only action a dialog can produce is the
    // one it was asking about.
    let answer = confirm::dialog(ctx, palette, state);
    if answer != Action::None {
        action = answer;
    }

    action
}

/// The clickable key chip.
fn chip_button(ui: &mut egui::Ui, palette: Theme, chord: &str, conflict: bool) -> egui::Response {
    let color = if conflict { palette.accent } else { palette.ink };
    ui.add(
        egui::Button::new(RichText::new(chord).monospace().size(10.5).color(color))
            .fill(palette.surface)
            .stroke(Stroke::new(1.0, palette.border)),
    )
}

// The tests are a sibling file, not an inline module: the drawing code above is
// dense enough that the assertions read better on their own page.
#[cfg(test)]
mod tests;
