//! Headless tests for the workbench: one frame in, the reported rectangles out.
//!
//! Split from `manager.rs` when the parent crossed 800 lines. Nothing here needs a
//! window: `draw` takes a context, and the hit rectangles it reports are the same
//! ones the gate clicks in the real process.

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
fn the_list_shows_the_same_todos_the_card_does() {
    // One parser now, but this test stays: it pins what the *list* relies on --
    // the marks a shipped note actually uses (`-`/`*`, `x`/`X`) and that a plain
    // bullet is not a todo.
    let items = hub_core::todo::lines("- [ ] 买菜\n* [x] 洗衣\n- [X] 回邮件\nnot a todo\n");
    let seen: Vec<(&str, bool)> = items.iter().map(|i| (i.text.as_str(), i.done)).collect();
    assert_eq!(seen, vec![("买菜", false), ("洗衣", true), ("回邮件", true)]);
}

#[test]
fn an_empty_todo_note_has_no_items_rather_than_a_blank_one() {
    assert!(hub_core::todo::lines("").is_empty());
    assert!(hub_core::todo::lines("普通段落\n- 列表但不是待办").is_empty());
}

#[test]
fn a_row_title_falls_back_to_the_first_line_like_the_note_does() {
    // The list and the note must agree on what a note is called, or the same
    // note has two names depending on where you look at it.
    let row = row("a", "", "\n买菜清单\n鸡蛋", "", false);
    assert_eq!(row.title, "买菜清单");
}

#[test]
fn a_row_preview_never_repeats_its_own_title() {
    // "标题\n正文" is the common shape, and without the check the row read the same
    // sentence twice, one line apart -- which is how the first screenshot of this
    // layout looked.
    let noted = row("a", "", "周报\n本周三条结论", "", false);
    assert_eq!(noted.title, "周报");
    assert_eq!(noted.preview, "本周三条结论");

    // A note that is only a title has no preview; the row then draws one line and
    // centres it instead of leaving a hole where the preview would be.
    let bare = row("b", "", "周报", "", false);
    assert_eq!(bare.preview, "");
}

#[test]
fn tabs_are_stable_for_the_settings_file_and_the_log() {
    // The ids end up in log lines; a rename would break a saved expectation.
    assert_eq!(Tab::Notes.id(), "notes");
    assert_eq!(Tab::Trash.id(), "trash");
    assert_eq!(Tab::ALL.len(), 4);
    // The rail's tile is a character from the CJK face, and the caption is what
    // the log and the screenshots show. Both are asserted so a rename cannot slip
    // through as "just a label".
    assert_eq!(Tab::Notes.glyph(), "笔");
    assert_eq!(Tab::Settings.caption(), "SETTINGS");
}

/// Run one frame and hand back the rectangles it reported.
fn frame(state: &mut State, rows: &[Row]) -> Vec<(String, egui::Rect)> {
    frame_with(state, rows, &[])
}

fn frame_with(
    state: &mut State,
    rows: &[Row],
    todos: &[TodoGroup],
) -> Vec<(String, egui::Rect)> {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(900.0, 620.0),
        )),
        ..Default::default()
    };
    let table = Table::defaults();
    let mut hits = Vec::new();
    let _ = ctx.run(input, |ctx| {
        let _ = draw(ctx, Theme::standard(), state, rows, todos, &table);
        hits = state.hits.clone();
    });
    hits
}

fn named(hits: &[(String, egui::Rect)], name: &str) -> bool {
    hits.iter().any(|(other, _)| other == name)
}

#[test]
fn a_row_reports_its_own_buttons_only_once_it_is_selected() {
    // The buttons are drawn under the selected row, so the gate needs two things
    // to be true: the row is addressable before it is selected (to select it) and
    // the buttons appear only then. Asserting the absence as well as the presence
    // is what stops "report every row's buttons" from passing by accident.
    let rows = vec![
        row("note-1", "一", "一\n正文", "", false),
        row("note-2", "二", "二\n正文", "", false),
    ];
    let mut state = State::default();
    let before = frame(&mut state, &rows);
    assert!(named(&before, "row:note-1"), "the row itself must be clickable");
    assert!(!named(&before, "pin:note-1"), "no row was selected yet");

    state.selected = Some("note-2".to_string());
    let after = frame(&mut state, &rows);
    assert!(named(&after, "open:note-2"));
    assert!(named(&after, "pin:note-2"));
    assert!(named(&after, "trash:note-2"));
    assert!(!named(&after, "pin:note-1"), "only the selected row has buttons");
}

