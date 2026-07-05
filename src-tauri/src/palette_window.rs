//! Command palette window sizing and pure geometry helpers.

pub const PALETTE_DOT_SIZE: f64 = 72.0;
pub const PALETTE_MIN_WIDTH: f64 = 380.0;
pub const PALETTE_MIN_HEIGHT: f64 = 160.0;
pub const PALETTE_DOT_SIZE_TOLERANCE: f64 = 0.5;

/// True when logical inner size matches the min-dot footprint.
pub fn is_dot_logical_size(logical_width: f64, logical_height: f64) -> bool {
    logical_width <= PALETTE_DOT_SIZE + PALETTE_DOT_SIZE_TOLERANCE
        && logical_height <= PALETTE_DOT_SIZE + PALETTE_DOT_SIZE_TOLERANCE
}

/// Window center in screen coordinates (physical px, top-left origin).
pub fn window_center(origin_x: f64, origin_y: f64, width: f64, height: f64) -> (f64, f64) {
    (origin_x + width / 2.0, origin_y + height / 2.0)
}

/// Top-left position to center a window of `win_width`×`win_height` in a work area.
pub fn center_in_work_area(
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    win_width: i32,
    win_height: i32,
) -> (i32, i32) {
    let x = work_x + (work_width as i32 - win_width) / 2;
    let y = work_y + (work_height as i32 - win_height) / 2;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_size_predicate_matches_tolerance() {
        assert!(is_dot_logical_size(72.0, 72.0));
        assert!(is_dot_logical_size(72.4, 72.3));
        assert!(!is_dot_logical_size(72.6, 72.0));
        assert!(!is_dot_logical_size(100.0, 460.0));
    }

    #[test]
    fn center_in_work_area_offsets_window() {
        assert_eq!(center_in_work_area(0, 0, 1000, 800, 400, 300), (300, 250));
    }
}
