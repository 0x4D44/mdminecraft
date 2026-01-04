//! egui UI integration for debug HUD and overlays.

use std::fmt;

use egui_wgpu::ScreenDescriptor;

/// References required to render an egui frame.
pub struct UiRenderContext<'a> {
    /// GPU device for updating buffers and textures.
    pub device: &'a wgpu::Device,
    /// Queue used to submit resource updates.
    pub queue: &'a wgpu::Queue,
    /// Command encoder for recording the render pass.
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// Target texture view to render egui into.
    pub view: &'a wgpu::TextureView,
    /// Screen descriptor describing resolution and scale.
    pub screen: ScreenDescriptor,
}

/// UI overlay manager using egui.
pub struct UiManager {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    state: egui_winit::State,
}

impl UiManager {
    /// Create a new UI manager.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        window: &winit::window::Window,
    ) -> Self {
        let context = egui::Context::default();

        let viewport_id = context.viewport_id();
        let state = egui_winit::State::new(context.clone(), viewport_id, window, None, None);

        let renderer = egui_wgpu::Renderer::new(device, surface_format, None, 1);

        Self {
            context,
            renderer,
            state,
        }
    }

    /// Handle window event.
    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    /// Prepare UI for rendering (call before begin_frame).
    pub fn prepare(&mut self, window: &winit::window::Window) -> egui::FullOutput {
        let raw_input = self.state.take_egui_input(window);
        self.context.run(raw_input, |_ctx| {
            // UI will be built in render()
        })
    }

    /// Render UI with custom content.
    pub fn render<F>(&mut self, ctx: UiRenderContext<'_>, window: &winit::window::Window, ui_fn: F)
    where
        F: FnOnce(&egui::Context),
    {
        // Take input and run UI
        let raw_input = self.state.take_egui_input(window);
        let full_output = self.context.run(raw_input, ui_fn);

        // Handle platform output
        self.state
            .handle_platform_output(window, full_output.platform_output);

        // Convert egui shapes to render primitives
        let paint_jobs = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        // Upload textures
        for (id, image_delta) in full_output.textures_delta.set {
            self.renderer
                .update_texture(ctx.device, ctx.queue, id, &image_delta);
        }

        // Update buffers
        self.renderer
            .update_buffers(ctx.device, ctx.queue, ctx.encoder, &paint_jobs, &ctx.screen);

        // Render
        {
            let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear, render over existing content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            self.renderer
                .render(&mut render_pass, &paint_jobs, &ctx.screen);
        }

        // Free textures
        for id in full_output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
    }
}

/// Debug HUD showing performance and world info.
/// Coarse-grained state describing who currently owns input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlMode {
    /// Menu UI owns the cursor and gameplay is paused.
    #[default]
    Menu,
    /// Gameplay is active with physics-grounded movement.
    GameplayPhysics,
    /// Gameplay is active with free-flying camera controls.
    GameplayFly,
    /// UI overlay is focused while gameplay previews inputs.
    UiOverlay,
}

impl fmt::Display for ControlMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            ControlMode::Menu => "Menu",
            ControlMode::GameplayPhysics => "Gameplay — Physics",
            ControlMode::GameplayFly => "Gameplay — Fly",
            ControlMode::UiOverlay => "UI Overlay",
        };
        write!(f, "{label}")
    }
}

/// Aggregated state values rendered via the debug HUD overlay.
pub struct DebugHud {
    /// Whether the HUD is visible
    pub visible: bool,
    /// FPS history (last 120 frames)
    fps_history: Vec<f32>,
    /// Current FPS
    pub fps: f32,
    /// Frame time in ms
    pub frame_time_ms: f32,
    /// Camera position
    pub camera_pos: [f32; 3],
    /// Camera rotation (yaw, pitch)
    pub camera_rot: [f32; 2],
    /// Number of chunks loaded
    pub chunks_loaded: usize,
    /// Number of chunks visible (after frustum culling)
    pub chunks_visible: usize,
    /// Total vertices
    pub total_vertices: usize,
    /// Total triangles
    pub total_triangles: usize,
    /// Mining progress (0-100%)
    pub mining_progress: Option<f32>,
    /// Current control mode/context info
    pub control_mode: ControlMode,
    /// Whether the cursor is currently captured
    pub cursor_captured: bool,
    /// Active mouse sensitivity value
    pub mouse_sensitivity: f32,
    /// Human-readable weather state label
    pub weather_state: String,
    /// Precipitation intensity used by shaders
    pub weather_intensity: f32,
    /// Number of chunk meshes uploaded during the last frame
    pub chunk_uploads_last_frame: u32,
    /// Active particle count sent to the GPU
    pub particle_count: usize,
    /// Particle budget ceiling for throttling
    pub particle_budget: usize,
}

impl DebugHud {
    /// Create a new debug HUD.
    pub fn new() -> Self {
        Self {
            visible: true,
            fps_history: Vec::with_capacity(120),
            fps: 0.0,
            frame_time_ms: 0.0,
            camera_pos: [0.0; 3],
            camera_rot: [0.0; 2],
            chunks_loaded: 0,
            chunks_visible: 0,
            total_vertices: 0,
            total_triangles: 0,
            mining_progress: None,
            control_mode: ControlMode::default(),
            cursor_captured: false,
            mouse_sensitivity: 0.0,
            weather_state: "Unknown".to_string(),
            weather_intensity: 0.0,
            chunk_uploads_last_frame: 0,
            particle_count: 0,
            particle_budget: 0,
        }
    }

