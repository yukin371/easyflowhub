//! The workbench window: one viewport, no island, and no side effects of its own.
//!
//! `Ctrl+Alt+M` has been registered since 1b.3 and pointed at a log line. This is
//! what it opens. The window is deliberately *not* a note: no RichEdit, no
//! autosave, no always-on-top -- it is the place you go to decide things, and a
//! panel that floats over every other program is the wrong shape for that.
//!
//! ## Where the work happens
//!
//! Drawing and the row model live in `hub-ui`; this file owns the window and the
//! effects. Every button comes back as an `Action` and is applied here, against
//! the one store connection the process has. That is the same split as the note
//! shell, and it is the reason the manager can be tested without a database: the
//! widget crate never sees SQL, and this file never draws.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use hub_core::{clock, todo, NoteWrite, Store, Table};
use hub_ui::manager::{self, Action, Pending, Row, Tab, TodoGroup};
use hub_ui::Theme;

use crate::hub::Request;
use crate::log::Log;

/// Title of the workbench window. Also how it is found again after eframe
/// recreates it (a monitor change does that).
pub const TITLE: &str = "easyflowhub-manager";

pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("manager")
}

pub struct Spawn {
    pub log: Arc<Log>,
    pub store: Option<Arc<Mutex<Store>>>,
    pub shortcuts: Arc<RwLock<Table>>,
    /// Work that has to run on the controller's pass: opening a note creates a
    /// viewport, and rebinding re-registers global keys. Neither may happen from
    /// inside a frame of this window.
    pub inbox: Arc<Mutex<Vec<Request>>>,
}

pub struct Shell {
    state: manager::State,
    rows: Vec<Row>,
    /// Every live note's checkbox lines, grouped by note.
    ///
    /// Not just the `待办箱`: the pane is the place to review everything that is
    /// still open, and the shipped panel grouped by note for the same reason. The
    /// groups are built in [`Shell::reload`] from the rows it already reads, so this
    /// is not a second query.
    todos: Vec<TodoGroup>,
    log: Arc<Log>,
    store: Option<Arc<Mutex<Store>>>,
    shortcuts: Arc<RwLock<Table>>,
    inbox: Arc<Mutex<Vec<Request>>>,
    closing: bool,
    /// Every control's rectangle as last written to the log, keyed by the name the
    /// widget gave it (`new-note`, `pin:note-1`, ...). Only logged when it moves: a
    /// per-frame line per button would bury the rest of the log.
    hit_logged: HashMap<String, [i32; 4]>,
}

impl Shell {
    pub fn new(spawn: Spawn) -> Self {
        let mut shell = Self {
            state: manager::State::default(),
            rows: Vec::new(),
            todos: Vec::new(),
            log: spawn.log,
            store: spawn.store,
            shortcuts: spawn.shortcuts,
            inbox: spawn.inbox,
            closing: false,
            hit_logged: HashMap::new(),
        };
        shell.reload();
        shell.log.mark(
            0,
            "manager_open",
            &format!(
                "notes={} trash={} todos={} store={}",
                shell.live_count(),
                shell.trash_count(),
                shell.todo_items(),
                if shell.store.is_some() { "on" } else { "off" }
            ),
        );
        shell
    }

    pub fn closing(&self) -> bool {
        self.closing
    }

    /// Show a specific section. The launcher's "待办清单" / "回收站" / "设置"
    /// cards are shortcuts into this window, not separate windows: one workbench
    /// with a different starting tab is the shipped behaviour, and a second
    /// window per card would leave four of them behind.
    pub fn show_tab(&mut self, tab: Tab) {
        self.state.tab = tab;
        self.state.selected = None;
        self.state.status.clear();
        self.log
            .mark(0, "manager_tab", &format!("tab={}", tab.id()));
    }

    /// Constant across frames: eframe diffs the builder against the one it
    /// recorded, so a user-resized window keeps its size instead of snapping back.
    pub fn builder(&self) -> egui::ViewportBuilder {
        egui::ViewportBuilder::default()
            .with_title(TITLE)
            // 860 x 600: the sidebar is 216 and the content column needs room for a
            // title, a description and two buttons on one line. At the old 760 the
            // header's action cluster had to be capped so hard that the description
            // wrapped on the first long sentence.
            .with_inner_size([860.0, 600.0])
            .with_min_inner_size([620.0, 420.0])
            .with_decorations(true)
            .with_resizable(true)
            .with_taskbar(true)
            .with_visible(true)
    }

