//! Game world state - the actual 3D voxel game

use crate::{
    config::{load_block_registry, ControlsConfig},
    input::{ActionState, InputProcessor},
    scripted_input::ScriptedInputPlayer,
};
use anyhow::Result;
use glam::IVec3;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_core::{ItemStack, ItemType, ToolMaterial, ToolType};
use mdminecraft_render::{
    mesh_chunk, raycast, ChunkManager, ControlMode, DebugHud, Frustum, InputContext, InputState,
    ParticleEmitter, ParticleSystem, ParticleVertex, RaycastHit, Renderer, RendererConfig,
    TimeOfDay, UiRenderContext, WindowConfig, WindowManager,
};
#[cfg(feature = "ui3d_billboards")]
use mdminecraft_ui3d::render::{
    BillboardEmitter, BillboardFlags, BillboardInstance, BillboardRenderer,
};
use mdminecraft_world::{
    lighting::{init_skylight, stitch_light_seams, LightType},
    BlockId, BlockPropertiesRegistry, Chunk, ChunkPos, TerrainGenerator, Voxel, WeatherState,
    WeatherToggle, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::Instant;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

const MAX_PARTICLES: usize = 8_192;
const PRECIPITATION_SPAWN_RATE: f32 = 480.0;
const PRECIPITATION_RADIUS: f32 = 18.0;
const PRECIPITATION_CEILING_OFFSET: f32 = 12.0;
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

/// Hotbar for item selection
struct Hotbar {
    slots: [Option<ItemStack>; 9],
    selected: usize,
}

impl Hotbar {
    fn new() -> Self {
        Self {
            slots: [
                Some(ItemStack::new(
                    ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood),
                    1,
                )),
                Some(ItemStack::new(
                    ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Stone),
                    1,
                )),
                Some(ItemStack::new(
                    ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron),
                    1,
                )),
                Some(ItemStack::new(
                    ItemType::Tool(ToolType::Shovel, ToolMaterial::Wood),
                    1,
                )),
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

    fn scroll(&mut self, delta: i32) {
        let len = self.slots.len() as i32;
        if len == 0 {
            return;
        }
        let mut idx = self.selected as i32 + delta;
        idx = ((idx % len) + len) % len;
        self.selected = idx as usize;
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
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy)]
struct AABB {
    min: glam::Vec3,
    max: glam::Vec3,
}

impl AABB {
    fn from_center_size(center: glam::Vec3, size: glam::Vec3) -> Self {
        let half_size = size * 0.5;
        Self {
            min: center - half_size,
            max: center + half_size,
        }
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

/// Helper: given time slices, return how many frames are needed to finish mining.
fn frames_to_complete(time_required: f32, dt_slices: &[f32]) -> usize {
    let mut acc = 0.0;
    for (i, dt) in dt_slices.iter().enumerate() {
        acc += *dt;
        if acc >= time_required {
            return i + 1;
        }
    }
    dt_slices.len()
}

#[cfg(test)]
mod tests {
    use super::frames_to_complete;

    #[test]
    fn mining_completion_is_fps_independent() {
        let required = 1.2_f32; // seconds
        let frames_60hz: Vec<f32> = std::iter::repeat(1.0 / 60.0).take(200).collect();
        let frames_30hz: Vec<f32> = std::iter::repeat(1.0 / 30.0).take(200).collect();

        let f60 = frames_to_complete(required, &frames_60hz);
        let f30 = frames_to_complete(required, &frames_30hz);

        let t60 = (f60 as f32) * (1.0 / 60.0);
        let t30 = (f30 as f32) * (1.0 / 30.0);

        assert!(t60 >= required - 1e-3);
        assert!(t30 >= required - 1e-3);
    }
}

/// Runtime particle instance stored on the CPU before uploading to the GPU each frame.
struct ParticleInstance {
    position: glam::Vec3,
    velocity: glam::Vec3,
    color: glam::Vec4,
    lifetime: f32,
    age: f32,
    scale: f32,
    gravity: f32,
}

impl ParticleInstance {
    fn new(
        position: glam::Vec3,
        velocity: glam::Vec3,
        color: glam::Vec4,
        lifetime: f32,
        scale: f32,
        gravity: f32,
    ) -> Self {
        Self {
            position,
            velocity,
            color,
            lifetime,
            age: 0.0,
            scale,
            gravity,
        }
    }

    fn update(&mut self, dt: f32) {
        self.velocity.y -= self.gravity * dt;
        self.position += self.velocity * dt;
        self.age += dt;
    }

    fn remaining(&self) -> f32 {
        (self.lifetime - self.age).max(0.0)
    }

    fn is_alive(&self) -> bool {
        self.age < self.lifetime
    }

    fn to_vertex(&self) -> ParticleVertex {
        let mut color = self.color;
        color.w *= (self.remaining() / self.lifetime).clamp(0.0, 1.0);
        ParticleVertex {
            position: self.position.to_array(),
            color: color.to_array(),
            lifetime: self.remaining(),
            scale: self.scale,
        }
    }
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

        tracing::info!(
            "Took {:.1} damage, health now {:.1}/20",
            amount,
            self.current
        );
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
    controls: Arc<ControlsConfig>,
    input_processor: InputProcessor,
    #[allow(dead_code)]
    actions: ActionState,
    scripted_input: Option<ScriptedInputPlayer>,
    particle_emitter: ParticleEmitter,
    particles: Vec<ParticleInstance>,
    weather: WeatherToggle,
    weather_timer: f32,
    next_weather_change: f32,
    precipitation_accumulator: f32,
    rng: StdRng,
    weather_blend: f32,
    /// Last frame delta time (seconds)
    frame_dt: f32,
    #[cfg(feature = "ui3d_billboards")]
    billboard_renderer: Option<BillboardRenderer>,
    #[cfg(feature = "ui3d_billboards")]
    billboard_emitter: BillboardEmitter,
}

impl GameWorld {
    /// Create a new game world
    pub fn new(
        event_loop: &EventLoopWindowTarget<()>,
        controls: Arc<ControlsConfig>,
        scripted_input_path: Option<PathBuf>,
    ) -> Result<Self> {
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
        #[cfg(feature = "ui3d_billboards")]
        let billboard_renderer = {
            let resources = renderer.render_resources().expect("GPU not initialized");
            let format = renderer
                .surface_format()
                .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
            match BillboardRenderer::new(
                resources.device,
                format,
                resources.pipeline.camera_bind_group_layout(),
                resources.pipeline.atlas_view(),
                resources.pipeline.atlas_sampler(),
            ) {
                Ok(r) => Some(r),
                Err(err) => {
                    tracing::warn!(?err, "Failed to initialize billboard renderer");
                    None
                }
            }
        };

        // Generate world
        let registry = load_block_registry();
        let world_seed = 12345u64;
        let generator = TerrainGenerator::new(world_seed);

        let mut chunk_manager = ChunkManager::new();
        let mut chunks = HashMap::new();
        let chunk_radius = 2;
        let mut total_vertices = 0;
        let mut total_indices = 0;
        let mut rng = StdRng::seed_from_u64(world_seed ^ 0x5eed_a11c);

        {
            let resources = renderer.render_resources().expect("GPU not initialized");

            for x in -chunk_radius..=chunk_radius {
                for z in -chunk_radius..=chunk_radius {
                    let chunk_pos = ChunkPos::new(x, z);
                    let mut chunk = generator.generate_chunk(chunk_pos);
                    let _ = init_skylight(&mut chunk, &registry);
                    let mesh = mesh_chunk(&chunk, &registry, renderer.atlas_metadata());

                    total_vertices += mesh.vertices.len();
                    total_indices += mesh.indices.len();

                    let chunk_bind_group = resources
                        .pipeline
                        .create_chunk_bind_group(resources.device, chunk_pos);

                    chunk_manager.add_chunk(resources.device, &mesh, chunk_pos, chunk_bind_group);
                    chunks.insert(chunk_pos, chunk);
                }
            }
        }

        // Stitch skylight across chunk seams for the generated region.
        let chunk_positions: Vec<_> = chunks.keys().copied().collect();
        for pos in chunk_positions {
            let _ = stitch_light_seams(&mut chunks, &registry, pos, LightType::Skylight);
        }

        tracing::info!(
            "Generated {} chunks ({} vertices, {} indices)",
            chunk_manager.chunk_count(),
            total_vertices,
            total_indices
        );

        // Determine spawn point near world origin so the player doesn't start mid-air.
        let spawn_point = Self::determine_spawn_point(&chunks, &registry)
            .unwrap_or_else(|| glam::Vec3::new(0.0, 100.0, 0.0));

        // Setup camera
        renderer.camera_mut().position = spawn_point;
        renderer.camera_mut().yaw = 0.0;
        renderer.camera_mut().pitch = -0.3;

        // Setup state
        let mut debug_hud = DebugHud::new();
        debug_hud.chunks_loaded = chunk_manager.chunk_count();
        debug_hud.total_vertices = total_vertices;
        debug_hud.total_triangles = total_indices / 3;

        let input = InputState::new();
        let input_processor = InputProcessor::new(&controls);
        let scripted_input = scripted_input_path
            .as_ref()
            .map(|path| ScriptedInputPlayer::from_path(path))
            .transpose()?;

        let mut world = Self {
            window,
            renderer,
            chunk_manager,
            chunks,
            registry,
            block_properties: BlockPropertiesRegistry::new(),
            input,
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
            controls,
            input_processor,
            actions: ActionState::default(),
            scripted_input,
            particle_emitter: ParticleEmitter::new(),
            particles: Vec::new(),
            weather: WeatherToggle::new(),
            weather_timer: 0.0,
            next_weather_change: rng.gen_range(45.0..120.0),
            precipitation_accumulator: 0.0,
            rng,
            weather_blend: 0.0,
            frame_dt: 0.0,
            #[cfg(feature = "ui3d_billboards")]
            billboard_renderer,
            #[cfg(feature = "ui3d_billboards")]
            billboard_emitter: BillboardEmitter::default(),
        };

        world.player_physics.last_ground_y = spawn_point.y;

        let _ = world.input.enter_gameplay(&world.window);

        Ok(world)
    }

    fn column_ground_height(
        chunks: &HashMap<ChunkPos, Chunk>,
        registry: &BlockRegistry,
        world_x: f32,
        world_z: f32,
    ) -> f32 {
        let block_x = world_x.floor() as i32;
        let block_z = world_z.floor() as i32;
        let chunk_x = block_x.div_euclid(CHUNK_SIZE_X as i32);
        let chunk_z = block_z.div_euclid(CHUNK_SIZE_Z as i32);
        let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
        if let Some(chunk) = chunks.get(&chunk_pos) {
            let local_x = block_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
            let local_z = block_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
            for y in (0..CHUNK_SIZE_Y).rev() {
                let voxel = chunk.voxel(local_x, y, local_z);
                if voxel.id != BLOCK_AIR
                    && registry
                        .descriptor(voxel.id)
                        .map(|d| d.opaque)
                        .unwrap_or(true)
                {
                    return y as f32 + 1.0;
                }
            }
        }
        50.0
    }

    fn determine_spawn_point(
        chunks: &HashMap<ChunkPos, Chunk>,
        registry: &BlockRegistry,
    ) -> Option<glam::Vec3> {
        let origin = ChunkPos::new(0, 0);
        let chunk = chunks.get(&origin)?;
        let base_x = origin.x * CHUNK_SIZE_X as i32;
        let base_z = origin.z * CHUNK_SIZE_Z as i32;
        let mut best: Option<(i32, i32, usize)> = None;

        for local_z in 0..CHUNK_SIZE_Z {
            for local_x in 0..CHUNK_SIZE_X {
                for y in (0..CHUNK_SIZE_Y).rev() {
                    let voxel = chunk.voxel(local_x, y, local_z);
                    if voxel.id != BLOCK_AIR
                        && registry
                            .descriptor(voxel.id)
                            .map(|d| d.opaque)
                            .unwrap_or(true)
                    {
                        let world_x = base_x + local_x as i32;
                        let world_z = base_z + local_z as i32;
                        if best.is_none_or(|(_, _, best_y)| y > best_y) {
                            best = Some((world_x, world_z, y));
                        }
                        break;
                    }
                }
            }
        }

        best.map(|(world_x, world_z, y)| {
            glam::Vec3::new(world_x as f32 + 0.5, y as f32 + 1.8, world_z as f32 + 0.5)
        })
    }

    /// Handle an event
    pub fn handle_event(
        &mut self,
        event: &Event<()>,
        _event_loop: &EventLoopWindowTarget<()>,
    ) -> GameAction {
        // Let UI handle events first
        if let Event::WindowEvent { ref event, .. } = event {
            if let Some(mut ui) = self.renderer.ui_mut() {
                ui.handle_event(&self.window, event);
            }
            self.input.handle_event(event);
        }

        if let Event::DeviceEvent { ref event, .. } = event {
            self.input.handle_device_event(event);
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
                                let _ = self.input.enter_menu(&self.window);
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
            PhysicalKey::Code(KeyCode::KeyO) => {
                self.weather.toggle();
                self.weather_timer = 0.0;
                self.next_weather_change = self.rng.gen_range(45.0..120.0);
                tracing::info!(state = ?self.weather.state, "Weather toggled");
            }
            _ => {}
        }
    }

    fn process_actions(&mut self, dt: f32) {
        let actions = if let Some(player) = self.scripted_input.as_mut() {
            player.advance(dt)
        } else {
            let snapshot = self.input.snapshot_view();
            self.input_processor.process(&snapshot)
        };
        self.apply_actions(&actions);
        self.actions = actions;
    }

    fn apply_actions(&mut self, actions: &ActionState) {
        if actions.toggle_cursor {
            if self.input.cursor_captured {
                let _ = self.input.enter_ui_overlay(&self.window);
            } else {
                let _ = self.input.enter_gameplay(&self.window);
            }
        }

        if actions.toggle_fly {
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

        if actions.context != InputContext::Gameplay {
            return;
        }

        if let Some(slot) = actions.hotbar_slot {
            self.hotbar.select_slot(slot as usize);
        } else if actions.hotbar_scroll != 0 {
            self.hotbar.scroll(actions.hotbar_scroll);
        }
    }

    fn weather_intensity(&self) -> f32 {
        self.weather_blend
    }

    fn weather_state_label(&self) -> &'static str {
        match self.weather.state {
            WeatherState::Clear => "Clear",
            WeatherState::Precipitation => "Precipitation",
        }
    }

    fn update_weather(&mut self, dt: f32) {
        self.weather_timer += dt;
        if self.weather_timer >= self.next_weather_change {
            self.weather_timer = 0.0;
            self.next_weather_change = self.rng.gen_range(60.0..150.0);

            let target_state = if self.weather.is_precipitating() {
                if self.rng.gen_bool(0.7) {
                    WeatherState::Clear
                } else {
                    WeatherState::Precipitation
                }
            } else if self.rng.gen_bool(0.55) {
                WeatherState::Precipitation
            } else {
                WeatherState::Clear
            };

            if target_state != self.weather.state {
                let previous = self.weather.state;
                self.weather.set_state(target_state);
                tracing::info!(?previous, ?target_state, "Weather changed");
            }
        }

        if self.weather.is_precipitating() {
            self.spawn_precipitation_particles(dt);
        } else {
            self.precipitation_accumulator = 0.0;
        }

        // Smoothly blend precipitation intensity for shaders/HUD
        let target = if self.weather.is_precipitating() {
            1.0
        } else {
            0.0
        };
        let smoothing = 1.0 - (-dt * 3.0).exp();
        self.weather_blend += (target - self.weather_blend) * smoothing;
    }

    fn spawn_precipitation_particles(&mut self, dt: f32) {
        let camera_pos = self.renderer.camera().position;
        self.precipitation_accumulator += PRECIPITATION_SPAWN_RATE * dt;
        let is_snow = camera_pos.y > 90.0;

        while self.precipitation_accumulator >= 1.0 {
            self.precipitation_accumulator -= 1.0;

            let offset_x = self
                .rng
                .gen_range(-PRECIPITATION_RADIUS..PRECIPITATION_RADIUS);
            let offset_z = self
                .rng
                .gen_range(-PRECIPITATION_RADIUS..PRECIPITATION_RADIUS);
            let spawn_height =
                camera_pos.y + PRECIPITATION_CEILING_OFFSET + self.rng.gen_range(0.0..4.0);
            let position = glam::Vec3::new(
                camera_pos.x + offset_x,
                spawn_height,
                camera_pos.z + offset_z,
            );

            let wind = glam::Vec3::new(
                self.rng.gen_range(-2.0..2.0),
                0.0,
                self.rng.gen_range(-2.0..2.0),
            );

            let (velocity, color, lifetime, scale, gravity) = if is_snow {
                (
                    glam::Vec3::new(wind.x * 0.4, -5.0, wind.z * 0.4),
                    glam::Vec4::new(0.95, 0.97, 1.0, 0.9),
                    self.rng.gen_range(2.5..3.5),
                    self.rng.gen_range(6.0..10.0),
                    6.0,
                )
            } else {
                (
                    glam::Vec3::new(wind.x * 0.2, -28.0, wind.z * 0.2),
                    glam::Vec4::new(0.5, 0.65, 1.0, 0.6),
                    self.rng.gen_range(1.0..1.6),
                    self.rng.gen_range(5.0..7.0),
                    38.0,
                )
            };

            self.particles.push(ParticleInstance::new(
                position, velocity, color, lifetime, scale, gravity,
            ));
        }

        self.enforce_particle_budget();
    }

    fn spawn_block_break_particles(&mut self, block_center: glam::Vec3, block_id: BlockId) {
        if block_id == BLOCK_AIR {
            return;
        }

        let color = self.block_color(block_id);
        for _ in 0..24 {
            let offset = glam::Vec3::new(
                self.rng.gen_range(-0.45..0.45),
                self.rng.gen_range(-0.45..0.45),
                self.rng.gen_range(-0.45..0.45),
            );
            let velocity = glam::Vec3::new(
                self.rng.gen_range(-2.2..2.2),
                self.rng.gen_range(1.6..4.0),
                self.rng.gen_range(-2.2..2.2),
            );
            let lifetime = self.rng.gen_range(0.6..1.2);
            let scale = self.rng.gen_range(5.0..9.0);
            self.particles.push(ParticleInstance::new(
                block_center + offset,
                velocity,
                color,
                lifetime,
                scale,
                14.0,
            ));
        }

        self.enforce_particle_budget();
    }

    fn block_color(&self, block_id: BlockId) -> glam::Vec4 {
        if let Some(descriptor) = self.registry.descriptor(block_id) {
            match descriptor.name.as_str() {
                "stone" => glam::Vec4::new(0.6, 0.62, 0.66, 1.0),
                "dirt" => glam::Vec4::new(0.42, 0.28, 0.16, 1.0),
                "grass" => glam::Vec4::new(0.32, 0.62, 0.2, 1.0),
                "sand" => glam::Vec4::new(0.93, 0.86, 0.64, 1.0),
                "gravel" => glam::Vec4::new(0.55, 0.55, 0.55, 1.0),
                "water" => glam::Vec4::new(0.25, 0.35, 0.9, 0.5),
                "ice" => glam::Vec4::new(0.66, 0.82, 1.0, 0.8),
                "snow" => glam::Vec4::new(0.95, 0.95, 1.0, 1.0),
                "clay" => glam::Vec4::new(0.68, 0.64, 0.72, 1.0),
                "bedrock" => glam::Vec4::new(0.3, 0.3, 0.3, 1.0),
                _ => glam::Vec4::new(0.8, 0.8, 0.8, 1.0),
            }
        } else {
            glam::Vec4::new(0.8, 0.8, 0.8, 1.0)
        }
    }

    fn update_particles(&mut self, dt: f32) {
        for particle in &mut self.particles {
            particle.update(dt);
        }
        self.particles.retain(|particle| particle.is_alive());
    }

    fn enforce_particle_budget(&mut self) {
        if self.particles.len() > MAX_PARTICLES {
            let overflow = self.particles.len() - MAX_PARTICLES;
            self.particles.drain(0..overflow);
        }
    }

    fn populate_particle_emitter(&mut self) {
        self.particle_emitter.clear();
        for particle in &self.particles {
            if !particle.is_alive() {
                continue;
            }
            self.particle_emitter.spawn(particle.to_vertex());
        }
        self.debug_hud.particle_count = self.particle_emitter.vertices.len();
        self.debug_hud.particle_budget = MAX_PARTICLES;
    }

    #[cfg(feature = "ui3d_billboards")]
    fn populate_billboards(&mut self) {
        self.billboard_emitter.clear();

        if let Some(hit) = self.selected_block {
            let pos = glam::Vec3::new(
                hit.block_pos.x as f32 + 0.5,
                hit.block_pos.y as f32 + 0.5,
                hit.block_pos.z as f32 + 0.5,
            );

            self.billboard_emitter.submit(
                1,
                BillboardInstance {
                    position: pos.to_array(),
                    size: [0.6, 0.6],
                    color: [1.0, 1.0, 1.0, 0.35],
                    flags: (BillboardFlags::OVERLAY_NO_DEPTH | BillboardFlags::EMISSIVE).bits(),
                    ..Default::default()
                },
            );
        }
    }

    fn current_control_mode(&self) -> ControlMode {
        match self.actions.context {
            InputContext::Gameplay => {
                if self.player_physics.physics_enabled {
                    ControlMode::GameplayPhysics
                } else {
                    ControlMode::GameplayFly
                }
            }
            InputContext::UiOverlay => ControlMode::UiOverlay,
            InputContext::Menu => ControlMode::Menu,
        }
    }

    fn apply_mouse_look(
        camera: &mut mdminecraft_render::Camera,
        actions: &ActionState,
        controls: &ControlsConfig,
        cursor_captured: bool,
    ) {
        if !cursor_captured || actions.context != InputContext::Gameplay {
            return;
        }

        let mut look = if actions.look_delta.0.abs() > f32::EPSILON
            || actions.look_delta.1.abs() > f32::EPSILON
        {
            actions.look_delta
        } else {
            actions.raw_look_delta
        };

        let sensitivity = controls.mouse_sensitivity.max(0.0001);
        look.0 *= sensitivity;
        let invert = if controls.invert_y { -1.0 } else { 1.0 };
        look.1 = look.1 * sensitivity * invert;

        if look.0 != 0.0 || look.1 != 0.0 {
            camera.rotate(-look.0, -look.1);
        }
    }

    fn apply_physics_movement(&mut self, actions: &ActionState, dt: f32) {
        let mut position = self.renderer.camera().position;
        let yaw = self.renderer.camera().yaw;
        let forward_h = glam::Vec3::new(yaw.cos(), 0.0, yaw.sin()).normalize_or_zero();
        let right_h = glam::Vec3::new(-forward_h.z, 0.0, forward_h.x).normalize_or_zero();

        let mut fall_damage: Option<f32> = None;

        {
            let physics = &mut self.player_physics;
            physics.velocity.y += physics.gravity * dt;
            if physics.velocity.y < physics.terminal_velocity {
                physics.velocity.y = physics.terminal_velocity;
            }

            let mut axis = glam::Vec2::new(actions.move_x, actions.move_y);
            if axis.length_squared() > 1.0 {
                axis = axis.normalize();
            }

            let mut move_speed = if actions.sprint { 6.0 } else { 4.3 };
            if actions.crouch {
                move_speed *= 0.5;
            }

            if axis.length_squared() > 0.0 {
                let move_dir = forward_h * axis.y + right_h * axis.x;
                position += move_dir * move_speed * dt;
            }

            position.y += physics.velocity.y * dt;

            let was_on_ground = physics.on_ground;
            let was_falling = physics.velocity.y < 0.0;
            let player_aabb = physics.get_aabb(position);
            let feet_y = player_aabb.min.y;
            let head_y = player_aabb.max.y;
            let ground_y =
                Self::column_ground_height(&self.chunks, &self.registry, position.x, position.z);

            if feet_y < ground_y {
                let correction = ground_y - feet_y;
                position.y += correction;

                if !was_on_ground && was_falling {
                    fall_damage = Some(physics.last_ground_y - ground_y);
                }

                physics.velocity.y = 0.0;
                physics.on_ground = true;
                physics.last_ground_y = head_y;
            } else {
                if physics.on_ground {
                    physics.last_ground_y = head_y;
                }
                physics.on_ground = false;
            }

            if actions.jump && physics.on_ground {
                physics.velocity.y = physics.jump_strength;
                physics.on_ground = false;
            }
        }

        if let Some(dist) = fall_damage {
            self.calculate_fall_damage(dist);
        }

        self.renderer.camera_mut().position = position;
    }

    fn apply_fly_movement(&mut self, actions: &ActionState, dt: f32) {
        let (forward, right, mut position) = {
            let camera = self.renderer.camera();
            (camera.forward(), camera.right(), camera.position)
        };

        let mut movement = glam::Vec3::ZERO;
        if actions.move_y.abs() > f32::EPSILON || actions.move_x.abs() > f32::EPSILON {
            movement += forward.normalize_or_zero() * actions.move_y;
            let right_h = glam::Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
            movement += right_h * actions.move_x;
        }
        if actions.move_z.abs() > f32::EPSILON {
            movement += glam::Vec3::Y * actions.move_z;
        }

        if movement.length_squared() > 0.0 {
            let speed = if actions.sprint { 20.0 } else { 10.0 };
            position += movement.normalize() * speed * dt;
        }

        self.renderer.camera_mut().position = position;
    }

    fn update_and_render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.frame_dt = dt;
        self.debug_hud.chunk_uploads_last_frame = 0;

        self.process_actions(dt);

        // Update time-of-day
        self.time_of_day.update(dt);

        // Update player health
        self.player_health.update(dt);

        // Update environment and effects
        self.update_weather(dt);
        self.update_particles(dt);
        self.debug_hud.particle_count = self.particles.len();

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
        self.debug_hud.control_mode = self.current_control_mode();
        self.debug_hud.cursor_captured = self.input.cursor_captured;
        self.debug_hud.mouse_sensitivity = self.controls.mouse_sensitivity;
        self.debug_hud.weather_state = self.weather_state_label().to_string();
        self.debug_hud.weather_intensity = self.weather_intensity();

        // Update camera from input
        self.update_camera(dt);

        // Raycast for block selection
        if self.input.cursor_captured {
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
            self.handle_block_interaction(dt);
        } else {
            self.selected_block = None;
        }

        // Render
        self.render();
    }

    fn update_camera(&mut self, dt: f32) {
        let actions = self.actions.clone();

        {
            let cursor_captured = self.input.cursor_captured;
            let camera = self.renderer.camera_mut();
            Self::apply_mouse_look(camera, &actions, &self.controls, cursor_captured);
        }

        if actions.context != InputContext::Gameplay {
            return;
        }

        if self.player_physics.physics_enabled {
            self.apply_physics_movement(&actions, dt);
        } else {
            self.apply_fly_movement(&actions, dt);
        }
    }

    fn handle_block_interaction(&mut self, dt: f32) {
        if let Some(hit) = self.selected_block {
            // Left click/hold: mine block
            if self.input.is_mouse_pressed(MouseButton::Left) {
                self.handle_mining(hit, dt);
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

    fn handle_mining(&mut self, hit: RaycastHit, dt: f32) {
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
        let mining_new_block = self
            .mining_progress
            .as_ref()
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
            progress.time_mining += dt;

            let percent = (progress.time_mining / progress.time_required * 100.0).min(100.0);
            self.debug_hud.mining_progress = Some(percent);

            // Check if mining is complete
            if progress.time_mining >= progress.time_required {
                // Mine the block!
                let mut spawn_particles_at: Option<glam::Vec3> = None;
                let mut mined = false;

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
                    spawn_particles_at = Some(glam::Vec3::new(
                        hit.block_pos.x as f32 + 0.5,
                        hit.block_pos.y as f32 + 0.5,
                        hit.block_pos.z as f32 + 0.5,
                    ));
                    mined = true;

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
                }

                if mined {
                    // Update lighting (skylight + seams)
                    self.recompute_chunk_lighting(chunk_pos);

                    // Regenerate mesh with updated lighting/geometry
                    if let Some(chunk) = self.chunks.get(&chunk_pos) {
                        let mesh =
                            mesh_chunk(chunk, &self.registry, self.renderer.atlas_metadata());
                        if let Some(resources) = self.renderer.render_resources() {
                            let chunk_bind_group = resources
                                .pipeline
                                .create_chunk_bind_group(resources.device, chunk_pos);
                            self.chunk_manager.add_chunk(
                                resources.device,
                                &mesh,
                                chunk_pos,
                                chunk_bind_group,
                            );
                            self.debug_hud.chunk_uploads_last_frame += 1;
                        }
                    }
                }

                if let Some(center) = spawn_particles_at {
                    self.spawn_block_break_particles(center, block_id);
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
            let mut spawn_particles_at: Option<glam::Vec3> = None;
            let mut placed = false;

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
                        spawn_particles_at = Some(glam::Vec3::new(
                            place_pos.x as f32 + 0.5,
                            place_pos.y as f32 + 0.5,
                            place_pos.z as f32 + 0.5,
                        ));
                        placed = true;

                        // Decrease block count
                        if let Some(item) = self.hotbar.selected_item_mut() {
                            if item.count > 0 {
                                item.count -= 1;
                                if item.count == 0 {
                                    self.hotbar.slots[self.hotbar.selected] = None;
                                }
                            }
                        }
                    }
                }
            }

            if placed {
                // Update lighting (skylight + seams)
                self.recompute_chunk_lighting(chunk_pos);

                // Regenerate mesh using updated chunk data
                if let Some(chunk) = self.chunks.get(&chunk_pos) {
                    let mesh = mesh_chunk(chunk, &self.registry, self.renderer.atlas_metadata());
                    if let Some(resources) = self.renderer.render_resources() {
                        let chunk_bind_group = resources
                            .pipeline
                            .create_chunk_bind_group(resources.device, chunk_pos);
                        self.chunk_manager.add_chunk(
                            resources.device,
                            &mesh,
                            chunk_pos,
                            chunk_bind_group,
                        );
                        self.debug_hud.chunk_uploads_last_frame += 1;
                    }
                }
            }

            if let Some(center) = spawn_particles_at {
                self.spawn_block_break_particles(center, block_id);
            }
        }
    }

    /// Recompute skylight for a chunk and stitch across neighboring seams.
    fn recompute_chunk_lighting(&mut self, chunk_pos: ChunkPos) {
        if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
            let _ = init_skylight(chunk, &self.registry);
        }

        let neighbors = [
            chunk_pos,
            ChunkPos::new(chunk_pos.x + 1, chunk_pos.z),
            ChunkPos::new(chunk_pos.x - 1, chunk_pos.z),
            ChunkPos::new(chunk_pos.x, chunk_pos.z + 1),
            ChunkPos::new(chunk_pos.x, chunk_pos.z - 1),
        ];

        for pos in neighbors {
            if self.chunks.contains_key(&pos) {
                let _ =
                    stitch_light_seams(&mut self.chunks, &self.registry, pos, LightType::Skylight);
            }
        }
    }

    fn render(&mut self) {
        if let Some(frame) = self.renderer.begin_frame() {
            let weather_intensity = self.weather_intensity();
            self.populate_particle_emitter();
            #[cfg(feature = "ui3d_billboards")]
            self.populate_billboards();

            let resources = self.renderer.render_resources().unwrap();
            let particle_system = if self.particle_emitter.vertices.is_empty() {
                None
            } else {
                Some(ParticleSystem::from_emitter(
                    resources.device,
                    &self.particle_emitter,
                ))
            };

            // Update time uniforms
            resources.skybox_pipeline.update_time(
                resources.queue,
                &self.time_of_day,
                weather_intensity,
            );
            resources
                .pipeline
                .update_time(resources.queue, &self.time_of_day, weather_intensity);

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
                render_pass.set_bind_group(0, resources.skybox_pipeline.time_bind_group(), &[]);
                render_pass.draw(0..3, 0..1);
            }

            // Create frustum for culling
            let camera = self.renderer.camera();
            let view_proj = camera.projection_matrix() * camera.view_matrix();
            let frustum = Frustum::from_matrix(&view_proj);

            // Render voxels with frustum culling
            self.chunks_visible = 0;
            {
                let mut render_pass = resources
                    .pipeline
                    .begin_render_pass(&mut encoder, &frame.view);

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

            if let Some(system) = particle_system.as_ref() {
                let depth_view = resources.pipeline.depth_view();
                let mut render_pass = resources.particle_pipeline.begin_render_pass(
                    &mut encoder,
                    &frame.view,
                    depth_view,
                );
                render_pass.set_pipeline(resources.particle_pipeline.pipeline());
                render_pass.set_bind_group(0, resources.pipeline.camera_bind_group(), &[]);
                system.render(&mut render_pass);
            }

            // Render wireframe highlight (if block selected)
            if let Some(hit) = self.selected_block {
                let highlight_pos = [
                    hit.block_pos.x as f32,
                    hit.block_pos.y as f32,
                    hit.block_pos.z as f32,
                ];
                let highlight_color = [0.0, 0.0, 0.0, 0.8];
                resources.wireframe_pipeline.update_highlight(
                    resources.queue,
                    highlight_pos,
                    highlight_color,
                );

                let depth_view = resources.pipeline.depth_view();
                let mut render_pass = resources.wireframe_pipeline.begin_render_pass(
                    &mut encoder,
                    &frame.view,
                    depth_view,
                );

                render_pass.set_pipeline(resources.wireframe_pipeline.pipeline());
                render_pass.set_bind_group(0, resources.pipeline.camera_bind_group(), &[]);
                render_pass.set_bind_group(
                    1,
                    resources.wireframe_pipeline.highlight_bind_group(),
                    &[],
                );
                render_pass
                    .set_vertex_buffer(0, resources.wireframe_pipeline.vertex_buffer().slice(..));
                render_pass.draw(0..24, 0..1);
            }

            #[cfg(feature = "ui3d_billboards")]
            if let Some(renderer) = self.billboard_renderer.as_mut() {
                let depth_view = resources.pipeline.depth_view();
                if let Err(err) = renderer.render(
                    resources.device,
                    resources.queue,
                    &mut encoder,
                    &frame.view,
                    depth_view,
                    resources.pipeline.camera_bind_group(),
                    &mut self.billboard_emitter,
                ) {
                    tracing::warn!(?err, "Billboard rendering failed");
                }
            }

            // Render UI overlay
            if let Some(mut ui) = self.renderer.ui_mut() {
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
            tracing::info!(
                "Fell {:.1} blocks, took {:.1} fall damage",
                fall_distance,
                damage
            );
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
                            ui.label(egui::RichText::new(format!("{}", i + 1)).size(10.0).color(
                                if is_selected {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::GRAY
                                },
                            ));
                            ui.label(egui::RichText::new(&item_name).size(8.0).color(
                                if is_selected {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::LIGHT_GRAY
                                },
                            ));

                            // Show count or durability
                            if let Some(stack) = item_stack {
                                match stack.item_type {
                                    ItemType::Tool(_, _) => {
                                        if let Some(durability) = stack.durability {
                                            let max_durability =
                                                stack.max_durability().unwrap_or(1);
                                            let durability_percent =
                                                (durability as f32 / max_durability as f32 * 100.0)
                                                    as u32;
                                            let color = if durability_percent < 20 {
                                                egui::Color32::RED
                                            } else if durability_percent < 50 {
                                                egui::Color32::YELLOW
                                            } else {
                                                egui::Color32::GREEN
                                            };
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}%",
                                                    durability_percent
                                                ))
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
                ui.label(
                    egui::RichText::new(health_text)
                        .size(14.0)
                        .color(text_color),
                );
            });
        });
}
