//! S2 evidence: what the text island actually did.
//!
//! Everything here runs on the parked window — no screenshot, no foreground
//! window, no human. The two numbers that matter:
//!
//! * `type` = insert + paint for one character. **The gate** (p95 ≤ 16 ms).
//! * `compose` = the same keystroke plus `DwmFlush`. Reported only: the compositor
//!   runs at 60 Hz, so this is `type + (0..16.7 ms)` by construction.
//!
//! The rest (pixel probe, char format echo, undo/redo, scroll) exists so "the
//! theme applied" and "undo works" are measurements instead of impressions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use hub_editor::keys::{ESCAPE, TAB, Z};
use hub_editor::measure::{CharTiming, PixelProbe};
use hub_editor::Editor;

use crate::bench::{distribution, Distribution};

/// Typing corpus: CJK on purpose. Latin text would not exercise the fallback font
/// path and would make the numbers look better than the product will be.
const CORPUS: &str = "天地玄黄宇宙洪荒日月盈昃辰宿列张寒来暑往秋收冬藏闰余成岁律吕调阳云腾致雨露结为霜金生丽水玉出昆冈剑号巨阙珠称夜光";

pub fn corpus_text(chars: usize) -> String {
    CORPUS.chars().cycle().take(chars).collect()
}

#[derive(Clone, Debug)]
pub struct Evidence {
    pub class: String,
    pub limit: usize,
    pub face: String,
    pub height_twips: i32,
    pub color: u32,
    pub pixels: PixelProbe,
    pub load_1k_ms: f64,
    pub load_10k_ms: f64,
    /// Display lines in the 10k document. Evidence that "must scroll" is true.
    pub lines: i32,
    /// `first_visible_line` after scrolling a 20-character document to the caret.
    pub scroll_small: i32,
    /// `first_visible_line` after scrolling a 10k document to the caret.
    pub scroll_large: i32,
    /// Scroll offset in pixels for the 10k document, in case line numbers lie.
    pub offset_large: i32,
    /// Sampled pixels exactly matching the configured text colour, empty document.
    pub fg_empty: usize,
    /// Same, with 20 characters in the document.
    pub fg_typed: usize,
    /// Pixel evidence that characters actually reach the surface: ink share before
    /// and after typing 20 characters into an empty document.
    pub ink_empty: f64,
    pub ink_typed: f64,
    /// `EM_REPLACESEL` with a 10k document: the paste path (B4).
    pub paste_ms: f64,
    pub paste_chars: usize,
    /// `set_rect` must report "nothing moved" for an identical rectangle (D2).
    pub resize_same_moved: bool,
    /// ...and must move when the layout actually changed (A3).
    pub resize_changed_moved: bool,
    pub resize_restored_moved: bool,
    /// Undo units for a contiguous typing run (RichEdit coalesces: expect 1).
    pub undo_coalesced: usize,
    /// Undo units once the typing group is broken by a caret move (expect >= 2,
    /// which is what proves the stack has depth rather than a single snapshot).
    pub undo_grouped: usize,
    pub undo_steps: usize,
    pub redo_steps: usize,
    pub undo_ok: bool,
    pub redo_ok: bool,
    /// Esc reached the subclass and the shell callback fired (B7).
    pub escape_ok: bool,
    /// Tab inserted a tab instead of moving focus (D5).
    pub tab_ok: bool,
    /// A key nobody intercepts still reaches the control (D5 fallback).
    pub fallthrough_ok: bool,
    /// The control has an input-method context (B9, automatable half).
    pub imm_ok: bool,
    pub burst_ms: f64,
}

#[derive(Clone, Debug)]
pub struct TypeReport {
    pub n: usize,
    pub type_us: Distribution,
    pub insert_us: Distribution,
    pub repaint_us: Distribution,
    pub compose_us: Distribution,
    pub compose_n: usize,
    /// Whether the `compose_us` series is admissible at all. See
    /// [`COMPOSE_PLAUSIBLE_US`] - a dormant compositor must not be reported as
    /// editor latency, and it must not be silently dropped either.
    pub compose_valid: bool,
}

