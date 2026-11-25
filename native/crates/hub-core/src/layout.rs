//! Panel placement: where a cursor-anchored panel goes, and how it is clamped.
//!
//! All values are **physical pixels**. The caller converts from logical sizes
//! using the target monitor's scale before calling `place`, because the panel
//! has to be sized in physical pixels to allocate its buffer (§5.1.1) — mixing
//! the two units is what produces "appears at half size for one frame".

/// Axis-aligned rectangle in physical pixels, `right`/`bottom` exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// Static shape of the panel, expressed in logical pixels plus the constants
/// that decide the column count.
#[derive(Debug, Clone, Copy)]
pub struct PanelSpec {
    pub desired_width: f32,
    pub desired_height: f32,
    /// Gap kept between the panel and the work-area edge.
    pub margin: i32,
    pub padding: i32,
    pub gap: i32,
    /// Fixed threshold from §5.1.1: below this cell width the grid drops to 3.
    pub min_cell_width: f32,
}

impl Default for PanelSpec {
    fn default() -> Self {
        Self {
            desired_width: 440.0,
            desired_height: 860.0,
            margin: 12,
            padding: 16,
            gap: 8,
            min_cell_width: 96.0,
        }
    }
}

/// Resolved placement for one reveal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub columns: u8,
}

/// Number of grid columns that fits `width` physical pixels.
///
/// Kept as a table lookup over `{4, 3}` on purpose: continuous reflow would
/// invalidate the per-cell texture cache and hit-test grid on every clamp, which
/// is exactly the kind of jitter §5.1.1 forbids in the reveal path.
pub fn columns_for(spec: &PanelSpec, width: i32) -> u8 {
    for columns in [4u8, 3u8] {
        let gaps = (columns as i32 - 1) * spec.gap;
        let cell = (width - 2 * spec.padding - gaps) as f32 / columns as f32;
        if cell >= spec.min_cell_width {
            return columns;
        }
    }
    3
}

/// Clamp `value` into `[low, high]`, tolerating `high < low` (tiny work area).
fn clamp_axis(value: i32, low: i32, high: i32) -> i32 {
    if high < low {
        low
    } else {
        value.clamp(low, high)
    }
}

/// Place the panel at `cursor`, fully inside `work`.
///
/// * horizontal: centred on the cursor, then clamped into the work area
/// * vertical: one third above the cursor (the cursor stays over the panel's
///   upper third, which keeps the grid reachable without moving the mouse)
/// * size: never larger than the work area minus the margin on both sides, so a
///   1366x768 screen gets a 704px tall panel instead of one that disappears
///   behind the taskbar
pub fn place(cursor: (i32, i32), work: Rect, spec: &PanelSpec) -> Placement {
    let margin = spec.margin;
    let max_width = work.width() - 2 * margin;
    let max_height = work.height() - 2 * margin;

    let width = (spec.desired_width.round() as i32).min(max_width).max(1);
    let height = (spec.desired_height.round() as i32).min(max_height).max(1);

    let x = clamp_axis(
        cursor.0 - width / 2,
        work.left + margin,
        work.right - margin - width,
    );
    let y = clamp_axis(
        cursor.1 - height / 3,
        work.top + margin,
        work.bottom - margin - height,
    );

    Placement {
        x,
        y,
        width,
        height,
        columns: columns_for(spec, width),
    }
}

// ---------------------------------------------------------------- note windows

/// Minimum logical size of a note window (parity Q3).
///
/// Small enough to be a scratch pad, large enough that the 32 px strip still fits
/// three slots: at 150 logical pixels the slots need 38 + flexible + 38, and 74
/// of them are fixed chrome.
pub const MIN_NOTE_WIDTH: f32 = 150.0;
pub const MIN_NOTE_HEIGHT: f32 = 60.0;

/// Height of the title strip, logical pixels (parity Q6-Q11).
///
/// Lives here rather than in `hub-ui` because the text island's top edge is
/// derived from it: the RichEdit child window is placed at `strip` and the two
/// values must not be able to drift apart.
pub const STRIP_HEIGHT: f32 = 32.0;

/// Gap between the cursor and the note's nearest corner, physical pixels.
const NOTE_OFFSET: i32 = 8;

/// Static shape of a note window, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteSpec {
    pub width: f32,
    pub height: f32,
    /// Gap kept between the window and the work-area edge.
    pub margin: i32,
}

