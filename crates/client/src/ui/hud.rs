//! HUD rendering for crosshair and hotbar display.
//!
//! This module provides HUD rendering logic that can work in both
//! headless and windowed modes. The actual GPU rendering is delegated
//! to the render crate when in windowed mode.

use super::hotbar::HOTBAR_SIZE;

/// HUD state managing crosshair and hotbar display.
#[derive(Debug, Clone)]
pub struct Hud {
    /// Whether the crosshair is visible.
    pub crosshair_visible: bool,
    /// Whether the hotbar is visible.
    pub hotbar_visible: bool,
    /// Screen dimensions (width, height) for layout calculations.
    pub screen_size: (u32, u32),
}

impl Hud {
    /// Create a new HUD with default settings.
    pub fn new(screen_size: (u32, u32)) -> Self {
        Self {
            crosshair_visible: true,
            hotbar_visible: true,
            screen_size,
        }
    }

    /// Update screen size for layout recalculation.
    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
    }

    /// Get the crosshair position (center of screen).
    pub fn crosshair_position(&self) -> (f32, f32) {
        let x = self.screen_size.0 as f32 / 2.0;
        let y = self.screen_size.1 as f32 / 2.0;
        (x, y)
    }

    /// Get the hotbar layout (position and slot dimensions).
    ///
    /// Returns (x, y, slot_width, slot_height) where:
    /// - (x, y) is the top-left corner of the hotbar
    /// - slot_width/height are dimensions of each slot
    pub fn hotbar_layout(&self) -> HotbarLayout {
        const SLOT_SIZE: u32 = 40;
        const SLOT_SPACING: u32 = 4;
        const PADDING: u32 = 10;

        let total_width = SLOT_SIZE * HOTBAR_SIZE as u32 + SLOT_SPACING * (HOTBAR_SIZE - 1) as u32;
        let x = (self.screen_size.0 as i32 - total_width as i32) / 2;
        let y = self.screen_size.1 as i32 - SLOT_SIZE as i32 - PADDING as i32;

        HotbarLayout {
            x,
            y,
            slot_size: SLOT_SIZE,
            slot_spacing: SLOT_SPACING,
        }
    }

    /// Get the position of a specific hotbar slot.
    pub fn hotbar_slot_position(&self, slot_index: usize) -> (i32, i32) {
        assert!(slot_index < HOTBAR_SIZE, "Invalid hotbar slot index");
        let layout = self.hotbar_layout();
        let x = layout.x + (slot_index as i32 * (layout.slot_size + layout.slot_spacing) as i32);
        let y = layout.y;
        (x, y)
    }

    /// Show or hide the crosshair.
    pub fn set_crosshair_visible(&mut self, visible: bool) {
        self.crosshair_visible = visible;
    }

    /// Show or hide the hotbar.
    pub fn set_hotbar_visible(&mut self, visible: bool) {
        self.hotbar_visible = visible;
    }
}

/// Layout information for the hotbar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotbarLayout {
    /// X position of the hotbar (top-left corner).
    pub x: i32,
    /// Y position of the hotbar (top-left corner).
    pub y: i32,
    /// Size of each slot (square).
    pub slot_size: u32,
    /// Spacing between slots.
    pub slot_spacing: u32,
}

impl Default for Hud {
    fn default() -> Self {
        Self::new((800, 600))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_creation() {
        let hud = Hud::new((1920, 1080));
        assert!(hud.crosshair_visible);
        assert!(hud.hotbar_visible);
        assert_eq!(hud.screen_size, (1920, 1080));
    }

    #[test]
    fn crosshair_position_is_center() {
        let hud = Hud::new((800, 600));
        let (x, y) = hud.crosshair_position();
        assert_eq!(x, 400.0);
        assert_eq!(y, 300.0);
    }

    #[test]
    fn crosshair_position_updates_with_screen_size() {
        let mut hud = Hud::new((800, 600));
        hud.set_screen_size(1920, 1080);
        let (x, y) = hud.crosshair_position();
        assert_eq!(x, 960.0);
        assert_eq!(y, 540.0);
    }

    #[test]
    fn hotbar_layout_is_centered_horizontally() {
        let hud = Hud::new((800, 600));
        let layout = hud.hotbar_layout();

        // Calculate expected total width
        const SLOT_SIZE: u32 = 40;
        const SLOT_SPACING: u32 = 4;
        let total_width = SLOT_SIZE * 9 + SLOT_SPACING * 8;

        // Should be centered
        let expected_x = (800 - total_width as i32) / 2;
        assert_eq!(layout.x, expected_x);
    }

    #[test]
    fn hotbar_layout_is_at_bottom() {
        let hud = Hud::new((800, 600));
        let layout = hud.hotbar_layout();

        // Should be near bottom with padding
        assert!(layout.y > 500);
        assert!(layout.y < 600);
    }

    #[test]
    fn hotbar_slot_positions_are_evenly_spaced() {
        let hud = Hud::new((800, 600));
        let layout = hud.hotbar_layout();

        let (x0, y0) = hud.hotbar_slot_position(0);
        let (x1, y1) = hud.hotbar_slot_position(1);
        let (x2, _) = hud.hotbar_slot_position(2);

        // Y should be same for all slots
        assert_eq!(y0, y1);
        assert_eq!(y0, layout.y);

        // X should be evenly spaced
        let spacing = x1 - x0;
        assert_eq!(x2 - x1, spacing);
        assert_eq!(spacing, (layout.slot_size + layout.slot_spacing) as i32);
    }

    #[test]
    fn hotbar_slot_position_for_all_slots() {
        let hud = Hud::new((800, 600));

        // Should not panic for any valid slot
        for i in 0..HOTBAR_SIZE {
            let (x, y) = hud.hotbar_slot_position(i);
            assert!(x >= 0);
            assert!(y >= 0);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid hotbar slot index")]
    fn hotbar_slot_position_panics_on_invalid_index() {
        let hud = Hud::new((800, 600));
        hud.hotbar_slot_position(HOTBAR_SIZE);
    }

    #[test]
    fn set_crosshair_visible() {
        let mut hud = Hud::new((800, 600));
        assert!(hud.crosshair_visible);

        hud.set_crosshair_visible(false);
        assert!(!hud.crosshair_visible);

        hud.set_crosshair_visible(true);
        assert!(hud.crosshair_visible);
    }

    #[test]
    fn set_hotbar_visible() {
        let mut hud = Hud::new((800, 600));
        assert!(hud.hotbar_visible);

        hud.set_hotbar_visible(false);
        assert!(!hud.hotbar_visible);

        hud.set_hotbar_visible(true);
        assert!(hud.hotbar_visible);
    }

    #[test]
    fn default_creates_hud_with_standard_resolution() {
        let hud = Hud::default();
        assert_eq!(hud.screen_size, (800, 600));
        assert!(hud.crosshair_visible);
        assert!(hud.hotbar_visible);
    }

    #[test]
    fn hotbar_layout_changes_with_screen_size() {
        let mut hud = Hud::new((800, 600));
        let layout1 = hud.hotbar_layout();

        hud.set_screen_size(1920, 1080);
        let layout2 = hud.hotbar_layout();

        // Position should change
        assert_ne!(layout1.x, layout2.x);
        assert_ne!(layout1.y, layout2.y);

        // But slot size should remain the same
        assert_eq!(layout1.slot_size, layout2.slot_size);
        assert_eq!(layout1.slot_spacing, layout2.slot_spacing);
    }
}