/// Upper bound for one composed frame that still means "the compositor was
/// presenting". Two frames at 60 Hz is 33 ms; 40 leaves room for a hitch.
///
/// Measured 2026-11-25: on a parked window with an idle desktop `DwmFlush`
/// returned **254 ms p50**, because nothing on screen changes, so DWM has nothing
/// to present. The same code reported 2.7 ms while the desktop was active. That
/// difference is compositor idle time, not editor cost, so the series is marked
/// invalid instead of recorded.
pub const COMPOSE_PLAUSIBLE_US: u64 = 40_000;

pub fn compose_is_plausible(us: u64) -> bool {
    us <= COMPOSE_PLAUSIBLE_US
}

impl TypeReport {
    pub fn p95_ms(&self) -> f64 {
        self.type_us.p95 as f64 / 1000.0
    }

    pub fn within(&self, budget_p95_ms: f64, budget_p99_ms: f64) -> bool {
        self.p95_ms() <= budget_p95_ms && self.type_us.p99 as f64 / 1000.0 <= budget_p99_ms
    }
}

/// Identity and pixel evidence. Runs `set_text` several times, so call it once.
pub fn capture(editor: &mut Editor) -> Evidence {
    let class = editor.class_name();
    let limit = editor.limit();
    let format = editor.char_format();

    let started = Instant::now();
    editor.set_text(&corpus_text(1_000));
    let load_1k_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    editor.set_text(&corpus_text(10_000));
    let load_10k_ms = started.elapsed().as_secs_f64() * 1000.0;

    // Scroll evidence. Two preconditions, both established by measurement rather
    // than assumed (see `examples/scroll_probe.rs`):
    //
    //  * `EM_SCROLLCARET` is a **no-op while the control lacks focus**. Moving the
    //    selection does not scroll either. On a parked, never-focused island both
    //    probes therefore returned 0 and looked like a broken control.
    //  * `EM_LINESCROLL` does work without focus, which is what proved the failure
    //    was the caret path and not "the document does not need to scroll".
    //
    // The 10k document wraps to ~589 display lines inside an 1148 px island, so
    // "large must be > 0" is a meaningful assertion.
    let lines = editor.line_count();
    editor.focus();

    editor.set_text(&corpus_text(10_000));
    editor.caret_to_end();
    editor.scroll_caret();
    let scroll_large = editor.first_visible_line();
    let offset_large = editor.scroll_pos().1;

    editor.set_text(&corpus_text(20));
    editor.caret_to_end();
    editor.scroll_caret();
    let scroll_small = editor.first_visible_line();

    // A3/D2: the island follows the layout, and does not move when it did not.
    let original = editor.rect();
    let resize_same_moved = editor.set_rect(original);
    let narrowed = hub_core::PixelRect {
        width: (original.width - 80).max(1),
        ..original
    };
    let resize_changed_moved = editor.set_rect(narrowed);
    let resize_restored_moved = editor.set_rect(original);

    let undo_coalesced = undo_units_contiguous(editor);
    let undo_grouped = undo_units_grouped(editor);
    let (undo_steps, redo_steps, undo_ok, redo_ok) = undo_roundtrip(editor);

    // B4: the paste path, as opposed to the load path, goes through EM_REPLACESEL.
    editor.set_text("");
    let started = Instant::now();
    editor.replace_selection(&corpus_text(10_000));
    let paste_ms = started.elapsed().as_secs_f64() * 1000.0;
    let paste_chars = editor.char_count();

    // B7/D5: the subclass intercepts Esc and Tab, and nothing else.
    let escaped = Arc::new(AtomicBool::new(false));
    let flag = escaped.clone();
    // Replaces the shell's callback: this runs once at the end of the bench, and
    // the bench exits immediately afterwards.
    editor.on_escape(Box::new(move || {
        flag.store(true, Ordering::SeqCst);
    }));

    editor.set_text("ab");
    editor.caret_to_end();
    let consumed = editor.key_down(ESCAPE);
    let escape_ok = consumed == 0 && escaped.load(Ordering::SeqCst);

    editor.set_text("ab");
    editor.caret_to_end();
    editor.key_down(TAB);
    let tab_ok = editor.text() == "ab\t";

    editor.set_text("x");
    editor.caret_to_end();
    let after_key = editor.key_press(Z);
    let fallthrough_ok = after_key.ends_with('z');
    let imm_ok = editor.imm_context();

    // A4: "the text colour applied" as a pixel count. An empty document must have
    // no pixel of the configured colour, a document with glyphs must have some.
    editor.set_text("");
    let empty_probe = editor.pixel_probe();
    let ink_empty = empty_probe.ink_share();
    let fg_empty = empty_probe.fg_hits;

    editor.set_text(&corpus_text(20));
    editor.caret_to_end();
    let typed_probe = editor.pixel_probe();
    let ink_typed = typed_probe.ink_share();
    let fg_typed = typed_probe.fg_hits;

    Evidence {
        class,
        limit,
        face: format.face,
        height_twips: format.height_twips,
        color: format.color,
        pixels: typed_probe,
        load_1k_ms,
        load_10k_ms,
        lines,
        scroll_small,
        scroll_large,
        offset_large,
        fg_empty,
        fg_typed,
        ink_empty,
        ink_typed,
        paste_ms,
        paste_chars,
        resize_same_moved,
        resize_changed_moved,
        resize_restored_moved,
        undo_coalesced,
        undo_grouped,
        undo_steps,
        redo_steps,
        undo_ok,
        redo_ok,
        escape_ok,
        tab_ok,
        fallthrough_ok,
        imm_ok,
        burst_ms: 0.0,
    }
}

