//! Simple 3D voxel viewer demo.

use anyhow::Result;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_render::{
    mesh_chunk, ChunkManager, DebugHud, InputState, Renderer, RendererConfig, WindowConfig,
    WindowManager,
};
use mdminecraft_world::{ChunkPos, TerrainGenerator};
use std::time::Instant;
use winit::event::{Event, WindowEvent};
use winit::keyboard::KeyCode;

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create window
    let window_config = WindowConfig {
        title: "mdminecraft 3D Viewer".to_string(),
        width: 1280,
        height: 720,
        vsync: true,
    };

    let window_manager = WindowManager::new(window_config)?;

    // Create renderer
    let renderer_config = RendererConfig {
        width: 1280,
        height: 720,
        headless: false,
    };
    let mut renderer = Renderer::new(renderer_config);

    // Initialize GPU (blocking on async)
    pollster::block_on(renderer.initialize_gpu(window_manager.window()))?;

    // Generate multiple chunks in a grid
    let registry = BlockRegistry::new(vec![]); // Empty registry for now
    let world_seed = 12345u64;
    let generator = TerrainGenerator::new(world_seed);

    let mut chunk_manager = ChunkManager::new();
    let chunk_radius = 2; // 5×5 grid of chunks
    let mut total_vertices = 0;
    let mut total_indices = 0;

    {
        let resources = renderer
            .render_resources()
            .expect("GPU resources not initialized");

        for x in -chunk_radius..=chunk_radius {
            for z in -chunk_radius..=chunk_radius {
                let chunk_pos = ChunkPos::new(x, z);
                let chunk = generator.generate_chunk(chunk_pos);
                let mesh = mesh_chunk(&chunk, &registry);

                total_vertices += mesh.vertices.len();
                total_indices += mesh.indices.len();

                // Create chunk bind group
                let chunk_bind_group = resources.pipeline.create_chunk_bind_group(
                    resources.device,
                    chunk_pos,
                );

                chunk_manager.add_chunk(
                    resources.device,
                    &mesh,
                    chunk_pos,
                    chunk_bind_group,
                );
            }
        }
    }

    tracing::info!(
        chunks = chunk_manager.chunk_count(),
        total_vertices = total_vertices,
        total_indices = total_indices,
        "Generated chunk meshes"
    );

    // Setup camera position (center of chunk grid, elevated)
    renderer.camera_mut().position = glam::Vec3::new(0.0, 100.0, 0.0);
    renderer.camera_mut().yaw = 0.0;
    renderer.camera_mut().pitch = -0.3;

    // Input state
    let mut input = InputState::new();
    let mut last_frame = Instant::now();

    // Debug HUD
    let mut debug_hud = DebugHud::new();
    debug_hud.chunks_loaded = chunk_manager.chunk_count();
    debug_hud.total_vertices = total_vertices;
    debug_hud.total_triangles = total_indices / 3;

    // Run event loop
    window_manager.run(move |event, window| {
        // Let UI handle events first
        if let Event::WindowEvent { ref event, .. } = event {
            if let Some(ui) = renderer.ui_mut() {
                ui.handle_event(window, event);
            }
            input.handle_event(event);
        }

        match event {
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => return false,

                    WindowEvent::KeyboardInput { event, .. } => {
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                            return false;
                        }

                        // Toggle cursor grab with Tab
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Tab) = event.physical_key {
                            if event.state.is_pressed() {
                                let _ = input.toggle_cursor_grab(window);
                            }
                        }

                        // Toggle debug HUD with F3
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::F3) = event.physical_key {
                            if event.state.is_pressed() {
                                debug_hud.toggle();
                            }
                        }
                    }

                    WindowEvent::Resized(new_size) => {
                        renderer.resize((new_size.width, new_size.height));
                    }

                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last_frame).as_secs_f32();
                        last_frame = now;

                        // Update debug HUD
                        debug_hud.update_fps(dt);
                        let camera = renderer.camera();
                        debug_hud.camera_pos = [
                            camera.position.x,
                            camera.position.y,
                            camera.position.z,
                        ];
                        debug_hud.camera_rot = [camera.yaw, camera.pitch];

                        // Update camera from input
                        update_camera(&mut renderer, &input, dt);

                        // Render
                        if let Some(frame) = renderer.begin_frame() {
                            let resources = renderer.render_resources().unwrap();

                            let mut encoder = resources.device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("Render Encoder"),
                                },
                            );

                            // Render voxels
                            {
                                let mut render_pass =
                                    resources.pipeline.begin_render_pass(&mut encoder, &frame.view);

                                render_pass.set_pipeline(resources.pipeline.pipeline());
                                render_pass.set_bind_group(
                                    0,
                                    resources.pipeline.camera_bind_group(),
                                    &[],
                                );

                                // Render all chunks
                                for chunk_data in chunk_manager.chunks() {
                                    render_pass.set_bind_group(1, &chunk_data.chunk_bind_group, &[]);
                                    render_pass
                                        .set_vertex_buffer(0, chunk_data.vertex_buffer.slice(..));
                                    render_pass.set_index_buffer(
                                        chunk_data.index_buffer.slice(..),
                                        wgpu::IndexFormat::Uint32,
                                    );
                                    render_pass.draw_indexed(0..chunk_data.index_count, 0, 0..1);
                                }
                            }

                            // TODO: Render UI overlay (requires API restructuring to avoid borrow conflicts)

                            resources.queue.submit(std::iter::once(encoder.finish()));
                            frame.present();
                        }

                        window.request_redraw();
                        input.reset_frame();
                    }

                    _ => {}
                }
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

fn update_camera(renderer: &mut Renderer, input: &InputState, dt: f32) {
    use winit::keyboard::KeyCode;

    let camera = renderer.camera_mut();

    // Mouse look
    if input.cursor_grabbed {
        let sensitivity = 0.002;
        let mouse_delta = input.mouse_delta;
        camera.rotate(
            mouse_delta.0 as f32 * sensitivity,
            -mouse_delta.1 as f32 * sensitivity,
        );
    }

    // WASD movement
    let speed = 10.0 * dt;
    let mut movement = glam::Vec3::ZERO;

    if input.is_key_pressed(KeyCode::KeyW) {
        movement += camera.forward();
    }
    if input.is_key_pressed(KeyCode::KeyS) {
        movement -= camera.forward();
    }
    if input.is_key_pressed(KeyCode::KeyA) {
        movement -= camera.right();
    }
    if input.is_key_pressed(KeyCode::KeyD) {
        movement += camera.right();
    }
    if input.is_key_pressed(KeyCode::Space) {
        movement += glam::Vec3::Y;
    }
    if input.is_key_pressed(KeyCode::ShiftLeft) {
        movement -= glam::Vec3::Y;
    }

    if movement.length() > 0.0 {
        camera.translate(movement.normalize() * speed);
    }
}
