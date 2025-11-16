#![warn(missing_docs)]
//! UI overlays using egui (HUD, menus, debug info).

use egui::{Color32, Context, Pos2, Stroke};

/// UI state and rendering.
pub struct UiState {
    /// Whether the debug panel (F3) is visible.
    pub debug_visible: bool,

    /// FPS history for graphing (last 120 frames).
    fps_history: Vec<f32>,

    /// Frame time history in milliseconds.
    frame_time_history: Vec<f32>,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

impl UiState {
    /// Create a new UI state.
    pub fn new() -> Self {
        Self {
            debug_visible: false,
            fps_history: Vec::with_capacity(120),
            frame_time_history: Vec::with_capacity(120),
        }
    }

    /// Toggle debug panel visibility.
    pub fn toggle_debug(&mut self) {
        self.debug_visible = !self.debug_visible;
    }

    /// Update FPS history.
    pub fn update_fps(&mut self, fps: f32, frame_time_ms: f32) {
        self.fps_history.push(fps);
        if self.fps_history.len() > 120 {
            self.fps_history.remove(0);
        }

        self.frame_time_history.push(frame_time_ms);
        if self.frame_time_history.len() > 120 {
            self.frame_time_history.remove(0);
        }
    }

    /// Render the UI.
    pub fn render(
        &self,
        ctx: &Context,
        camera_pos: [f32; 3],
        camera_yaw: f32,
        camera_pitch: f32,
        camera_speed: f32,
        render_stats: (u32, u32, usize),
    ) {
        // Always render crosshair
        self.render_crosshair(ctx);

        // Render debug panel if visible
        if self.debug_visible {
            self.render_debug_panel(ctx, camera_pos, camera_yaw, camera_pitch, camera_speed, render_stats);
        }
    }

    /// Render crosshair in the center of the screen.
    fn render_crosshair(&self, ctx: &Context) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("crosshair"),
        ));

        let screen_rect = ctx.screen_rect();
        let center = screen_rect.center();

        let size = 10.0;
        let thickness = 2.0;
        let color = Color32::WHITE;
        let stroke = Stroke::new(thickness, color);

        // Horizontal line
        painter.line_segment(
            [
                Pos2::new(center.x - size, center.y),
                Pos2::new(center.x + size, center.y),
            ],
            stroke,
        );

        // Vertical line
        painter.line_segment(
            [
                Pos2::new(center.x, center.y - size),
                Pos2::new(center.x, center.y + size),
            ],
            stroke,
        );
    }

    /// Render debug panel (F3 overlay).
    fn render_debug_panel(
        &self,
        ctx: &Context,
        camera_pos: [f32; 3],
        camera_yaw: f32,
        camera_pitch: f32,
        camera_speed: f32,
        render_stats: (u32, u32, usize),
    ) {
        egui::Window::new("Debug Info")
            .fixed_pos(Pos2::new(10.0, 10.0))
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .show(ctx, |ui| {
                ui.heading("mdminecraft Debug (F3)");
                ui.separator();

                // FPS stats
                if let Some(&current_fps) = self.fps_history.last() {
                    let avg_fps = if !self.fps_history.is_empty() {
                        self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32
                    } else {
                        0.0
                    };
                    let min_fps = self.fps_history.iter().copied().fold(f32::INFINITY, f32::min);
                    let max_fps = self.fps_history.iter().copied().fold(0.0, f32::max);

                    ui.label(format!("FPS: {:.0} (avg: {:.0}, min: {:.0}, max: {:.0})",
                        current_fps, avg_fps, min_fps, max_fps));
                }

                // Frame time
                if let Some(&frame_time) = self.frame_time_history.last() {
                    let avg_frame_time = if !self.frame_time_history.is_empty() {
                        self.frame_time_history.iter().sum::<f32>() / self.frame_time_history.len() as f32
                    } else {
                        0.0
                    };
                    ui.label(format!("Frame time: {:.2}ms (avg: {:.2}ms)", frame_time, avg_frame_time));
                }

                ui.separator();

                // Camera info
                ui.label(format!("Position: ({:.1}, {:.1}, {:.1})",
                    camera_pos[0], camera_pos[1], camera_pos[2]));
                ui.label(format!("Yaw: {:.2}°  Pitch: {:.2}°",
                    camera_yaw.to_degrees(), camera_pitch.to_degrees()));
                ui.label(format!("Speed: {:.0} blocks/sec", camera_speed));

                ui.separator();

                // Render stats
                let (total_indices, total_triangles, chunk_count) = render_stats;
                ui.label(format!("Chunks rendered: {}", chunk_count));
                ui.label(format!("Triangles: {}", total_triangles));
                ui.label(format!("Indices: {}", total_indices));

                ui.separator();

                // World info
                ui.label("World size: 7×7 chunks (112×112 blocks)");
                ui.label("Render distance: 128 blocks");

                ui.separator();

                // Controls
                ui.label("Controls:");
                ui.label("  WASD - Move");
                ui.label("  Space - Up, Ctrl - Down");
                ui.label("  Shift - Sprint (4x speed)");
                ui.label("  Ctrl - Slow (0.25x speed)");
                ui.label("  Mouse - Look");
                ui.label("  F3 - Toggle debug");
                ui.label("  ESC - Exit");
            });
    }
}