/// Undo units produced by one uninterrupted typing run. RichEdit merges them, so
/// this is expected to be 1 - the point is that the record states the measured
/// value instead of claiming one step per character.
fn undo_units_contiguous(editor: &mut Editor) -> usize {
    editor.set_text("起点");
    editor.clear_undo();
    editor.caret_to_end();
    for ch in corpus_text(10).chars() {
        editor.append_char(ch);
    }
    undo_until_empty(editor)
}

/// Undo units after breaking the typing group the way a user actually does: by
/// moving the caret. `EM_STOPGROUPTYPING` was tried first and does **not** split a
/// typing run on `RICHEDIT50W` (measured: still 1 unit), while a caret move does
/// (measured: 2). If this ever reads 1, the undo stack is not multi-level and B5's
/// "stack is correct" claim would be hollow.
fn undo_units_grouped(editor: &mut Editor) -> usize {
    editor.set_text("起点");
    editor.clear_undo();
    editor.caret_to_end();
    for ch in corpus_text(10).chars() {
        editor.append_char(ch);
    }
    // Break the group: select-all then collapse back to the end.
    editor.select_all();
    editor.caret_to_end();
    for ch in corpus_text(10).chars() {
        editor.append_char(ch);
    }
    undo_until_empty(editor)
}

fn undo_until_empty(editor: &mut Editor) -> usize {
    let mut steps = 0;
    while steps < 100 && editor.undo(1) {
        steps += 1;
    }
    steps
}

/// Type 50 characters, undo until the stack is empty, redo until it is empty.
///
/// Undo steps are counted rather than assumed: RichEdit coalesces consecutive
/// typing into one undo unit, so "50 edits = 50 undos" would be a false claim. What
/// must hold is that undoing everything restores the original text and redoing
/// everything restores the edited text.
fn undo_roundtrip(editor: &mut Editor) -> (usize, usize, bool, bool) {
    let original = "起点".to_string();
    editor.set_text(&original);
    editor.clear_undo();
    editor.caret_to_end();

    for ch in corpus_text(50).chars() {
        editor.append_char(ch);
    }
    let edited = editor.text();

    let mut undo_steps = 0;
    while undo_steps < 200 && editor.undo(1) {
        undo_steps += 1;
    }
    let undo_ok = editor.text() == original;

    let mut redo_steps = 0;
    while redo_steps < 200 && editor.text() != edited {
        editor.redo(1);
        redo_steps += 1;
        if redo_steps > 200 {
            break;
        }
    }
    let redo_ok = editor.text() == edited;

    (undo_steps, redo_steps, undo_ok, redo_ok)
}