    pub fn frame(&mut self, ctx: &egui::Context) {
        if self.closing {
            return;
        }
        // Alt+F4 and the title bar's close button. Treated as a close request
        // rather than an error, like the note's.
        if ctx.input(|input| input.viewport().close_requested()) {
            self.close(ctx, "os");
            return;
        }
        // Esc closes the workbench, but not while a key is being captured: there
        // Esc is "cancel the rebind", and the panel handles it itself.
        let escape = hub_ui::keys::chord_from_egui(ctx)
            .map(|chord| chord.vk == 0x1B)
            .unwrap_or(false);
        if escape && self.state.capturing.is_none() {
            // Layered like the todo card's Esc: the most local thing first. A
            // confirmation that is open gets cancelled; only when there is nothing
            // to back out of does Esc mean "close the window". Without this the
            // first Esc out of a "delete forever?" question would take the whole
            // workbench with it.
            if self.state.dismiss() {
                self.log
                    .mark(0, "manager_confirm", "action=cancel reason=esc");
                ctx.request_repaint_of(viewport_id());
            } else {
                self.close(ctx, "esc");
                return;
            }
        }

        let table = self
            .shortcuts
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        // The dialog is view state, so "did it ask" is not in the action stream.
        // Comparing the pending action across the frame is how the log learns about
        // it -- and the log is the only place a post-mortem can look.
        let asked = self.state.pending.clone();
        let action = manager::draw(
            ctx,
            Theme::standard(),
            &mut self.state,
            &self.rows,
            &self.todos,
            &table,
        );
        self.note_confirm(&asked, &action);
        self.log_hit(ctx);
        self.act(ctx, action);
    }

    /// One line per transition of the confirmation: asked, answered, dropped.
    fn note_confirm(&self, before: &Option<Pending>, action: &Action) {
        match (before, &self.state.pending) {
            (None, Some(now)) => self.log.mark(
                0,
                "manager_confirm",
                &format!("action=ask verb={} id={}", now.verb(), now.id()),
            ),
            (Some(then), None) => {
                let outcome = if matches!(action, Action::Purge(_)) {
                    "ok"
                } else {
                    "cancel"
                };
                self.log.mark(
                    0,
                    "manager_confirm",
                    &format!("action={outcome} verb={} id={}", then.verb(), then.id()),
                );
            }
            _ => {}
        }
    }

    /// Write every control's rectangle that moved this frame.
    ///
    /// Physical client pixels, which is what a click posts and what `click_client()`
    /// in the gate takes -- the script must not have to know the monitor scale.
    ///
    /// A control that stops being reported is forgotten, so that when it comes back
    /// it announces itself again. Without that, closing and reopening the confirm
    /// dialog would write no second line (same rectangle, nothing "moved") and a
    /// gate watching the log from a given point could never see it reopen.
    fn log_hit(&mut self, ctx: &egui::Context) {
        let scale = ctx.pixels_per_point();
        for (name, rect) in &self.state.hits {
            let now = [
                (rect.min.x * scale).round() as i32,
                (rect.min.y * scale).round() as i32,
                (rect.width() * scale).round() as i32,
                (rect.height() * scale).round() as i32,
            ];
            if now[2] <= 0 || now[3] <= 0 {
                continue;
            }
            if self.hit_logged.get(name) == Some(&now) {
                continue;
            }
            self.hit_logged.insert(name.clone(), now);
            self.log.mark(
                0,
                "manager_hit",
                &format!(
                    "button={name} x={} y={} w={} h={}",
                    now[0], now[1], now[2], now[3]
                ),
            );
        }
        let live: Vec<String> = self.state.hits.iter().map(|(name, _)| name.clone()).collect();
        self.hit_logged.retain(|name, _| live.contains(name));
    }

