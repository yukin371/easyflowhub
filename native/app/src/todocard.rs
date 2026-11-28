//! The desktop todo card, as a viewport of the shell process.
//!
//! One card exists at a time. It is created on demand and destroyed on close (the
//! shipped `create_todo_card_window` was the same shape, minus the process); the
//! window is borderless, always on top, and can be pinned to the desktop.
//!
//! ## Where the work happens
//!
//! `hub-ui::todocard` draws and decides what was clicked; this file owns the
//! window, the store and the effects. Writes follow the frontend's F05 rule: locate
//! the todo by its text in the *latest* body, then rewrite that line. A line number
//! from the snapshot is a hint, and a hint that is silently trusted is how a tick
//! lands on the task above the one the user clicked.
//!
//! ## Why the hits are logged
//!
//! The gate has to click the checkbox a user would click. Deriving the position
//! from text metrics in Python would be a second implementation of the layout, so
//! the card reports the rectangles it actually used. Same contract as the note's
//! `island_rect`: the log states the layout, the gate presses there.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hub_core::store::TODO_INBOX_TITLE;
use hub_core::{clock, new_note_id, todo, NoteRow, NoteWrite, Store};
use hub_ui::todocard::{self, Action, Group, Note, State};
use hub_ui::Theme;

use crate::hub::Request;
use crate::log::Log;

/// Title of the card window. Also how it is found again for the pin and the drag.
pub const TITLE: &str = "easyflowhub-todo-card";

/// How often an open card re-reads the database. The shipped card polled every
/// 30 s; a card that only updates when it is clicked would show yesterday's list
/// after a note window saved something.
const REFRESH: Duration = Duration::from_secs(30);

pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("todo-card")
}

pub struct Spawn {
    pub log: Arc<Log>,
    pub store: Option<Arc<Mutex<Store>>>,
    /// Opening the source note is a request to the controller: this window must not
    /// create viewports from inside its own frame.
    pub inbox: Arc<Mutex<Vec<Request>>>,
}

pub struct Shell {
    state: State,
    groups: Vec<Group>,
    pinned: bool,
    hwnd: Option<isize>,
    log: Arc<Log>,
    store: Option<Arc<Mutex<Store>>>,
    inbox: Arc<Mutex<Vec<Request>>>,
    closing: bool,
    /// Cursor position on the previous frame of a drag; `None` when not dragging.
    drag: Option<(i32, i32)>,
    /// Whether this pass's hit rectangles have been written to the log.
    hits_logged: bool,
    next_refresh: Instant,
}

impl Shell {
    pub fn new(spawn: Spawn) -> Self {
        let mut shell = Self {
            state: State::default(),
            groups: Vec::new(),
            pinned: false,
            hwnd: None,
            log: spawn.log,
            store: spawn.store,
            inbox: spawn.inbox,
            closing: false,
            drag: None,
            hits_logged: false,
            next_refresh: Instant::now() + REFRESH,
        };
        shell.reload();
        shell
    }

    pub fn closing(&self) -> bool {
        self.closing
    }

    /// Constant across frames: eframe diffs it, so a window the user resized keeps
    /// its size instead of snapping back to 320×400 on the next frame.
    pub fn builder(&self) -> egui::ViewportBuilder {
        egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([todocard::WIDTH, todocard::HEIGHT])
            .with_min_inner_size([todocard::MIN_WIDTH, todocard::MIN_HEIGHT])
            .with_decorations(false)
            .with_resizable(true)
            .with_taskbar(false)
            .with_always_on_top()
            .with_visible(true)
    }

