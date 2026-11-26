use super::*;
use hub_core::Store;
use std::path::PathBuf;

fn temp_store() -> (Arc<Mutex<Store>>, PathBuf) {
    // Unique per test: the id is the assertion (a shared file would make one
    // test's rows visible to another's count).
    let dir = std::env::temp_dir().join(format!(
        "hub-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let store = Store::open_in(&dir).expect("store");
    (Arc::new(Mutex::new(store)), dir)
}

fn log() -> Log {
    Log::new(std::env::temp_dir().join("hub-save-test.log"))
}

fn rows(store: &Arc<Mutex<Store>>) -> Vec<NoteRow> {
    store.lock().unwrap().notes(false).expect("list")
}

#[test]
fn nothing_is_written_before_the_debounce_elapses() {
    let (store, _dir) = temp_store();
    let start = Instant::now();
    let mut pump = Pump::new(Some(store.clone()), None);
    let id = pump.id().to_string();

    pump.edit.store(true, Ordering::SeqCst); // the control saw a keystroke
    pump.tick(start, || "买菜清单".to_string(), &log());
    assert!(rows(&store).is_empty(), "wrote on the first frame");

    // 800 ms later: still nothing, because the timer measures silence.
    pump.tick(start + Duration::from_millis(800), || "买菜清单".to_string(), &log());
    assert!(rows(&store).is_empty());

    // Past 900 ms: the row appears, with a title derived from the first line.
    pump.tick(start + Duration::from_millis(950), || "买菜清单".to_string(), &log());
    let all = rows(&store);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, id);
    assert_eq!(all[0].title, "买菜清单");
    assert_eq!(all[0].content, "买菜清单");
}

#[test]
fn a_keystroke_restarts_the_timer_instead_of_letting_it_fire() {
    // The shipped debounce measures the quiet period. A save at t=900 while the
    // user is still typing would write half a sentence and then write again --
    // two `updated_at` values for one edit, and a list that flickers.
    let (store, _dir) = temp_store();
    let start = Instant::now();
    let mut pump = Pump::new(Some(store.clone()), None);

    pump.edit.store(true, Ordering::SeqCst);
    pump.tick(start, || "一".to_string(), &log()); // arms t+900
    pump.edit.store(true, Ordering::SeqCst);
    pump.tick(start + Duration::from_millis(500), || "一二".to_string(), &log()); // re-arms t+1400
    pump.tick(start + Duration::from_millis(950), || "一二".to_string(), &log());
    assert!(rows(&store).is_empty(), "the timer did not restart");

    pump.tick(start + Duration::from_millis(1450), || "一二".to_string(), &log());
    assert_eq!(rows(&store).len(), 1);
}

#[test]
fn a_window_that_never_received_a_keystroke_leaves_no_row() {
    // Parity Q23. The flush path is the one that matters: opening a note and
    // closing it immediately must not litter the list with empty shells.
    let (store, _dir) = temp_store();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("", "close", &log());
    assert!(rows(&store).is_empty(), "an empty note was persisted");
}

#[test]
fn a_persisted_note_can_be_cleared_to_empty() {
    // The other half of Q23, and the reason `persisted` exists: once the row is
    // there, clearing the editor must write the empty body. Skipping it leaves
    // the old text in SQLite and it comes back on the next open.
    let (store, _dir) = temp_store();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("原本的字", "close", &log());
    assert_eq!(rows(&store).len(), 1);

    pump.flush("", "close", &log());
    let all = rows(&store);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].content, "", "the stale body survived a clear");
    assert_eq!(all[0].title, "");
}

#[test]
fn closing_flushes_without_waiting_for_the_timer() {
    // Parity Q21: type one character, press Esc, reopen -- the character is there.
    let (store, _dir) = temp_store();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("一个字", "close", &log());
    let all = rows(&store);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].content, "一个字");
}

#[test]
fn a_flush_with_nothing_new_is_not_a_second_write() {
    // Otherwise every close rewrites `updated_at` and the list reorders itself
    // just because the user looked at a note.
    let (store, _dir) = temp_store();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("内容", "close", &log());
    let first = rows(&store)[0].updated_at.clone();
    pump.flush("内容", "close", &log());
    assert_eq!(rows(&store)[0].updated_at, first, "a no-op close rewrote the row");
}

#[test]
fn crlf_from_the_control_is_stored_as_lf() {
    // The control reports one CRLF per paragraph break; every other reader of
    // this database expects LF, and a browser shows the stray `\r` as a glyph.
    let (store, _dir) = temp_store();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("第一行\r\n第二行\r\n", "close", &log());
    let all = rows(&store);
    assert_eq!(all[0].content, "第一行\n第二行\n");
    assert!(!all[0].content.contains('\r'), "a CR survived into the row");
}

#[test]
fn a_resumed_note_updates_its_row_instead_of_creating_one() {
    let (store, _dir) = temp_store();
    let mut first = Pump::new(Some(store.clone()), None);
    first.flush("原始内容", "close", &log());
    let row = rows(&store).remove(0);

    let mut again = Pump::new(Some(store.clone()), Some(&row));
    assert_eq!(again.take_initial().as_deref(), Some("原始内容"));
    again.flush("改过的内容", "close", &log());

    let all = rows(&store);
    assert_eq!(all.len(), 1, "the resume created a second row");
    assert_eq!(all[0].id, row.id);
    assert_eq!(all[0].content, "改过的内容");
    assert_eq!(all[0].created_at, row.created_at, "identity moved");
}