    fn close(&mut self, ctx: &egui::Context, reason: &str) {
        self.closing = true;
        self.log.mark(
            0,
            "manager_close",
            &format!("reason={reason} notes={}", self.rows.len()),
        );
        // Targeted at this viewport: `send_viewport_cmd` means "the viewport of
        // this pass", and the controller also runs passes for the notes.
        ctx.send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::Close);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    fn act(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::None | Action::SetTab(_) => {}
            Action::Close => self.close(ctx, "button"),
            Action::NewNote => {
                // The hotkey's own request, pushed the same way: opening a note
                // creates a viewport, and a frame of this window must not do that.
                self.state.status = "已新建笔记".to_string();
                self.log.mark(0, "manager_new_note", "from=button");
                self.push(Request::NewNote);
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
            Action::Open(id) => {
                self.state.status = "已打开".to_string();
                self.push(Request::OpenNote(id));
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
            Action::TodoCard => {
                // Same request as the tray item and `--todo-card`: the controller owns
                // the singleton, so the button can never make a second card.
                self.state.status = "已打开待办卡片".to_string();
                self.log.mark(0, "manager_todo_card", "from=button");
                self.push(Request::TodoCard);
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
            Action::ToggleTodo { note, line, text } => {
                self.toggle_todo(ctx, &note, line, &text);
            }
            Action::SetTodoFilter(filter) => {
                self.state.status = format!("筛选：{}", filter.label());
                self.log
                    .mark(0, "manager_todo_filter", &format!("filter={}", filter.id()));
                // The pane already asked for a frame; this is the one that also
                // updates the count of notes at the right of the filter row.
                self.refresh(ctx);
            }
            Action::Pin { id, pinned } => {
                let ok = self.with_store(|store| store.set_note_pin(&id, pinned));
                self.state.status = if ok == Some(true) {
                    if pinned { "已置顶" } else { "已取消置顶" }.to_string()
                } else {
                    "置顶失败".to_string()
                };
                self.log
                    .mark(0, "manager_pin", &format!("id={id} pinned={pinned} ok={ok:?}"));
                self.reload();
                self.refresh(ctx);
            }
            Action::Trash(id) => {
                let at = clock::now_iso();
                let ok = self.with_store(|store| store.soft_delete_note(&id, &at));
                self.state.status = if ok == Some(true) {
                    "已移到回收站".to_string()
                } else {
                    "删除失败".to_string()
                };
                self.log.mark(0, "manager_trash", &format!("id={id} ok={ok:?}"));
                self.reload();
                self.refresh(ctx);
            }
            Action::Restore(id) => {
                let ok = self.with_store(|store| store.restore_note(&id));
                self.state.status = if ok == Some(true) {
                    "已恢复".to_string()
                } else {
                    "恢复失败".to_string()
                };
                self.log.mark(0, "manager_restore", &format!("id={id} ok={ok:?}"));
                self.reload();
                self.refresh(ctx);
            }
            Action::Purge(id) => {
                let ok = self.with_store(|store| store.purge_note(&id));
                self.state.status = if ok == Some(true) {
                    "已彻底删除".to_string()
                } else {
                    "删除失败".to_string()
                };
                // Permanent, so it is logged as such: "where did my note go" needs
                // an answer that outlives the session.
                self.log
                    .mark(0, "manager_purge", &format!("id={id} ok={ok:?} permanent=true"));
                self.reload();
                self.refresh(ctx);
            }
            Action::BeginCapture(command) => {
                self.state.status = format!("按新键替换 {}", command.label());
                ctx.request_repaint();
            }
            Action::CancelCapture => {
                self.state.status = "已取消改键".to_string();
            }
            Action::Rebind(command, chord) => {
                // The controller owns the table and the global registrations, so
                // the change travels as a request: re-registering a hotkey from
                // inside a frame would run while this window is mid-paint.
                self.state.status = format!("{} → {}", command.label(), chord.display());
                self.log.mark(
                    0,
                    "manager_rebind",
                    &format!("command={} key={}", command.id(), chord.display()),
                );
                self.push(Request::Rebind {
                    command,
                    chord: chord.display(),
                });
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
        }
    }

    /// Tick or untick one checkbox line, in the note that holds it.
    ///
    /// The line number the pane reported belongs to the frame that drew it, so it is
    /// treated as a hint: `todo::locate` re-finds the task by its text in the body as
    /// it is *now*. Same rule as the desktop card, and the same reason -- a tick
    /// applies to a task, not to whatever happens to be on row 7 after another window
    /// inserted a line above it.
    fn toggle_todo(&mut self, ctx: &egui::Context, id: &str, line: usize, text: &str) {
        let at = clock::now_iso();
        let Some(store) = self.store.clone() else {
            self.state.status = "数据库未打开".to_string();
            return;
        };
        let mut guard = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(row) = guard.note(id).ok().flatten() else {
            self.state.status = "这条笔记已不存在".to_string();
            self.log.mark(
                0,
                "manager_todo_toggle",
                &format!("id={id} line={line} refused=missing"),
            );
            drop(guard);
            self.reload();
            self.refresh(ctx);
            return;
        };
        let target = match todo::locate(&row.content, line, text) {
            todo::Locate::Exact(at) | todo::Locate::Relocated(at) => at,
            // Refuse rather than guess: two tasks with the same words are two tasks,
            // and ticking the wrong one is silent data loss.
            todo::Locate::Conflict => {
                self.state.status = "同名待办不止一条，未改动".to_string();
                self.log.mark(
                    0,
                    "manager_todo_toggle",
                    &format!("id={id} line={line} refused=conflict"),
                );
                return;
            }
            todo::Locate::Missing => {
                self.state.status = "这条待办已被其他窗口删除".to_string();
                self.log.mark(
                    0,
                    "manager_todo_toggle",
                    &format!("id={id} line={line} refused=missing"),
                );
                drop(guard);
                self.reload();
                self.refresh(ctx);
                return;
            }
        };
        let next = match todo::toggle(&row.content, target, &at) {
            Ok(next) => next,
            Err(error) => {
                self.state.status = "这一行已不是待办".to_string();
                self.log.mark(
                    0,
                    "manager_todo_toggle",
                    &format!("id={id} line={target} refused={error:?}"),
                );
                return;
            }
        };
        // Read the state back out of the rewrite rather than inverting `item.done`:
        // the pane's copy of the line is one frame old, and the log has to state what
        // the database now says.
        let done = todo::lines(&next)
            .into_iter()
            .find(|item| item.index == target)
            .map(|item| item.done)
            .unwrap_or(false);
        let write = NoteWrite {
            id: row.id,
            title: row.title,
            content: next,
            tags: row.tags,
            created_at: row.created_at,
            updated_at: at,
        };
        let ok = match guard.save_note_body(&write) {
            // `true` means the row was *created*. A tick almost always updates an
            // existing note, so the log says which happened instead of calling the
            // normal case a failure.
            Ok(created) => format!("created={created}"),
            Err(error) => format!("error=\"{error}\""),
        };
        self.log.mark(
            0,
            "manager_todo_toggle",
            &format!("id={id} line={target} done={done} {ok}"),
        );
        self.state.status = if done {
            "已完成".to_string()
        } else {
            "已取消完成".to_string()
        };
        drop(guard);
        self.reload();
        self.refresh(ctx);
    }

    /// Ask for one more frame after a write changed the list.
    ///
    /// `reload()` swaps the rows while this frame is already half-painted, and egui
    /// paints only when something asks it to. Without this the window keeps showing
    /// the order from before the write -- a restored row still sitting in the trash,
    /// a purged one still in the list -- until the mouse happens to move, which
    /// reads exactly like "the button did nothing".
    fn refresh(&self, ctx: &egui::Context) {
        ctx.request_repaint_of(viewport_id());
    }

    fn push(&self, request: Request) {
        self.inbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
    }

    /// Run one write against the store, returning `None` when there is no database.
    fn with_store<F>(&self, work: F) -> Option<bool>
    where
        F: FnOnce(&mut Store) -> Result<bool, hub_core::StoreError>,
    {
        let store = self.store.as_ref()?;
        let mut guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match work(&mut guard) {
            Ok(changed) => Some(changed),
            Err(error) => {
                self.log.mark(0, "manager_store", &format!("error=\"{error}\""));
                None
            }
        }
    }

    /// Re-read the list after a write.
    ///
    /// Not a per-frame query: the list only changes when this window changes it, or
    /// when a note window saves. A five-hundred-row read sixty times a second would
    /// be the manager becoming the reason the notes feel slow.
    fn reload(&mut self) {
        self.rows.clear();
        self.todos.clear();
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.notes(true) {
            Ok(rows) => {
                self.rows = rows
                    .iter()
                    .map(|row| {
                        Row::new(
                            &row.id,
                            &row.title,
                            &row.content,
                            &row.tags,
                            &row.updated_at,
                            row.is_pinned,
                            row.deleted_at.clone(),
                        )
                    })
                    .collect();
                // From the rows just read, not from a second query: the todo pane
                // needs whole bodies, and this is the one place that already has
                // them. Trashed notes are excluded -- a deleted note's tasks are not
                // open work, and ticking one would resurrect a row whose window is
                // gone.
                self.todos = rows
                    .iter()
                    .filter(|row| row.deleted_at.is_none())
                    .filter_map(|row| TodoGroup::from_note(&row.id, &row.title, &row.content))
                    .collect();
            }
            Err(error) => self
                .log
                .mark(0, "manager_store", &format!("list error=\"{error}\"")),
        }
    }

    fn live_count(&self) -> usize {
        self.rows.iter().filter(|row| row.deleted_at.is_none()).count()
    }

    fn trash_count(&self) -> usize {
        self.rows.iter().filter(|row| row.deleted_at.is_some()).count()
    }

    fn todo_items(&self) -> usize {
        self.todos.iter().map(|group| group.items.len()).sum()
    }
}