    /// Update FPS from frame time.
    pub fn update_fps(&mut self, dt: f32) {
        self.frame_time_ms = dt * 1000.0;
        self.fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };

        self.fps_history.push(self.fps);
        if self.fps_history.len() > 120 {
            self.fps_history.remove(0);
        }
    }

    /// Toggle HUD visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Render the HUD using egui.
    pub fn render(&self, ctx: &egui::Context) {
        if !self.visible {
            return;
        }

        egui::Window::new("Debug Info")
            .default_pos([10.0, 10.0])
            .default_size([300.0, 400.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Performance");
                ui.separator();

                ui.label(format!("FPS: {:.1}", self.fps));
                ui.label(format!("Frame Time: {:.2} ms", self.frame_time_ms));

                // FPS graph (simplified - egui 0.26 plot API may differ)
                if !self.fps_history.is_empty() {
                    let min_fps = self
                        .fps_history
                        .iter()
                        .cloned()
                        .fold(f32::INFINITY, f32::min);
                    let max_fps = self.fps_history.iter().cloned().fold(0.0f32, f32::max);
                    ui.label(format!("FPS range: {:.1} - {:.1}", min_fps, max_fps));
                }

                ui.add_space(10.0);
                ui.heading("Camera");
                ui.separator();

                ui.label(format!(
                    "Position: ({:.1}, {:.1}, {:.1})",
                    self.camera_pos[0], self.camera_pos[1], self.camera_pos[2]
                ));
                ui.label(format!(
                    "Rotation: Yaw {:.1}°, Pitch {:.1}°",
                    self.camera_rot[0].to_degrees(),
                    self.camera_rot[1].to_degrees()
                ));

                ui.add_space(10.0);
                ui.heading("World");
                ui.separator();

                ui.label(format!("Chunks Loaded: {}", self.chunks_loaded));
                ui.label(format!(
                    "Chunks Visible: {} ({:.1}%)",
                    self.chunks_visible,
                    if self.chunks_loaded > 0 {
                        (self.chunks_visible as f32 / self.chunks_loaded as f32) * 100.0
                    } else {
                        0.0
                    }
                ));
                ui.label(format!("Total Vertices: {}", self.total_vertices));
                ui.label(format!("Total Triangles: {}", self.total_triangles));
                ui.label(format!(
                    "Chunk Uploads (frame): {}",
                    self.chunk_uploads_last_frame
                ));
                ui.label(format!(
                    "Particles: {}/{}",
                    self.particle_count, self.particle_budget
                ));

                // Mining progress
                if let Some(progress) = self.mining_progress {
                    ui.add_space(10.0);
                    ui.heading("Mining");
                    ui.separator();
                    ui.label(format!("Progress: {:.1}%", progress));
                    ui.add(egui::ProgressBar::new(progress / 100.0).show_percentage());
                }

                ui.add_space(10.0);
                ui.heading("Input");
                ui.separator();
                ui.label(format!("Mode: {}", self.control_mode));
                ui.label(format!(
                    "Cursor Captured: {}",
                    if self.cursor_captured { "Yes" } else { "No" }
                ));
                ui.label(format!("Mouse Sensitivity: {:.3}", self.mouse_sensitivity));

                ui.add_space(10.0);
                ui.heading("Atmosphere");
                ui.separator();
                ui.label(format!("Weather: {}", self.weather_state));
                ui.label(format!(
                    "Precipitation Intensity: {:.2}",
                    self.weather_intensity
                ));

                ui.add_space(10.0);
                ui.label("Press F3 to toggle this HUD");
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_mode_display_labels() {
        assert_eq!(ControlMode::Menu.to_string(), "Menu");
        assert_eq!(ControlMode::GameplayPhysics.to_string(), "Gameplay — Physics");
        assert_eq!(ControlMode::GameplayFly.to_string(), "Gameplay — Fly");
        assert_eq!(ControlMode::UiOverlay.to_string(), "UI Overlay");
    }

    #[test]
    fn debug_hud_updates_fps_history() {
        let mut hud = DebugHud::new();
        for _ in 0..130 {
            hud.update_fps(1.0 / 60.0);
        }
        assert_eq!(hud.fps_history.len(), 120);
        assert!(hud.fps > 0.0);
        assert!(hud.frame_time_ms > 0.0);
    }

    #[test]
    fn debug_hud_toggle_visibility() {
        let mut hud = DebugHud::new();
        assert!(hud.visible);
        hud.toggle();
        assert!(!hud.visible);
    }

    #[test]
    fn debug_hud_renders_without_panic() {
        let mut hud = DebugHud::new();
        hud.mining_progress = Some(42.0);
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            hud.render(ctx);
        });
    }
}

impl Default for DebugHud {
    fn default() -> Self {
        Self::new()
    }
}