#[test]
fn the_todo_tab_offers_the_card_and_the_notes_tab_does_not() {
    // The card could only be opened from the tray in takeover mode. The button
    // lives in the todo header now, where the frontend panel keeps it.
    let rows = vec![row("note-1", "一", "一", "", false)];
    let mut state = State::default();
    assert!(!named(&frame(&mut state, &rows), "todo-card"));
    state.tab = Tab::Todos;
    let hits = frame(&mut state, &rows);
    assert!(named(&hits, "todo-card"), "todo tab must offer the card");
    assert!(named(&hits, "tab:todos"));
}

#[test]
fn every_section_is_reachable_from_the_rail() {
    // The rail is hand-painted, so "the tab is drawn" and "the tab is clickable"
    // are not the same claim any more: the rectangle must be reported or the gate
    // has nothing to click. Settings is pinned at the bottom, which is exactly the
    // case a fixed-height rail would have pushed off the frame.
    let rows = vec![row("note-1", "一", "一", "", false)];
    let mut state = State::default();
    let hits = frame(&mut state, &rows);
    for tab in Tab::ALL {
        assert!(
            named(&hits, &format!("tab:{}", tab.id())),
            "{} must report a rectangle",
            tab.id()
        );
    }
    // The header belongs to the panel, not the rail: notes offers the primary
    // action, and the card button is not on this section.
    assert!(named(&hits, "new-note"));
    assert!(!named(&hits, "todo-card"));
    // And the rail must not overlap the panel: the first row of the list starts to
    // the right of the last rail rectangle.
    let rail_right = hits
        .iter()
        .filter(|(name, _)| name.starts_with("tab:"))
        .map(|(_, rect)| rect.right())
        .fold(0.0_f32, f32::max);
    let row_left = hits
        .iter()
        .find(|(name, _)| name == "row:note-1")
        .map(|(_, rect)| rect.left())
        .expect("the row must be reported");
    assert!(
        row_left > rail_right,
        "row {row_left} must start right of the rail {rail_right}"
    );
}

#[test]
fn clicking_a_trashed_row_selects_it_so_it_can_be_restored() {
    // A trashed row has no "open" action, and the first version therefore never
    // set `selected` for it: 恢复 was unreachable for every row except the one
    // just deleted. The gate found this by clicking and watching nothing happen;
    // it is pinned here because the rule is one line and easy to undo.
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(900.0, 620.0));
    let table = Table::defaults();
    let rows = vec![row("gone", "删掉的", "删掉的", "", true)];
    let mut state = State {
        tab: Tab::Trash,
        ..Default::default()
    };
    let pass = |state: &mut State, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            let _ = draw(ctx, Theme::standard(), state, &rows, &[], &table);
        });
    };
    pass(&mut state, Vec::new());
    let rect = state
        .hits
        .iter()
        .find(|(name, _)| name == "row:gone")
        .expect("the trashed row must be reported")
        .1;
    let at = rect.center();
    let press = |pressed: bool| {
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }]
    };
    pass(&mut state, press(true));
    pass(&mut state, press(false));
    assert_eq!(state.selected.as_deref(), Some("gone"));
    // The buttons are drawn for the row that was *already* selected when the
    // frame started, so they appear on the next frame -- a click asks for a
    // repaint, which is that frame in the real window.
    pass(&mut state, Vec::new());
    assert!(named(&state.hits, "restore:gone"), "恢复 must become reachable");
    assert!(!named(&state.hits, "open:gone"), "a trashed row cannot be opened");
}

// ── the todo pane ────────────────────────────────────────────────────────────

fn group(id: &str, title: &str, content: &str) -> TodoGroup {
    TodoGroup::from_note(id, title, content).expect("the fixture is a note with todos")
}

fn rect_of(hits: &[(String, egui::Rect)], name: &str) -> egui::Rect {
    hits.iter()
        .find(|(other, _)| other == name)
        .unwrap_or_else(|| panic!("{name} was not reported"))
        .1
}

