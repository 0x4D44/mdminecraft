//! Game world state - the actual 3D voxel game

use anyhow::Result;
use glam::IVec3;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_core::{ItemStack, ItemType, ToolMaterial, ToolType};
use mdminecraft_core::item::FoodType;
use mdminecraft_render::{
    mesh_chunk, raycast, ChunkManager, DebugHud, Frustum, InputState, RaycastHit, Renderer,
    RendererConfig, TimeOfDay, WindowConfig, WindowManager,
};
use mdminecraft_ui3d::{Button3D, ButtonState, Text3D, UI3DManager, UIElementHandle, screen_to_ray};
use mdminecraft_world::{BlockId, BlockPropertiesRegistry, Chunk, ChunkPos, TerrainGenerator, Voxel, BLOCK_AIR, Mob, MobSpawner};
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
                ItemType::Item(1) => "Stick".to_string(),
                ItemType::Item(item_id) => format!("Item {}", item_id),
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
            58 => "Crafting Table",
            _ => "Unknown",
        }
    }
}

/// Crafting recipe pattern - represents one crafting recipe
#[derive(Debug, Clone)]
struct CraftingRecipe {
    /// Name of the recipe (for display)
    name: String,
    /// 3x3 pattern where None = any/empty, Some = required item type
    pattern: [Option<ItemType>; 9],
    /// Output item
    output: ItemType,
    /// Output count
    output_count: u32,
    /// Whether the pattern needs to match exactly (true) or can be shifted (false)
    exact_position: bool,
}

impl CraftingRecipe {
    /// Check if the given grid matches this recipe pattern
    fn matches(&self, grid: &[Option<ItemType>; 9]) -> bool {
        if self.exact_position {
            // Exact position matching
            self.pattern.iter().zip(grid.iter()).all(|(pattern_slot, grid_slot)| {
                match (pattern_slot, grid_slot) {
                    (None, _) => true, // Pattern doesn't care what's here
                    (Some(required), Some(actual)) => required == actual,
                    (Some(_), None) => false, // Pattern requires something but grid is empty
                }
            })
        } else {
            // Allow pattern to be shifted within the grid
            // Try all possible offsets
            for offset_y in 0..=2 {
                for offset_x in 0..=2 {
                    if self.matches_at_offset(grid, offset_x, offset_y) {
                        return true;
                    }
                }
            }
            false
        }
    }

