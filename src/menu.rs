//! Main menu system

use anyhow::Result;
use std::sync::Arc;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

/// Menu action to communicate with main state machine
pub enum MenuAction {
    /// Continue displaying menu
    Continue,
    /// Start the game
    StartGame,
    /// Quit application
    Quit,
}

/// Main menu state
pub struct MenuState {
    window: Arc<Window>,
    egui_state: egui_winit::State,
    egui_ctx: egui::Context,
    wgpu_device: wgpu::Device,
    wgpu_queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    egui_renderer: egui_wgpu::Renderer,
}

impl MenuState {
    /// Create a new menu state
    pub fn new(event_loop: &EventLoopWindowTarget<()>) -> Result<Self> {
        // Create window
        let window = Arc::new(
            winit::window::WindowBuilder::new()
                .with_title("mdminecraft")
                .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720))
                .build(event_loop)?,
        );

        // Initialize egui
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
        );

        // Initialize wgpu
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        // Initialize egui renderer
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1);

        Ok(Self {
            window,
            egui_state,
            egui_ctx,
            wgpu_device: device,
            wgpu_queue: queue,
            surface,
            surface_config,
            egui_renderer,
        })
    }

    /// Handle an event
    pub fn handle_event(
        &mut self,
        event: &Event<()>,
        _elwt: &EventLoopWindowTarget<()>,
    ) -> MenuAction {
        match event {
            Event::WindowEvent { event, window_id } if *window_id == self.window.id() => {
                // Let egui handle the event first
                let response = self.egui_state.on_window_event(&self.window, event);
                if response.consumed {
                    return MenuAction::Continue;
                }

                match event {
                    WindowEvent::CloseRequested => {
                        return MenuAction::Quit;
                    }
                    WindowEvent::Resized(new_size) => {
                        if new_size.width > 0 && new_size.height > 0 {
                            self.surface_config.width = new_size.width;
                            self.surface_config.height = new_size.height;
                            self.surface
                                .configure(&self.wgpu_device, &self.surface_config);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        return self.render();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                self.window.request_redraw();
            }
            _ => {}
        }

        MenuAction::Continue
    }

    /// Render the menu
    fn render(&mut self) -> MenuAction {
        let mut action = MenuAction::Continue;

        // Get surface texture
        let output = match self.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(e) => {
                tracing::warn!("Failed to get surface texture: {}", e);
                return MenuAction::Continue;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Prepare egui
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            // Main menu panel
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::from_rgb(20, 20, 30)))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(150.0);

                        // Title
                        ui.heading(
                            egui::RichText::new("mdminecraft")
                                .size(72.0)
                                .color(egui::Color32::from_rgb(100, 200, 255)),
                        );

                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new("A Deterministic Voxel Sandbox Engine")
                                .size(16.0)
                                .color(egui::Color32::LIGHT_GRAY),
                        );

                        ui.add_space(100.0);

                        // Menu buttons
                        let button_width = 300.0;
                        let button_height = 50.0;

                        if ui
                            .add_sized(
                                [button_width, button_height],
                                egui::Button::new(egui::RichText::new("Play").size(24.0)),
                            )
                            .clicked()
                        {
                            action = MenuAction::StartGame;
                        }

                        ui.add_space(15.0);

                        if ui
                            .add_sized(
                                [button_width, button_height],
                                egui::Button::new(egui::RichText::new("Settings").size(24.0)),
                            )
                            .clicked()
                        {
                            // TODO: Settings menu
                            tracing::info!("Settings not yet implemented");
                        }

                        ui.add_space(15.0);

                        if ui
                            .add_sized(
                                [button_width, button_height],
                                egui::Button::new(egui::RichText::new("Quit").size(24.0)),
                            )
                            .clicked()
                        {
                            action = MenuAction::Quit;
                        }

                        ui.add_space(100.0);

                        // Version info
                        ui.label(
                            egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .size(12.0)
                                .color(egui::Color32::DARK_GRAY),
                        );
                    });
                });
        });

        // Handle platform output
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        // Render egui
        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder =
            self.wgpu_device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Menu Render Encoder"),
                });

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(
                &self.wgpu_device,
                &self.wgpu_queue,
                *id,
                image_delta,
            );
        }

        self.egui_renderer.update_buffers(
            &self.wgpu_device,
            &self.wgpu_queue,
            &mut encoder,
            &tris,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Menu Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.egui_renderer
                .render(&mut render_pass, &tris, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.wgpu_queue.submit(std::iter::once(encoder.finish()));
        output.present();

        action
    }
}
