//! Logical points → physical pixels for the text island.
//!
//! The shell lays out in egui's logical points; the text island is a native child
//! window positioned in physical pixels. Doing that conversion inside the host
//! code would put rounding in the hot path of every frame, so it lives here with
//! tests instead: a half-pixel error on a 150% screen shows up as a one-pixel gap
//! between the editor and its border, which reads as "misaligned" rather than
//! "wrong maths".

/// A rectangle in physical pixels, relative to the parent's client area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PixelRect {
    /// Convert a logical rect (`[x, y, width, height]`, points) to pixels.
    ///
    /// All four values are scaled. Only the size used to be: the origin was
    /// copied through, which on a 150% screen put the island at the strip's
    /// *logical* height (32 px) instead of its physical one (48 px), so the text
    /// overlapped the title bar's bottom edge by 16 px and the island's own height
    /// ran 16 px past the client area. A rect whose origin and size use different
    /// units is not a rect.
    ///
    /// Rounds rather than truncates: truncation accumulates as a one-pixel drift
    /// toward the top-left as `ppp` grows. Never returns a zero or negative size —
    /// a child window of 0 width makes the OS deliver no paint messages at all,
    /// which would look exactly like "the editor stopped rendering".
    pub fn from_logical(rect: [f32; 4], pixels_per_point: f32) -> Self {
        let scale = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
            pixels_per_point
        } else {
            1.0
        };

        Self {
            x: (rect[0] * scale).round() as i32,
            y: (rect[1] * scale).round() as i32,
            width: (rect[2] * scale).round().max(1.0) as i32,
            height: (rect[3] * scale).round().max(1.0) as i32,
        }
    }

    /// True when moving from `self` to `other` would not change the window.
    ///
    /// The host calls `SetWindowPos` only on change: doing it every frame forces a
    /// repaint every frame, which spends the whole typing budget on nothing.
    pub fn same_as(&self, other: &PixelRect) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_by_the_monitor_ratio() {
        let rect = PixelRect::from_logical([16.0, 60.0, 408.0, 700.0], 1.5);
        assert_eq!(
            rect,
            PixelRect {
                x: 24,
                y: 90,
                width: 612,
                height: 1050
            }
        );
    }

    #[test]
    fn rounds_instead_of_truncating() {
        // 15 * 1.5 = 22.5 must become 23: truncation would leave a 1px gap on
        // every odd coordinate at 150%.
        let rect = PixelRect::from_logical([15.0, 0.0, 15.0, 10.0], 1.5);
        assert_eq!(rect.x, 23);
        assert_eq!(rect.width, 23);
    }

    #[test]
    fn the_origin_uses_the_same_unit_as_the_size() {
        // The title strip is 32 logical px. At 150% the island must start at 48,
        // not 32: the 16 px difference is text drawn over the strip's hairline.
        let rect = PixelRect::from_logical([0.0, crate::layout::STRIP_HEIGHT, 400.0, 268.0], 1.5);
        assert_eq!(rect.y, 48);
        assert_eq!(rect.height, 402);
    }

    #[test]
    fn never_produces_a_zero_sized_window() {
        let rect = PixelRect::from_logical([0.0, 0.0, 0.2, 0.2], 1.0);
        assert_eq!(rect.width, 1);
        assert_eq!(rect.height, 1);
    }

    #[test]
    fn falls_back_to_one_on_a_broken_scale() {
        for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let rect = PixelRect::from_logical([0.0, 0.0, 10.0, 10.0], ppp);
            assert_eq!((rect.width, rect.height), (10, 10));
        }
    }

    #[test]
    fn equality_is_all_four_edges() {
        let a = PixelRect::from_logical([1.0, 2.0, 3.0, 4.0], 1.0);
        let b = PixelRect::from_logical([1.0, 2.0, 3.0, 5.0], 1.0);
        assert!(a.same_as(&a));
        assert!(!a.same_as(&b));
    }
}