#[test]
fn a_resumed_note_shows_its_tags_so_a_save_does_not_delete_them() {
    // The round trip that matters: tags live in their own column, and the
    // editor must see them as the trailing line again or the next autosave
    // writes a body without them.
    let (store, _dir) = temp_store();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("买菜\n# 工作 重要", "close", &log());
    let row = rows(&store).remove(0);
    assert_eq!(row.tags, "工作 重要");

    let mut again = Pump::new(Some(store.clone()), Some(&row));
    let shown = again.take_initial().expect("loaded text");
    assert_eq!(shown, "买菜\n# 工作 重要");
    again.flush(&shown, "close", &log());
    assert_eq!(rows(&store)[0].tags, "工作 重要", "tags were lost on re-save");
}

#[test]
fn without_a_store_the_note_stays_in_memory_and_says_so_once() {
    let mut pump = Pump::new(None, None);
    assert!(pump.disabled());
    let log = log();
    let start = Instant::now();
    // Two frames: the second must not repeat the explanation.
    pump.tick(start, || "x".to_string(), &log);
    pump.tick(start + IDLE, || "x".to_string(), &log);
    assert!(!pump.saver.persisted);
}

#[test]
fn the_change_flag_raises_and_requests_a_frame() {
    let flag = Arc::new(AtomicBool::new(false));
    let ctx = egui::Context::default();
    assert!(!ctx.has_requested_repaint(), "fixture must start clean");

    handler(flag.clone(), ctx.clone(), egui::ViewportId::ROOT)();

    assert!(flag.load(Ordering::SeqCst), "the frame reads this flag");
    assert!(
        ctx.has_requested_repaint(),
        "without a frame request the save waits for an unrelated message"
    );
}

#[test]
fn a_scan_that_finds_no_change_does_not_arm_a_write() {
    // Without this the 2 s scan would schedule a save every time it looked, and
    // an idle note would write a no-op transaction -- and a log line -- twice a
    // minute, forever.
    let (store, _dir) = temp_store();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("稳定", "close", &log());
    let idle = Instant::now() + Duration::from_secs(3);

    pump.tick(idle, || "稳定".to_string(), &log());

    assert!(!pump.saver.dirty("稳定"));
    assert!(
        !pump.saver.due(idle),
        "the scan armed a write for an unchanged document"
    );
    assert_eq!(rows(&store).len(), 1, "a no-op scan wrote a row");
}

#[test]
fn a_scan_that_finds_a_change_arms_the_debounce() {
    // The other half of the scan, and the reason the gate can type a Return
    // whose `WM_CHAR` never arrives: the change is noticed by comparison, not by
    // a signal, and it still reaches the database one idle period later.
    let (store, _dir) = temp_store();
    let start = Instant::now();
    let mut pump = Pump::new(Some(store.clone()), None);
    pump.flush("旧", "close", &log());
    let scanned = start + SCAN + Duration::from_millis(500);

    // No flag is raised: this is the control changing under a message nobody
    // listed in `dispatch`.
    pump.tick(scanned, || "旧新".to_string(), &log());
    assert_eq!(
        rows(&store)[0].content,
        "旧",
        "the scan wrote instead of arming the debounce"
    );

    pump.tick(scanned + AUTOSAVE + Duration::from_millis(60), || "旧新".to_string(), &log());
    assert_eq!(rows(&store)[0].content, "旧新", "the armed write never happened");
}

#[test]
fn a_rebased_baseline_is_not_an_edit() {
    // What the control reports after a load decides the baseline. If the control
    // hands back a form the shell did not push in (a trailing paragraph mark,
    // CRLF instead of LF), comparing against the pushed string would make every
    // reopened note rewrite its row one second after opening.
    let mut pump = Pump::new(None, None);
    pump.saver.store = Some(temp_store().0); // a store, so `dirty` is meaningful
    pump.rebase("来自控件\r\n第二行");
    assert!(!pump.saver.dirty("来自控件\r\n第二行"), "the loaded text reads as dirty");
    assert!(pump.saver.dirty("来自控件\r\n第二行\r\n"), "a real change was missed");
}

#[test]
fn a_draft_appears_while_typing_and_goes_away_once_saved() {
    // Q24's two halves in one run: the prompt reads a file, so something has to
    // write it -- and it has to be gone once the row is the truth, or every
    // successful save would leave a prompt behind for the next open.
    let (store, root) = temp_store();
    let backups = backup::dir(&root);
    let mut pump = Pump::new(Some(store.clone()), None);
    let id = pump.id().to_string();
    let log = log();
    let start = Instant::now();

    pump.edit.store(true, Ordering::SeqCst);
    pump.tick(start, || "还没存".to_string(), &log);
    let draft = backup::read(&backups, &id).expect("draft after a keystroke");
    assert_eq!(draft.content, "还没存");
    assert_eq!(rows(&store).len(), 0, "the draft was written as a row");

    pump.tick(
        start + AUTOSAVE + Duration::from_millis(20),
        || "还没存".to_string(),
        &log,
    );
    assert_eq!(rows(&store).len(), 1, "the debounce never fired");
    assert!(
        backup::read(&backups, &id).is_none(),
        "the draft outlived the save it was protecting"
    );
}

#[test]
fn wait_never_returns_zero() {
    // A zero sleep is a busy loop: every frame would ask for the next frame
    // immediately, and the note would burn a core while nothing is happening.
    let mut pump = Pump::new(None, None);
    pump.saver.store = Some(temp_store().0); // pretend a store exists
    pump.saver.touched(Instant::now());
    let wait = pump.saver.wait(Instant::now() + AUTOSAVE);
    assert_eq!(wait, FLOOR);
}