    pub fn frame(&mut self, ctx: &egui::Context) {
        if self.closing {
            return;
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            self.close(ctx, "os");
            return;
        }
        if self.hwnd.is_none() {
            self.attach();
        }

        // Esc unwinds one layer at a time: the row being edited, then the composer,
        // and only then the card. A card that closed while the user was fixing a
        // typo would throw the edit away for a keystroke that meant "never mind".
        let escape = hub_ui::keys::chord_from_egui(ctx)
            .map(|chord| chord.vk == 0x1B)
            .unwrap_or(false);
        if escape {
            let layer = if self.state.cancel_edit() {
                "edit"
            } else if self.state.cancel_compose() {
                "compose"
            } else {
                self.close(ctx, "esc");
                return;
            };
            self.log.mark(0, "todocard_esc", &format!("action={layer}"));
            ctx.request_repaint();
            return;
        }

        let scale = ctx.pixels_per_point().max(0.5);
        let action = todocard::draw(
            ctx,
            Theme::standard(),
            &mut self.state,
            &self.groups,
            self.pinned,
            scale,
        );
        self.log_hits(scale);
        self.drag_frame(ctx);
        self.act(ctx, action);

        if Instant::now() >= self.next_refresh {
            self.reload();
            self.next_refresh = Instant::now() + REFRESH;
        }
        ctx.request_repaint_after(self.next_refresh.saturating_duration_since(Instant::now()));
    }

    /// Find the OS window once, so the pin and the drag have a handle.
    fn attach(&mut self) {
        let Some(hwnd) = hub_win::window::find_by_title(TITLE) else {
            return;
        };
        self.hwnd = Some(hwnd);
        let pinned = self.pinned && hub_win::window::attach_to_desktop(hwnd, true);
        if self.pinned && !pinned {
            self.pinned = false;
            self.state.error = "无法固定到桌面".to_string();
        }
        self.log.mark(
            0,
            "todocard_window",
            &format!("hwnd=0x{hwnd:X} pinned={pinned}"),
        );
    }

    fn log_hits(&mut self, scale: f32) {
        if self.hits_logged {
            return;
        }
        if self.state.hits.is_empty() && !self.groups.is_empty() {
            return;
        }
        for hit in &self.state.hits {
            // Physical client pixels, which is what a click posts: the log is read
            // by a script that must not have to know the monitor scale.
            let rect = hit.checkbox;
            self.log.mark(
                0,
                "todocard_hit",
                &format!(
                    "note={} line={} x={} y={} w={} h={}",
                    hit.note,
                    hit.line,
                    (rect.min.x * scale).round() as i32,
                    (rect.min.y * scale).round() as i32,
                    (rect.width() * scale).round() as i32,
                    (rect.height() * scale).round() as i32
                ),
            );
        }
        self.hits_logged = true;
    }

    fn act(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::None => {}
            Action::Close => self.close(ctx, "button"),
            Action::Refresh => self.reload(),
            Action::TogglePin => self.toggle_pin(),
            Action::BeginDrag => self.begin_drag(),
            Action::Toggle { note, line, text } => self.toggle(&note, line, &text),
            Action::Commit {
                note,
                line,
                from,
                text,
            } => {
                if let Some(target) = self.locate(&note, line, &from) {
                    self.commit(&note, target, &text);
                }
            }
            Action::Create(text) => self.create(&text),
            Action::OpenSource(id) => {
                self.push(Request::OpenNote(id));
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
        }
    }

    /// Resolve the row the user clicked against the body as it is *now*.
    fn locate(&mut self, id: &str, line: usize, text: &str) -> Option<usize> {
        let Some(store) = self.store.clone() else {
            self.state.error = "数据库未打开".to_string();
            return None;
        };
        let guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(row) = guard.note(id).ok().flatten() else {
            self.state.error = "这条笔记已不存在".to_string();
            return None;
        };
        match todo::locate(&row.content, line, text) {
            todo::Locate::Exact(at) | todo::Locate::Relocated(at) => Some(at),
            todo::Locate::Conflict => {
                self.state.error = "同名待办不止一条，未改动".to_string();
                self.log
                    .mark(0, "todocard_write", &format!("id={id} line={line} refused=conflict"));
                None
            }
            todo::Locate::Missing => {
                self.state.error = "这条待办已被其他窗口删除".to_string();
                self.log
                    .mark(0, "todocard_write", &format!("id={id} line={line} refused=missing"));
                self.reload();
                None
            }
        }
    }