/// B1/B2: `n` timed keystrokes into a document of `doc_chars`.
pub fn type_bench(editor: &Editor, doc_chars: usize, n: usize, gap: Duration) -> TypeReport {
    editor.set_text(&corpus_text(doc_chars));
    editor.caret_to_end();

    let characters: Vec<char> = CORPUS.chars().collect();
    let mut timings: Vec<CharTiming> = Vec::with_capacity(n);
    let mut compose: Vec<u64> = Vec::with_capacity(n.min(20));

    // One probe decides whether the series is worth taking. If the compositor is
    // dormant, another 20 samples would only produce 20 copies of the same idle
    // timeout (and ~5 s of run time).
    let mut compose_valid = if n > 0 {
        let probe = hub_editor::measure::compose_us();
        let plausible = compose_is_plausible(probe);
        if plausible {
            compose.push(probe);
        }
        plausible
    } else {
        false
    };

    for index in 0..n {
        let ch = characters[index % characters.len()];
        timings.push(editor.append_char(ch));

        // Composition is measured on a subset: DwmFlush blocks up to a full frame,
        // and 200 of those would add 3 s to the run for a number that is reported
        // rather than gated.
        if compose_valid && index % 10 == 0 && compose.len() < 20 {
            let sample = hub_editor::measure::compose_us();
            if compose_is_plausible(sample) {
                compose.push(sample);
            } else {
                // Went dormant mid-run: drop the partial series rather than average
                // a real frame with an idle timeout.
                compose.clear();
                compose_valid = false;
            }
        }

        std::thread::sleep(gap);
    }

    let type_us: Vec<u64> = timings.iter().map(|timing| timing.gate_us()).collect();
    let insert_us: Vec<u64> = timings.iter().map(|timing| timing.insert_us).collect();
    let repaint_us: Vec<u64> = timings.iter().map(|timing| timing.repaint_us).collect();

    TypeReport {
        n: timings.len(),
        type_us: distribution(&type_us),
        insert_us: distribution(&insert_us),
        repaint_us: distribution(&repaint_us),
        compose_us: distribution(&compose),
        compose_n: compose.len(),
        compose_valid,
    }
}

pub fn report(evidence: &Evidence, one_k: &TypeReport, ten_k: &TypeReport) -> String {
    let px = &evidence.pixels;
    format!(
        "native editor (RichEdit island)
  class={} limit={} face={} height={}twips color=0x{:06X}
  load_ms 1k={:.1} 10k={:.1} paste10k={:.1} ({} chars) burst200={:.1}
  pixels samples={} dominant=0x{:06X} share={:.3} distinct={} ink={:.3}
         ink_empty={:.3} ink_typed={:.3} fg_empty={} fg_typed={}
  scroll lines={} first_visible small={} large={} offset_y={}
  resize same_moved={} changed_moved={} restored_moved={}
  undo coalesced={} grouped={} | roundtrip steps={} ok={} redo={} ok={}
  keys esc_ok={} tab_ok={} fallthrough_ok={} imm_ok={}

  type ms (gate)    n={} min={:.2} p50={:.2} p95={:.2} p99={:.2} max={:.2}
    insert ms           min={:.2} p50={:.2} p95={:.2}
    repaint ms (forced) min={:.2} p50={:.2} p95={:.2}
  compose ms (report) {}

  1k  doc: type p95={:.2} ms (n={})
  10k doc: type p95={:.2} ms (n={}) ratio={:.2}x
",
        evidence.class,
        evidence.limit,
        evidence.face,
        evidence.height_twips,
        evidence.color,
        evidence.load_1k_ms,
        evidence.load_10k_ms,
        evidence.paste_ms,
        evidence.paste_chars,
        evidence.burst_ms,
        px.samples,
        px.dominant,
        px.dominant_share,
        px.distinct,
        px.ink_share(),
        evidence.ink_empty,
        evidence.ink_typed,
        evidence.fg_empty,
        evidence.fg_typed,
        evidence.lines,
        evidence.scroll_small,
        evidence.scroll_large,
        evidence.offset_large,
        evidence.resize_same_moved,
        evidence.resize_changed_moved,
        evidence.resize_restored_moved,
        evidence.undo_coalesced,
        evidence.undo_grouped,
        evidence.undo_steps,
        evidence.undo_ok,
        evidence.redo_steps,
        evidence.redo_ok,
        evidence.escape_ok,
        evidence.tab_ok,
        evidence.fallthrough_ok,
        evidence.imm_ok,
        one_k.n,
        one_k.type_us.min as f64 / 1000.0,
        one_k.type_us.p50 as f64 / 1000.0,
        one_k.type_us.p95 as f64 / 1000.0,
        one_k.type_us.p99 as f64 / 1000.0,
        one_k.type_us.max as f64 / 1000.0,
        one_k.insert_us.min as f64 / 1000.0,
        one_k.insert_us.p50 as f64 / 1000.0,
        one_k.insert_us.p95 as f64 / 1000.0,
        one_k.repaint_us.min as f64 / 1000.0,
        one_k.repaint_us.p50 as f64 / 1000.0,
        one_k.repaint_us.p95 as f64 / 1000.0,
        compose_line(one_k),
        one_k.p95_ms(),
        one_k.n,
        ten_k.p95_ms(),
        ten_k.n,
        if one_k.p95_ms() > 0.0 {
            ten_k.p95_ms() / one_k.p95_ms()
        } else {
            0.0
        }
    )
}

