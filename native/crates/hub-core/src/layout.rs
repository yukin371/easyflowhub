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
}
