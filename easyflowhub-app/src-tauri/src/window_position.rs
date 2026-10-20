use tauri::{Manager, PhysicalPosition};

/// Anchor a logical-sized window at the cursor, on the cursor's monitor.
///
/// Uses the Manager window (or any available window) to resolve the monitor
/// under the cursor, then delegates the DPI conversion and work-area clamping
/// to `centered_position`. Returns `None` when no anchor window or monitor
/// can be resolved. Used by both the Launcher and quick-note windows.
pub(crate) fn cursor_anchored_position(
    app: &tauri::AppHandle,
    width: f64,
    height: f64,
) -> Option<PhysicalPosition<i32>> {
    let anchor_window = app
        .get_webview_window("manager")
        .or_else(|| app.webview_windows().into_values().next())?;

    let cursor = anchor_window.cursor_position().ok()?;
    let monitor = anchor_window
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| anchor_window.primary_monitor().ok().flatten())?;

    let work_area = monitor.work_area();
    let (x, y) = centered_position(
        (cursor.x, cursor.y),
        (width, height),
        monitor.scale_factor(),
        (
            work_area.position.x,
            work_area.position.y,
            work_area.size.width,
            work_area.size.height,
        ),
    );
    Some(PhysicalPosition::new(x, y))
}

/// Center a logical-sized window on a physical desktop point, then keep it
/// inside the target monitor's physical work area (which may have a negative origin).
pub(crate) fn centered_position(
    cursor: (f64, f64),
    logical_size: (f64, f64),
    scale_factor: f64,
    work_area: (i32, i32, u32, u32),
) -> (i32, i32) {
    let (cursor_x, cursor_y) = cursor;
    let (logical_width, logical_height) = logical_size;
    let (work_x, work_y, work_width, work_height) = work_area;
    let width = (logical_width * scale_factor).round();
    let height = (logical_height * scale_factor).round();
    let min_x = f64::from(work_x);
    let min_y = f64::from(work_y);
    let max_x = (min_x + f64::from(work_width) - width).max(min_x);
    let max_y = (min_y + f64::from(work_height) - height).max(min_y);

    (
        (cursor_x - width / 2.0).round().clamp(min_x, max_x) as i32,
        (cursor_y - height / 2.0).round().clamp(min_y, max_y) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::centered_position;

    #[test]
    fn launcher_center_follows_pointer_at_normal_and_scaled_dpi() {
        let work_area = (0, 0, 1920, 1040);

        assert_eq!(centered_position((960.0, 540.0), (720.0, 520.0), 1.0, work_area), (600, 280));
        assert_eq!(centered_position((960.0, 540.0), (720.0, 520.0), 1.5, work_area), (420, 150));
    }

    #[test]
    fn quick_note_uses_the_target_monitor_scale() {
        assert_eq!(
            centered_position((2000.0, 1000.0), (400.0, 300.0), 2.0, (0, 0, 3840, 2120)),
            (1600, 700),
        );
    }

    #[test]
    fn keeps_windows_inside_work_area_near_monitor_edges_and_taskbars() {
        let work_area = (48, 24, 1872, 1016);

        assert_eq!(centered_position((50.0, 26.0), (720.0, 520.0), 1.0, work_area), (48, 24));
        assert_eq!(centered_position((1918.0, 1038.0), (720.0, 520.0), 1.0, work_area), (1200, 520));
    }

    #[test]
    fn supports_monitors_to_the_left_and_above_the_primary() {
        assert_eq!(
            centered_position((-960.0, 500.0), (720.0, 520.0), 1.0, (-1920, 0, 1920, 1040)),
            (-1320, 240),
        );
        assert_eq!(
            centered_position((960.0, -540.0), (720.0, 520.0), 1.0, (0, -1080, 1920, 1080)),
            (600, -800),
        );
    }

    #[test]
    fn oversized_windows_keep_their_header_at_the_work_area_origin() {
        assert_eq!(
            centered_position((-400.0, 200.0), (720.0, 520.0), 1.5, (-800, 40, 800, 600)),
            (-800, 40),
        );
    }
}