pub fn report_json(
    evidence: &Evidence,
    one_k: &TypeReport,
    ten_k: &TypeReport,
    budget_p95_ms: f64,
    budget_p99_ms: f64,
) -> String {
    format!(
        r#"{{"kind":"native-s2-editor","at":"{}","class":"{}","limit":{},"face":"{}","height_twips":{},"color":"0x{:06X}","pixels":{{"samples":{},"dominant":"0x{:06X}","share":{:.4},"distinct":{},"ink_empty":{:.4},"ink_typed":{:.4},"fg_empty":{},"fg_typed":{}}},"load_ms":{{"doc_1k":{:.2},"doc_10k":{:.2},"paste_10k":{:.2}}},"paste_chars":{},"scroll":{{"lines":{},"small":{},"large":{},"offset_y":{}}},"resize":{{"same_moved":{},"changed_moved":{},"restored_moved":{}}},"undo":{{"coalesced":{},"grouped":{},"steps":{},"ok":{}}},"redo":{{"steps":{},"ok":{}}},"keys":{{"esc":{},"tab":{},"fallthrough":{},"imm":{}}},"burst_ms":{:.2},"doc_1k":{},"doc_10k":{},"budget":{{"p95":{},"p99":{}}},"pass":{}}}"#,
        timestamp(),
        evidence.class,
        evidence.limit,
        evidence.face,
        evidence.height_twips,
        evidence.color,
        evidence.pixels.samples,
        evidence.pixels.dominant,
        evidence.pixels.dominant_share,
        evidence.pixels.distinct,
        evidence.ink_empty,
        evidence.ink_typed,
        evidence.fg_empty,
        evidence.fg_typed,
        evidence.load_1k_ms,
        evidence.load_10k_ms,
        evidence.paste_ms,
        evidence.paste_chars,
        evidence.lines,
        evidence.scroll_small,
        evidence.scroll_large,
        evidence.offset_large,
        evidence.resize_same_moved,
        evidence.resize_changed_moved,
        evidence.resize_restored_moved,
        evidence.undo_coalesced,
        evidence.undo_grouped,
        evidence.undo_steps,
        evidence.undo_ok,
        evidence.redo_steps,
        evidence.redo_ok,
        evidence.escape_ok,
        evidence.tab_ok,
        evidence.fallthrough_ok,
        evidence.imm_ok,
        evidence.burst_ms,
        report_block(one_k),
        report_block(ten_k),
        budget_p95_ms,
        budget_p99_ms,
        one_k.within(budget_p95_ms, budget_p99_ms)
    )
}