/// One frame over both lists, returning what it produced.
///
/// The trash tests need rows *and* possibly todos; the todo tests need the reverse.
/// One helper with both parameters keeps the frame exactly as the window runs it.
fn run(
    ctx: &egui::Context,
    state: &mut State,
    rows: &[Row],
    todos: &[TodoGroup],
    events: Vec<egui::Event>,
) -> Action {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(900.0, 620.0),
        )),
        events,
        ..Default::default()
    };
    let mut action = Action::None;
    let _ = ctx.run(input, |ctx| {
        action = draw(ctx, Theme::standard(), state, rows, todos, &Table::defaults());
    });
    action
}

/// Press and release at the centre of `name`, returning the action of the release.
fn click(ctx: &egui::Context, state: &mut State, rows: &[Row], name: &str) -> Action {
    run(ctx, state, rows, &[], Vec::new());
    let at = rect_of(&state.hits, name).center();
    let button = |pressed: bool| {
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }]
    };
    run(ctx, state, rows, &[], button(true));
    run(ctx, state, rows, &[], button(false))
}

/// Press and release at the centre of `name`, returning the action of the release.
fn click_name(state: &mut State, todos: &[TodoGroup], name: &str) -> Action {
    let ctx = egui::Context::default();
    run(&ctx, state, &[], todos, Vec::new());
    let at = rect_of(&state.hits, name).center();
    let button = |pressed: bool| {
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }]
    };
    run(&ctx, state, &[], todos, button(true));
    run(&ctx, state, &[], todos, button(false))
}

#[test]
fn a_group_skips_notes_that_have_no_checkboxes() {
    // Every note in a store is a candidate, and prose has no todos. A heading over
    // nothing would push the real groups off the pane, and the shipped panel does not
    // list notes with no tasks either.
    assert!(TodoGroup::from_note("a", "散文", "今天很好\n天气不错").is_none());
    let only_done = TodoGroup::from_note("b", "做完了", "- [x] 交房租").expect("a done line");
    assert_eq!(
        todo::counts(&[only_done]),
        (0, 1),
        "完全做完的笔记仍是一个组，否则「全部」与计数会对不上"
    );
    let noted = group("c", "清单", "清单\n- [ ] 买菜\n- [x] 洗衣");
    assert_eq!(noted.title, "清单", "组的标题就是笔记的标题");
    assert_eq!(todo::counts(&[noted]), (1, 2));
}

#[test]
fn a_line_offers_a_box_to_tick_and_a_text_to_jump() {
    // The shipped panel splits these: the box changes the task, the text takes you to
    // the note the task came from. One target could not express both, and the gate
    // needs both addressable to make the two claims separately.
    let todos = vec![group("box", "清单", "清单\n- [ ] 买菜\n- [x] 洗衣")];
    let mut state = State {
        tab: Tab::Todos,
        ..Default::default()
    };
    let hits = frame_with(&mut state, &[], &todos);
    assert!(named(&hits, "todo-tick:box:1"), "the box must be a target");
    assert!(named(&hits, "todo-open:box:1"), "the text must be a target");
    // Two targets for one line, side by side and not on top of each other: a box
    // nested inside the text's rectangle would make the click ambiguous.
    let tick = rect_of(&hits, "todo-tick:box:1");
    let text = rect_of(&hits, "todo-open:box:1");
    assert!(
        tick.width() >= 24.0 && tick.height() >= 24.0,
        "the box target is {}x{} -- 14 px of drawn square is not a 14 px control",
        tick.width(),
        tick.height()
    );
    assert!(text.left() >= tick.right(), "the text starts after the box's column");
}

#[test]
fn clicking_the_box_asks_to_toggle_that_line_and_carries_its_text() {
    // The text travels with the action because the controller re-locates the task by
    // its words: a line number from this frame is a hint, and another window may have
    // inserted a line above it since.
    let todos = vec![group("box", "清单", "清单\n- [ ] 买菜\n- [x] 洗衣")];
    let mut state = State {
        tab: Tab::Todos,
        ..Default::default()
    };
    assert_eq!(
        click_name(&mut state, &todos, "todo-tick:box:1"),
        Action::ToggleTodo {
            note: "box".to_string(),
            line: 1,
            text: "买菜".to_string(),
        }
    );
}

