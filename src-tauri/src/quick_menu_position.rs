//! Screen-space positioning for the native quick menu popup.
//!
//! macOS context menus anchor their top edge at the popup point and grow downward
//! (decreasing Y in Cocoa's bottom-left coordinate system). Near the bottom of the
//! display we flip so the menu bottom aligns with the visible screen bottom.

/// Approximate row height for a standard `NSMenu` item (points).
pub const MENU_ROW_HEIGHT: f64 = 22.0;

/// Cocoa screen coordinates: menu top Y when opening at the cursor (default).
pub fn popup_top_y_at_cursor(cursor_y: f64) -> f64 {
    cursor_y
}

/// Cocoa screen coordinates: menu top Y when flipped upward against the visible bottom.
pub fn popup_top_y_flipped(visible_bottom: f64, menu_height: f64, visible_top: f64) -> f64 {
    (visible_bottom + menu_height).min(visible_top)
}

/// Whether the menu should flip upward instead of using the default cursor anchor.
pub fn should_flip_menu_up(cursor_y: f64, menu_height: f64, visible_bottom: f64) -> bool {
    cursor_y - menu_height < visible_bottom
}

/// Pick the menu top Y in screen coordinates.
pub fn popup_top_y(cursor_y: f64, menu_height: f64, visible_bottom: f64, visible_top: f64) -> f64 {
    if should_flip_menu_up(cursor_y, menu_height, visible_bottom) {
        popup_top_y_flipped(visible_bottom, menu_height, visible_top)
    } else {
        popup_top_y_at_cursor(cursor_y)
    }
}

pub fn estimated_menu_height(row_count: usize) -> f64 {
    row_count as f64 * MENU_ROW_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_cursor_anchor_when_room_below() {
        assert!(!should_flip_menu_up(400.0, 200.0, 0.0));
        assert_eq!(popup_top_y(400.0, 200.0, 0.0, 1000.0), 400.0);
    }

    #[test]
    fn flips_when_menu_would_cross_visible_bottom() {
        assert!(should_flip_menu_up(50.0, 200.0, 0.0));
        assert_eq!(popup_top_y(50.0, 200.0, 0.0, 1000.0), 200.0);
    }

    #[test]
    fn clamps_flipped_top_to_visible_top_when_menu_is_very_tall() {
        assert_eq!(popup_top_y(10.0, 900.0, 0.0, 800.0), 800.0);
    }
}