fn report_block(report: &TypeReport) -> String {
    format!(
        r#"{{"n":{},"insert_us":{{"p50":{},"p95":{},"p99":{}}},"repaint_us":{{"p50":{},"p95":{},"p99":{}}},"type_us":{{"min":{},"p50":{},"p95":{},"p99":{},"max":{}}},"compose_us":{{"valid":{},"n":{},"p50":{},"p95":{}}}}}"#,
        report.n,
        report.insert_us.p50,
        report.insert_us.p95,
        report.insert_us.p99,
        report.repaint_us.p50,
        report.repaint_us.p95,
        report.repaint_us.p99,
        report.type_us.min,
        report.type_us.p50,
        report.type_us.p95,
        report.type_us.p99,
        report.type_us.max,
        report.compose_valid,
        report.compose_n,
        report.compose_us.p50,
        report.compose_us.p95
    )
}

/// Human form of the reported-only compose number, including the reason it is
/// missing when the compositor was dormant.
fn compose_line(report: &TypeReport) -> String {
    if report.compose_valid {
        format!(
            "n={} min={:.2} p50={:.2} p95={:.2} max={:.2} (not gated)",
            report.compose_n,
            report.compose_us.min as f64 / 1000.0,
            report.compose_us.p50 as f64 / 1000.0,
            report.compose_us.p95 as f64 / 1000.0,
            report.compose_us.max as f64 / 1000.0
        )
    } else {
        format!(
            "n/a - compositor dormant (DwmFlush exceeded {} ms with nothing on screen)",
            COMPOSE_PLAUSIBLE_US / 1000
        )
    }
}

fn timestamp() -> String {
    // No chrono: the spike's dependency budget is part of the design. The archive
    // script adds a filename timestamp; this one is for the JSON body.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    format!("unix:{now}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_cycles_and_has_the_requested_length() {
        assert_eq!(corpus_text(5).chars().count(), 5);
        assert_eq!(corpus_text(0), "");
        let long = corpus_text(10_000);
        assert_eq!(long.chars().count(), 10_000);
        // Cycled, not truncated: the 10k document must contain repeats.
        assert!(long.chars().count() > CORPUS.chars().count());
    }

    #[test]
    fn budget_check_uses_p95_and_p99() {
        let report = TypeReport {
            n: 1,
            type_us: Distribution {
                n: 1,
                min: 1_000,
                p50: 1_000,
                p95: 15_000,
                p99: 24_000,
                max: 24_000,
            },
            insert_us: Distribution::default(),
            repaint_us: Distribution::default(),
            compose_us: Distribution::default(),
            compose_n: 0,
            compose_valid: false,
        };
        assert!(report.within(16.0, 25.0));
        assert!(!report.within(16.0, 23.0));
        assert!(!report.within(14.0, 30.0));
    }

    #[test]
    fn only_a_frame_sized_flush_counts_as_composition() {
        // 16.7 ms is one frame at 60 Hz; two frames still means "presenting".
        assert!(compose_is_plausible(0));
        assert!(compose_is_plausible(16_700));
        assert!(compose_is_plausible(COMPOSE_PLAUSIBLE_US));
        // The 254 ms observed on an idle desktop is compositor sleep, not latency.
        assert!(!compose_is_plausible(254_835));
        assert!(!compose_is_plausible(u64::MAX));
    }

    #[test]
    fn dormant_compose_is_worded_as_missing_not_as_a_measurement() {
        let report = TypeReport {
            n: 0,
            type_us: Distribution::default(),
            insert_us: Distribution::default(),
            repaint_us: Distribution::default(),
            compose_us: Distribution::default(),
            compose_n: 0,
            compose_valid: false,
        };
        let line = compose_line(&report);
        assert!(line.contains("n/a"), "{line}");
        assert!(line.contains("dormant"), "{line}");
        // It must not render as "0.00 ms", which would read like a fast frame.
        assert!(!line.contains("p50=0.00"), "{line}");
    }
}
