//! Simple 3D voxel viewer demo.

use anyhow::Result;
use glam::IVec3;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_render::{
    mesh_chunk, raycast, ChunkManager, DebugHud, Frustum, InputState, RaycastHit, Renderer,
    RendererConfig, TimeOfDay, WindowConfig, WindowManager,
};
use mdminecraft_world::{Chunk, ChunkPos, TerrainGenerator, Voxel, BLOCK_AIR};
use std::collections::HashMap;
use std::time::Instant;
use winit::event::{Event, MouseButton, WindowEvent};
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
    let mut chunks = HashMap::new(); // Store chunks mutably for modification
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

                // Store chunk data for modification
                chunks.insert(chunk_pos, chunk);
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

    // Frustum culling stats
    let mut chunks_visible = 0;

    // Time-of-day system
    let mut time_of_day = TimeOfDay::new();

    // Block selection tracking
    let mut selected_block: Option<RaycastHit> = None;

    // Run event loop
    window_manager.run(move |event, window| {
        // Let UI handle events first
        if let Event::WindowEvent { ref event, .. } = event {
            if let Some(mut ui) = renderer.ui_mut() {
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

                        // Time controls
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::KeyP) = event.physical_key {
                            if event.state.is_pressed() {
                                time_of_day.toggle_pause();
                                tracing::info!("Time paused: {}", !time_of_day.is_daytime());
                            }
                        }
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::BracketLeft) = event.physical_key {
                            if event.state.is_pressed() {
                                time_of_day.decrease_speed();
                                tracing::info!("Time speed decreased");
                            }
                        }
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::BracketRight) = event.physical_key {
                            if event.state.is_pressed() {
                                time_of_day.increase_speed();
                                tracing::info!("Time speed increased");
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

                        // Update time-of-day
                        time_of_day.update(dt);

                        // Update debug HUD
                        debug_hud.update_fps(dt);
                        let camera = renderer.camera();
                        debug_hud.camera_pos = [
                            camera.position.x,
                            camera.position.y,
                            camera.position.z,
                        ];
                        debug_hud.camera_rot = [camera.yaw, camera.pitch];
                        debug_hud.chunks_visible = chunks_visible;

                        // Update camera from input
                        update_camera(&mut renderer, &input, dt);

                        // Raycast for block selection (only when cursor is grabbed)
                        if input.cursor_grabbed {
                            let camera = renderer.camera();
                            let ray_origin = camera.position;
                            let ray_dir = camera.forward();

                            selected_block = raycast(ray_origin, ray_dir, 8.0, |block_pos| {
                                // Convert world position to chunk+local coordinates
                                let chunk_x = block_pos.x.div_euclid(16);
                                let chunk_z = block_pos.z.div_euclid(16);
                                let local_x = block_pos.x.rem_euclid(16) as usize;
                                let local_y = block_pos.y as usize;
                                let local_z = block_pos.z.rem_euclid(16) as usize;

                                // Check bounds
                                if local_y >= 256 {
                                    return false;
                                }

                                // Get chunk and check block
                                if let Some(chunk) = chunks.get(&ChunkPos::new(chunk_x, chunk_z)) {
                                    let voxel = chunk.voxel(local_x, local_y, local_z);
                                    voxel.id != BLOCK_AIR
                                } else {
                                    false
                                }
                            });

                            // Handle block breaking/placing
                            if let Some(hit) = selected_block {
                                // Left click: break block
                                if input.is_mouse_clicked(MouseButton::Left) {
                                    let chunk_x = hit.block_pos.x.div_euclid(16);
                                    let chunk_z = hit.block_pos.z.div_euclid(16);
                                    let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

                                    if let Some(chunk) = chunks.get_mut(&chunk_pos) {
                                        let local_x = hit.block_pos.x.rem_euclid(16) as usize;
                                        let local_y = hit.block_pos.y as usize;
                                        let local_z = hit.block_pos.z.rem_euclid(16) as usize;

                                        // Set block to air
                                        chunk.set_voxel(local_x, local_y, local_z, Voxel::default());

                                        // Regenerate mesh
                                        let mesh = mesh_chunk(chunk, &registry);
                                        if let Some(resources) = renderer.render_resources() {
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
                                        tracing::info!("Broke block at {:?}", hit.block_pos);
                                    }
                                }

                                // Right click: place block
                                if input.is_mouse_clicked(MouseButton::Right) {
                                    // Place block adjacent to hit face
                                    let place_pos = IVec3::new(
                                        hit.block_pos.x + hit.face_normal.x,
                                        hit.block_pos.y + hit.face_normal.y,
                                        hit.block_pos.z + hit.face_normal.z,
                                    );

                                    let chunk_x = place_pos.x.div_euclid(16);
                                    let chunk_z = place_pos.z.div_euclid(16);
                                    let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

                                    if let Some(chunk) = chunks.get_mut(&chunk_pos) {
                                        let local_x = place_pos.x.rem_euclid(16) as usize;
                                        let local_y = place_pos.y as usize;
                                        let local_z = place_pos.z.rem_euclid(16) as usize;

                                        // Only place if within bounds and target is air
                                        if local_y < 256 {
                                            let current = chunk.voxel(local_x, local_y, local_z);
                                            if current.id == BLOCK_AIR {
                                                // Place stone block (id = 1)
                                                let new_voxel = Voxel {
                                                    id: 1,
                                                    state: 0,
                                                    light_sky: 0,
                                                    light_block: 0,
                                                };
                                                chunk.set_voxel(local_x, local_y, local_z, new_voxel);

                                                // Regenerate mesh
                                                let mesh = mesh_chunk(chunk, &registry);
                                                if let Some(resources) = renderer.render_resources() {
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
                                                tracing::info!("Placed block at {:?}", place_pos);
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Clear selection when cursor not grabbed
                            selected_block = None;
                        }

                        // Render
                        if let Some(frame) = renderer.begin_frame() {
                            let resources = renderer.render_resources().unwrap();

                            // Update time uniforms for both pipelines
                            resources.skybox_pipeline.update_time(resources.queue, &time_of_day);
                            resources.pipeline.update_time(resources.queue, &time_of_day);

                            let mut encoder = resources.device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("Render Encoder"),
                                },
                            );

                            // Render skybox (background)
                            {
                                let mut render_pass =
                                    resources.skybox_pipeline.begin_render_pass(&mut encoder, &frame.view);
                                render_pass.set_pipeline(resources.skybox_pipeline.pipeline());
                                render_pass.draw(0..3, 0..1);  // Full-screen triangle
                            }

                            // Create frustum for culling
                            let camera = renderer.camera();
                            let view_proj = camera.projection_matrix() * camera.view_matrix();
                            let frustum = Frustum::from_matrix(&view_proj);

                            // Render voxels with frustum culling
                            chunks_visible = 0;
                            {
                                let mut render_pass =
                                    resources.pipeline.begin_render_pass(&mut encoder, &frame.view);

                                render_pass.set_pipeline(resources.pipeline.pipeline());
                                render_pass.set_bind_group(
                                    0,
                                    resources.pipeline.camera_bind_group(),
                                    &[],
                                );
                                render_pass.set_bind_group(
                                    2,
                                    resources.pipeline.texture_bind_group(),
                                    &[],
                                );

                                // Render only visible chunks
                                for chunk_data in chunk_manager.chunks() {
                                    // Frustum culling test
                                    if !frustum.is_chunk_visible(chunk_data.chunk_pos) {
                                        continue;
                                    }

                                    chunks_visible += 1;

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

                            // Render wireframe highlight (if block selected)
                            if let Some(hit) = selected_block {
                                // Update wireframe position and color
                                let highlight_pos = [
                                    hit.block_pos.x as f32,
                                    hit.block_pos.y as f32,
                                    hit.block_pos.z as f32,
                                ];
                                let highlight_color = [0.0, 0.0, 0.0, 0.8]; // Black with transparency
                                resources.wireframe_pipeline.update_highlight(
                                    resources.queue,
                                    highlight_pos,
                                    highlight_color,
                                );

                                // Render wireframe
                                let depth_view = resources.pipeline.depth_view();
                                let mut render_pass = resources.wireframe_pipeline.begin_render_pass(
                                    &mut encoder,
                                    &frame.view,
                                    depth_view,
                                );

                                render_pass.set_pipeline(resources.wireframe_pipeline.pipeline());
                                render_pass.set_bind_group(
                                    0,
                                    resources.pipeline.camera_bind_group(),
                                    &[],
                                );
                                render_pass.set_bind_group(
                                    1,
                                    resources.wireframe_pipeline.highlight_bind_group(),
                                    &[],
                                );
                                render_pass.set_vertex_buffer(
                                    0,
                                    resources.wireframe_pipeline.vertex_buffer().slice(..),
                                );
                                render_pass.draw(0..24, 0..1); // 24 vertices for cube wireframe
                            }

                            // Render UI overlay
                            if let Some(mut ui) = renderer.ui_mut() {
                                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                    size_in_pixels: [1280, 720],
                                    pixels_per_point: 1.0,
                                };

                                ui.render(
                                    resources.device,
                                    resources.queue,
                                    &mut encoder,
                                    &frame.view,
                                    screen_descriptor,
                                    window,
                                    |ctx| {
                                        debug_hud.render(ctx);
                                    },
                                );
                            }

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