#[test]
fn clicking_the_text_opens_the_note_the_task_lives_in() {
    let todos = vec![group("box", "清单", "清单\n- [ ] 买菜\n- [x] 洗衣")];
    let mut state = State {
        tab: Tab::Todos,
        ..Default::default()
    };
    assert_eq!(
        click_name(&mut state, &todos, "todo-open:box:1"),
        Action::Open("box".to_string())
    );
}

#[test]
fn the_default_filter_hides_completed_lines_and_all_shows_them() {
    // A to-do list that shows everything it has ever held is an archive. The default
    // is 未完成, and 全部 is one click away -- chip, not a setting.
    let todos = vec![group("box", "清单", "清单\n- [ ] 买菜\n- [x] 洗衣")];
    let mut state = State {
        tab: Tab::Todos,
        ..Default::default()
    };
    assert_eq!(state.filter, Filter::Open, "未完成 is the default view");
    let hits = frame_with(&mut state, &[], &todos);
    assert!(named(&hits, "todo-open:box:1"));
    assert!(!named(&hits, "todo-open:box:2"), "已完成 must be hidden by default");
    assert!(named(&hits, "todo-filter:open"));
    assert!(named(&hits, "todo-filter:all"));

    state.filter = Filter::All;
    let hits = frame_with(&mut state, &[], &todos);
    assert!(named(&hits, "todo-open:box:2"), "全部 must show what was ticked");
}

#[test]
fn the_all_chip_is_an_action_so_the_log_can_say_what_the_filter_is() {
    // The filter is view state, but an empty list is the symptom it explains, and a
    // post-mortem cannot read view state. It travels as an action for the log only.
    let todos = vec![group("box", "清单", "清单\n- [ ] 买菜")];
    let mut state = State {
        tab: Tab::Todos,
        ..Default::default()
    };
    assert_eq!(
        click_name(&mut state, &todos, "todo-filter:all"),
        Action::SetTodoFilter(Filter::All)
    );
    assert_eq!(state.filter, Filter::All);
}

// ── the trash pane ───────────────────────────────────────────────────────────

/// Two trashed notes and one live one, so "the trash" and "the list" differ.
fn trash_rows() -> Vec<Row> {
    vec![
        row("gone-a", "删掉的甲", "删掉的甲\n正文", "", true),
        row("gone-b", "删掉的乙", "删掉的乙", "", true),
        row("live", "留下的", "留下的\n正文", "", false),
    ]
}

fn trash_state() -> State {
    State {
        tab: Tab::Trash,
        ..Default::default()
    }
}

#[test]
fn a_trashed_row_offers_a_box_that_picks_instead_of_selecting() {
    // The trash needs two verbs per row: pick it for the batch, or select it to get
    // 恢复/彻底删除. They are one hit area split by x, so both halves are pinned: the
    // box must exist, its target must be the column rather than the drawn square, and
    // a pick must not select (a selection reveals a button strip and moves every row
    // below it, so the second tick would land on the wrong row).
    let rows = trash_rows();
    let mut state = trash_state();
    let hits = frame(&mut state, &rows);
    assert!(named(&hits, "check:gone-a"), "a trashed row must be pickable");
    assert!(!named(&hits, "check:live"), "the notes tab has no boxes");
    let check = rect_of(&hits, "check:gone-a");
    assert!(
        check.width() >= 24.0 && check.height() >= 24.0,
        "the target is the column, not the 14 px square: {}x{}",
        check.width(),
        check.height()
    );

    let ctx = egui::Context::default();
    assert_eq!(click(&ctx, &mut state, &rows, "check:gone-a"), Action::None);
    assert!(state.checked.contains("gone-a"), "the box ticks");
    assert_eq!(state.selected, None, "picking is not selecting");

    // The text of the same row still selects it, which is what reveals the buttons.
    assert_eq!(click(&ctx, &mut state, &rows, "row:gone-a"), Action::None);
    assert_eq!(state.selected.as_deref(), Some("gone-a"));
}