impl Default for NoteSpec {
    fn default() -> Self {
        Self {
            width: 400.0,
            height: 300.0,
            margin: 12,
        }
    }
}

impl NoteSpec {
    /// Physical size for `scale`, floor-raised to the minimum.
    ///
    /// The floor is applied *after* the multiplication on purpose: clamping the
    /// logical value first would let a 0.9 scale produce 135x54 physical, which
    /// the OS refuses to shrink to and the layout would claim is legal.
    pub fn physical(self, scale: f32) -> (i32, i32) {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        let width = (self.width * scale).round() as i32;
        let height = (self.height * scale).round() as i32;
        let min_width = (MIN_NOTE_WIDTH * scale).round() as i32;
        let min_height = (MIN_NOTE_HEIGHT * scale).round() as i32;
        (width.max(min_width).max(1), height.max(min_height).max(1))
    }
}

/// Resolved note geometry. Separate from `Placement` because a note has no grid
/// and reusing that struct would mean carrying a meaningless `columns` field
/// through every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotePlacement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl NotePlacement {
    pub fn right(self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(self) -> i32 {
        self.y + self.height
    }
}

/// Place a note near the cursor, fully inside `work`.
///
/// Anchored **below-right** of the cursor instead of centred: a 400x300 note
/// centred on the pointer covers the text the user was reading, which is the
/// opposite of what a scratch pad is for. When the anchored rectangle would leave
/// the work area the window flips to the other side of the cursor; the flip is
/// decided before clamping so an edge case resolves to "flip" rather than "slide
/// under the cursor".
pub fn place_note(cursor: (i32, i32), work: Rect, spec: NoteSpec, scale: f32) -> NotePlacement {
    let margin = spec.margin.max(0);
    let (width, height) = spec.physical(scale);

    let max_width = (work.width() - 2 * margin).max(1);
    let max_height = (work.height() - 2 * margin).max(1);
    let width = width.min(max_width);
    let height = height.min(max_height);

    let right_edge = work.right - margin - width;
    let bottom_edge = work.bottom - margin - height;

    let x = if cursor.0 + NOTE_OFFSET + width > work.right - margin {
        cursor.0 - NOTE_OFFSET - width
    } else {
        cursor.0 + NOTE_OFFSET
    };
    let y = if cursor.1 + NOTE_OFFSET + height > work.bottom - margin {
        cursor.1 - NOTE_OFFSET - height
    } else {
        cursor.1 + NOTE_OFFSET
    };

    NotePlacement {
        x: clamp_axis(x, work.left + margin, right_edge),
        y: clamp_axis(y, work.top + margin, bottom_edge),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big_work() -> Rect {
        // 3840x2160 @150% with a 72px taskbar.
        Rect::new(0, 0, 3840, 2088)
    }

    #[test]
    fn centres_on_the_cursor_inside_a_large_work_area() {
        let placement = place((1920, 1044), big_work(), &PanelSpec::default());
        assert_eq!((placement.width, placement.height), (440, 860));
        assert_eq!(placement.x, 1920 - 220);
        assert_eq!(placement.y, 1044 - 860 / 3);
        assert_eq!(placement.columns, 4);
    }

    #[test]
    fn clamps_to_the_top_left_corner() {
        let placement = place((0, 0), big_work(), &PanelSpec::default());
        assert_eq!((placement.x, placement.y), (12, 12));
    }

    #[test]
    fn clamps_to_the_bottom_right_corner() {
        let placement = place((3840, 2088), big_work(), &PanelSpec::default());
        assert_eq!(placement.x + placement.width, 3840 - 12);
        assert_eq!(placement.y + placement.height, 2088 - 12);
    }

    #[test]
    fn shrinks_height_on_a_small_work_area() {
        // 1366x768 with a 40px taskbar.
        let work = Rect::new(0, 0, 1366, 728);
        let placement = place((683, 364), work, &PanelSpec::default());
        assert_eq!(placement.height, 728 - 24);
        assert_eq!(placement.y, 12);
        assert_eq!(placement.width, 440);
    }

    #[test]
    fn uses_the_secondary_monitor_work_area() {
        // Secondary screen to the right of the primary one, 1920x1080 @100%.
        let work = Rect::new(3840, 0, 5760, 1040);
        let placement = place((4500, 500), work, &PanelSpec::default());
        assert!(placement.x >= work.left + 12);
        assert!(placement.x + placement.width <= work.right - 12);
        assert!(placement.y >= work.top + 12);
        assert!(placement.y + placement.height <= work.bottom - 12);
    }

    #[test]
    fn stays_inside_when_the_cursor_is_outside_the_work_area() {
        let placement = place((-500, -500), big_work(), &PanelSpec::default());
        assert_eq!((placement.x, placement.y), (12, 12));
    }

    #[test]
    fn drops_to_three_columns_when_cells_get_too_narrow() {
        let spec = PanelSpec::default();
        assert_eq!(columns_for(&spec, 440), 4);
        // 4 columns need 16*2 + 8*3 + 96*4 = 440 exactly, so 439 must drop.
        assert_eq!(columns_for(&spec, 439), 3);
        assert_eq!(columns_for(&spec, 300), 3);
    }

    #[test]
    fn tiny_work_area_never_produces_negative_size() {
        let work = Rect::new(0, 0, 20, 20);
        let placement = place((10, 10), work, &PanelSpec::default());
        assert_eq!((placement.width, placement.height), (1, 1));
        assert_eq!((placement.x, placement.y), (12, 12));
    }

    // ------------------------------------------------------------ note windows

    #[test]
    fn note_default_is_400x300_logical_at_150_percent() {
        let (width, height) = NoteSpec::default().physical(1.5);
        assert_eq!((width, height), (600, 450));
        let (width, height) = NoteSpec::default().physical(1.0);
        assert_eq!((width, height), (400, 300));
    }

    #[test]
    fn note_minimum_survives_a_scale_below_one() {
        // 100x40 logical at 0.9 would be 90x36 physical; the floor is applied
        // after scaling, so the result is 135x54, not 90x36.
        let spec = NoteSpec { width: 100.0, height: 40.0, margin: 12 };
        assert_eq!(spec.physical(0.9), (135, 54));
        // A non-finite scale must not propagate NaN into the layout.
        assert_eq!(NoteSpec::default().physical(f32::NAN), (400, 300));
    }

    #[test]
    fn note_opens_below_right_of_the_cursor() {
        let placement = place_note((500, 400), big_work(), NoteSpec::default(), 1.0);
        assert_eq!((placement.x, placement.y), (508, 408));
        assert_eq!((placement.width, placement.height), (400, 300));
    }

    #[test]
    fn note_flips_to_the_left_near_the_right_edge() {
        // 3800 + 8 + 400 > 3840 - 12, so the window must go to the left side.
        let placement = place_note((3800, 400), big_work(), NoteSpec::default(), 1.0);
        assert_eq!(placement.x, 3800 - 8 - 400);
        assert!(placement.right() <= 3840 - 12);
    }

    #[test]
    fn note_flips_up_near_the_bottom_edge() {
        let placement = place_note((400, 2050), big_work(), NoteSpec::default(), 1.0);
        assert_eq!(placement.y, 2050 - 8 - 300);
        assert!(placement.bottom() <= 2088 - 12);
    }

    #[test]
    fn note_never_leaves_the_work_area() {
        for cursor in [(-500, -500), (0, 0), (3840, 2088), (3839, 2087)] {
            let placement = place_note(cursor, big_work(), NoteSpec::default(), 1.5);
            assert!(placement.x >= 12, "cursor {cursor:?} -> {placement:?}");
            assert!(placement.y >= 12, "cursor {cursor:?} -> {placement:?}");
            assert!(placement.right() <= 3840 - 12, "cursor {cursor:?} -> {placement:?}");
            assert!(placement.bottom() <= 2088 - 12, "cursor {cursor:?} -> {placement:?}");
        }
    }

    #[test]
    fn note_shrinks_to_a_small_work_area() {
        let work = Rect::new(0, 0, 200, 100);
        let placement = place_note((50, 50), work, NoteSpec::default(), 1.0);
        assert_eq!(placement.width, 200 - 24);
        assert_eq!(placement.height, 100 - 24);
    }

    #[test]
    fn strip_height_leaves_a_usable_body_at_the_minimum_size() {
        // The strip must not eat the whole minimum window: 60 - 32 leaves 28 px,
        // which is one line of 11 pt text plus padding. Used as a tripwire if
        // either constant is raised without revisiting the other.
        assert!(MIN_NOTE_HEIGHT - STRIP_HEIGHT >= 24.0);
        assert!(STRIP_HEIGHT == 32.0);
        assert!(MIN_NOTE_WIDTH == 150.0);
    }
}