    fn matches_at_offset(&self, grid: &[Option<ItemType>; 9], offset_x: usize, offset_y: usize) -> bool {
        // Extract the bounding box of the pattern
        let pattern_bounds = self.get_pattern_bounds();
        if pattern_bounds.is_none() {
            return false; // Empty pattern
        }
        let (min_x, min_y, max_x, max_y) = pattern_bounds.unwrap();
        let pattern_width = max_x - min_x + 1;
        let pattern_height = max_y - min_y + 1;

        // Check if pattern fits at this offset
        if offset_x + pattern_width > 3 || offset_y + pattern_height > 3 {
            return false;
        }

        // Check if all other grid positions are empty
        for y in 0..3 {
            for x in 0..3 {
                let grid_idx = y * 3 + x;
                let is_in_pattern_area = x >= offset_x && x < offset_x + pattern_width
                    && y >= offset_y && y < offset_y + pattern_height;

                if is_in_pattern_area {
                    // Check if pattern matches here
                    let pattern_x = x - offset_x + min_x;
                    let pattern_y = y - offset_y + min_y;
                    let pattern_idx = pattern_y * 3 + pattern_x;

                    match (&self.pattern[pattern_idx], &grid[grid_idx]) {
                        (None, _) => continue,
                        (Some(required), Some(actual)) if required == actual => continue,
                        (Some(_), Some(_)) => return false, // Mismatch
                        (Some(_), None) => return false, // Required but empty
                        (None, _) => continue,
                    }
                } else {
                    // Outside pattern area - must be empty
                    if grid[grid_idx].is_some() {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn get_pattern_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let mut min_x = 3;
        let mut min_y = 3;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut found_any = false;

        for y in 0..3 {
            for x in 0..3 {
                let idx = y * 3 + x;
                if self.pattern[idx].is_some() {
                    found_any = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }

        if found_any {
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }
}

/// Recipe book containing all available crafting recipes
struct RecipeBook {
    recipes: Vec<CraftingRecipe>,
}

impl RecipeBook {
    fn new() -> Self {
        let mut book = Self { recipes: Vec::new() };
        book.register_default_recipes();
        book
    }

    fn register_default_recipes(&mut self) {
        // Recipe 1: Wood -> Planks (1:4 ratio)
        self.recipes.push(CraftingRecipe {
            name: "Planks".to_string(),
            pattern: [
                None, None, None,
                None, Some(ItemType::Block(3)), None, // Wood in center
                None, None, None,
            ],
            output: ItemType::Block(7), // Planks
            output_count: 4,
            exact_position: false,
        });

        // Recipe 2: Planks -> Sticks (2 planks vertical -> 4 sticks)
        self.recipes.push(CraftingRecipe {
            name: "Sticks".to_string(),
            pattern: [
                None, Some(ItemType::Block(7)), None, // Planks
                None, Some(ItemType::Block(7)), None, // Planks
                None, None, None,
            ],
            output: ItemType::Item(1), // Stick item ID
            output_count: 4,
            exact_position: false,
        });

        // Recipe 3: Sticks + Planks -> Wood Pickaxe (T pattern)
        self.recipes.push(CraftingRecipe {
            name: "Wood Pickaxe".to_string(),
            pattern: [
                Some(ItemType::Block(7)), Some(ItemType::Block(7)), Some(ItemType::Block(7)), // 3 planks
                None, Some(ItemType::Item(1)), None, // Stick
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood),
            output_count: 1,
            exact_position: false,
        });

        // Recipe 4: Sticks + Cobblestone -> Stone Pickaxe (T pattern)
        self.recipes.push(CraftingRecipe {
            name: "Stone Pickaxe".to_string(),
            pattern: [
                Some(ItemType::Block(6)), Some(ItemType::Block(6)), Some(ItemType::Block(6)), // 3 cobblestone
                None, Some(ItemType::Item(1)), None, // Stick
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Stone),
            output_count: 1,
            exact_position: false,
        });

        // Recipe 5: 4 Planks in 2x2 -> Crafting Table
        self.recipes.push(CraftingRecipe {
            name: "Crafting Table".to_string(),
            pattern: [
                Some(ItemType::Block(7)), Some(ItemType::Block(7)), None, // 2 planks
                Some(ItemType::Block(7)), Some(ItemType::Block(7)), None, // 2 planks
                None, None, None,
            ],
            output: ItemType::Block(58), // Crafting table block
            output_count: 1,
            exact_position: false,
        });

        // Recipe 6: Wood Axe (3 planks + 2 sticks, L-shape)
        self.recipes.push(CraftingRecipe {
            name: "Wood Axe".to_string(),
            pattern: [
                Some(ItemType::Block(7)), Some(ItemType::Block(7)), None, // 2 planks
                Some(ItemType::Block(7)), Some(ItemType::Item(1)), None, // Plank + stick
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Axe, ToolMaterial::Wood),
            output_count: 1,
            exact_position: false,
        });

        // Recipe 7: Stone Axe (3 cobblestone + 2 sticks)
        self.recipes.push(CraftingRecipe {
            name: "Stone Axe".to_string(),
            pattern: [
                Some(ItemType::Block(6)), Some(ItemType::Block(6)), None, // 2 cobblestone
                Some(ItemType::Block(6)), Some(ItemType::Item(1)), None, // Cobblestone + stick
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Axe, ToolMaterial::Stone),
            output_count: 1,
            exact_position: false,
        });

        // Recipe 8: Stone Sword (2 cobblestone + 1 stick, vertical)
        self.recipes.push(CraftingRecipe {
            name: "Stone Sword".to_string(),
            pattern: [
                None, Some(ItemType::Block(6)), None, // Cobblestone
                None, Some(ItemType::Block(6)), None, // Cobblestone
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Sword, ToolMaterial::Stone),
            output_count: 1,
            exact_position: false,
        });

        // Recipe 9: Wood Sword (2 planks + 1 stick)
        self.recipes.push(CraftingRecipe {
            name: "Wood Sword".to_string(),
            pattern: [
                None, Some(ItemType::Block(7)), None, // Plank
                None, Some(ItemType::Block(7)), None, // Plank
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Sword, ToolMaterial::Wood),
            output_count: 1,
            exact_position: false,
        });

        // Recipe 10: Wood Shovel (1 plank + 2 sticks)
        self.recipes.push(CraftingRecipe {
            name: "Wood Shovel".to_string(),
            pattern: [
                None, Some(ItemType::Block(7)), None, // Plank
                None, Some(ItemType::Item(1)), None, // Stick
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Shovel, ToolMaterial::Wood),
            output_count: 1,
            exact_position: false,
        });

        // Recipe 11: Stone Shovel (1 cobblestone + 2 sticks)
        self.recipes.push(CraftingRecipe {
            name: "Stone Shovel".to_string(),
            pattern: [
                None, Some(ItemType::Block(6)), None, // Cobblestone
                None, Some(ItemType::Item(1)), None, // Stick
                None, Some(ItemType::Item(1)), None, // Stick
            ],
            output: ItemType::Tool(ToolType::Shovel, ToolMaterial::Stone),
            output_count: 1,
            exact_position: false,
        });
    }

    /// Find the first recipe that matches the given grid
    fn find_matching_recipe(&self, grid: &[Option<ItemType>; 9]) -> Option<&CraftingRecipe> {
        self.recipes.iter().find(|recipe| recipe.matches(grid))
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

/// Player hunger tracking
struct PlayerHunger {
    /// Current hunger (0-20, like health)
    current: f32,
    /// Maximum hunger
    max: f32,
    /// Hunger drain rate (per second)
    drain_rate: f32,
    /// Saturation (extra food buffer)
    saturation: f32,
}

impl PlayerHunger {
    fn new() -> Self {
        Self {
            current: 20.0,
            max: 20.0,
            drain_rate: 0.1, // Lose 0.1 hunger per second (2 minutes to empty)
            saturation: 5.0,
        }
    }

    /// Update hunger over time
    fn update(&mut self, dt: f32) {
        // Drain saturation first, then hunger
        if self.saturation > 0.0 {
            self.saturation -= self.drain_rate * dt;
            if self.saturation < 0.0 {
                // Overflow into hunger
                self.current += self.saturation;
                self.saturation = 0.0;
            }
        } else {
            self.current -= self.drain_rate * dt;
            self.current = self.current.max(0.0);
        }
    }

    /// Eat food to restore hunger
    fn eat(&mut self, hunger_restored: f32, saturation_restored: f32) {
        self.current = (self.current + hunger_restored).min(self.max);
        self.saturation = (self.saturation + saturation_restored).min(20.0);
        tracing::info!("Ate food: +{:.1} hunger, +{:.1} saturation. Now: {:.1}/20 hunger, {:.1} saturation",
            hunger_restored, saturation_restored, self.current, self.saturation);
    }

    /// Check if hunger is full (> 18)
    fn is_full(&self) -> bool {
        self.current >= 18.0
    }

    /// Check if hungry (< 6)
    fn is_hungry(&self) -> bool {
        self.current < 6.0
    }

    /// Check if starving (0 hunger)
    fn is_starving(&self) -> bool {
        self.current <= 0.0
    }

    /// Get hunger percentage for UI
    fn hunger_percent(&self) -> f32 {
        self.current / self.max
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
    /// Requires hunger level to enable regeneration
    fn update(&mut self, dt: f32, hunger: &PlayerHunger) {
        self.time_since_damage += dt;

        if self.invulnerability_time > 0.0 {
            self.invulnerability_time -= dt;
        }

        // Regenerate health based on hunger level
        if self.current < self.max && self.time_since_damage > 3.0 {
            if hunger.is_full() {
                // Fast regeneration when hunger is full (>18)
                self.heal(1.0 * dt); // 1 HP per second
            } else if hunger.current > 6.0 {
                // Slow regeneration when hunger is decent
                self.heal(0.3 * dt); // 0.3 HP per second
            }
            // No regeneration if hungry (<= 6 hunger)
        }

        // Starvation damage when hunger is 0
        if hunger.is_starving() && self.time_since_damage > 1.0 {
            self.damage(0.5); // 0.5 damage, overrides invuln for starvation
            self.invulnerability_time = 0.0; // Reset invuln for next starve tick
            self.time_since_damage = 0.0;
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
    player_hunger: PlayerHunger,
    chunks_visible: usize,
    mining_progress: Option<MiningProgress>,
    spawn_point: glam::Vec3,
    // 3D UI system
    ui_manager: Option<UI3DManager>,
    ui_position_label: Option<UIElementHandle>,
    ui_block_label: Option<UIElementHandle>,
    ui_demo_button: Option<UIElementHandle>,
    ui_hovered_button: Option<UIElementHandle>,
    // Mob system
    mobs: Vec<Mob>,
    mob_spawner: MobSpawner,
    mob_labels: Vec<Option<UIElementHandle>>,
    current_tick: u64,
    targeted_mob: Option<usize>,  // Index of mob being looked at
    // Inventory UI
    inventory_open: bool,
    inventory_slots: Vec<Option<UIElementHandle>>,
    // Crafting UI
    crafting_open: bool,
    crafting_grid: Vec<Option<UIElementHandle>>,  // 3x3 grid UI handles
    crafting_result: Option<UIElementHandle>,
    craft_button: Option<UIElementHandle>,
    // Crafting system
    recipe_book: RecipeBook,
    crafting_grid_items: [Option<ItemType>; 9],  // What's currently in the crafting grid
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

        // Mob spawning
        let mob_spawner = MobSpawner::new(world_seed);
        let mut mobs = Vec::new();

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

                    // Spawn mobs for this chunk
                    let chunk_mobs = Self::spawn_mobs_for_chunk(&mob_spawner, &chunk, world_seed);
                    mobs.extend(chunk_mobs);

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
        tracing::info!("Spawned {} mobs", mobs.len());

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
            player_hunger: PlayerHunger::new(),
            chunks_visible: 0,
            mining_progress: None,
            spawn_point,
            ui_manager,
            ui_position_label: None,
            ui_block_label: None,
            ui_demo_button: None,
            ui_hovered_button: None,
            mobs,
            mob_spawner,
            mob_labels: Vec::new(),
            current_tick: 0,
            targeted_mob: None,
            inventory_open: false,
            inventory_slots: Vec::new(),
            crafting_open: false,
            crafting_grid: Vec::new(),
            crafting_result: None,
            craft_button: None,
            recipe_book: RecipeBook::new(),
            crafting_grid_items: [None; 9],
        })
    }

    /// Spawn mobs for a chunk based on its terrain
    fn spawn_mobs_for_chunk(spawner: &MobSpawner, chunk: &Chunk, world_seed: u64) -> Vec<Mob> {
        use mdminecraft_world::{BiomeAssigner, CHUNK_SIZE_X, CHUNK_SIZE_Z};

        // Extract surface heights from chunk
        let mut surface_heights = [[0i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                // Find the highest non-air block
                let mut height = 0;
                for y in (0..256).rev() {
                    if chunk.voxel(x, y, z).id != BLOCK_AIR {
                        height = y as i32;
                        break;
                    }
                }
                surface_heights[z][x] = height;
            }
        }

        // Determine chunk biome (use center of chunk)
        let biome_assigner = BiomeAssigner::new(world_seed);
        let chunk_pos = chunk.position();
        let center_x = chunk_pos.x * CHUNK_SIZE_X as i32 + (CHUNK_SIZE_X as i32 / 2);
        let center_z = chunk_pos.z * CHUNK_SIZE_Z as i32 + (CHUNK_SIZE_Z as i32 / 2);
        let biome = biome_assigner.get_biome(center_x, center_z);

        spawner.generate_spawns(chunk_pos.x, chunk_pos.z, biome, &surface_heights)
    }

    /// Update all mobs in the world
    fn update_mobs(&mut self) {
        self.current_tick += 1;

        // Get player position for hostile AI
        let player_pos = {
            let camera = self.renderer.camera();
            (camera.position.x as f64, camera.position.y as f64, camera.position.z as f64)
        };

        // Update mob AI and collect attacks
        let mut total_damage = 0.0;
        for mob in &mut self.mobs {
            if mob.mob_type.is_hostile() {
                // Hostile mobs track and attack player
                if let Some(damage) = mob.update_hostile(self.current_tick, player_pos) {
                    total_damage += damage;
                    tracing::info!("{:?} attacked player for {:.1} damage!", mob.mob_type, damage);
                }
            } else {
                // Passive mobs wander
                mob.update(self.current_tick);
            }
        }

        // Apply damage to player
        if total_damage > 0.0 {
            self.player_health.damage(total_damage);
            tracing::info!("Player health: {:.1}/{:.1}", self.player_health.current, self.player_health.max);
        }

        // Remove dead mobs and generate loot
        let mut i = 0;
        while i < self.mobs.len() {
            if self.mobs[i].is_dead() {
                let mob_type = self.mobs[i].mob_type;
                tracing::info!("Removing dead {:?}", mob_type);

                // Generate loot drops
                self.generate_mob_loot(mob_type);

                self.mobs.remove(i);

                // Remove corresponding label
                if i < self.mob_labels.len() {
                    if let Some(handle) = self.mob_labels.remove(i) {
                        if let Some(ui_manager) = &mut self.ui_manager {
                            ui_manager.remove_text(handle);
                        }
                    }
                }

                // Update targeted mob index if needed
                if let Some(targeted) = self.targeted_mob {
                    if targeted == i {
                        self.targeted_mob = None;
                    } else if targeted > i {
                        self.targeted_mob = Some(targeted - 1);
                    }
                }
            } else {
                i += 1;
            }
        }
    }

    /// Generate loot from a killed mob and add to player inventory
    fn generate_mob_loot(&mut self, mob_type: mdminecraft_world::MobType) {
        let loot_table = mob_type.get_loot_drops();

        // Use current tick for deterministic "randomness"
        let seed = self.current_tick;

        for (idx, (item_code, min_count, max_count)) in loot_table.iter().enumerate() {
            if *max_count == 0 && *min_count == 0 {
                continue;
            }

            // Deterministic "random" count based on tick + item index
            let count = if *max_count > *min_count {
                let range = max_count - min_count + 1;
                let pseudo_random = ((seed + idx as u64) * 48271) % range as u64;
                min_count + pseudo_random as u32
            } else {
                *min_count
            };

            if count == 0 {
                continue; // No drop this time
            }

            // Decode item type
            let item_type = if *item_code >= 2000 {
                // Food item
                let food_id = item_code - 2000;
                match food_id {
                    1 => ItemType::Food(FoodType::RawMeat),
                    2 => ItemType::Food(FoodType::RawMeat), // Rotten flesh -> raw meat for now
                    _ => ItemType::Item(food_id),
                }
            } else if *item_code >= 1000 {
                // Generic item
                ItemType::Item(item_code - 1000)
            } else {
                // Block
                ItemType::Block(*item_code)
            };

            // Try to add to hotbar
            let stack = ItemStack::new(item_type, count);
            if self.try_add_to_hotbar(stack.clone()) {
                tracing::info!("Looted: {} x{} from {:?}",
                    self.hotbar.item_name(Some(&stack)), count, mob_type);
            } else {
                tracing::warn!("Hotbar full! Lost loot: {} x{}",
                    self.hotbar.item_name(Some(&stack)), count);
            }
        }
    }

    /// Try to add an item stack to the hotbar
    /// Returns true if successful, false if hotbar is full
    fn try_add_to_hotbar(&mut self, stack: ItemStack) -> bool {
        // First try to merge with existing stack
        for slot in &mut self.hotbar.slots {
            if let Some(existing) = slot {
                if existing.item_type == stack.item_type && existing.can_add(stack.count) {
                    existing.count += stack.count;
                    return true;
                }
            }
        }

        // Try to find empty slot
        for slot in &mut self.hotbar.slots {
            if slot.is_none() {
                *slot = Some(stack);
                return true;
            }
        }

        false // Hotbar is full
    }

    /// Try to eat food from selected hotbar slot
    fn try_eat_food(&mut self) {
        let selected_slot = self.hotbar.selected;

        if let Some(item_stack) = &self.hotbar.slots[selected_slot] {
            // Check if the item is food
            if let ItemType::Food(food_type) = item_stack.item_type {
                // Get food values based on type
                let (hunger_restored, saturation_restored) = match food_type {
                    FoodType::Apple => (4.0, 2.4),
                    FoodType::Bread => (5.0, 6.0),
                    FoodType::RawMeat => (3.0, 1.8),
                    FoodType::CookedMeat => (8.0, 12.8),
                };

                // Eat the food
                self.player_hunger.eat(hunger_restored, saturation_restored);

                // Consume one from stack
                if item_stack.count > 1 {
                    self.hotbar.slots[selected_slot].as_mut().unwrap().count -= 1;
                } else {
                    self.hotbar.slots[selected_slot] = None;
                }

                tracing::info!("Ate {:?}!", food_type);
            } else {
                tracing::info!("Selected item is not food!");
            }
        } else {
            tracing::info!("No item selected to eat!");
        }
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
            PhysicalKey::Code(KeyCode::KeyE) => {
                self.inventory_open = !self.inventory_open;
                tracing::info!("Inventory: {}", if self.inventory_open { "OPEN" } else { "CLOSED" });
            }
            PhysicalKey::Code(KeyCode::KeyC) => {
                self.crafting_open = !self.crafting_open;
                tracing::info!("Crafting: {}", if self.crafting_open { "OPEN" } else { "CLOSED" });
            }
            PhysicalKey::Code(KeyCode::KeyR) => {
                self.try_eat_food();
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

        // Update player hunger
        self.player_hunger.update(dt);

        // Update player health (requires hunger for regeneration)
        self.player_health.update(dt, &self.player_hunger);

        // Check for death
        if self.player_health.is_dead() {
            self.handle_death();
        }

        // Update mobs
        self.update_mobs();

        // Update debug HUD
        self.debug_hud.update_fps(dt);
        let camera = self.renderer.camera();
        self.debug_hud.camera_pos = [camera.position.x, camera.position.y, camera.position.z];
        self.debug_hud.camera_rot = [camera.yaw, camera.pitch];
        self.debug_hud.chunks_visible = self.chunks_visible;

        // Update player stats for HUD display
        self.debug_hud.player_health = self.player_health.current;
        self.debug_hud.player_max_health = self.player_health.max;
        self.debug_hud.player_hunger = self.player_hunger.current;
        self.debug_hud.player_max_hunger = self.player_hunger.max;

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

            // Raycast for mob targeting
            self.targeted_mob = self.raycast_mobs(ray_origin, ray_dir);

            // Handle mob attacking
            self.handle_mob_attack();
        } else {
            self.selected_block = None;
            self.targeted_mob = None;
        }

        // Update 3D UI labels
        self.update_ui_labels();

        // Update demo button
        self.update_demo_button();

        // Update mob labels
        self.update_mob_labels();

        // Update inventory UI
        self.update_inventory_ui();

        // Update crafting UI
        self.update_crafting_ui();

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

    /// Update 3D labels above mobs
    fn update_mob_labels(&mut self) {
        // Read targeted_mob before borrowing ui_manager
        let targeted_mob = self.targeted_mob;

        if let Some(ui_manager) = &mut self.ui_manager {
            let resources = self.renderer.render_resources().expect("GPU not initialized");

            // Ensure mob_labels has the same length as mobs
            while self.mob_labels.len() < self.mobs.len() {
                self.mob_labels.push(None);
            }

            // Remove excess labels if we have fewer mobs
            while self.mob_labels.len() > self.mobs.len() {
                if let Some(handle) = self.mob_labels.pop().flatten() {
                    ui_manager.remove_text(handle);
                }
            }

            // Update each mob's label
            for (i, mob) in self.mobs.iter().enumerate() {
                let mob_pos = glam::Vec3::new(mob.x as f32, mob.y as f32 + 1.0, mob.z as f32);

                // Show name, health, and targeting indicator
                let health_bar = create_health_bar(mob.health_percent(), 10);
                let targeted = if targeted_mob == Some(i) { " <--" } else { "" };
                let hostile_tag = if mob.mob_type.is_hostile() { " [HOSTILE]" } else { "" };
                let mob_text = format!("{:?}{}\n{} {:.0}/{:.0}{}",
                    mob.mob_type,
                    hostile_tag,
                    health_bar,
                    mob.health,
                    mob.max_health,
                    targeted
                );

                // Color: Red for hostile mobs, health-based for passive
                let color = if mob.mob_type.is_hostile() {
                    [1.0, 0.2, 0.2, 1.0] // Bright red for hostiles
                } else if mob.health_percent() > 0.66 {
                    [0.0, 1.0, 0.0, 1.0] // Green for healthy passive
                } else if mob.health_percent() > 0.33 {
                    [1.0, 1.0, 0.0, 1.0] // Yellow for damaged passive
                } else {
                    [1.0, 0.0, 0.0, 1.0] // Red for critical passive
                };

                if let Some(handle) = self.mob_labels.get(i).and_then(|h| *h) {
                    // Update existing label
                    ui_manager.set_text_content(resources.device, handle, &mob_text);
                    ui_manager.set_text_position(resources.device, handle, mob_pos);
                    // Note: Can't update color on existing text, would need to recreate
                } else {
                    // Create new label
                    let text = Text3D::new(mob_pos, mob_text)
                        .with_font_size(0.18)
                        .with_color(color)
                        .with_billboard(true);
                    let handle = ui_manager.add_text(resources.device, text);
                    self.mob_labels[i] = Some(handle);
                }
            }
        }
    }

    /// Update 3D inventory UI
    fn update_inventory_ui(&mut self) {
        if let Some(ui_manager) = &mut self.ui_manager {
            let resources = self.renderer.render_resources().expect("GPU not initialized");
            let camera = self.renderer.camera();

            if self.inventory_open {
                // Create inventory UI if it doesn't exist
                if self.inventory_slots.is_empty() {
                    tracing::info!("Creating 3D inventory UI");

                    // Position inventory panel in front of player
                    let panel_pos = camera.position + camera.forward() * 3.0;

                    // Create 3x3 grid for hotbar (9 slots)
                    let slot_size = 0.4;
                    let slot_spacing = 0.5;
                    let start_x = -slot_spacing;
                    let start_y = slot_spacing;

                    for slot_idx in 0..9 {
                        let row = slot_idx / 3;
                        let col = slot_idx % 3;

                        let slot_pos = glam::Vec3::new(
                            panel_pos.x + start_x + (col as f32 * slot_spacing),
                            panel_pos.y + start_y - (row as f32 * slot_spacing),
                            panel_pos.z,
                        );

                        // Get item in this slot
                        let item_text = if let Some(stack) = self.hotbar.slots[slot_idx].as_ref() {
                            format!("{}\nx{}", self.hotbar.item_name(Some(stack)), stack.count)
                        } else {
                            format!("Slot {}", slot_idx + 1)
                        };

                        // Create button for this slot
                        let button = Button3D::new(slot_pos, item_text)
                            .with_size(slot_size, slot_size)
                            .with_font_size(0.15)
                            .with_billboard(true)
                            .with_callback((100 + slot_idx) as u32); // IDs 100-108 for inventory

                        let handle = ui_manager.add_button(resources.device, button);
                        self.inventory_slots.push(Some(handle));
                    }

                    tracing::info!("Created {} inventory slot buttons", self.inventory_slots.len());
                }

                // Update slot contents if items have changed
                for (slot_idx, slot_handle) in self.inventory_slots.iter().enumerate() {
                    if let Some(handle) = slot_handle {
                        let item_text = if let Some(stack) = self.hotbar.slots[slot_idx].as_ref() {
                            format!("{}\nx{}", self.hotbar.item_name(Some(stack)), stack.count)
                        } else {
                            format!("Slot {}", slot_idx + 1)
                        };

                        // Update button text
                        ui_manager.set_button_text(resources.device, *handle, item_text);
                    }
                }
            } else {
                // Inventory closed - remove UI elements
                if !self.inventory_slots.is_empty() {
                    tracing::info!("Closing 3D inventory UI");
                    for slot_handle in self.inventory_slots.drain(..) {
                        if let Some(handle) = slot_handle {
                            ui_manager.remove_button(handle);
                        }
                    }
                }
            }
        }
    }

    /// Update 3D crafting table UI
    fn update_crafting_ui(&mut self) {
        if let Some(ui_manager) = &mut self.ui_manager {
            let resources = self.renderer.render_resources().expect("GPU not initialized");
            let camera = self.renderer.camera();

            if self.crafting_open {
                // Create crafting UI if it doesn't exist
                if self.crafting_grid.is_empty() {
                    tracing::info!("Creating 3D crafting UI");

                    // Position crafting table to the right of player
                    let right = camera.right();
                    let panel_pos = camera.position + camera.forward() * 2.5 + right * 2.0;

                    // Create 3x3 crafting grid
                    let slot_size = 0.35;
                    let slot_spacing = 0.45;
                    let grid_start_x = -slot_spacing;
                    let grid_start_y = slot_spacing;

                    // Grid slots (9 slots for input)
                    for row in 0..3 {
                        for col in 0..3 {
                            let slot_idx = row * 3 + col;
                            let slot_pos = glam::Vec3::new(
                                panel_pos.x + grid_start_x + (col as f32 * slot_spacing),
                                panel_pos.y + grid_start_y - (row as f32 * slot_spacing),
                                panel_pos.z,
                            );

                            let button = Button3D::new(slot_pos, format!("[{}]", slot_idx + 1))
                                .with_size(slot_size, slot_size)
                                .with_font_size(0.12)
                                .with_billboard(true)
                                .with_callback((200 + slot_idx) as u32); // IDs 200-208 for crafting grid

                            let handle = ui_manager.add_button(resources.device, button);
                            self.crafting_grid.push(Some(handle));
                        }
                    }

                    // Result slot (to the right of the grid)
                    let result_pos = panel_pos + glam::Vec3::new(slot_spacing * 2.5, 0.0, 0.0);
                    let result_text = Text3D::new(result_pos, "Result:\n???")
                        .with_font_size(0.2)
                        .with_color([0.0, 1.0, 0.5, 1.0])
                        .with_billboard(true);
                    self.crafting_result = Some(ui_manager.add_text(resources.device, result_text));

                    // Craft button (below result)
                    let craft_pos = result_pos - glam::Vec3::new(0.0, 0.8, 0.0);
                    let craft_button = Button3D::new(craft_pos, "CRAFT")
                        .with_size(0.6, 0.3)
                        .with_font_size(0.18)
                        .with_billboard(true)
                        .with_callback(999); // ID 999 for craft button
                    self.craft_button = Some(ui_manager.add_button(resources.device, craft_button));

                    // Title text
                    let title_pos = panel_pos + glam::Vec3::new(0.0, slot_spacing * 2.0, 0.0);
                    let title = Text3D::new(title_pos, "Crafting Table")
                        .with_font_size(0.25)
                        .with_color([1.0, 1.0, 0.2, 1.0])
                        .with_billboard(true);
                    ui_manager.add_text(resources.device, title);

                    tracing::info!("Created crafting UI with {} grid slots", self.crafting_grid.len());
                }

                // Update crafting grid items to match hotbar (for simplicity, hotbar IS the crafting grid)
                for i in 0..9 {
                    self.crafting_grid_items[i] = self.hotbar.slots[i].as_ref().map(|s| s.item_type);
                }

                // Update result preview based on recipe matching
                if let Some(result_handle) = self.crafting_result {
                    let result_text = if let Some(recipe) = self.recipe_book.find_matching_recipe(&self.crafting_grid_items) {
                        format!("Result:\n{} x{}\n(Click CRAFT)", recipe.name, recipe.output_count)
                    } else {
                        "Result:\n???\n(No recipe)".to_string()
                    };

                    ui_manager.set_text_content(resources.device, result_handle, result_text.to_string());
                }
            } else {
                // Crafting closed - remove UI elements
                if !self.crafting_grid.is_empty() {
                    tracing::info!("Closing 3D crafting UI");

                    // Remove grid buttons
                    for grid_handle in self.crafting_grid.drain(..) {
                        if let Some(handle) = grid_handle {
                            ui_manager.remove_button(handle);
                        }
                    }

                    // Remove result label
                    if let Some(handle) = self.crafting_result.take() {
                        ui_manager.remove_text(handle);
                    }

                    // Remove craft button
                    if let Some(handle) = self.craft_button.take() {
                        ui_manager.remove_button(handle);
                    }
                }
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
                            100..=108 => {
                                let slot_idx = (callback_id - 100) as usize;
                                self.handle_inventory_slot_click(slot_idx);
                            }
                            200..=208 => {
                                let grid_idx = (callback_id - 200) as usize;
                                self.handle_crafting_slot_click(grid_idx);
                            }
                            999 => {
                                self.handle_craft_button_click();
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

    /// Handle clicking on an inventory slot
    fn handle_inventory_slot_click(&mut self, slot_idx: usize) {
        tracing::info!("Inventory slot {} clicked", slot_idx);
        if let Some(stack) = &self.hotbar.slots[slot_idx] {
            tracing::info!("  Contains: {} x{}", self.hotbar.item_name(Some(stack)), stack.count);
        } else {
            tracing::info!("  Empty slot");
        }
    }

    /// Handle clicking on a crafting grid slot
    fn handle_crafting_slot_click(&mut self, grid_idx: usize) {
        tracing::info!("Crafting grid slot {} clicked", grid_idx);
        // TODO: Implement item placement in crafting grid
    }

    /// Handle clicking the craft button
    fn handle_craft_button_click(&mut self) {
        tracing::info!("CRAFT button clicked!");

        // Find matching recipe
        let recipe = self.recipe_book.find_matching_recipe(&self.crafting_grid_items);
        if recipe.is_none() {
            tracing::info!("No matching recipe found!");
            return;
        }
        let recipe = recipe.unwrap();

        tracing::info!("Crafting: {} x{}", recipe.name, recipe.output_count);

        // Count required items for consumption
        let mut required_items: std::collections::HashMap<ItemType, u32> = std::collections::HashMap::new();
        for slot_item in &recipe.pattern {
            if let Some(item_type) = slot_item {
                *required_items.entry(*item_type).or_insert(0) += 1;
            }
        }

        // Check if we have enough items (should always pass since recipe matched, but be safe)
        for (item_type, count) in &required_items {
            let available: u32 = self.hotbar.slots
                .iter()
                .filter_map(|slot| slot.as_ref())
                .filter(|stack| stack.item_type == *item_type)
                .map(|stack| stack.count)
                .sum();

            if available < *count {
                tracing::warn!("Not enough {:?}: need {}, have {}", item_type, count, available);
                return;
            }
        }

        // Consume required items from hotbar
        for (item_type, mut count_to_remove) in required_items {
            for slot in &mut self.hotbar.slots {
                if count_to_remove == 0 {
                    break;
                }

                if let Some(stack) = slot {
                    if stack.item_type == item_type {
                        let removed = count_to_remove.min(stack.count);
                        stack.count -= removed;
                        count_to_remove -= removed;

                        // Remove empty stacks
                        if stack.count == 0 {
                            *slot = None;
                        }
                    }
                }
            }
        }

        // Create output item
        let output_stack = ItemStack::new(recipe.output, recipe.output_count);

        // Try to add to existing stack of same type
        let mut added = false;
        for slot in &mut self.hotbar.slots {
            if let Some(stack) = slot {
                if stack.item_type == output_stack.item_type && stack.can_add(output_stack.count) {
                    stack.count += output_stack.count;
                    tracing::info!("Added {} to existing stack in hotbar", recipe.name);
                    added = true;
                    break;
                }
            }
        }

        // If not added to existing stack, find empty slot
        if !added {
            for slot in &mut self.hotbar.slots {
                if slot.is_none() {
                    *slot = Some(output_stack.clone());
                    tracing::info!("Created {} x{} in new hotbar slot", recipe.name, output_stack.count);
                    added = true;
                    break;
                }
            }
        }

        if !added {
            tracing::warn!("No space in hotbar for crafted item!");
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

    /// Raycast to find which mob the player is looking at
    fn raycast_mobs(&self, ray_origin: glam::Vec3, ray_dir: glam::Vec3) -> Option<usize> {
        let max_distance = 8.0;
        let mut closest_distance = max_distance;
        let mut closest_mob_idx = None;

        for (idx, mob) in self.mobs.iter().enumerate() {
            let mob_pos = glam::Vec3::new(mob.x as f32, mob.y as f32 + 0.5, mob.z as f32);
            let mob_size = mob.mob_type.size();

            // Simple sphere collision check
            let to_mob = mob_pos - ray_origin;
            let distance_along_ray = to_mob.dot(ray_dir);

            if distance_along_ray < 0.0 || distance_along_ray > max_distance {
                continue;
            }

            let closest_point = ray_origin + ray_dir * distance_along_ray;
            let distance_to_mob = closest_point.distance(mob_pos);

            if distance_to_mob <= mob_size && distance_along_ray < closest_distance {
                closest_distance = distance_along_ray;
                closest_mob_idx = Some(idx);
            }
        }

        closest_mob_idx
    }

    /// Handle attacking mobs
    fn handle_mob_attack(&mut self) {
        if !self.input.is_mouse_pressed(MouseButton::Left) {
            return;
        }

        if let Some(mob_idx) = self.targeted_mob {
            if mob_idx < self.mobs.len() {
                // Get weapon damage
                let damage = if let Some((tool_type, material)) = self.hotbar.selected_tool() {
                    // Tool damage varies by type and material
                    match tool_type {
                        mdminecraft_core::ToolType::Pickaxe => match material {
                            mdminecraft_core::ToolMaterial::Wood => 2.0,
                            mdminecraft_core::ToolMaterial::Stone => 3.0,
                            mdminecraft_core::ToolMaterial::Iron => 4.0,
                            _ => 2.0,
                        },
                        mdminecraft_core::ToolType::Axe => match material {
                            mdminecraft_core::ToolMaterial::Wood => 3.0,
                            mdminecraft_core::ToolMaterial::Stone => 4.0,
                            mdminecraft_core::ToolMaterial::Iron => 5.0,
                            _ => 3.0,
                        },
                        mdminecraft_core::ToolType::Sword => match material {
                            mdminecraft_core::ToolMaterial::Wood => 4.0,
                            mdminecraft_core::ToolMaterial::Stone => 5.0,
                            mdminecraft_core::ToolMaterial::Iron => 6.0,
                            _ => 4.0,
                        },
                        _ => 1.0, // Other tools do minimal damage
                    }
                } else {
                    1.0 // Bare hands
                };

                let died = self.mobs[mob_idx].damage(damage);
                tracing::info!(
                    "Hit {:?} for {:.1} damage! Health: {:.1}/{:.1}",
                    self.mobs[mob_idx].mob_type,
                    damage,
                    self.mobs[mob_idx].health,
                    self.mobs[mob_idx].max_health
                );

                if died {
                    tracing::info!("{:?} died!", self.mobs[mob_idx].mob_type);
                }
            }
        }
    }
}

/// Create a simple ASCII health bar
fn create_health_bar(health_percent: f32, length: usize) -> String {
    let filled = (health_percent * length as f32).round() as usize;
    let empty = length.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
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