    fn toggle(&mut self, id: &str, line: usize, text: &str) {
        let Some(store) = self.store.clone() else {
            self.state.error = "数据库未打开".to_string();
            return;
        };
        let at = clock::now_iso();
        let Some(target) = self.locate(id, line, text) else {
            return;
        };
        let mut guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(row) = guard.note(id).ok().flatten() else {
            return;
        };
        match todo::toggle(&row.content, target, &at) {
            Ok(next) => {
                let done = !todo::lines(&row.content)
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
                    // `true` means the row was *created*. A tick almost always
                    // updates an existing note, so the log says which happened
                    // instead of calling the normal case a failure.
                    Ok(created) => format!("created={created}"),
                    Err(error) => format!("error=\"{error}\""),
                };
                self.log.mark(
                    0,
                    "todocard_toggle",
                    &format!("id={id} line={target} done={done} {ok}"),
                );
                self.state.error.clear();
            }
            Err(error) => {
                self.state.error = "这一行已不是待办".to_string();
                self.log
                    .mark(0, "todocard_write", &format!("id={id} line={target} refused={error:?}"));
            }
        }
        drop(guard);
        self.reload();
    }

    fn commit(&mut self, id: &str, line: usize, text: &str) {
        let Some(store) = self.store.clone() else {
            self.state.error = "数据库未打开".to_string();
            return;
        };
        let at = clock::now_iso();
        let mut guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(row) = guard.note(id).ok().flatten() else {
            return;
        };
        match todo::set_text(&row.content, line, text) {
            Ok(next) => {
                let write = NoteWrite {
                    id: row.id,
                    title: row.title,
                    content: next,
                    tags: row.tags,
                    created_at: row.created_at,
                    updated_at: at,
                };
                let ok = match guard.save_note_body(&write) {
                    Ok(created) => format!("created={created}"),
                    Err(error) => format!("error=\"{error}\""),
                };
                self.log.mark(
                    0,
                    "todocard_commit",
                    &format!("id={id} line={line} {ok}"),
                );
                self.state.cancel_edit();
                self.state.error.clear();
            }
            Err(error) => {
                self.state.error = "待办内容不能为空".to_string();
                self.log
                    .mark(0, "todocard_write", &format!("id={id} line={line} refused={error:?}"));
            }
        }
        drop(guard);
        self.reload();
    }

    /// Append a new open todo, creating the inbox note when there is none.
    fn create(&mut self, text: &str) {
        let Some(store) = self.store.clone() else {
            self.state.error = "数据库未打开".to_string();
            return;
        };
        let at = clock::now_iso();
        let mut guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let existing = guard.todo_inbox().ok().flatten();
        let result = match &existing {
            Some(row) => todo::append(&row.content, text).map(|content| (row.id.clone(), content, false)),
            None => Ok((new_note_id(), format!("- [ ] {}", text.trim()), true)),
        };
        match result {
            Ok((id, content, fresh)) => {
                let written = if fresh {
                    guard.upsert_note(&NoteRow {
                        id: id.clone(),
                        title: TODO_INBOX_TITLE.to_string(),
                        content,
                        tags: String::new(),
                        created_at: at.clone(),
                        updated_at: at,
                        is_pinned: false,
                        deleted_at: None,
                    })
                } else {
                    let row = existing.expect("checked above");
                    guard.save_note_body(&NoteWrite {
                        id: row.id,
                        title: row.title,
                        content,
                        tags: row.tags,
                        created_at: row.created_at,
                        updated_at: at,
                    })
                };
                let outcome = match written {
                    Ok(created) => format!("created={created}"),
                    Err(error) => format!("error=\"{error}\""),
                };
                self.log.mark(
                    0,
                    "todocard_create",
                    &format!("id={id} fresh={fresh} {outcome}"),
                );
                self.state.cancel_compose();
                self.state.error.clear();
            }
            Err(_) => self.state.error = "待办内容不能为空".to_string(),
        }
        drop(guard);
        self.reload();
    }

    fn toggle_pin(&mut self) {
        let next = !self.pinned;
        let ok = match self.hwnd {
            Some(hwnd) => {
                // Order matters and matches the shipped command: a pinned card is a
                // child of the desktop, so it must stop being topmost first, and an
                // unpinned one has to regain topmost or it slides under the note it
                // was floating over.
                let attached = hub_win::window::attach_to_desktop(hwnd, next);
                let level = hub_win::window::set_topmost(hwnd, !next);
                attached && level
            }
            None => false,
        };
        if ok {
            self.pinned = next;
            self.state.error.clear();
        } else {
            self.state.error = "无法固定到桌面".to_string();
        }
        self.log
            .mark(0, "todocard_pin", &format!("pinned={next} ok={ok}"));
    }

    fn begin_drag(&mut self) {
        if self.hwnd.is_none() {
            return;
        }
        self.drag = Some(hub_win::window::cursor_position());
        self.log.mark(0, "todocard_drag", "phase=begin");
    }

    /// One frame of following the cursor. Same reasoning as `note::drag`: egui's
    /// client coordinates move with the window, so the delta comes from the screen.
    fn drag_frame(&mut self, ctx: &egui::Context) {
        let Some(previous) = self.drag else {
            return;
        };
        if !ctx.input(|input| input.pointer.primary_down()) {
            self.drag = None;
            self.log.mark(0, "todocard_drag", "phase=end");
            return;
        }
        let Some(hwnd) = self.hwnd else {
            self.drag = None;
            return;
        };
        let now = hub_win::window::cursor_position();
        self.drag = Some(now);
        let (dx, dy) = (now.0 - previous.0, now.1 - previous.1);
        if dx == 0 && dy == 0 {
            return;
        }
        let (x, y) = hub_win::window::client_origin(hwnd);
        hub_win::window::move_to(hwnd, x + dx, y + dy);
    }

    fn close(&mut self, ctx: &egui::Context, reason: &str) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.log.mark(
            0,
            "todocard_close",
            &format!("reason={reason} groups={}", self.groups.len()),
        );
        // Targeted at this viewport: the controller also runs passes for the notes,
        // and `send_viewport_cmd` would close whichever ran last.
        ctx.send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::Close);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    fn push(&self, request: Request) {
        self.inbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
    }

    /// Re-read the list. Not per frame: the card only changes when this window or
    /// another one writes, so a 60 Hz query would make the card the reason notes
    /// feel slow.
    fn reload(&mut self) {
        self.groups.clear();
        self.hits_logged = false;
        let Some(store) = self.store.as_ref() else {
            self.log.mark(0, "todocard_reload", "store=off groups=0 open=0 done=0");
            return;
        };
        let guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.notes(false) {
            Ok(rows) => {
                let notes: Vec<Note<'_>> = rows
                    .iter()
                    .map(|row| Note {
                        id: &row.id,
                        title: &row.title,
                        content: &row.content,
                    })
                    .collect();
                self.groups = todocard::groups(&notes);
            }
            Err(error) => self
                .log
                .mark(0, "todocard_store", &format!("list error=\"{error}\"")),
        }
        let open: usize = self
            .groups
            .iter()
            .map(|group| group.pending().count())
            .sum();
        let done: usize = self.groups.iter().map(|group| group.done().count()).sum();
        self.log.mark(
            0,
            "todocard_reload",
            &format!(
                "store=on groups={} open={open} done={done} pinned={}",
                self.groups.len(),
                self.pinned
            ),
        );
    }
}
