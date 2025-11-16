//! egui UI integration for debug HUD and overlays.

use egui_wgpu::ScreenDescriptor;

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
        let state = egui_winit::State::new(
            context.clone(),
            viewport_id,
            window,
            None,
            None,
        );

        let renderer = egui_wgpu::Renderer::new(device, surface_format, None, 1);

        Self {
            context,
            renderer,
            state,
        }
    }

    /// Handle window event.
    pub fn handle_event(&mut self, window: &winit::window::Window, event: &winit::event::WindowEvent) -> bool {
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
    pub fn render<F>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_descriptor: ScreenDescriptor,
        window: &winit::window::Window,
        ui_fn: F,
    ) where
        F: FnOnce(&egui::Context),
    {
        // Take input and run UI
        let raw_input = self.state.take_egui_input(window);
        let full_output = self.context.run(raw_input, ui_fn);

        // Handle platform output
        self.state.handle_platform_output(window, full_output.platform_output);

        // Convert egui shapes to render primitives
        let paint_jobs = self.context.tessellate(full_output.shapes, full_output.pixels_per_point);

        // Upload textures
        for (id, image_delta) in full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, id, &image_delta);
        }

        // Update buffers
        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Render
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear, render over existing content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            self.renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        // Free textures
        for id in full_output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
    }
}

/// Debug HUD showing performance and world info.
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
    /// Total vertices
    pub total_vertices: usize,
    /// Total triangles
    pub total_triangles: usize,
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
            total_vertices: 0,
            total_triangles: 0,
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
                    let min_fps = self.fps_history.iter().cloned().fold(f32::INFINITY, f32::min);
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
                ui.label(format!("Total Vertices: {}", self.total_vertices));
                ui.label(format!("Total Triangles: {}", self.total_triangles));

                ui.add_space(10.0);
                ui.label("Press F3 to toggle this HUD");
            });
    }
}

impl Default for DebugHud {
    fn default() -> Self {
        Self::new()
    }
}