#[test]
fn the_batch_bar_shows_up_only_when_something_is_picked() {
    // A bar that is always there with a "恢复选中 (0)" button is a button that can only
    // fail. It appears with the first tick and leaves with the restore.
    let rows = trash_rows();
    let mut state = trash_state();
    let hits = frame(&mut state, &rows);
    assert!(!named(&hits, "restore-selected"), "nothing is picked yet");
    assert!(!named(&hits, "clear-selection"));

    let ctx = egui::Context::default();
    click(&ctx, &mut state, &rows, "check:gone-a");
    let hits = frame(&mut state, &rows);
    assert!(named(&hits, "restore-selected"), "one tick is enough to offer it");
    assert!(named(&hits, "clear-selection"), "and there is a way back out");
}

#[test]
fn restore_selected_carries_every_picked_id_in_a_stable_order() {
    // The ids travel with the action, so the log names the rows instead of "whatever
    // was ticked". A `HashSet` would have made the line differ between runs.
    let rows = trash_rows();
    let mut state = trash_state();
    let ctx = egui::Context::default();
    click(&ctx, &mut state, &rows, "check:gone-b");
    click(&ctx, &mut state, &rows, "check:gone-a");
    assert_eq!(state.checked.len(), 2);

    assert_eq!(
        click(&ctx, &mut state, &rows, "restore-selected"),
        Action::RestoreSelected(vec!["gone-a".to_string(), "gone-b".to_string()])
    );
    assert!(!state.checked.is_empty(), "the controller clears it, not the click");
}

#[test]
fn clearing_the_pick_leaves_the_rows_alone() {
    let rows = trash_rows();
    let mut state = trash_state();
    let ctx = egui::Context::default();
    click(&ctx, &mut state, &rows, "check:gone-a");
    assert_eq!(click(&ctx, &mut state, &rows, "clear-selection"), Action::None);
    assert!(state.checked.is_empty(), "取消选择 clears the set, it does not restore");
    let hits = frame(&mut state, &rows);
    assert!(!named(&hits, "restore-selected"));
    assert!(named(&hits, "row:gone-a"), "the row is still in the trash");
}

#[test]
fn the_other_two_sub_tabs_answer_instead_of_listing_notes() {
    // 收集箱 and 脚本 are the Tauri manager's; the chip leads to a sentence saying so.
    // A dimmed chip would be a dead control, and an empty list would be a claim about
    // data this window cannot see.
    let rows = trash_rows();
    let mut state = trash_state();
    let hits = frame(&mut state, &rows);
    for view in TrashView::ALL {
        assert!(
            named(&hits, &format!("trash-view:{}", view.id())),
            "{} must be addressable",
            view.id()
        );
    }
    assert!(!named(&hits, "trash-notice"), "笔记 is the default, and it has a list");

    let ctx = egui::Context::default();
    assert_eq!(
        click(&ctx, &mut state, &rows, "trash-view:inbox"),
        Action::SetTrashView(TrashView::Inbox)
    );
    let hits = frame(&mut state, &rows);
    assert!(named(&hits, "trash-notice"), "the notice replaces the list");
    assert!(!named(&hits, "row:gone-a"), "and the rows are not behind it");
    assert!(TrashView::Inbox.notice().is_some() && TrashView::Notes.notice().is_none());
}

#[test]
fn empty_trash_asks_before_it_takes_anything() {
    // Same rule as 彻底删除: one click is a question, only the dialog's yes deletes.
    // It also has no business being on screen when the trash is empty.
    let rows = trash_rows();
    let mut state = trash_state();
    let hits = frame(&mut state, &rows);
    assert!(named(&hits, "empty-trash"));
    assert_eq!(state.pending, None);

    let ctx = egui::Context::default();
    assert_eq!(click(&ctx, &mut state, &rows, "empty-trash"), Action::None);
    assert_eq!(
        state.pending,
        Some(super::Pending::EmptyTrash { count: 2 }),
        "the question has to name how many rows it would take"
    );

    // Nothing in the trash, nothing to empty.
    let mut empty = trash_state();
    let hits = frame(&mut empty, &[row("live", "留下的", "留下的", "", false)]);
    assert!(!named(&hits, "empty-trash"), "no rows, no destructive button");
}
