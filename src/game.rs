//! Game world state - the actual 3D voxel game

use anyhow::Result;
use glam::IVec3;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_core::{ItemStack, ItemType, ToolMaterial, ToolType};
use mdminecraft_render::{
    mesh_chunk, raycast, ChunkManager, DebugHud, Frustum, InputState, RaycastHit, Renderer,
    RendererConfig, TimeOfDay, WindowConfig, WindowManager,
};
use mdminecraft_ui3d::{Button3D, ButtonState, Text3D, UI3DManager, UIElementHandle, screen_to_ray};
use mdminecraft_world::{BlockId, BlockPropertiesRegistry, Chunk, ChunkPos, TerrainGenerator, Voxel, BLOCK_AIR};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{Event, MouseButton, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::KeyCode;
use winit::window::Window;

use crate::font_utils::find_system_font;

/// Game action to communicate with main state machine
pub enum GameAction {
    /// Continue playing
    Continue,
    /// Return to main menu
    ReturnToMenu,
    /// Quit application
    Quit,
}

/// Hotbar for item selection
struct Hotbar {
    slots: [Option<ItemStack>; 9],
    selected: usize,
}

impl Hotbar {
    fn new() -> Self {
        Self {
            slots: [
                Some(ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood), 1)),
                Some(ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Stone), 1)),
                Some(ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron), 1)),
                Some(ItemStack::new(ItemType::Tool(ToolType::Shovel, ToolMaterial::Wood), 1)),
                Some(ItemStack::new(ItemType::Block(2), 64)), // Dirt
                Some(ItemStack::new(ItemType::Block(3), 64)), // Wood
                Some(ItemStack::new(ItemType::Block(1), 64)), // Stone
                Some(ItemStack::new(ItemType::Block(6), 64)), // Cobblestone
                Some(ItemStack::new(ItemType::Block(7), 64)), // Planks
            ],
            selected: 0,
        }
    }

    fn select_slot(&mut self, slot: usize) {
        if slot < 9 {
            self.selected = slot;
        }
    }

    fn selected_item(&self) -> Option<&ItemStack> {
        self.slots[self.selected].as_ref()
    }

    fn selected_item_mut(&mut self) -> Option<&mut ItemStack> {
        self.slots[self.selected].as_mut()
    }

    /// Get the tool being held (if any)
    fn selected_tool(&self) -> Option<(ToolType, ToolMaterial)> {
        if let Some(item) = self.selected_item() {
            match item.item_type {
                ItemType::Tool(tool_type, material) => Some((tool_type, material)),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Get the block to place (if selected item is a block)
    fn selected_block(&self) -> Option<BlockId> {
        if let Some(item) = self.selected_item() {
            match item.item_type {
                ItemType::Block(block_id) => Some(block_id),
                _ => None,
            }
        } else {
            None
        }
    }

    fn item_name(&self, item_stack: Option<&ItemStack>) -> String {
        if let Some(stack) = item_stack {
            match stack.item_type {
                ItemType::Tool(tool_type, material) => {
                    format!("{:?} {:?}", material, tool_type)
                }
                ItemType::Block(block_id) => self.block_name(block_id).to_string(),
                ItemType::Food(food_type) => format!("{:?}", food_type),
                ItemType::Item(_) => "Item".to_string(),
            }
        } else {
            "Empty".to_string()
        }
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
    /// Previous Y position for fall damage calculation
    last_ground_y: f32,
}

/// Mining progress tracking
struct MiningProgress {
    /// Block position being mined
    block_pos: IVec3,
    /// Time spent mining this block
    time_mining: f32,
    /// Total time required to mine this block
    time_required: f32,
}

/// Player health and survival stats
struct PlayerHealth {
    /// Current health (0-20, measured in half-hearts)
    current: f32,
    /// Maximum health
    max: f32,
    /// Health regeneration rate (hearts per second when full hunger)
    regeneration_rate: f32,
    /// Time since last damage (for regeneration delay)
    time_since_damage: f32,
    /// Invulnerability time after taking damage
    invulnerability_time: f32,
}

impl PlayerHealth {
    fn new() -> Self {
        Self {
            current: 20.0,
            max: 20.0,
            regeneration_rate: 0.0, // Disabled for now, could be 1.0 per second
            time_since_damage: 0.0,
            invulnerability_time: 0.0,
        }
    }

    /// Take damage
    fn damage(&mut self, amount: f32) {
        if self.invulnerability_time > 0.0 {
            return; // Still invulnerable
        }

        self.current = (self.current - amount).max(0.0);
        self.time_since_damage = 0.0;
        self.invulnerability_time = 0.5; // 0.5 second invulnerability

        tracing::info!("Took {:.1} damage, health now {:.1}/20", amount, self.current);
    }

    /// Heal health
    fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }

    /// Check if player is dead
    fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    /// Update health (regeneration, timers)
    fn update(&mut self, dt: f32) {
        self.time_since_damage += dt;

        if self.invulnerability_time > 0.0 {
            self.invulnerability_time -= dt;
        }

        // Regenerate health if enabled and enough time has passed
        if self.regeneration_rate > 0.0 && self.time_since_damage > 3.0 && self.current < self.max {
            self.heal(self.regeneration_rate * dt);
        }
    }

    /// Reset health to full (for respawn)
    fn reset(&mut self) {
        self.current = self.max;
        self.time_since_damage = 0.0;
        self.invulnerability_time = 0.0;
    }
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
            last_ground_y: 100.0, // Initial spawn height
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
    block_properties: BlockPropertiesRegistry,
    input: InputState,
    last_frame: Instant,
    debug_hud: DebugHud,
    time_of_day: TimeOfDay,
    selected_block: Option<RaycastHit>,
    hotbar: Hotbar,
    player_physics: PlayerPhysics,
    player_health: PlayerHealth,
    chunks_visible: usize,
    mining_progress: Option<MiningProgress>,
    spawn_point: glam::Vec3,
    // 3D UI system
    ui_manager: Option<UI3DManager>,
    ui_position_label: Option<UIElementHandle>,
    ui_block_label: Option<UIElementHandle>,
    ui_demo_button: Option<UIElementHandle>,
    ui_hovered_button: Option<UIElementHandle>,
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

        let spawn_point = glam::Vec3::new(0.0, 100.0, 0.0);

        // Initialize 3D UI system
        let ui_manager = {
            let resources = renderer.render_resources().expect("GPU not initialized");

            match find_system_font() {
                Ok(font_path) => {
                    tracing::info!("Loading font from: {}", font_path);

                    // Get the camera bind group layout from the voxel pipeline
                    let camera_layout = resources.pipeline.camera_bind_group_layout();

                    match UI3DManager::with_system_font(
                        resources.device,
                        resources.queue,
                        wgpu::TextureFormat::Bgra8UnormSrgb,
                        camera_layout,
                        &font_path,
                        48.0,
                    ) {
                        Ok(manager) => {
                            tracing::info!("3D UI system initialized");
                            Some(manager)
                        }
                        Err(e) => {
                            tracing::warn!("Failed to initialize 3D UI: {}. Continuing without 3D UI.", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Could not find system font: {}. 3D UI disabled.", e);
                    None
                }
            }
        };

        Ok(Self {
            window,
            renderer,
            chunk_manager,
            chunks,
            registry,
            block_properties: BlockPropertiesRegistry::new(),
            input: InputState::new(),
            last_frame: Instant::now(),
            debug_hud,
            time_of_day: TimeOfDay::new(),
            selected_block: None,
            hotbar: Hotbar::new(),
            player_physics: PlayerPhysics::new(),
            player_health: PlayerHealth::new(),
            chunks_visible: 0,
            mining_progress: None,
            spawn_point,
            ui_manager,
            ui_position_label: None,
            ui_block_label: None,
            ui_demo_button: None,
            ui_hovered_button: None,
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
                    let item_name = self.hotbar.item_name(self.hotbar.selected_item());
                    tracing::info!("Selected slot {}: {}", slot + 1, item_name);
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

        // Update player health
        self.player_health.update(dt);

        // Check for death
        if self.player_health.is_dead() {
            self.handle_death();
        }

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

        // Update 3D UI labels
        self.update_ui_labels();

        // Update demo button
        self.update_demo_button();

        // Handle UI interactions
        self.handle_ui_interaction();

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
            let was_on_ground = self.player_physics.on_ground;
            let current_y = camera.position.y;
            let was_falling = self.player_physics.velocity.y < 0.0;

            if current_y < 50.0 {
                camera.position.y = 50.0;

                // Calculate fall damage if landing
                if !was_on_ground && was_falling {
                    let fall_distance = self.player_physics.last_ground_y - 50.0;
                    let _ = camera; // Release borrow before calling calculate_fall_damage
                    self.calculate_fall_damage(fall_distance);
                }

                self.player_physics.velocity.y = 0.0;
                self.player_physics.on_ground = true;
                self.player_physics.last_ground_y = 50.0;
            } else {
                // In air
                if self.player_physics.on_ground {
                    // Just left the ground, remember this position
                    self.player_physics.last_ground_y = current_y;
                }
                self.player_physics.on_ground = false;
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
            // Left click/hold: mine block
            if self.input.is_mouse_pressed(MouseButton::Left) {
                self.handle_mining(hit);
            } else {
                // Reset mining progress if not holding left click
                self.mining_progress = None;
            }

            // Right click: place block
            if self.input.is_mouse_clicked(MouseButton::Right) {
                self.handle_block_placement(hit);
            }
        } else {
            // No block selected, reset mining progress
            self.mining_progress = None;
        }
    }

    fn handle_mining(&mut self, hit: RaycastHit) {
        let chunk_x = hit.block_pos.x.div_euclid(16);
        let chunk_z = hit.block_pos.z.div_euclid(16);
        let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

        // Get the block we're trying to mine
        let block_id = if let Some(chunk) = self.chunks.get(&chunk_pos) {
            let local_x = hit.block_pos.x.rem_euclid(16) as usize;
            let local_y = hit.block_pos.y as usize;
            let local_z = hit.block_pos.z.rem_euclid(16) as usize;

            if local_y >= 256 {
                return;
            }

            chunk.voxel(local_x, local_y, local_z).id
        } else {
            return;
        };

        // Get block properties
        let block_props = self.block_properties.get(block_id);

        // Check if we're starting to mine a new block
        let mining_new_block = self.mining_progress.as_ref()
            .map(|p| p.block_pos != hit.block_pos)
            .unwrap_or(true);

        if mining_new_block {
            // Calculate mining time based on tool and block properties
            let tool = self.hotbar.selected_tool();
            let mining_time = block_props.calculate_mining_time(tool, false);

            self.mining_progress = Some(MiningProgress {
                block_pos: hit.block_pos,
                time_mining: 0.0,
                time_required: mining_time,
            });

            tracing::debug!(
                "Started mining block {} at {:?} (requires {:.2}s)",
                block_id,
                hit.block_pos,
                mining_time
            );
        }

        // Update mining progress
        if let Some(progress) = &mut self.mining_progress {
            let dt = (Instant::now() - self.last_frame).as_secs_f32();
            progress.time_mining += dt;

            let percent = (progress.time_mining / progress.time_required * 100.0).min(100.0);
            self.debug_hud.mining_progress = Some(percent);

            // Check if mining is complete
            if progress.time_mining >= progress.time_required {
                // Mine the block!
                if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                    let local_x = hit.block_pos.x.rem_euclid(16) as usize;
                    let local_y = hit.block_pos.y as usize;
                    let local_z = hit.block_pos.z.rem_euclid(16) as usize;

                    // Check if tool can harvest this block
                    let tool = self.hotbar.selected_tool();
                    let can_harvest = block_props.can_harvest(tool);

                    if can_harvest {
                        tracing::info!(
                            "Successfully mined block {} at {:?}",
                            block_id,
                            hit.block_pos
                        );
                    } else {
                        tracing::warn!(
                            "Mined block {} but cannot harvest (wrong tool tier)",
                            block_id
                        );
                    }

                    // Remove the block
                    chunk.set_voxel(local_x, local_y, local_z, Voxel::default());

                    // Damage tool durability
                    if let Some(item) = self.hotbar.selected_item_mut() {
                        if matches!(item.item_type, ItemType::Tool(_, _)) {
                            item.damage_durability(1);
                            if item.is_broken() {
                                tracing::info!("Tool broke!");
                                // Remove the broken tool
                                self.hotbar.slots[self.hotbar.selected] = None;
                            }
                        }
                    }

                    // Regenerate mesh
                    let mesh = mesh_chunk(chunk, &self.registry);
                    if let Some(resources) = self.renderer.render_resources() {
                        let chunk_bind_group =
                            resources.pipeline.create_chunk_bind_group(resources.device, chunk_pos);
                        self.chunk_manager
                            .add_chunk(resources.device, &mesh, chunk_pos, chunk_bind_group);
                    }
                }

                // Reset mining progress
                self.mining_progress = None;
                self.debug_hud.mining_progress = None;
            }
        }
    }

    fn handle_block_placement(&mut self, hit: RaycastHit) {
        // Only place if we have a block selected
        if let Some(block_id) = self.hotbar.selected_block() {
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
                            id: block_id,
                            state: 0,
                            light_sky: 0,
                            light_block: 0,
                        };
                        chunk.set_voxel(local_x, local_y, local_z, new_voxel);

                        // Decrease block count
                        if let Some(item) = self.hotbar.selected_item_mut() {
                            if item.count > 0 {
                                item.count -= 1;
                                if item.count == 0 {
                                    self.hotbar.slots[self.hotbar.selected] = None;
                                }
                            }
                        }

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

            // Render 3D UI elements (after voxels, before wireframe)
            if let Some(ui_manager) = &self.ui_manager {
                let depth_view = resources.pipeline.depth_view();
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("3D UI Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                ui_manager.render(&mut render_pass, resources.pipeline.camera_bind_group());
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
                        render_health_bar(ctx, &self.player_health);
                    },
                );
            }

            resources.queue.submit(std::iter::once(encoder.finish()));
            frame.present();
        }

        self.input.reset_frame();
    }

    /// Calculate and apply fall damage
    fn calculate_fall_damage(&mut self, fall_distance: f32) {
        // Minecraft formula: damage = fall_distance - 3.0
        // Player takes damage for falls > 3 blocks
        if fall_distance > 3.0 {
            let damage = (fall_distance - 3.0) * 1.0; // 1 damage per block fallen
            self.player_health.damage(damage);
            tracing::info!("Fell {:.1} blocks, took {:.1} fall damage", fall_distance, damage);
        }
    }

    /// Update 3D UI labels
    fn update_ui_labels(&mut self) {
        if let Some(ui_manager) = &mut self.ui_manager {
            let resources = self.renderer.render_resources().expect("GPU not initialized");
            let camera = self.renderer.camera();

            // Position label - floating above player
            let label_pos = camera.position + glam::Vec3::new(0.0, 3.0, 0.0);
            let position_text = format!(
                "X: {:.1}  Y: {:.1}  Z: {:.1}",
                camera.position.x, camera.position.y, camera.position.z
            );

            if let Some(handle) = self.ui_position_label {
                ui_manager.set_text_content(resources.device, handle, position_text);
                ui_manager.set_text_position(resources.device, handle, label_pos);
            } else {
                let text = Text3D::new(label_pos, position_text)
                    .with_font_size(0.3)
                    .with_color([1.0, 1.0, 0.0, 1.0])
                    .with_billboard(true);
                self.ui_position_label = Some(ui_manager.add_text(resources.device, text));
            }

            // Block info label - show when looking at a block
            if let Some(hit) = self.selected_block {
                let block_pos = hit.block_pos;
                let block_label_pos = glam::Vec3::new(
                    block_pos.x as f32 + 0.5,
                    block_pos.y as f32 + 1.5,
                    block_pos.z as f32 + 0.5,
                );

                // Get block info
                let chunk_x = block_pos.x.div_euclid(16);
                let chunk_z = block_pos.z.div_euclid(16);
                let local_x = block_pos.x.rem_euclid(16) as usize;
                let local_y = block_pos.y as usize;
                let local_z = block_pos.z.rem_euclid(16) as usize;

                if let Some(chunk) = self.chunks.get(&ChunkPos::new(chunk_x, chunk_z)) {
                    if local_y < 256 {
                        let voxel = chunk.voxel(local_x, local_y, local_z);
                        let block_name = self.hotbar.block_name(voxel.id);
                        let block_text = format!("{}\n({}, {}, {})", block_name, block_pos.x, block_pos.y, block_pos.z);

                        if let Some(handle) = self.ui_block_label {
                            ui_manager.set_text_content(resources.device, handle, block_text);
                            ui_manager.set_text_position(resources.device, handle, block_label_pos);
                        } else {
                            let text = Text3D::new(block_label_pos, block_text)
                                .with_font_size(0.25)
                                .with_color([0.5, 1.0, 1.0, 1.0])
                                .with_billboard(true);
                            self.ui_block_label = Some(ui_manager.add_text(resources.device, text));
                        }
                    }
                }
            } else if let Some(handle) = self.ui_block_label {
                // Hide block label when not looking at a block
                ui_manager.remove_text(handle);
                self.ui_block_label = None;
            }
        }
    }

    /// Create and manage 3D UI demo button
    fn update_demo_button(&mut self) {
        if let Some(ui_manager) = &mut self.ui_manager {
            let resources = self.renderer.render_resources().expect("GPU not initialized");
            let camera = self.renderer.camera();

            // Create demo button if it doesn't exist
            if self.ui_demo_button.is_none() {
                let button_pos = camera.position + camera.forward() * 5.0;
                let button = Button3D::new(button_pos, "Click Me!")
                    .with_size(2.0, 0.6)
                    .with_font_size(0.35)
                    .with_callback(1) // ID 1 for this button
                    .with_billboard(true);

                self.ui_demo_button = Some(ui_manager.add_button(resources.device, button));
                tracing::info!("Created demo 3D button");
            }
        }
    }

    /// Handle UI interactions (hover, click)
    fn handle_ui_interaction(&mut self) {
        if let Some(ui_manager) = &mut self.ui_manager {
            let resources = self.renderer.render_resources().expect("GPU not initialized");
            let camera = self.renderer.camera();

            // Get mouse position (center of screen for now since cursor is grabbed)
            let screen_size = (1280, 720);
            let mouse_pos = if self.input.cursor_grabbed {
                // Center of screen when cursor is grabbed
                (screen_size.0 as f32 / 2.0, screen_size.1 as f32 / 2.0)
            } else {
                // TODO: Get actual mouse position when cursor is not grabbed
                (screen_size.0 as f32 / 2.0, screen_size.1 as f32 / 2.0)
            };

            // Convert screen position to ray
            let view_matrix = camera.view_matrix();
            let proj_matrix = camera.projection_matrix();
            let (ray_origin, ray_dir) = screen_to_ray(mouse_pos, screen_size, &view_matrix, &proj_matrix);

            // Raycast against buttons
            if let Some((handle, _hit)) = ui_manager.raycast_buttons(ray_origin, ray_dir, camera.position) {
                // Set hover state
                if self.ui_hovered_button != Some(handle) {
                    // Reset previous hovered button
                    if let Some(prev_handle) = self.ui_hovered_button {
                        ui_manager.set_button_state(resources.device, prev_handle, ButtonState::Normal);
                    }

                    // Set new hovered button
                    ui_manager.set_button_state(resources.device, handle, ButtonState::Hover);
                    self.ui_hovered_button = Some(handle);
                    tracing::debug!("Hovering button {}", handle);
                }

                // Handle click
                if self.input.is_mouse_pressed(MouseButton::Left) {
                    ui_manager.set_button_state(resources.device, handle, ButtonState::Pressed);
                    if let Some(callback_id) = ui_manager.get_button_callback(handle) {
                        tracing::info!("Button {} clicked! Callback ID: {}", handle, callback_id);
                        // Handle button action based on callback_id
                        match callback_id {
                            1 => {
                                tracing::info!("Demo button was clicked!");
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                // No button hovered, reset hover state
                if let Some(prev_handle) = self.ui_hovered_button {
                    ui_manager.set_button_state(resources.device, prev_handle, ButtonState::Normal);
                    self.ui_hovered_button = None;
                }
            }
        }
    }

    /// Handle player death
    fn handle_death(&mut self) {
        tracing::info!("Player died! Respawning at spawn point...");

        // Respawn player at spawn point
        let camera = self.renderer.camera_mut();
        camera.position = self.spawn_point;

        // Reset health
        self.player_health.reset();

        // Reset physics
        self.player_physics.velocity = glam::Vec3::ZERO;
        self.player_physics.on_ground = false;
        self.player_physics.last_ground_y = self.spawn_point.y;

        // TODO: Drop inventory items, show death screen, etc.
    }
}

fn render_hotbar(ctx: &egui::Context, hotbar: &Hotbar) {
    egui::Area::new(egui::Id::new("hotbar"))
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -20.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for i in 0..9 {
                    let is_selected = i == hotbar.selected;
                    let item_stack = hotbar.slots[i].as_ref();
                    let item_name = hotbar.item_name(item_stack);

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
                        ui.set_min_size(egui::vec2(60.0, 60.0));
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
                                egui::RichText::new(&item_name)
                                    .size(8.0)
                                    .color(if is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::LIGHT_GRAY
                                    }),
                            );

                            // Show count or durability
                            if let Some(stack) = item_stack {
                                match stack.item_type {
                                    ItemType::Tool(_, _) => {
                                        if let Some(durability) = stack.durability {
                                            let max_durability = stack.max_durability().unwrap_or(1);
                                            let durability_percent = (durability as f32 / max_durability as f32 * 100.0) as u32;
                                            let color = if durability_percent < 20 {
                                                egui::Color32::RED
                                            } else if durability_percent < 50 {
                                                egui::Color32::YELLOW
                                            } else {
                                                egui::Color32::GREEN
                                            };
                                            ui.label(
                                                egui::RichText::new(format!("{}%", durability_percent))
                                                    .size(7.0)
                                                    .color(color),
                                            );
                                        }
                                    }
                                    ItemType::Block(_) | ItemType::Item(_) | ItemType::Food(_) => {
                                        if stack.count > 1 {
                                            ui.label(
                                                egui::RichText::new(format!("x{}", stack.count))
                                                    .size(9.0)
                                                    .color(egui::Color32::WHITE),
                                            );
                                        }
                                    }
                                }
                            }
                        });
                    });
                }
            });
        });
}

