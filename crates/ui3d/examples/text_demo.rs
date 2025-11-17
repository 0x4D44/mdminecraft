//! Text rendering demo - Shows 3D text in world space

use anyhow::Result;
use mdminecraft_render::{Camera, Renderer, RendererConfig, WindowConfig, WindowManager};
use mdminecraft_ui3d::{
    render::{FontAtlasBuilder, TextRenderer},
    Text3D,
};
use std::time::Instant;
use winit::event::{Event, WindowEvent};
use winit::keyboard::KeyCode;
use wgpu::util::DeviceExt;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Create window
    let window_config = WindowConfig {
        title: "3D Text Demo".to_string(),
        width: 1280,
        height: 720,
        vsync: true,
    };

    let window_manager = WindowManager::new(window_config)?;
    let window = window_manager.window();

    // Create renderer
    let renderer_config = RendererConfig {
        width: 1280,
        height: 720,
        headless: false,
    };
    let mut renderer = Renderer::new(renderer_config);

    // Initialize GPU
    pollster::block_on(renderer.initialize_gpu(window.clone()))?;

    // Try to load a system font
    let font_path = find_system_font()?;
    tracing::info!("Using font: {}", font_path);

    // Build font atlas
    let atlas = FontAtlasBuilder::from_file(&font_path)?
        .with_font_size(48.0)
        .build()?;

    tracing::info!(
        "Font atlas created: {}x{} pixels",
        atlas.width,
        atlas.height
    );

    // Create camera bind group layout for text renderer
    let camera_bind_group_layout = {
        let resources = renderer.render_resources().expect("GPU not initialized");
        resources.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    };

    // Create text renderer
    let text_renderer = {
        let resources = renderer.render_resources().expect("GPU not initialized");
        TextRenderer::new(
            resources.device,
            resources.queue,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            &camera_bind_group_layout,
            atlas,
        )?
    };

    tracing::info!("Text renderer initialized");

    // Create some 3D text objects
    let texts = vec![
        Text3D::new(glam::Vec3::new(0.0, 2.0, -5.0), "Hello, 3D World!")
            .with_font_size(0.5)
            .with_color([1.0, 1.0, 0.0, 1.0]),
        Text3D::new(glam::Vec3::new(-3.0, 1.0, -8.0), "mdminecraft")
            .with_font_size(0.3)
            .with_color([0.4, 0.8, 1.0, 1.0]),
        Text3D::new(glam::Vec3::new(3.0, 1.5, -6.0), "3D UI in Rust!")
            .with_font_size(0.4)
            .with_color([1.0, 0.5, 0.8, 1.0]),
        Text3D::new(glam::Vec3::new(0.0, 0.5, -10.0), "Billboard Text")
            .with_font_size(0.35)
            .with_color([0.5, 1.0, 0.5, 1.0])
            .with_billboard(true),
    ];

    // Generate meshes for all text
    let mut text_meshes = Vec::new();
    let mut text_buffers = Vec::new();

    {
        let resources = renderer.render_resources().expect("GPU not initialized");

        for text in &texts {
            let (vertices, indices) = text_renderer.generate_text_mesh(text);

            // Create GPU buffers
            let vertex_buffer =
                resources
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Text Vertex Buffer"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

            let index_buffer =
                resources
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Text Index Buffer"),
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });

            text_meshes.push((vertices.len(), indices.len()));
            text_buffers.push((vertex_buffer, index_buffer));
        }
    }

    tracing::info!("Generated {} text meshes", texts.len());

    // Setup camera
    renderer.camera_mut().position = glam::Vec3::new(0.0, 1.5, 0.0);
    renderer.camera_mut().yaw = 0.0;
    renderer.camera_mut().pitch = 0.0;

    let mut last_frame = Instant::now();
    let mut mouse_grabbed = false;
    let mut mouse_delta = (0.0, 0.0);
    let mut keys_pressed = std::collections::HashSet::new();

    // Run event loop
    window_manager.run(move |event, window| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                return false;
            }

            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event, .. },
                ..
            } => {
                if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                    if event.state.is_pressed() {
                        keys_pressed.insert(code);

                        // ESC to quit
                        if code == KeyCode::Escape {
                            return false;
                        }

                        // Tab to toggle cursor grab
                        if code == KeyCode::Tab {
                            mouse_grabbed = !mouse_grabbed;
                            if mouse_grabbed {
                                let _ = window.set_cursor_grab(
                                    winit::window::CursorGrabMode::Confined,
                                );
                                window.set_cursor_visible(false);
                            } else {
                                let _ = window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                window.set_cursor_visible(true);
                            }
                        }
                    } else {
                        keys_pressed.remove(&code);
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                if mouse_grabbed {
                    let center_x = 1280.0 / 2.0;
                    let center_y = 720.0 / 2.0;
                    mouse_delta = (position.x - center_x, position.y - center_y);
                    let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                        center_x, center_y,
                    ));
                }
            }

            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                let now = Instant::now();
                let dt = (now - last_frame).as_secs_f32();
                last_frame = now;

                // Update camera
                let camera = renderer.camera_mut();

                // Mouse look
                if mouse_grabbed {
                    let sensitivity = 0.002;
                    camera.rotate(
                        mouse_delta.0 as f32 * sensitivity,
                        -mouse_delta.1 as f32 * sensitivity,
                    );
                    mouse_delta = (0.0, 0.0);
                }

                // WASD movement
                let speed = 3.0 * dt;
                if keys_pressed.contains(&KeyCode::KeyW) {
                    camera.translate(camera.forward() * speed);
                }
                if keys_pressed.contains(&KeyCode::KeyS) {
                    camera.translate(-camera.forward() * speed);
                }
                if keys_pressed.contains(&KeyCode::KeyA) {
                    camera.translate(-camera.right() * speed);
                }
                if keys_pressed.contains(&KeyCode::KeyD) {
                    camera.translate(camera.right() * speed);
                }
                if keys_pressed.contains(&KeyCode::Space) {
                    camera.translate(glam::Vec3::Y * speed);
                }
                if keys_pressed.contains(&KeyCode::ShiftLeft) {
                    camera.translate(-glam::Vec3::Y * speed);
                }

                // Render
                if let Some(frame) = renderer.begin_frame() {
                    let resources = renderer.render_resources().unwrap();

                    let mut encoder = resources.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        },
                    );

                    // Clear to dark blue background
                    {
                        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Clear Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &frame.view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.1,
                                        g: 0.2,
                                        b: 0.3,
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                    }

                    // Render text (without depth buffer for this simple demo)
                    {
                        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Text Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &frame.view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        // Draw all text
                        for (i, ((vertex_buffer, index_buffer), (_vertex_count, index_count))) in
                            text_buffers.iter().zip(text_meshes.iter()).enumerate()
                        {
                            let use_billboard = texts[i].billboard;
                            render_pass.set_pipeline(text_renderer.pipeline(use_billboard));
                            render_pass.set_bind_group(
                                0,
                                resources.pipeline.camera_bind_group(),
                                &[],
                            );
                            render_pass.set_bind_group(1, text_renderer.font_bind_group(), &[]);
                            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                            render_pass.set_index_buffer(
                                index_buffer.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            render_pass.draw_indexed(0..*index_count as u32, 0, 0..1);
                        }
                    }

                    resources.queue.submit(std::iter::once(encoder.finish()));
                    frame.present();
                }

                window.request_redraw();
            }

            Event::AboutToWait => {
                window.request_redraw();
            }

            _ => {}
        }

        true
    })?;

    Ok(())
}

/// Try to find a usable system font
fn find_system_font() -> Result<String> {
    // Try common font locations
    let candidates = vec![
        // Linux
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        // macOS
        "/System/Library/Fonts/Helvetica.ttc",
        "/Library/Fonts/Arial.ttf",
        // Windows
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\Arial.ttf",
    ];

    for path in candidates {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    anyhow::bail!("Could not find a system font. Please install DejaVu Sans or specify a font path.")
}
