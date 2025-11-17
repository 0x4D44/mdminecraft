//! Game world state - the actual 3D voxel game

use anyhow::Result;
use glam::IVec3;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_render::{
    mesh_chunk, raycast, ChunkManager, DebugHud, Frustum, InputState, RaycastHit, Renderer,
    RendererConfig, TimeOfDay, WindowConfig, WindowManager,
};
use mdminecraft_world::{BlockId, Chunk, ChunkPos, TerrainGenerator, Voxel, BLOCK_AIR};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{Event, MouseButton, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::KeyCode;
use winit::window::Window;

/// Game action to communicate with main state machine
pub enum GameAction {
    /// Continue playing
    Continue,
    /// Return to main menu
    ReturnToMenu,
    /// Quit application
    Quit,
}

/// Hotbar for block selection
struct Hotbar {
    slots: [BlockId; 9],
    selected: usize,
}

impl Hotbar {
    fn new() -> Self {
        Self {
            slots: [2, 1, 3, 4, 5, 6, 7, 8, 9],
            selected: 1,
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

/// AABB for collision detection
#[derive(Debug, Clone, Copy)]
struct AABB {
    min: glam::Vec3,
    max: glam::Vec3,
}

impl AABB {
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

    fn intersects(&self, other: &AABB) -> bool {
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
            gravity: -20.0,
            jump_strength: 8.0,
            terminal_velocity: -50.0,
            player_height: 1.8,
            player_width: 0.6,
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

    fn get_aabb(&self, position: glam::Vec3) -> AABB {
        let size = glam::Vec3::new(self.player_width, self.player_height, self.player_width);
        let center = position + glam::Vec3::new(0.0, self.player_height * 0.5, 0.0);
        AABB::from_center_size(center, size)
    }
}

/// The game world state
pub struct GameWorld {
    window: Arc<Window>,
    renderer: Renderer,
    chunk_manager: ChunkManager,
    chunks: HashMap<ChunkPos, Chunk>,
    registry: BlockRegistry,
    input: InputState,
    last_frame: Instant,
    debug_hud: DebugHud,
    time_of_day: TimeOfDay,
    selected_block: Option<RaycastHit>,
    hotbar: Hotbar,
    player_physics: PlayerPhysics,
    chunks_visible: usize,
}

impl GameWorld {
    /// Create a new game world
    pub fn new(event_loop: &EventLoopWindowTarget<()>) -> Result<Self> {
        tracing::info!("Initializing game world...");

        // Create window
        let window_config = WindowConfig {
            title: "mdminecraft - Game".to_string(),
            width: 1280,
            height: 720,
            vsync: true,
        };

        let window_manager = WindowManager::new_with_event_loop(window_config, event_loop)?;
        let window = window_manager.into_window();

        // Create renderer
        let renderer_config = RendererConfig {
            width: 1280,
            height: 720,
            headless: false,
        };
        let mut renderer = Renderer::new(renderer_config);

        // Initialize GPU
        pollster::block_on(renderer.initialize_gpu(window.clone()))?;

        // Generate world
        let registry = BlockRegistry::new(vec![]);
        let world_seed = 12345u64;
        let generator = TerrainGenerator::new(world_seed);

        let mut chunk_manager = ChunkManager::new();
        let mut chunks = HashMap::new();
        let chunk_radius = 2;
        let mut total_vertices = 0;
        let mut total_indices = 0;

        {
            let resources = renderer.render_resources().expect("GPU not initialized");

            for x in -chunk_radius..=chunk_radius {
                for z in -chunk_radius..=chunk_radius {
                    let chunk_pos = ChunkPos::new(x, z);
                    let chunk = generator.generate_chunk(chunk_pos);
                    let mesh = mesh_chunk(&chunk, &registry);

                    total_vertices += mesh.vertices.len();
                    total_indices += mesh.indices.len();

                    let chunk_bind_group =
                        resources.pipeline.create_chunk_bind_group(resources.device, chunk_pos);

                    chunk_manager.add_chunk(resources.device, &mesh, chunk_pos, chunk_bind_group);
                    chunks.insert(chunk_pos, chunk);
                }
            }
        }

        tracing::info!(
            "Generated {} chunks ({} vertices, {} indices)",
            chunk_manager.chunk_count(),
            total_vertices,
            total_indices
        );

        // Setup camera
        renderer.camera_mut().position = glam::Vec3::new(0.0, 100.0, 0.0);
        renderer.camera_mut().yaw = 0.0;
        renderer.camera_mut().pitch = -0.3;

        // Setup state
        let mut debug_hud = DebugHud::new();
        debug_hud.chunks_loaded = chunk_manager.chunk_count();
        debug_hud.total_vertices = total_vertices;
        debug_hud.total_triangles = total_indices / 3;

        Ok(Self {
            window,
            renderer,
            chunk_manager,
            chunks,
            registry,
            input: InputState::new(),
            last_frame: Instant::now(),
            debug_hud,
            time_of_day: TimeOfDay::new(),
            selected_block: None,
            hotbar: Hotbar::new(),
            player_physics: PlayerPhysics::new(),
            chunks_visible: 0,
        })
    }

    /// Handle an event
    pub fn handle_event(
        &mut self,
        event: &Event<()>,
        elwt: &EventLoopWindowTarget<()>,
    ) -> GameAction {
        // Let UI handle events first
        if let Event::WindowEvent { ref event, .. } = event {
            if let Some(mut ui) = self.renderer.ui_mut() {
                ui.handle_event(&self.window, event);
            }
            self.input.handle_event(event);
        }

        match event {
            Event::WindowEvent { event, window_id } if *window_id == self.window.id() => {
                match event {
                    WindowEvent::CloseRequested => {
                        return GameAction::Quit;
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        // ESC returns to menu
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Escape) =
                            event.physical_key
                        {
                            if event.state.is_pressed() {
                                return GameAction::ReturnToMenu;
                            }
                        }

                        self.handle_keyboard_input(event);
                    }
                    WindowEvent::Resized(new_size) => {
                        self.renderer.resize((new_size.width, new_size.height));
                    }
                    WindowEvent::RedrawRequested => {
                        self.update_and_render();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                self.window.request_redraw();
            }
            _ => {}
        }

        GameAction::Continue
    }

    fn handle_keyboard_input(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::PhysicalKey;

        if !event.state.is_pressed() {
            return;
        }

        match event.physical_key {
            PhysicalKey::Code(KeyCode::Tab) => {
                let _ = self.input.toggle_cursor_grab(&self.window);
            }
            PhysicalKey::Code(KeyCode::F3) => {
                self.debug_hud.toggle();
            }
            PhysicalKey::Code(KeyCode::KeyF) => {
                self.player_physics.toggle_physics();
                tracing::info!(
                    "Physics mode: {}",
                    if self.player_physics.physics_enabled {
                        "ENABLED"
                    } else {
                        "DISABLED (fly mode)"
                    }
                );
            }
            PhysicalKey::Code(KeyCode::KeyP) => {
                self.time_of_day.toggle_pause();
            }
            PhysicalKey::Code(KeyCode::BracketLeft) => {
                self.time_of_day.decrease_speed();
            }
            PhysicalKey::Code(KeyCode::BracketRight) => {
                self.time_of_day.increase_speed();
            }
            PhysicalKey::Code(code) => {
                // Hotbar selection (1-9)
                let slot = match code {
                    KeyCode::Digit1 => Some(0),
                    KeyCode::Digit2 => Some(1),
                    KeyCode::Digit3 => Some(2),
                    KeyCode::Digit4 => Some(3),
                    KeyCode::Digit5 => Some(4),
                    KeyCode::Digit6 => Some(5),
                    KeyCode::Digit7 => Some(6),
                    KeyCode::Digit8 => Some(7),
                    KeyCode::Digit9 => Some(8),
                    _ => None,
                };
                if let Some(slot) = slot {
                    self.hotbar.select_slot(slot);
                    let block_name = self.hotbar.block_name(self.hotbar.selected_block());
                    tracing::info!("Selected slot {}: {}", slot + 1, block_name);
                }
            }
            _ => {}
        }
    }

    fn update_and_render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Update time-of-day
        self.time_of_day.update(dt);

        // Update debug HUD
        self.debug_hud.update_fps(dt);
        let camera = self.renderer.camera();
        self.debug_hud.camera_pos = [camera.position.x, camera.position.y, camera.position.z];
        self.debug_hud.camera_rot = [camera.yaw, camera.pitch];
        self.debug_hud.chunks_visible = self.chunks_visible;

        // Update camera from input
        self.update_camera(dt);

        // Raycast for block selection
        if self.input.cursor_grabbed {
            let camera = self.renderer.camera();
            let ray_origin = camera.position;
            let ray_dir = camera.forward();

            self.selected_block = raycast(ray_origin, ray_dir, 8.0, |block_pos| {
                let chunk_x = block_pos.x.div_euclid(16);
                let chunk_z = block_pos.z.div_euclid(16);
                let local_x = block_pos.x.rem_euclid(16) as usize;
                let local_y = block_pos.y as usize;
                let local_z = block_pos.z.rem_euclid(16) as usize;

                if local_y >= 256 {
                    return false;
                }

                if let Some(chunk) = self.chunks.get(&ChunkPos::new(chunk_x, chunk_z)) {
                    let voxel = chunk.voxel(local_x, local_y, local_z);
                    voxel.id != BLOCK_AIR
                } else {
                    false
                }
            });

            // Handle block breaking/placing
            self.handle_block_interaction();
        } else {
            self.selected_block = None;
        }

        // Render
        self.render();
    }

    fn update_camera(&mut self, dt: f32) {
        let camera = self.renderer.camera_mut();

        // Mouse look
        if self.input.cursor_grabbed {
            let sensitivity = 0.002;
            let mouse_delta = self.input.mouse_delta;
            camera.rotate(
                mouse_delta.0 as f32 * sensitivity,
                -mouse_delta.1 as f32 * sensitivity,
            );
        }

        if self.player_physics.physics_enabled {
            // Physics-based movement (simplified from viewer.rs)
            self.player_physics.velocity.y += self.player_physics.gravity * dt;
            if self.player_physics.velocity.y < self.player_physics.terminal_velocity {
                self.player_physics.velocity.y = self.player_physics.terminal_velocity;
            }

            // Basic WASD movement (without full collision for simplicity)
            let move_speed = 4.3;
            let mut horizontal_input = glam::Vec2::ZERO;

            if self.input.is_key_pressed(KeyCode::KeyW) {
                horizontal_input.y += 1.0;
            }
            if self.input.is_key_pressed(KeyCode::KeyS) {
                horizontal_input.y -= 1.0;
            }
            if self.input.is_key_pressed(KeyCode::KeyA) {
                horizontal_input.x -= 1.0;
            }
            if self.input.is_key_pressed(KeyCode::KeyD) {
                horizontal_input.x += 1.0;
            }

            if horizontal_input.length() > 0.0 {
                horizontal_input = horizontal_input.normalize();
                let forward = camera.forward();
                let right = camera.right();

                let forward_h = glam::Vec3::new(forward.x, 0.0, forward.z).normalize();
                let right_h = glam::Vec3::new(right.x, 0.0, right.z).normalize();

                let move_dir = forward_h * horizontal_input.y + right_h * horizontal_input.x;
                camera.position += move_dir * move_speed * dt;
            }

            // Simplified ground check - just stop falling at y=50
            if camera.position.y < 50.0 {
                camera.position.y = 50.0;
                self.player_physics.velocity.y = 0.0;
                self.player_physics.on_ground = true;
            }

            // Jump
            if self.input.is_key_pressed(KeyCode::Space) && self.player_physics.on_ground {
                self.player_physics.velocity.y = self.player_physics.jump_strength;
                self.player_physics.on_ground = false;
            }
        } else {
            // Free fly mode
            let speed = 10.0 * dt;
            let mut movement = glam::Vec3::ZERO;

            if self.input.is_key_pressed(KeyCode::KeyW) {
                movement += camera.forward();
            }
            if self.input.is_key_pressed(KeyCode::KeyS) {
                movement -= camera.forward();
            }
            if self.input.is_key_pressed(KeyCode::KeyA) {
                movement -= camera.right();
            }
            if self.input.is_key_pressed(KeyCode::KeyD) {
                movement += camera.right();
            }
            if self.input.is_key_pressed(KeyCode::Space) {
                movement += glam::Vec3::Y;
            }
            if self.input.is_key_pressed(KeyCode::ShiftLeft) {
                movement -= glam::Vec3::Y;
            }

            if movement.length() > 0.0 {
                camera.translate(movement.normalize() * speed);
            }
        }
    }

    fn handle_block_interaction(&mut self) {
        if let Some(hit) = self.selected_block {
            // Left click: break block
            if self.input.is_mouse_clicked(MouseButton::Left) {
                let chunk_x = hit.block_pos.x.div_euclid(16);
                let chunk_z = hit.block_pos.z.div_euclid(16);
                let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

                if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                    let local_x = hit.block_pos.x.rem_euclid(16) as usize;
                    let local_y = hit.block_pos.y as usize;
                    let local_z = hit.block_pos.z.rem_euclid(16) as usize;

                    chunk.set_voxel(local_x, local_y, local_z, Voxel::default());

                    // Regenerate mesh
                    let mesh = mesh_chunk(chunk, &self.registry);
                    if let Some(resources) = self.renderer.render_resources() {
                        let chunk_bind_group =
                            resources.pipeline.create_chunk_bind_group(resources.device, chunk_pos);
                        self.chunk_manager
                            .add_chunk(resources.device, &mesh, chunk_pos, chunk_bind_group);
                    }
                }
            }

            // Right click: place block
            if self.input.is_mouse_clicked(MouseButton::Right) {
                let place_pos = IVec3::new(
                    hit.block_pos.x + hit.face_normal.x,
                    hit.block_pos.y + hit.face_normal.y,
                    hit.block_pos.z + hit.face_normal.z,
                );

                let chunk_x = place_pos.x.div_euclid(16);
                let chunk_z = place_pos.z.div_euclid(16);
                let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

                if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                    let local_x = place_pos.x.rem_euclid(16) as usize;
                    let local_y = place_pos.y as usize;
                    let local_z = place_pos.z.rem_euclid(16) as usize;

                    if local_y < 256 {
                        let current = chunk.voxel(local_x, local_y, local_z);
                        if current.id == BLOCK_AIR {
                            let new_voxel = Voxel {
                                id: self.hotbar.selected_block(),
                                state: 0,
                                light_sky: 0,
                                light_block: 0,
                            };
                            chunk.set_voxel(local_x, local_y, local_z, new_voxel);

                            // Regenerate mesh
                            let mesh = mesh_chunk(chunk, &self.registry);
                            if let Some(resources) = self.renderer.render_resources() {
                                let chunk_bind_group = resources
                                    .pipeline
                                    .create_chunk_bind_group(resources.device, chunk_pos);
                                self.chunk_manager
                                    .add_chunk(resources.device, &mesh, chunk_pos, chunk_bind_group);
                            }
                        }
                    }
                }
            }
        }
    }

    fn render(&mut self) {
        if let Some(frame) = self.renderer.begin_frame() {
            let resources = self.renderer.render_resources().unwrap();

            // Update time uniforms
            resources
                .skybox_pipeline
                .update_time(resources.queue, &self.time_of_day);
            resources
                .pipeline
                .update_time(resources.queue, &self.time_of_day);

            let mut encoder =
                resources
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

            // Render skybox
            {
                let mut render_pass = resources
                    .skybox_pipeline
                    .begin_render_pass(&mut encoder, &frame.view);
                render_pass.set_pipeline(resources.skybox_pipeline.pipeline());
                render_pass.draw(0..3, 0..1);
            }

            // Create frustum for culling
            let camera = self.renderer.camera();
            let view_proj = camera.projection_matrix() * camera.view_matrix();
            let frustum = Frustum::from_matrix(&view_proj);

            // Render voxels with frustum culling
            self.chunks_visible = 0;
            {
                let mut render_pass =
                    resources.pipeline.begin_render_pass(&mut encoder, &frame.view);

                render_pass.set_pipeline(resources.pipeline.pipeline());
                render_pass.set_bind_group(0, resources.pipeline.camera_bind_group(), &[]);
                render_pass.set_bind_group(2, resources.pipeline.texture_bind_group(), &[]);

                for chunk_data in self.chunk_manager.chunks() {
                    if !frustum.is_chunk_visible(chunk_data.chunk_pos) {
                        continue;
                    }

                    self.chunks_visible += 1;

                    render_pass.set_bind_group(1, &chunk_data.chunk_bind_group, &[]);
                    render_pass.set_vertex_buffer(0, chunk_data.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        chunk_data.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(0..chunk_data.index_count, 0, 0..1);
                }
            }

            // Render wireframe highlight (if block selected)
            if let Some(hit) = self.selected_block {
                let highlight_pos = [
                    hit.block_pos.x as f32,
                    hit.block_pos.y as f32,
                    hit.block_pos.z as f32,
                ];
                let highlight_color = [0.0, 0.0, 0.0, 0.8];
                resources
                    .wireframe_pipeline
                    .update_highlight(resources.queue, highlight_pos, highlight_color);

                let depth_view = resources.pipeline.depth_view();
                let mut render_pass = resources.wireframe_pipeline.begin_render_pass(
                    &mut encoder,
                    &frame.view,
                    depth_view,
                );

                render_pass.set_pipeline(resources.wireframe_pipeline.pipeline());
                render_pass.set_bind_group(0, resources.pipeline.camera_bind_group(), &[]);
                render_pass.set_bind_group(1, resources.wireframe_pipeline.highlight_bind_group(), &[]);
                render_pass.set_vertex_buffer(
                    0,
                    resources.wireframe_pipeline.vertex_buffer().slice(..),
                );
                render_pass.draw(0..24, 0..1);
            }

            // Render UI overlay
            if let Some(mut ui) = self.renderer.ui_mut() {
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
                    &self.window,
                    |ctx| {
                        self.debug_hud.render(ctx);
                        render_hotbar(ctx, &self.hotbar);
                    },
                );
            }

            resources.queue.submit(std::iter::once(encoder.finish()));
            frame.present();
        }

        self.input.reset_frame();
    }
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
                            ui.label(
                                egui::RichText::new(format!("{}", i + 1))
                                    .size(10.0)
                                    .color(if is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::GRAY
                                    }),
                            );
                            ui.label(
                                egui::RichText::new(block_name)
                                    .size(9.0)
                                    .color(if is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::LIGHT_GRAY
                                    }),
                            );
                        });
                    });
                }
            });
        });
}
