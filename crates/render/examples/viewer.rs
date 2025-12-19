//! Simple 3D voxel viewer demo.

use anyhow::Result;
use glam::IVec3;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_render::{
    mesh_chunk, raycast, ChunkManager, DebugHud, Frustum, InputState, Renderer, RendererConfig,
    TimeOfDay, UiRenderContext, WindowConfig, WindowManager,
};
use mdminecraft_world::{BlockId, Chunk, ChunkPos, TerrainGenerator, Voxel, BLOCK_AIR};
use std::collections::HashMap;
use std::time::Instant;
use winit::event::{Event, MouseButton, WindowEvent};
use winit::keyboard::KeyCode;

/// Hotbar for block selection
struct Hotbar {
    slots: [BlockId; 9],
    selected: usize,
}

impl Hotbar {
    fn new() -> Self {
        Self {
            // Default blocks in hotbar slots
            slots: [
                2, // Dirt
                1, // Stone
                3, // Wood
                4, // Sand
                5, // Grass
                6, // Cobblestone
                7, // Planks
                8, // Bricks
                9, // Glass
            ],
            selected: 1, // Start with stone selected
        }
    }

    fn select_slot(&mut self, slot: usize) {
        if slot < 9 {
            self.selected = slot;
        }
    }

    fn selected_block(&self) -> BlockId {
        self.slots[self.selected]
    }

    fn block_name(&self, block_id: BlockId) -> &'static str {
        match block_id {
            0 => "Air",
            1 => "Stone",
            2 => "Dirt",
            3 => "Wood",
            4 => "Sand",
            5 => "Grass",
            6 => "Cobblestone",
            7 => "Planks",
            8 => "Bricks",
            9 => "Glass",
            _ => "Unknown",
        }
    }
}

/// Axis-Aligned Bounding Box for collision detection
#[derive(Debug, Clone, Copy)]
struct Aabb {
    min: glam::Vec3,
    max: glam::Vec3,
}

impl Aabb {
    fn new(min: glam::Vec3, max: glam::Vec3) -> Self {
        Self { min, max }
    }

    fn from_center_size(center: glam::Vec3, size: glam::Vec3) -> Self {
        let half_size = size * 0.5;
        Self {
            min: center - half_size,
            max: center + half_size,
        }
    }

    fn intersects(&self, other: &Aabb) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }
}

/// Player physics state
struct PlayerPhysics {
    velocity: glam::Vec3,
    on_ground: bool,
    gravity: f32,
    jump_strength: f32,
    terminal_velocity: f32,
    player_height: f32,
    player_width: f32,
    physics_enabled: bool,
}

impl PlayerPhysics {
    fn new() -> Self {
        Self {
            velocity: glam::Vec3::ZERO,
            on_ground: false,
            gravity: -20.0,           // m/s²
            jump_strength: 8.0,       // m/s
            terminal_velocity: -50.0, // m/s
            player_height: 1.8,       // blocks
            player_width: 0.6,        // blocks
            physics_enabled: true,
        }
    }

    fn toggle_physics(&mut self) {
        self.physics_enabled = !self.physics_enabled;
        if !self.physics_enabled {
            self.velocity = glam::Vec3::ZERO;
            self.on_ground = false;
        }
    }