fn render_health_bar(ctx: &egui::Context, health: &PlayerHealth) {
    egui::Area::new(egui::Id::new("health_bar"))
        .anchor(egui::Align2::LEFT_BOTTOM, [10.0, -70.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Draw hearts
                let max_hearts = (health.max / 2.0) as i32; // 20 health = 10 hearts
                let current_hearts = health.current / 2.0;

                for i in 0..max_hearts {
                    let heart_value = current_hearts - i as f32;
                    let (symbol, color) = if heart_value >= 1.0 {
                        ("♥", egui::Color32::from_rgb(255, 0, 0)) // Full heart - red
                    } else if heart_value >= 0.5 {
                        ("♥", egui::Color32::from_rgb(200, 0, 0)) // Half heart - darker red
                    } else {
                        ("♡", egui::Color32::from_rgb(100, 100, 100)) // Empty heart - gray
                    };

                    ui.label(egui::RichText::new(symbol).size(20.0).color(color));
                }

                // Show numerical health
                ui.add_space(10.0);
                let health_text = format!("{:.1}/{:.0}", health.current, health.max);
                let text_color = if health.current < 6.0 {
                    egui::Color32::RED
                } else if health.current < 10.0 {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::WHITE
                };
                ui.label(egui::RichText::new(health_text).size(14.0).color(text_color));
            });
        });
}