    fn get_aabb(&self, position: glam::Vec3) -> Aabb {
        let size = glam::Vec3::new(self.player_width, self.player_height, self.player_width);
        let center = position + glam::Vec3::new(0.0, self.player_height * 0.5, 0.0);
        Aabb::from_center_size(center, size)
    }
}

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
                let mesh = mesh_chunk(&chunk, &registry, renderer.atlas_metadata());

                total_vertices += mesh.vertices.len();
                total_indices += mesh.indices.len();

                // Create chunk bind group
                let chunk_bind_group = resources
                    .pipeline
                    .create_chunk_bind_group(resources.device, chunk_pos);

                chunk_manager.add_chunk(
                    resources.device,
                    resources.queue,
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

    // Hotbar for block selection
    let mut hotbar = Hotbar::new();

    // Player physics
    let mut player_physics = PlayerPhysics::new();

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
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Escape) =
                            event.physical_key
                        {
                            return false;
                        }

                        // Toggle cursor grab with Tab
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Tab) = event.physical_key
                        {
                            if event.state.is_pressed() {
                                let _ = input.toggle_cursor_grab(window);
                            }
                        }

                        // Toggle debug HUD with F3
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::F3) = event.physical_key
                        {
                            if event.state.is_pressed() {
                                debug_hud.toggle();
                            }
                        }

                        // Toggle physics with F
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::KeyF) =
                            event.physical_key
                        {
                            if event.state.is_pressed() {
                                player_physics.toggle_physics();
                                tracing::info!(
                                    "Physics mode: {}",
                                    if player_physics.physics_enabled {
                                        "ENABLED (gravity/collision)"
                                    } else {
                                        "DISABLED (fly mode)"
                                    }
                                );
                            }
                        }

                        // Time controls
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::KeyP) =
                            event.physical_key
                        {
                            if event.state.is_pressed() {
                                time_of_day.toggle_pause();
                                tracing::info!("Time paused: {}", !time_of_day.is_daytime());
                            }
                        }
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::BracketLeft) =
                            event.physical_key
                        {
                            if event.state.is_pressed() {
                                time_of_day.decrease_speed();
                                tracing::info!("Time speed decreased");
                            }
                        }
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::BracketRight) =
                            event.physical_key
                        {
                            if event.state.is_pressed() {
                                time_of_day.increase_speed();
                                tracing::info!("Time speed increased");
                            }
                        }

                        // Hotbar selection (1-9 keys)
                        if event.state.is_pressed() {
                            let slot = match event.physical_key {
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit1) => Some(0),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit2) => Some(1),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit3) => Some(2),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit4) => Some(3),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit5) => Some(4),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit6) => Some(5),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit7) => Some(6),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit8) => Some(7),
                                winit::keyboard::PhysicalKey::Code(KeyCode::Digit9) => Some(8),
                                _ => None,
                            };
                            if let Some(slot) = slot {
                                hotbar.select_slot(slot);
                                let block_name = hotbar.block_name(hotbar.selected_block());
                                tracing::info!("Selected slot {}: {}", slot + 1, block_name);
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
                        debug_hud.camera_pos =
                            [camera.position.x, camera.position.y, camera.position.z];
                        debug_hud.camera_rot = [camera.yaw, camera.pitch];
                        debug_hud.chunks_visible = chunks_visible;

                        // Update camera from input
                        update_camera(&mut renderer, &input, &mut player_physics, &chunks, dt);

                        // Raycast for block selection (only when cursor is grabbed).
                        let selected_block = if input.cursor_captured {
                            let camera = renderer.camera();
                            let ray_origin = camera.position;
                            let ray_dir = camera.forward();

                            raycast(ray_origin, ray_dir, 8.0, |block_pos| {
                                // Convert world position to chunk+local coordinates.
                                let chunk_x = block_pos.x.div_euclid(16);
                                let chunk_z = block_pos.z.div_euclid(16);
                                let local_x = block_pos.x.rem_euclid(16) as usize;
                                let local_y = block_pos.y as usize;
                                let local_z = block_pos.z.rem_euclid(16) as usize;

                                // Check bounds.
                                if local_y >= 256 {
                                    return false;
                                }

                                // Get chunk and check block.
                                chunks
                                    .get(&ChunkPos::new(chunk_x, chunk_z))
                                    .is_some_and(|chunk| {
                                        chunk.voxel(local_x, local_y, local_z).id != BLOCK_AIR
                                    })
                            })
                        } else {
                            None
                        };

                        // Handle block breaking/placing.
                        if let Some(hit) = selected_block {
                            // Left click: break block.
                            if input.is_mouse_clicked(MouseButton::Left) {
                                let chunk_x = hit.block_pos.x.div_euclid(16);
                                let chunk_z = hit.block_pos.z.div_euclid(16);
                                let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

                                if let Some(chunk) = chunks.get_mut(&chunk_pos) {
                                    let local_x = hit.block_pos.x.rem_euclid(16) as usize;
                                    let local_y = hit.block_pos.y as usize;
                                    let local_z = hit.block_pos.z.rem_euclid(16) as usize;

                                    // Set block to air.
                                    chunk.set_voxel(local_x, local_y, local_z, Voxel::default());

                                    // Regenerate mesh.
                                    let mesh =
                                        mesh_chunk(chunk, &registry, renderer.atlas_metadata());
                                    if let Some(resources) = renderer.render_resources() {
                                        let chunk_bind_group = resources
                                            .pipeline
                                            .create_chunk_bind_group(resources.device, chunk_pos);
                                        chunk_manager.add_chunk(
                                            resources.device,
                                            resources.queue,
                                            &mesh,
                                            chunk_pos,
                                            chunk_bind_group,
                                        );
                                    }
                                    tracing::info!("Broke block at {:?}", hit.block_pos);
                                }
                            }

                            // Right click: place block.
                            if input.is_mouse_clicked(MouseButton::Right) {
                                // Place block adjacent to hit face.
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

                                    // Only place if within bounds and target is air.
                                    if local_y < 256 {
                                        let current = chunk.voxel(local_x, local_y, local_z);
                                        if current.id == BLOCK_AIR {
                                            // Place selected block from hotbar.
                                            let new_voxel = Voxel {
                                                id: hotbar.selected_block(),
                                                state: 0,
                                                light_sky: 0,
                                                light_block: 0,
                                            };
                                            chunk.set_voxel(local_x, local_y, local_z, new_voxel);

                                            // Regenerate mesh.
                                            let mesh = mesh_chunk(
                                                chunk,
                                                &registry,
                                                renderer.atlas_metadata(),
                                            );
                                            if let Some(resources) = renderer.render_resources() {
                                                let chunk_bind_group =
                                                    resources.pipeline.create_chunk_bind_group(
                                                        resources.device,
                                                        chunk_pos,
                                                    );
                                                chunk_manager.add_chunk(
                                                    resources.device,
                                                    resources.queue,
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

                        // Render
                        if let Some(frame) = renderer.begin_frame() {
                            let resources = renderer.render_resources().unwrap();

                            // Update time uniforms for both pipelines
                            resources.skybox_pipeline.update_time(
                                resources.queue,
                                &time_of_day,
                                0.0,
                                0.0,
                            );
                            resources
                                .pipeline
                                .update_time(resources.queue, &time_of_day, 0.0, 0.0);

                            let mut encoder = resources.device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("Render Encoder"),
                                },
                            );

                            // Render skybox (background)
                            {
                                let mut render_pass = resources
                                    .skybox_pipeline
                                    .begin_render_pass(&mut encoder, &frame.view);
                                render_pass.set_pipeline(resources.skybox_pipeline.pipeline());
                                render_pass.draw(0..3, 0..1); // Full-screen triangle
                            }

                            // Create frustum for culling
                            let camera = renderer.camera();
                            let view_proj = camera.projection_matrix() * camera.view_matrix();
                            let frustum = Frustum::from_matrix(&view_proj);

                            // Render voxels with frustum culling
                            chunks_visible = 0;
                            {
                                let mut render_pass = resources
                                    .pipeline
                                    .begin_render_pass(&mut encoder, &frame.view);

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

                                    render_pass.set_bind_group(
                                        1,
                                        &chunk_data.chunk_bind_group,
                                        &[],
                                    );
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
                                let mut render_pass = resources
                                    .wireframe_pipeline
                                    .begin_render_pass(&mut encoder, &frame.view, depth_view);

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
                                    UiRenderContext {
                                        device: resources.device,
                                        queue: resources.queue,
                                        encoder: &mut encoder,
                                        view: &frame.view,
                                        screen: screen_descriptor,
                                    },
                                    window,
                                    |ctx| {
                                        debug_hud.render(ctx);
                                        render_hotbar(ctx, &hotbar);
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

fn update_camera(
    renderer: &mut Renderer,
    input: &InputState,
    physics: &mut PlayerPhysics,
    chunks: &HashMap<ChunkPos, Chunk>,
    dt: f32,
) {
    use winit::keyboard::KeyCode;

    let camera = renderer.camera_mut();

    // Mouse look
    if input.cursor_captured {
        let sensitivity = 0.002;
        let mouse_delta = input.mouse_delta;
        camera.rotate(
            mouse_delta.0 as f32 * sensitivity,
            -mouse_delta.1 as f32 * sensitivity,
        );
    }

    if physics.physics_enabled {
        // Physics-based movement
        update_camera_with_physics(camera, input, physics, chunks, dt);
    } else {
        // Free fly mode (original behavior)
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
}

fn update_camera_with_physics(
    camera: &mut mdminecraft_render::Camera,
    input: &InputState,
    physics: &mut PlayerPhysics,
    chunks: &HashMap<ChunkPos, Chunk>,
    dt: f32,
) {
    use winit::keyboard::KeyCode;

    // Apply gravity
    physics.velocity.y += physics.gravity * dt;
    if physics.velocity.y < physics.terminal_velocity {
        physics.velocity.y = physics.terminal_velocity;
    }

    // Horizontal movement (WASD)
    let move_speed = 4.3; // blocks per second
    let mut horizontal_input = glam::Vec2::ZERO;

    if input.is_key_pressed(KeyCode::KeyW) {
        horizontal_input.y += 1.0;
    }
    if input.is_key_pressed(KeyCode::KeyS) {
        horizontal_input.y -= 1.0;
    }
    if input.is_key_pressed(KeyCode::KeyA) {
        horizontal_input.x -= 1.0;
    }
    if input.is_key_pressed(KeyCode::KeyD) {
        horizontal_input.x += 1.0;
    }

    if horizontal_input.length() > 0.0 {
        horizontal_input = horizontal_input.normalize();
        let forward = camera.forward();
        let right = camera.right();

        // Project to horizontal plane
        let forward_h = glam::Vec3::new(forward.x, 0.0, forward.z).normalize();
        let right_h = glam::Vec3::new(right.x, 0.0, right.z).normalize();

        let move_dir = forward_h * horizontal_input.y + right_h * horizontal_input.x;
        physics.velocity.x = move_dir.x * move_speed;
        physics.velocity.z = move_dir.z * move_speed;
    } else {
        // Friction
        physics.velocity.x *= 0.5;
        physics.velocity.z *= 0.5;
    }

    // Jumping
    if input.is_key_pressed(KeyCode::Space) && physics.on_ground {
        physics.velocity.y = physics.jump_strength;
        physics.on_ground = false;
    }

    // Apply velocity and resolve collisions
    let mut new_position = camera.position + physics.velocity * dt;
    physics.on_ground = false;

    // Y-axis collision (gravity/jumping)
    let player_aabb = physics.get_aabb(new_position);
    if check_collision(&player_aabb, chunks) {
        // Resolve Y collision
        if physics.velocity.y < 0.0 {
            // Falling - land on ground
            new_position.y = camera.position.y;
            physics.velocity.y = 0.0;
            physics.on_ground = true;
        } else {
            // Hit ceiling
            new_position.y = camera.position.y;
            physics.velocity.y = 0.0;
        }
    }

    // X-axis collision
    let test_pos = glam::Vec3::new(new_position.x, camera.position.y, camera.position.z);
    let test_aabb = physics.get_aabb(test_pos);
    if check_collision(&test_aabb, chunks) {
        new_position.x = camera.position.x;
        physics.velocity.x = 0.0;
    }

    // Z-axis collision
    let test_pos = glam::Vec3::new(camera.position.x, camera.position.y, new_position.z);
    let test_aabb = physics.get_aabb(test_pos);
    if check_collision(&test_aabb, chunks) {
        new_position.z = camera.position.z;
        physics.velocity.z = 0.0;
    }

    camera.position = new_position;
}

fn check_collision(player_aabb: &Aabb, chunks: &HashMap<ChunkPos, Chunk>) -> bool {
    // Get range of blocks to check
    let min_block = player_aabb.min.floor().as_ivec3();
    let max_block = player_aabb.max.ceil().as_ivec3();

    for y in min_block.y..=max_block.y {
        for z in min_block.z..=max_block.z {
            for x in min_block.x..=max_block.x {
                // Convert world coords to chunk coords
                let chunk_x = x.div_euclid(16);
                let chunk_z = z.div_euclid(16);
                let local_x = x.rem_euclid(16) as usize;
                let local_y = y as usize;
                let local_z = z.rem_euclid(16) as usize;

                // Check bounds
                if local_y >= 256 || y < 0 {
                    continue;
                }

                // Get chunk and check block
                if let Some(chunk) = chunks.get(&ChunkPos::new(chunk_x, chunk_z)) {
                    let voxel = chunk.voxel(local_x, local_y, local_z);
                    if voxel.id != BLOCK_AIR {
                        // Block is solid, check AABB intersection
                        let block_aabb = Aabb::new(
                            glam::Vec3::new(x as f32, y as f32, z as f32),
                            glam::Vec3::new(x as f32 + 1.0, y as f32 + 1.0, z as f32 + 1.0),
                        );
                        if player_aabb.intersects(&block_aabb) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

fn render_hotbar(ctx: &egui::Context, hotbar: &Hotbar) {
    egui::Area::new(egui::Id::new("hotbar"))
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -20.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for i in 0..9 {
                    let is_selected = i == hotbar.selected;
                    let block_id = hotbar.slots[i];
                    let block_name = hotbar.block_name(block_id);

                    let frame = if is_selected {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200))
                            .stroke(egui::Stroke::new(3.0, egui::Color32::WHITE))
                            .inner_margin(8.0)
                    } else {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 180))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_GRAY))
                            .inner_margin(8.0)
                    };

                    frame.show(ui, |ui| {
                        ui.set_min_size(egui::vec2(50.0, 50.0));
                        ui.vertical_centered(|ui| {
                            // Show slot number at top
                            ui.label(egui::RichText::new(format!("{}", i + 1)).size(10.0).color(
                                if is_selected {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::GRAY
                                },
                            ));
                            // Show block name at bottom
                            ui.label(egui::RichText::new(block_name).size(9.0).color(
                                if is_selected {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::LIGHT_GRAY
                                },
                            ));
                        });
                    });
                }
            });
        });
}
