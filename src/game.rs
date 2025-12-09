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
    ArmorPiece, ArmorSlot, BlockId, BlockPropertiesRegistry, Chunk, ChunkPos, EnchantingTableState,
    FurnaceState, Inventory, ItemManager, ItemType as DroppedItemType, Mob, MobSpawner, MobType,
    PlayerArmor, Projectile, ProjectileManager, TerrainGenerator, Voxel, WeatherState,
    WeatherToggle, BLOCK_AIR, BLOCK_CRAFTING_TABLE, BLOCK_ENCHANTING_TABLE, BLOCK_FURNACE,
    BLOCK_FURNACE_LIT, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
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

/// Player state (alive, dead, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// Player is alive and can move/interact
    Alive,
    /// Player is dead and viewing death screen
    Dead,
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
                Some(ItemStack::new(ItemType::Item(1), 1)), // Bow
                Some(ItemStack::new(ItemType::Item(2), 64)), // Arrows
                Some(ItemStack::new(ItemType::Block(2), 64)), // Dirt
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

    /// Get the food type if selected item is food
    fn selected_food(&self) -> Option<mdminecraft_core::item::FoodType> {
        if let Some(item) = self.selected_item() {
            match item.item_type {
                ItemType::Food(food_type) => Some(food_type),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Consume one of the selected item (for eating food)
    /// Returns true if item was consumed
    fn consume_selected(&mut self) -> bool {
        if let Some(item) = self.slots[self.selected].as_mut() {
            if item.count > 1 {
                item.count -= 1;
                true
            } else {
                // Remove the item if count becomes 0
                self.slots[self.selected] = None;
                true
            }
        } else {
            false
        }
    }

    /// Check if the selected item is a bow
    fn has_bow_selected(&self) -> bool {
        if let Some(item) = self.selected_item() {
            matches!(item.item_type, ItemType::Item(1)) // Item ID 1 = Bow
        } else {
            false
        }
    }

    /// Check if player has arrows in hotbar
    fn has_arrows(&self) -> bool {
        self.slots.iter().any(|slot| {
            if let Some(item) = slot {
                matches!(item.item_type, ItemType::Item(2)) // Item ID 2 = Arrow
            } else {
                false
            }
        })
    }

    /// Consume one arrow from inventory
    fn consume_arrow(&mut self) -> bool {
        for slot in &mut self.slots {
            if let Some(item) = slot {
                if matches!(item.item_type, ItemType::Item(2)) {
                    if item.count > 1 {
                        item.count -= 1;
                    } else {
                        *slot = None;
                    }
                    return true;
                }
            }
        }
        false
    }

    fn item_name(&self, item_stack: Option<&ItemStack>) -> String {
        if let Some(stack) = item_stack {
            match stack.item_type {
                ItemType::Tool(tool_type, material) => {
                    format!("{:?} {:?}", material, tool_type)
                }
                ItemType::Block(block_id) => self.block_name(block_id).to_string(),
                ItemType::Food(food_type) => format!("{:?}", food_type),
                ItemType::Item(id) => match id {
                    1 => "Bow".to_string(),
                    2 => "Arrow".to_string(),
                    3 => "Stick".to_string(),
                    4 => "String".to_string(),
                    5 => "Flint".to_string(),
                    6 => "Feather".to_string(),
                    7 => "Iron Ingot".to_string(),
                    // Iron armor
                    10 => "Iron Helmet".to_string(),
                    11 => "Iron Chestplate".to_string(),
                    12 => "Iron Leggings".to_string(),
                    13 => "Iron Boots".to_string(),
                    14 => "Diamond".to_string(),
                    // Leather armor
                    20 => "Leather Helmet".to_string(),
                    21 => "Leather Chestplate".to_string(),
                    22 => "Leather Leggings".to_string(),
                    23 => "Leather Boots".to_string(),
                    // Diamond armor
                    30 => "Diamond Helmet".to_string(),
                    31 => "Diamond Chestplate".to_string(),
                    32 => "Diamond Leggings".to_string(),
                    33 => "Diamond Boots".to_string(),
                    // Resource items
                    100 => "Stick".to_string(),
                    101 => "Feather".to_string(),
                    102 => "Leather".to_string(),
                    103 => "Wool".to_string(),
                    104 => "Egg".to_string(),
                    105 => "Sapling".to_string(),
                    _ => format!("Item({})", id),
                },
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
#[allow(dead_code)]
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
    eye_height: f32,
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
#[cfg(test)]
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
    /// Current hunger (0-20, measured in half-shanks)
    hunger: f32,
    /// Maximum hunger
    max_hunger: f32,
    /// Time accumulator for hunger depletion
    hunger_timer: f32,
    /// Time accumulator for starvation damage
    starvation_timer: f32,
    /// Whether player is actively moving (depletes hunger faster)
    is_active: bool,
}

impl PlayerHealth {
    fn new() -> Self {
        Self {
            current: 20.0,
            max: 20.0,
            regeneration_rate: 0.5, // Regenerate 0.5 health per second when hunger >= 18
            time_since_damage: 0.0,
            invulnerability_time: 0.0,
            hunger: 20.0,
            max_hunger: 20.0,
            hunger_timer: 0.0,
            starvation_timer: 0.0,
            is_active: false,
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

    /// Eat food and restore hunger
    fn eat(&mut self, hunger_restore: f32) -> bool {
        if self.hunger >= self.max_hunger {
            return false; // Already full
        }
        self.hunger = (self.hunger + hunger_restore).min(self.max_hunger);
        tracing::info!("Ate food, hunger now {:.1}/20", self.hunger);
        true
    }

    /// Update health (regeneration, hunger depletion, starvation)
    fn update(&mut self, dt: f32) {
        self.time_since_damage += dt;

        if self.invulnerability_time > 0.0 {
            self.invulnerability_time -= dt;
        }

        // Hunger depletion
        self.hunger_timer += dt;
        let depletion_interval = if self.is_active { 30.0 } else { 60.0 }; // 30s active, 60s idle
        let depletion_amount = if self.is_active { 1.0 } else { 0.5 };

        if self.hunger_timer >= depletion_interval {
            self.hunger_timer = 0.0;
            self.hunger = (self.hunger - depletion_amount).max(0.0);
        }

        // Starvation damage when hunger is 0
        if self.hunger <= 0.0 {
            self.starvation_timer += dt;
            if self.starvation_timer >= 1.0 {
                self.starvation_timer = 0.0;
                // Bypass invulnerability for starvation
                self.current = (self.current - 1.0).max(0.0);
                tracing::info!("Starvation damage! Health now {:.1}/20", self.current);
            }
        } else {
            self.starvation_timer = 0.0;
        }

        // Regenerate health only when hunger >= 18 and enough time since damage
        if self.hunger >= 18.0 && self.time_since_damage > 3.0 && self.current < self.max {
            self.heal(self.regeneration_rate * dt);
        }
    }

    /// Reset health and hunger to full (for respawn)
    fn reset(&mut self) {
        self.current = self.max;
        self.hunger = self.max_hunger;
        self.time_since_damage = 0.0;
        self.invulnerability_time = 0.0;
        self.hunger_timer = 0.0;
        self.starvation_timer = 0.0;
    }

    /// Set active state for hunger depletion rate
    fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
}

/// Get hunger restoration amount for a food type
fn food_hunger_restore(food_type: mdminecraft_core::item::FoodType) -> f32 {
    use mdminecraft_core::item::FoodType;
    match food_type {
        FoodType::Apple => 4.0,
        FoodType::Bread => 5.0,
        FoodType::RawMeat => 3.0,
        FoodType::CookedMeat => 8.0,
    }
}

/// Player experience points (visual only - XP not functional yet)
struct PlayerXP {
    /// Current XP points
    current: u32,
    /// Current level
    level: u32,
    /// XP needed to reach next level
    next_level_xp: u32,
}

impl PlayerXP {
    fn new() -> Self {
        Self {
            current: 0,
            level: 0,
            next_level_xp: 7, // First level needs 7 XP
        }
    }

    /// Add XP and handle level ups
    fn add_xp(&mut self, amount: u32) {
        self.current += amount;
        while self.current >= self.next_level_xp {
            self.current -= self.next_level_xp;
            self.level += 1;
            // XP curve: increases with level
            self.next_level_xp = if self.level < 16 {
                2 * self.level + 7
            } else if self.level < 31 {
                5 * self.level - 38
            } else {
                9 * self.level - 158
            };
        }
    }

    /// Get progress to next level (0.0 to 1.0)
    fn progress(&self) -> f32 {
        if self.next_level_xp == 0 {
            0.0
        } else {
            self.current as f32 / self.next_level_xp as f32
        }
    }

    /// Reset XP on death (loses some levels)
    #[allow(dead_code)]
    fn reset(&mut self) {
        // Lose 3 levels on death (minimum 0)
        self.level = self.level.saturating_sub(3);
        self.current = 0;
        // Recalculate next level XP
        self.next_level_xp = if self.level < 16 {
            2 * self.level + 7
        } else if self.level < 31 {
            5 * self.level - 38
        } else {
            9 * self.level - 158
        };
    }

    /// Consume XP levels (for enchanting)
    /// Returns true if levels were successfully consumed
    fn consume_levels(&mut self, levels: u32) -> bool {
        if self.level >= levels {
            self.level -= levels;
            // Recalculate next level XP for current level
            self.next_level_xp = if self.level < 16 {
                2 * self.level + 7
            } else if self.level < 31 {
                5 * self.level - 38
            } else {
                9 * self.level - 158
            };
            // Reset progress within the current level
            self.current = 0;
            true
        } else {
            false
        }
    }
}

/// Experience orb that drops from mobs and is collected by the player
struct XPOrb {
    /// World position
    pos: glam::Vec3,
    /// Velocity
    vel: glam::Vec3,
    /// XP value
    value: u32,
    /// Lifetime in seconds (despawns after 5 minutes)
    lifetime: f32,
    /// Whether orb is on ground
    on_ground: bool,
}

impl XPOrb {
    /// Create a new XP orb at the given position
    fn new(pos: glam::Vec3, value: u32) -> Self {
        // Small random upward and outward velocity for visual scatter
        let angle = rand::random::<f32>() * std::f32::consts::TAU;
        let speed = 0.1 + rand::random::<f32>() * 0.1;
        let vel = glam::Vec3::new(
            angle.cos() * speed,
            0.3, // Upward pop
            angle.sin() * speed,
        );

        Self {
            pos,
            vel,
            value,
            lifetime: 300.0, // 5 minutes
            on_ground: false,
        }
    }

    /// Update physics and magnetic attraction to player
    /// Returns true if orb should be removed (despawned or collected)
    fn update(&mut self, dt: f32, player_pos: glam::Vec3) -> bool {
        // Decrement lifetime
        self.lifetime -= dt;
        if self.lifetime <= 0.0 {
            return true; // Despawn
        }

        // Calculate distance to player
        let to_player = player_pos - self.pos;
        let distance = to_player.length();

        // Magnetic attraction when within 2 blocks
        if distance < 2.0 && distance > 0.01 {
            let attraction_strength = 8.0; // Blocks per second
            let attraction = to_player.normalize() * attraction_strength * dt;
            self.vel += attraction;
        }

        // Apply gravity
        if !self.on_ground {
            self.vel.y -= 9.8 * dt; // Gravity
        }

        // Apply velocity
        self.pos += self.vel * dt;

        // Simple ground check (if Y velocity is near zero and position is low)
        if self.vel.y.abs() < 0.1 && self.pos.y < player_pos.y + 1.0 {
            self.on_ground = true;
            self.vel.y = 0.0;
            self.vel *= 0.9; // Friction
        }

        false // Keep orb
    }

    /// Check if player should collect this orb
    fn should_collect(&self, player_pos: glam::Vec3) -> bool {
        let distance = (self.pos - player_pos).length();
        distance < 0.5 // Collection radius
    }
}

/// Ray-AABB intersection test
/// Returns Some(t) where t is the distance along the ray, or None if no intersection
fn ray_aabb_intersect(
    origin: glam::Vec3,
    dir: glam::Vec3,
    min: glam::Vec3,
    max: glam::Vec3,
) -> Option<f32> {
    let inv_dir = glam::Vec3::new(
        if dir.x.abs() < 1e-6 {
            f32::MAX
        } else {
            1.0 / dir.x
        },
        if dir.y.abs() < 1e-6 {
            f32::MAX
        } else {
            1.0 / dir.y
        },
        if dir.z.abs() < 1e-6 {
            f32::MAX
        } else {
            1.0 / dir.z
        },
    );

    let t1 = (min.x - origin.x) * inv_dir.x;
    let t2 = (max.x - origin.x) * inv_dir.x;
    let t3 = (min.y - origin.y) * inv_dir.y;
    let t4 = (max.y - origin.y) * inv_dir.y;
    let t5 = (min.z - origin.z) * inv_dir.z;
    let t6 = (max.z - origin.z) * inv_dir.z;

    let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
    let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

    if tmax >= tmin && tmax >= 0.0 {
        Some(if tmin >= 0.0 { tmin } else { tmax })
    } else {
        None
    }
}

/// Calculate attack damage based on held item
fn calculate_attack_damage(tool: Option<(ToolType, ToolMaterial)>) -> f32 {
    match tool {
        Some((tool_type, material)) => {
            // Base damage varies by tool type
            let base = match tool_type {
                ToolType::Sword => 4.0,
                ToolType::Axe => 3.0,
                ToolType::Pickaxe => 2.0,
                ToolType::Shovel => 1.5,
                ToolType::Hoe => 1.0,
            };
            // Material multiplier
            let multiplier = match material {
                ToolMaterial::Wood => 1.0,
                ToolMaterial::Stone => 1.25,
                ToolMaterial::Iron => 1.5,
                ToolMaterial::Gold => 1.0, // Gold is fast but weak
                ToolMaterial::Diamond => 1.75,
            };
            base * multiplier
        }
        None => 1.0, // Fist damage
    }
}

impl PlayerPhysics {
    const GROUND_EPS: f32 = 0.001;

    fn new() -> Self {
        Self {
            velocity: glam::Vec3::ZERO,
            on_ground: false,
            gravity: -20.0,
            jump_strength: 8.0,
            terminal_velocity: -50.0,
            player_height: 1.8,
            eye_height: 1.62,
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

    /// Build an AABB using the camera position (eye). Feet are offset down by `eye_height`.
    fn get_aabb(&self, camera_pos: glam::Vec3) -> AABB {
        let feet = camera_pos - glam::Vec3::new(0.0, self.eye_height, 0.0);
        let size = glam::Vec3::new(self.player_width, self.player_height, self.player_width);
        let center = feet + glam::Vec3::new(0.0, self.player_height * 0.5, 0.0);
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
    /// Current player state (alive/dead)
    player_state: PlayerState,
    /// Cause of death message for display
    death_message: String,
    /// Whether respawn was requested from death screen
    respawn_requested: bool,
    /// Whether return to menu was requested from death screen
    menu_requested: bool,
    #[cfg(feature = "ui3d_billboards")]
    billboard_renderer: Option<BillboardRenderer>,
    #[cfg(feature = "ui3d_billboards")]
    billboard_emitter: BillboardEmitter,
    /// Dropped item manager for block drops and pickups
    item_manager: ItemManager,
    /// Whether the inventory UI is open
    inventory_open: bool,
    /// Full player inventory (36 slots: 9 hotbar + 27 main)
    inventory: mdminecraft_world::Inventory,
    /// Mob spawner for passive mobs
    #[allow(dead_code)]
    mob_spawner: MobSpawner,
    /// Active mobs in the world
    mobs: Vec<Mob>,
    /// Frame counter for tick-based updates
    frame_count: u64,
    /// Whether the crafting UI is open
    crafting_open: bool,
    /// Crafting grid (3x3)
    crafting_grid: [[Option<ItemStack>; 3]; 3],
    /// Whether the furnace UI is open
    furnace_open: bool,
    /// Currently open furnace position (if any)
    open_furnace_pos: Option<IVec3>,
    /// Furnace states by position
    furnaces: HashMap<IVec3, FurnaceState>,
    /// Whether enchanting table UI is open
    enchanting_open: bool,
    /// Currently open enchanting table position (if any)
    open_enchanting_pos: Option<IVec3>,
    /// Enchanting table states by position
    enchanting_tables: HashMap<IVec3, EnchantingTableState>,
    /// Player armor (helmet, chestplate, leggings, boots)
    player_armor: PlayerArmor,
    /// Projectile manager for arrows and other projectiles
    projectiles: ProjectileManager,
    /// Bow drawing state (charge progress 0.0 to 1.0)
    bow_charge: f32,
    /// Whether the bow is currently being drawn
    bow_drawing: bool,
    /// Attack cooldown timer (seconds remaining until next attack allowed)
    attack_cooldown: f32,
    /// Player experience points
    player_xp: PlayerXP,
    /// Experience orbs in the world
    xp_orbs: Vec<XPOrb>,
}

impl GameWorld {
    #[inline(always)]
    fn flat_directions(camera: &mdminecraft_render::Camera) -> (glam::Vec3, glam::Vec3) {
        let yaw = camera.yaw;
        let forward = glam::Vec3::new(yaw.cos(), 0.0, yaw.sin()).normalize_or_zero();
        let right = glam::Vec3::new(-forward.z, 0.0, forward.x).normalize_or_zero();
        (forward, right)
    }
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

        // Spawn passive mobs in generated chunks
        let mob_spawner = MobSpawner::new(world_seed);
        let mut mobs = Vec::new();
        for (pos, chunk) in &chunks {
            // Get biome at chunk center
            let chunk_center_x = pos.x * CHUNK_SIZE_X as i32 + CHUNK_SIZE_X as i32 / 2;
            let chunk_center_z = pos.z * CHUNK_SIZE_Z as i32 + CHUNK_SIZE_Z as i32 / 2;
            let biome = generator
                .biome_assigner()
                .get_biome(chunk_center_x, chunk_center_z);

            // Calculate surface heights for each (x, z) position
            let mut surface_heights = [[0i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];
            for (local_z, row) in surface_heights.iter_mut().enumerate() {
                for (local_x, height) in row.iter_mut().enumerate() {
                    // Find highest non-air block
                    for y in (0..CHUNK_SIZE_Y).rev() {
                        let voxel = chunk.voxel(local_x, y, local_z);
                        if voxel.id != BLOCK_AIR {
                            *height = y as i32;
                            break;
                        }
                    }
                }
            }

            let mut new_mobs = mob_spawner.generate_spawns(pos.x, pos.z, biome, &surface_heights);
            tracing::debug!(
                "Spawned {} mobs in chunk {:?} ({:?})",
                new_mobs.len(),
                pos,
                biome
            );
            mobs.append(&mut new_mobs);
        }
        tracing::info!("Spawned {} passive mobs", mobs.len());

        // Determine spawn point near world origin so the player doesn't start mid-air.
        let spawn_feet = Self::determine_spawn_point(&chunks, &registry)
            .unwrap_or_else(|| glam::Vec3::new(0.0, 100.0, 0.0));

        // Setup camera at eye height above feet
        renderer.camera_mut().position =
            spawn_feet + glam::Vec3::new(0.0, PlayerPhysics::new().eye_height, 0.0);
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
            spawn_point: spawn_feet,
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
            player_state: PlayerState::Alive,
            death_message: String::new(),
            respawn_requested: false,
            menu_requested: false,
            #[cfg(feature = "ui3d_billboards")]
            billboard_renderer,
            #[cfg(feature = "ui3d_billboards")]
            billboard_emitter: BillboardEmitter::default(),
            item_manager: ItemManager::new(),
            inventory_open: false,
            inventory: Inventory::new(),
            mob_spawner,
            mobs,
            frame_count: 0,
            crafting_open: false,
            crafting_grid: Default::default(),
            furnace_open: false,
            open_furnace_pos: None,
            furnaces: HashMap::new(),
            enchanting_open: false,
            open_enchanting_pos: None,
            enchanting_tables: HashMap::new(),
            player_armor: PlayerArmor::new(),
            projectiles: ProjectileManager::new(),
            bow_charge: 0.0,
            bow_drawing: false,
            attack_cooldown: 0.0,
            player_xp: PlayerXP::new(),
            xp_orbs: Vec::new(),
        };

        world.player_physics.last_ground_y = spawn_feet.y;

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
            // Feet rest slightly above block top to avoid initial intersection.
            glam::Vec3::new(
                world_x as f32 + 0.5,
                y as f32 + 1.0 + PlayerPhysics::GROUND_EPS,
                world_z as f32 + 0.5,
            )
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

                        // Check for death screen actions after render
                        if let Some(action) = self.check_death_screen_actions() {
                            return action;
                        }
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
            PhysicalKey::Code(KeyCode::KeyE) => {
                if self.crafting_open {
                    self.close_crafting();
                } else {
                    self.toggle_inventory();
                }
            }
            PhysicalKey::Code(KeyCode::Escape) => {
                if self.crafting_open {
                    self.close_crafting();
                } else if self.inventory_open {
                    self.toggle_inventory();
                }
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
            // Note: yaw is negated in view_matrix(), so we use positive look.0 here
            // to get the expected mouse direction (move right → look right)
            camera.rotate(look.0, -look.1);
        }
    }

    fn apply_physics_movement(&mut self, actions: &ActionState, dt: f32) {
        let camera_snapshot = self.renderer.camera().clone();
        let mut camera_pos = camera_snapshot.position;
        let (forward_h, right_h) = Self::flat_directions(&camera_snapshot);

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
                camera_pos += move_dir * move_speed * dt;
            }

            camera_pos.y += physics.velocity.y * dt;

            let was_on_ground = physics.on_ground;
            let was_falling = physics.velocity.y < 0.0;
            let player_aabb = physics.get_aabb(camera_pos);
            let feet_y = player_aabb.min.y;
            let ground_y = Self::column_ground_height(
                &self.chunks,
                &self.registry,
                player_aabb.min.x + physics.player_width * 0.5, // approx center footprint
                player_aabb.min.z + physics.player_width * 0.5,
            );

            if feet_y < ground_y {
                let correction = ground_y - feet_y + PlayerPhysics::GROUND_EPS;
                camera_pos.y += correction;

                if !was_on_ground && was_falling {
                    fall_damage = Some(physics.last_ground_y - ground_y);
                }

                physics.velocity.y = 0.0;
                physics.on_ground = true;
                physics.last_ground_y = ground_y;
            } else {
                if physics.on_ground {
                    physics.last_ground_y = ground_y;
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

        self.renderer.camera_mut().position = camera_pos;
    }

    fn apply_fly_movement(&mut self, actions: &ActionState, dt: f32) {
        let (forward, right, mut position) = {
            let camera = self.renderer.camera();
            let (f, r) = Self::flat_directions(camera);
            (f, r, camera.position)
        };

        let mut movement = glam::Vec3::ZERO;
        if actions.move_y.abs() > f32::EPSILON || actions.move_x.abs() > f32::EPSILON {
            movement += forward * actions.move_y;
            movement += right * actions.move_x;
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

        // Update dropped items and handle pickup
        self.update_dropped_items();

        // Update furnaces
        self.update_furnaces(dt);

        // Update mobs
        self.update_mobs(dt);

        // Update projectiles (arrows)
        self.update_projectiles();

        // Update XP orbs (physics, magnetic attraction, collection)
        let player_pos = self.renderer.camera().position;
        let mut xp_collected = 0u32;
        self.xp_orbs.retain_mut(|orb| {
            // Check if player should collect this orb
            if orb.should_collect(player_pos) {
                xp_collected += orb.value;
                return false; // Remove collected orb
            }

            // Update orb physics
            !orb.update(dt, player_pos) // Remove if update returns true (despawned)
        });

        // Add collected XP to player
        if xp_collected > 0 {
            self.player_xp.add_xp(xp_collected);
            tracing::info!("Collected {} XP (Level: {}, Progress: {:.1}%)",
                xp_collected,
                self.player_xp.level,
                self.player_xp.progress() * 100.0
            );
        }

        self.frame_count = self.frame_count.wrapping_add(1);

        // Check for death
        if self.player_health.is_dead() && self.player_state != PlayerState::Dead {
            self.handle_death("You died!");
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

        // Update camera from input (only if alive)
        if self.player_state == PlayerState::Alive {
            self.update_camera(dt);
        }

        // Raycast for block selection (only if alive)
        if self.input.cursor_captured && self.player_state == PlayerState::Alive {
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

        // Update player activity state for hunger depletion
        let is_moving = actions.move_x.abs() > 0.01 || actions.move_y.abs() > 0.01;
        self.player_health.set_active(is_moving || actions.sprint);
    }

    fn handle_block_interaction(&mut self, dt: f32) {
        // Handle bow charging and shooting (before other interactions)
        if self.hotbar.has_bow_selected() && self.hotbar.has_arrows() {
            if self.input.is_mouse_pressed(MouseButton::Right) {
                // Charging bow
                if !self.bow_drawing {
                    self.bow_drawing = true;
                    self.bow_charge = 0.0;
                }
                // Increase charge over time (max 1.0 at 1 second)
                self.bow_charge = (self.bow_charge + dt).min(1.0);
            } else if self.bow_drawing {
                // Released - shoot arrow!
                if self.bow_charge >= 0.1 {
                    // Consume an arrow
                    if self.hotbar.consume_arrow() {
                        // Get camera position and direction
                        let camera = self.renderer.camera();
                        let arrow = Projectile::shoot_arrow(
                            camera.position.x as f64,
                            camera.position.y as f64,
                            camera.position.z as f64,
                            camera.yaw,
                            camera.pitch,
                            self.bow_charge,
                        );
                        self.projectiles.spawn(arrow);
                        tracing::debug!("Shot arrow with charge {:.2}", self.bow_charge);
                    }
                }
                self.bow_drawing = false;
                self.bow_charge = 0.0;
            }
            // Skip other interactions while drawing bow
            if self.bow_drawing {
                return;
            }
        } else {
            // Not holding bow, reset bow state
            self.bow_drawing = false;
            self.bow_charge = 0.0;
        }

        // Update attack cooldown timer (counts down to 0)
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);

        // Left click: try to attack a mob first (on click, not hold)
        // Only attack if cooldown has reached 0
        if self.input.is_mouse_clicked(MouseButton::Left) && self.attack_cooldown <= 0.0 {
            if self.try_attack_mob() {
                // Attacked a mob successfully - set cooldown to 0.6 seconds
                self.attack_cooldown = 0.6;
                // Don't mine
                self.mining_progress = None;
                return;
            }
        }

        if let Some(hit) = self.selected_block {
            // Left click/hold: mine block
            if self.input.is_mouse_pressed(MouseButton::Left) {
                self.handle_mining(hit, dt);
            } else {
                // Reset mining progress if not holding left click
                self.mining_progress = None;
            }

            // Right click: equip armor, eat food, interact with block, or place block
            if self.input.is_mouse_clicked(MouseButton::Right) {
                // First, check if we're holding armor and try to equip it
                let mut equipped_armor = false;
                if let Some(stack) = &self.hotbar.slots[self.hotbar.selected] {
                    if let Some(dropped_type) = item_type_to_armor_dropped(stack.item_type) {
                        if let Some(armor_piece) = ArmorPiece::from_item(dropped_type) {
                            // Equip the armor piece
                            let old_piece = self.player_armor.equip(armor_piece);
                            // Consume the item from hotbar
                            self.hotbar.consume_selected();
                            equipped_armor = true;
                            tracing::info!("Equipped armor: {:?}", dropped_type);
                            // If there was already armor in that slot, we don't return it to inventory (simplified)
                            if old_piece.is_some() {
                                tracing::info!("Replaced existing armor piece");
                            }
                        }
                    }
                }

                if equipped_armor {
                    // Skip other interactions when equipping armor
                } else if let Some(food_type) = self.hotbar.selected_food() {
                    // Check if we're holding food and try to eat it
                    let hunger_restore = food_hunger_restore(food_type);
                    if self.player_health.eat(hunger_restore) {
                        self.hotbar.consume_selected();
                        // Skip other interactions when eating
                    }
                } else {
                    // Check if the block is a crafting table or furnace
                    let chunk_x = hit.block_pos.x.div_euclid(16);
                    let chunk_z = hit.block_pos.z.div_euclid(16);
                    let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
                    let block_id = if let Some(chunk) = self.chunks.get(&chunk_pos) {
                        let local_x = hit.block_pos.x.rem_euclid(16) as usize;
                        let local_y = hit.block_pos.y as usize;
                        let local_z = hit.block_pos.z.rem_euclid(16) as usize;
                        if local_y < 256 {
                            Some(chunk.voxel(local_x, local_y, local_z).id)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    match block_id {
                        Some(BLOCK_CRAFTING_TABLE) => {
                            self.open_crafting();
                        }
                        Some(BLOCK_FURNACE) | Some(BLOCK_FURNACE_LIT) => {
                            self.open_furnace(hit.block_pos);
                        }
                        Some(BLOCK_ENCHANTING_TABLE) => {
                            self.open_enchanting_table(hit.block_pos);
                        }
                        _ => {
                            self.handle_block_placement(hit);
                        }
                    }
                }
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

                    // Spawn dropped item if harvested successfully
                    let tool = self.hotbar.selected_tool();
                    let can_harvest = self.block_properties.get(block_id).can_harvest(tool);
                    if can_harvest {
                        if let Some((drop_type, count)) = DroppedItemType::from_block(block_id) {
                            let drop_x = hit.block_pos.x as f64 + 0.5;
                            let drop_y = hit.block_pos.y as f64 + 0.5;
                            let drop_z = hit.block_pos.z as f64 + 0.5;
                            self.item_manager
                                .spawn_item(drop_x, drop_y, drop_z, drop_type, count);
                            tracing::debug!(
                                "Dropped {:?} x{} at ({:.1}, {:.1}, {:.1})",
                                drop_type,
                                count,
                                drop_x,
                                drop_y,
                                drop_z
                            );
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
        let mut close_inventory_requested = false;
        let mut close_crafting_requested = false;
        let mut close_furnace_requested = false;
        let mut close_enchanting_requested = false;
        let mut enchanting_result: Option<EnchantingResult> = None;
        let mut crafted_item: Option<ItemStack> = None;

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
            let is_dead = self.player_state == PlayerState::Dead;
            let death_msg = self.death_message.clone();
            let inventory_open = self.inventory_open;
            let crafting_open = self.crafting_open;
            let furnace_open = self.furnace_open;
            let enchanting_open = self.enchanting_open;
            let mut respawn_clicked = false;
            let mut menu_clicked = false;

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
                        render_xp_bar(ctx, &self.player_xp);
                        render_health_bar(ctx, &self.player_health);
                        render_hunger_bar(ctx, &self.player_health);
                        render_armor_bar(ctx, &self.player_armor);
                        render_tool_durability(ctx, &self.hotbar);

                        // Show inventory if open
                        if inventory_open {
                            close_inventory_requested =
                                render_inventory(ctx, &self.hotbar, &self.inventory);
                        }

                        // Show crafting if open
                        if crafting_open {
                            let (close, item) =
                                render_crafting(ctx, &mut self.crafting_grid, &mut self.hotbar);
                            close_crafting_requested = close;
                            crafted_item = item;
                        }

                        // Show furnace if open
                        if furnace_open {
                            if let Some(pos) = self.open_furnace_pos {
                                if let Some(furnace) = self.furnaces.get_mut(&pos) {
                                    close_furnace_requested = render_furnace(ctx, furnace);
                                }
                            }
                        }

                        // Show enchanting table if open
                        if enchanting_open {
                            if let Some(pos) = self.open_enchanting_pos {
                                if let Some(table) = self.enchanting_tables.get_mut(&pos) {
                                    // Check if selected hotbar item is enchantable
                                    let selected_enchantable = self
                                        .hotbar
                                        .selected_item()
                                        .map(|item| item.is_enchantable())
                                        .unwrap_or(false);

                                    let result = render_enchanting_table(
                                        ctx,
                                        table,
                                        &self.player_xp,
                                        selected_enchantable,
                                    );
                                    close_enchanting_requested = result.close_requested;
                                    if result.enchantment_applied.is_some() {
                                        enchanting_result = Some(result);
                                    }
                                }
                            }
                        }

                        // Show death screen if player is dead
                        if is_dead {
                            let (respawn, menu) = render_death_screen(ctx, &death_msg);
                            respawn_clicked = respawn;
                            menu_clicked = menu;
                        }
                    },
                );
            }

            // Handle death screen button clicks
            if respawn_clicked {
                self.respawn_requested = true;
            }
            if menu_clicked {
                self.menu_requested = true;
            }

            resources.queue.submit(std::iter::once(encoder.finish()));
            frame.present();
        }

        // Handle inventory close (after frame render to avoid borrow issues)
        if close_inventory_requested {
            self.toggle_inventory();
        }

        // Handle crafting close
        if close_crafting_requested {
            self.close_crafting();
        }

        // Handle crafted item - add to hotbar
        if let Some(stack) = crafted_item {
            // Try to add to hotbar
            let mut added = false;
            for existing in self.hotbar.slots.iter_mut().flatten() {
                if existing.item_type == stack.item_type && existing.can_add(stack.count) {
                    existing.count += stack.count;
                    added = true;
                    break;
                }
            }
            if !added {
                for slot in &mut self.hotbar.slots {
                    if slot.is_none() {
                        *slot = Some(stack);
                        added = true;
                        break;
                    }
                }
            }
            if added {
                tracing::info!("Crafted item added to hotbar");
            }
        }

        // Handle furnace close
        if close_furnace_requested {
            self.close_furnace();
        }

        // Handle enchanting table close
        if close_enchanting_requested {
            self.close_enchanting_table();
        }

        // Handle enchanting result - apply enchantment to selected item
        if let Some(result) = enchanting_result {
            if let Some(enchantment) = result.enchantment_applied {
                // Apply enchantment to selected hotbar item
                if let Some(item) = self.hotbar.selected_item_mut() {
                    if item.add_enchantment(enchantment) {
                        // Consume XP levels
                        if self.player_xp.consume_levels(result.xp_to_consume) {
                            tracing::info!(
                                "Enchanted item with {:?} level {} (consumed {} XP levels)",
                                enchantment.enchantment_type,
                                enchantment.level,
                                result.xp_to_consume
                            );
                        }
                    } else {
                        tracing::warn!("Failed to apply enchantment to item");
                    }
                }
            }
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

            // Check if this killed the player
            if self.player_health.is_dead() && self.player_state != PlayerState::Dead {
                let msg = format!("You fell from a high place ({:.0} blocks)", fall_distance);
                self.handle_death(&msg);
            }
        }
    }

    /// Handle player death - enter death state and show death screen
    fn handle_death(&mut self, cause: &str) {
        if self.player_state == PlayerState::Dead {
            return; // Already dead
        }

        tracing::info!("Player died! Cause: {}", cause);

        self.player_state = PlayerState::Dead;
        self.death_message = cause.to_string();
        self.respawn_requested = false;
        self.menu_requested = false;

        // Release cursor so player can click UI buttons
        let _ = self.input.enter_menu(&self.window);

        // Stop player movement
        self.player_physics.velocity = glam::Vec3::ZERO;
    }

    /// Respawn the player at spawn point
    fn respawn(&mut self) {
        tracing::info!("Respawning player at spawn point...");

        // Respawn player at spawn point
        let camera = self.renderer.camera_mut();
        camera.position =
            self.spawn_point + glam::Vec3::new(0.0, self.player_physics.eye_height, 0.0);

        // Reset health
        self.player_health.reset();

        // Reset physics
        self.player_physics.velocity = glam::Vec3::ZERO;
        self.player_physics.on_ground = false;
        self.player_physics.last_ground_y = self.spawn_point.y;

        // Reset state
        self.player_state = PlayerState::Alive;
        self.death_message.clear();

        // Re-capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
    }

    /// Check if player requested respawn from death screen
    fn check_death_screen_actions(&mut self) -> Option<GameAction> {
        if self.player_state != PlayerState::Dead {
            return None;
        }

        if self.respawn_requested {
            self.respawn_requested = false;
            self.respawn();
            return None;
        }

        if self.menu_requested {
            self.menu_requested = false;
            self.player_state = PlayerState::Alive; // Reset state before returning
            self.player_health.reset();
            return Some(GameAction::ReturnToMenu);
        }

        None
    }

    /// Update dropped items and handle player pickup
    fn update_dropped_items(&mut self) {
        // Get player position (feet position)
        let camera_pos = self.renderer.camera().position;
        let player_x = camera_pos.x as f64;
        let player_y = (camera_pos.y - self.player_physics.eye_height) as f64;
        let player_z = camera_pos.z as f64;

        // Create closure to get ground height
        let chunks = &self.chunks;
        let registry = &self.registry;
        let get_ground_height = |x: f64, z: f64| -> f64 {
            Self::column_ground_height(chunks, registry, x as f32, z as f32) as f64
        };

        // Update item physics
        self.item_manager.update(get_ground_height);

        // Merge nearby items
        self.item_manager.merge_nearby_items();

        // Check for item pickup
        let picked_up = self.item_manager.pickup_items(player_x, player_y, player_z);

        // Add picked up items to hotbar
        for (drop_type, count) in picked_up {
            if let Some(core_item_type) = Self::convert_dropped_item_type(drop_type) {
                let stack = ItemStack::new(core_item_type, count);

                // Try to add to existing stack in hotbar first
                let mut added = false;
                for existing in self.hotbar.slots.iter_mut().flatten() {
                    if existing.item_type == core_item_type && existing.can_add(count) {
                        existing.count += count;
                        added = true;
                        break;
                    }
                }

                // If not merged, find empty slot
                if !added {
                    for slot in &mut self.hotbar.slots {
                        if slot.is_none() {
                            *slot = Some(stack);
                            added = true;
                            break;
                        }
                    }
                }

                if added {
                    tracing::info!("Picked up {:?} x{}", drop_type, count);
                }
            }
        }
    }

    /// Update mob AI and movement
    fn update_mobs(&mut self, _dt: f32) {
        // Use frame count as tick for deterministic behavior
        // TODO: Use proper SimTick for multiplayer sync
        let tick = self.frame_count;

        // Get player position for hostile mob targeting
        let player_pos = self.renderer.camera().position;
        let player_x = player_pos.x as f64;
        let player_y = player_pos.y as f64;
        let player_z = player_pos.z as f64;

        // Check if it's night time (hostile mobs spawn at night)
        // Time: 0.0-0.25 Night→Dawn, 0.75-1.0 Dusk→Night
        let time = self.time_of_day.time();
        let is_night = !(0.25..=0.75).contains(&time);

        // Update each mob and track damage to player
        let mut total_damage = 0.0f32;
        let mut exploded_creeper = false;
        let mut explosion_positions: Vec<(f64, f64, f64, f32)> = Vec::new();
        for mob in &mut self.mobs {
            // Spiders are only hostile at night, other hostile mobs always attack
            if mob.mob_type.is_hostile_at_time(is_night) {
                // Update hostile mob with player targeting
                let dealt_damage = mob.update_with_target(tick, player_x, player_y, player_z);
                if dealt_damage {
                    // Check if this was a creeper explosion
                    if mob.mob_type.explodes() && mob.dead {
                        // Creeper explosion - high damage!
                        total_damage += mob.mob_type.explosion_damage();
                        exploded_creeper = true;
                        // Record explosion position for block destruction
                        explosion_positions.push((
                            mob.x,
                            mob.y,
                            mob.z,
                            mob.mob_type.explosion_radius(),
                        ));
                        tracing::info!("Creeper exploded!");
                    } else {
                        // Normal attack damage
                        total_damage += mob.mob_type.attack_damage();
                    }
                }
            } else {
                // Update passive mob (or spider in daylight)
                mob.update(tick);
            }
        }

        // Handle creeper explosion block destruction
        for (ex, ey, ez, radius) in explosion_positions {
            self.destroy_blocks_in_radius(ex, ey, ez, radius);
        }

        // Apply accumulated damage to player (reduced by armor)
        if total_damage > 0.0 && self.player_state == PlayerState::Alive {
            // Armor reduces damage and takes durability hit
            let actual_damage = self.player_armor.take_damage(total_damage);
            self.player_health.damage(actual_damage);

            // Log armor reduction
            if actual_damage < total_damage {
                tracing::debug!(
                    "Armor reduced damage from {:.1} to {:.1}",
                    total_damage,
                    actual_damage
                );
            }

            // Determine damage source for potential death message
            let source = if exploded_creeper {
                "Creeper"
            } else if self.mobs.iter().any(|m| m.mob_type == MobType::Spider) {
                "Spider"
            } else if self.mobs.iter().any(|m| m.mob_type == MobType::Zombie) {
                "Zombie"
            } else {
                "Skeleton"
            };

            // Check for death
            if self.player_health.is_dead() {
                if exploded_creeper {
                    self.handle_death("Blown up by Creeper");
                } else {
                    self.handle_death(&format!("Slain by {}", source));
                }
            }
        }

        // Remove dead mobs and drop loot (and XP)
        let mut loot_drops: Vec<(f64, f64, f64, DroppedItemType, u32)> = Vec::new();
        let mut xp_orb_spawns: Vec<(f64, f64, f64, u32)> = Vec::new();
        self.mobs.retain(|mob| {
            if mob.dead {
                // Spawn XP orb based on mob type
                let xp_value = match mob.mob_type {
                    MobType::Zombie | MobType::Skeleton | MobType::Spider => 5,
                    MobType::Creeper => 5,
                    MobType::Pig | MobType::Cow | MobType::Sheep | MobType::Chicken => 1,
                };
                xp_orb_spawns.push((mob.x, mob.y + 0.5, mob.z, xp_value));
                // Drop loot based on mob type
                match mob.mob_type {
                    MobType::Zombie => {
                        // Zombies drop 0-2 rotten flesh
                        let count = (tick % 3) as u32;
                        if count > 0 {
                            loot_drops.push((
                                mob.x,
                                mob.y + 0.5,
                                mob.z,
                                DroppedItemType::RottenFlesh,
                                count,
                            ));
                        }
                    }
                    MobType::Skeleton => {
                        // Skeletons drop 0-2 bones
                        let bone_count = (tick % 3) as u32;
                        if bone_count > 0 {
                            loot_drops.push((
                                mob.x,
                                mob.y + 0.5,
                                mob.z,
                                DroppedItemType::Bone,
                                bone_count,
                            ));
                        }
                    }
                    MobType::Pig => {
                        loot_drops.push((
                            mob.x,
                            mob.y + 0.5,
                            mob.z,
                            DroppedItemType::RawPork,
                            1 + (tick % 3) as u32,
                        ));
                    }
                    MobType::Cow => {
                        loot_drops.push((
                            mob.x,
                            mob.y + 0.5,
                            mob.z,
                            DroppedItemType::RawBeef,
                            1 + (tick % 3) as u32,
                        ));
                        loot_drops.push((
                            mob.x,
                            mob.y + 0.5,
                            mob.z,
                            DroppedItemType::Leather,
                            (tick % 2) as u32 + 1,
                        ));
                    }
                    MobType::Sheep => {
                        loot_drops.push((mob.x, mob.y + 0.5, mob.z, DroppedItemType::Wool, 1));
                    }
                    MobType::Chicken => {
                        loot_drops.push((
                            mob.x,
                            mob.y + 0.5,
                            mob.z,
                            DroppedItemType::Feather,
                            1 + (tick % 2) as u32,
                        ));
                    }
                    MobType::Spider => {
                        // Spiders drop 0-2 string
                        let count = (tick % 3) as u32;
                        if count > 0 {
                            loot_drops.push((
                                mob.x,
                                mob.y + 0.5,
                                mob.z,
                                DroppedItemType::String,
                                count,
                            ));
                        }
                    }
                    MobType::Creeper => {
                        // Creepers drop 0-2 gunpowder
                        let count = (tick % 3) as u32;
                        if count > 0 {
                            loot_drops.push((
                                mob.x,
                                mob.y + 0.5,
                                mob.z,
                                DroppedItemType::Gunpowder,
                                count,
                            ));
                        }
                    }
                }
                false // Remove dead mob
            } else {
                true // Keep alive mob
            }
        });

        // Spawn loot drops
        for (x, y, z, item_type, count) in loot_drops {
            if count > 0 {
                self.item_manager.spawn_item(x, y, z, item_type, count);
            }
        }

        // Spawn XP orbs from killed mobs
        for (x, y, z, xp_value) in xp_orb_spawns {
            let pos = glam::Vec3::new(x as f32, y as f32, z as f32);
            self.xp_orbs.push(XPOrb::new(pos, xp_value));
        }

        // Spawn hostile mobs at night (every ~100 frames, max 10 hostile mobs)
        if is_night && tick.is_multiple_of(100) {
            let hostile_count = self.mobs.iter().filter(|m| m.is_hostile()).count();
            if hostile_count < 10 {
                // Spawn at random position around player (16-32 blocks away)
                let angle = ((tick * 7) % 360) as f64 * std::f64::consts::PI / 180.0;
                let distance = 16.0 + ((tick * 13) % 16) as f64;
                let spawn_x = player_x + angle.cos() * distance;
                let spawn_z = player_z + angle.sin() * distance;

                // Get ground height at spawn position
                let chunk_x = (spawn_x as i32).div_euclid(CHUNK_SIZE_X as i32);
                let chunk_z = (spawn_z as i32).div_euclid(CHUNK_SIZE_Z as i32);
                let local_x = (spawn_x as i32).rem_euclid(CHUNK_SIZE_X as i32) as usize;
                let local_z = (spawn_z as i32).rem_euclid(CHUNK_SIZE_Z as i32) as usize;

                if let Some(chunk) = self.chunks.get(&ChunkPos::new(chunk_x, chunk_z)) {
                    // Find ground level
                    let mut ground_y = 64;
                    for y in (0..CHUNK_SIZE_Y).rev() {
                        if chunk.voxel(local_x, y, local_z).id != BLOCK_AIR {
                            ground_y = y + 1;
                            break;
                        }
                    }

                    // Choose mob type (zombie, skeleton, spider, or creeper)
                    let mob_type = match tick % 4 {
                        0 => MobType::Zombie,
                        1 => MobType::Skeleton,
                        2 => MobType::Spider,
                        _ => MobType::Creeper,
                    };

                    let mob = Mob::new(spawn_x, ground_y as f64 + 0.5, spawn_z, mob_type);
                    self.mobs.push(mob);
                    tracing::debug!(
                        "Spawned {:?} at ({:.1}, {}, {:.1})",
                        mob_type,
                        spawn_x,
                        ground_y,
                        spawn_z
                    );
                }
            }
        }

        // Despawn hostile mobs during the day (too far from player or day time)
        if !is_night {
            self.mobs.retain(|mob| {
                if mob.is_hostile() {
                    // Despawn hostile mobs during the day
                    let dist = ((mob.x - player_x).powi(2) + (mob.z - player_z).powi(2)).sqrt();
                    dist < 48.0 // Keep if within 48 blocks during day transition
                } else {
                    true
                }
            });
        }
    }

    /// Update all projectiles - physics, collisions, and damage
    fn update_projectiles(&mut self) {
        // Update projectile physics
        self.projectiles.update();

        // Check for block collisions (stick arrows)
        for projectile in &mut self.projectiles.projectiles {
            if projectile.stuck || projectile.dead {
                continue;
            }

            // Check if projectile is inside a solid block
            let block_x = projectile.x.floor() as i32;
            let block_y = projectile.y.floor() as i32;
            let block_z = projectile.z.floor() as i32;

            if !(0..256).contains(&block_y) {
                projectile.stick();
                continue;
            }

            let chunk_x = block_x.div_euclid(CHUNK_SIZE_X as i32);
            let chunk_z = block_z.div_euclid(CHUNK_SIZE_Z as i32);
            let local_x = block_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
            let local_y = block_y as usize;
            let local_z = block_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

            if let Some(chunk) = self.chunks.get(&ChunkPos::new(chunk_x, chunk_z)) {
                let voxel = chunk.voxel(local_x, local_y, local_z);
                if voxel.id != BLOCK_AIR {
                    // Hit a block, stick the arrow
                    projectile.stick();
                }
            }
        }

        // Check for mob collisions
        for projectile in &mut self.projectiles.projectiles {
            if projectile.stuck || projectile.dead {
                continue;
            }

            for mob in &mut self.mobs {
                if mob.dead {
                    continue;
                }

                // Simple distance-based hit detection
                let mob_radius = mob.mob_type.size() as f64;
                let mob_height = mob_radius * 2.0;

                // Check if projectile is within mob bounds
                let dx = projectile.x - mob.x;
                let dy = projectile.y - (mob.y + mob_height / 2.0);
                let dz = projectile.z - mob.z;

                let horizontal_dist = (dx * dx + dz * dz).sqrt();
                let vertical_dist = dy.abs();

                if horizontal_dist < mob_radius + 0.3 && vertical_dist < mob_height / 2.0 + 0.3 {
                    // Hit the mob!
                    let damage = projectile.damage();
                    mob.damage(damage);
                    projectile.hit();

                    // Apply knockback from arrow direction
                    let knock_dir_x = projectile.vel_x;
                    let knock_dir_z = projectile.vel_z;
                    let knock_len = (knock_dir_x * knock_dir_x + knock_dir_z * knock_dir_z).sqrt();
                    if knock_len > 0.001 {
                        mob.apply_knockback(knock_dir_x / knock_len, knock_dir_z / knock_len, 0.3);
                    }

                    tracing::debug!("Arrow hit {:?} for {:.1} damage", mob.mob_type, damage);
                    break; // Only hit one mob per projectile
                }
            }
        }

        // Check for player picking up stuck arrows
        let player_pos = self.renderer.camera().position;
        let pickup_radius = 1.5_f64;
        let mut arrows_to_pickup = 0u32;

        self.projectiles.projectiles.retain(|projectile| {
            if projectile.stuck && !projectile.dead {
                // Check distance to player
                let dx = projectile.x - player_pos.x as f64;
                let dy = projectile.y - player_pos.y as f64;
                let dz = projectile.z - player_pos.z as f64;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                if dist < pickup_radius {
                    arrows_to_pickup += 1;
                    return false; // Remove from projectiles
                }
            }
            true
        });

        // Add picked up arrows to hotbar
        if arrows_to_pickup > 0 {
            let arrow_type = ItemType::Item(2); // Arrow
            let mut added = false;

            // Try to add to existing stack
            for existing in self.hotbar.slots.iter_mut().flatten() {
                if existing.item_type == arrow_type && existing.can_add(arrows_to_pickup) {
                    existing.count += arrows_to_pickup;
                    added = true;
                    break;
                }
            }

            // Try empty slot
            if !added {
                for slot in &mut self.hotbar.slots {
                    if slot.is_none() {
                        *slot = Some(ItemStack::new(arrow_type, arrows_to_pickup));
                        added = true;
                        break;
                    }
                }
            }

            if added {
                tracing::debug!("Picked up {} arrow(s)", arrows_to_pickup);
            }
        }
    }

    /// Destroy blocks in a radius (for creeper explosions)
    fn destroy_blocks_in_radius(&mut self, cx: f64, cy: f64, cz: f64, radius: f32) {
        let radius_i = radius.ceil() as i32;
        let mut affected_chunks: std::collections::HashSet<ChunkPos> =
            std::collections::HashSet::new();

        // Iterate over all blocks in the explosion radius
        for dx in -radius_i..=radius_i {
            for dy in -radius_i..=radius_i {
                for dz in -radius_i..=radius_i {
                    // Check if block is within spherical radius
                    let dist = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                    if dist > radius {
                        continue;
                    }

                    let block_x = cx.floor() as i32 + dx;
                    let block_y = cy.floor() as i32 + dy;
                    let block_z = cz.floor() as i32 + dz;

                    // Skip if out of world bounds
                    if !(1..255).contains(&block_y) {
                        continue; // Don't destroy bedrock layer or above world
                    }

                    let chunk_x = block_x.div_euclid(CHUNK_SIZE_X as i32);
                    let chunk_z = block_z.div_euclid(CHUNK_SIZE_Z as i32);
                    let local_x = block_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
                    let local_y = block_y as usize;
                    let local_z = block_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

                    let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
                    if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                        let voxel = chunk.voxel(local_x, local_y, local_z);
                        // Don't destroy bedrock (block 10) or air
                        if voxel.id != BLOCK_AIR && voxel.id != 10 {
                            chunk.set_voxel(local_x, local_y, local_z, Voxel::default());
                            affected_chunks.insert(chunk_pos);
                        }
                    }
                }
            }
        }

        // Update lighting and meshes for affected chunks
        for chunk_pos in affected_chunks {
            self.recompute_chunk_lighting(chunk_pos);
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
                }
            }
        }
    }

    /// Try to attack a mob that the player is looking at.
    /// Returns true if a mob was attacked.
    fn try_attack_mob(&mut self) -> bool {
        // Get camera position and direction
        let camera = self.renderer.camera();
        let origin = camera.position;
        let dir = camera.forward();

        // Attack reach (slightly more than block reach for mobs)
        const ATTACK_REACH: f32 = 4.0;

        // Check each mob to see if the ray hits it
        let mut closest_hit: Option<(usize, f32)> = None;

        for (idx, mob) in self.mobs.iter().enumerate() {
            // Simple AABB collision for mob
            let mob_size = mob.mob_type.size();
            let mob_height = mob_size * 2.0; // Approximate height

            let mob_min = glam::Vec3::new(
                mob.x as f32 - mob_size,
                mob.y as f32,
                mob.z as f32 - mob_size,
            );
            let mob_max = glam::Vec3::new(
                mob.x as f32 + mob_size,
                mob.y as f32 + mob_height,
                mob.z as f32 + mob_size,
            );

            // Simple ray-AABB intersection
            if let Some(t) = ray_aabb_intersect(origin, dir, mob_min, mob_max) {
                if t > 0.0
                    && t < ATTACK_REACH
                    && (closest_hit.is_none() || t < closest_hit.unwrap().1)
                {
                    closest_hit = Some((idx, t));
                }
            }
        }

        // Attack the closest mob
        if let Some((idx, _distance)) = closest_hit {
            let tool = self.hotbar.selected_tool();
            let mut damage = calculate_attack_damage(tool);

            // Critical hit detection: 1.5x damage if player is falling
            // Check if player has significant downward velocity
            let is_critical = self.player_physics.velocity.y < -0.1;
            if is_critical {
                damage *= 1.5;
            }

            // Calculate knockback direction
            let mob = &self.mobs[idx];
            let dx = mob.x - origin.x as f64;
            let dz = mob.z - origin.z as f64;

            // Apply damage and knockback
            let mob = &mut self.mobs[idx];
            let _died = mob.damage(damage);
            mob.apply_knockback(dx, dz, 0.5);

            if is_critical {
                tracing::info!(
                    "CRITICAL HIT! Attacked {:?} for {:.1} damage (health: {:.1})",
                    mob.mob_type,
                    damage,
                    mob.health
                );
            } else {
                tracing::info!(
                    "Attacked {:?} for {:.1} damage (health: {:.1})",
                    mob.mob_type,
                    damage,
                    mob.health
                );
            }

            // Use tool durability if we have a tool
            if let Some(item) = self.hotbar.selected_item_mut() {
                if let ItemType::Tool(_, _) = item.item_type {
                    if let Some(durability) = item.durability.as_mut() {
                        *durability = durability.saturating_sub(1);
                        if *durability == 0 {
                            // Tool broke
                            self.hotbar.slots[self.hotbar.selected] = None;
                            tracing::info!("Tool broke!");
                        }
                    }
                }
            }

            return true;
        }

        false
    }

    /// Toggle inventory UI open/closed
    fn toggle_inventory(&mut self) {
        self.inventory_open = !self.inventory_open;
        self.crafting_open = false; // Close crafting when toggling inventory
        if self.inventory_open {
            // Release cursor when inventory is open
            let _ = self.input.enter_ui_overlay(&self.window);
            tracing::info!("Inventory opened");
        } else {
            // Capture cursor when inventory is closed
            let _ = self.input.enter_gameplay(&self.window);
            tracing::info!("Inventory closed");
        }
    }

    /// Open crafting table UI
    fn open_crafting(&mut self) {
        self.crafting_open = true;
        self.inventory_open = false; // Close inventory when opening crafting
                                     // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        tracing::info!("Crafting table opened");
    }

    /// Close crafting UI
    fn close_crafting(&mut self) {
        self.crafting_open = false;
        // Clear crafting grid
        for row in &mut self.crafting_grid {
            for slot in row {
                *slot = None;
            }
        }
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        tracing::info!("Crafting closed");
    }

    /// Open furnace UI at the given position
    fn open_furnace(&mut self, block_pos: IVec3) {
        self.furnace_open = true;
        self.open_furnace_pos = Some(block_pos);
        self.inventory_open = false;
        self.crafting_open = false;
        // Create furnace state if it doesn't exist
        self.furnaces.entry(block_pos).or_default();
        // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        tracing::info!("Furnace opened at {:?}", block_pos);
    }

    /// Close furnace UI
    fn close_furnace(&mut self) {
        self.furnace_open = false;
        self.open_furnace_pos = None;
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        tracing::info!("Furnace closed");
    }

    /// Open enchanting table UI at the given position
    fn open_enchanting_table(&mut self, block_pos: IVec3) {
        self.enchanting_open = true;
        self.open_enchanting_pos = Some(block_pos);
        self.inventory_open = false;
        self.crafting_open = false;
        self.furnace_open = false;
        // Count nearby bookshelves first (before borrowing enchanting_tables)
        let bookshelf_count = self.count_nearby_bookshelves(block_pos);
        // Create enchanting table state if it doesn't exist and update bookshelf count
        let table = self.enchanting_tables.entry(block_pos).or_default();
        table.set_bookshelf_count(bookshelf_count);
        // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        tracing::info!(
            "Enchanting table opened at {:?} with {} bookshelves",
            block_pos,
            bookshelf_count
        );
    }

    /// Close enchanting table UI
    fn close_enchanting_table(&mut self) {
        self.enchanting_open = false;
        self.open_enchanting_pos = None;
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        tracing::info!("Enchanting table closed");
    }

    /// Count bookshelves within 2 blocks of the enchanting table (vanilla mechanics)
    fn count_nearby_bookshelves(&self, table_pos: IVec3) -> u32 {
        // Vanilla: bookshelves must be 2 blocks away, 1 block higher, with air in between
        // Simplified: check 5x5x2 area centered on table, 1 block up
        let bookshelf_id: BlockId = 47; // Bookshelf block ID from blocks.json

        let mut count = 0u32;
        for dy in 0..2 {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    // Skip center 3x3 area (too close to table)
                    if dx.abs() <= 1 && dz.abs() <= 1 {
                        continue;
                    }

                    let check_pos = IVec3::new(
                        table_pos.x + dx,
                        table_pos.y + dy,
                        table_pos.z + dz,
                    );

                    if let Some(block_id) = self.get_block_at(check_pos) {
                        if block_id == bookshelf_id {
                            count += 1;
                        }
                    }
                }
            }
        }

        count.min(15) // Cap at 15 bookshelves (vanilla limit)
    }

    /// Get block ID at world position
    fn get_block_at(&self, pos: IVec3) -> Option<BlockId> {
        let chunk_x = pos.x.div_euclid(16);
        let chunk_z = pos.z.div_euclid(16);
        let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

        if let Some(chunk) = self.chunks.get(&chunk_pos) {
            let local_x = pos.x.rem_euclid(16) as usize;
            let local_y = pos.y as usize;
            let local_z = pos.z.rem_euclid(16) as usize;

            if local_y < 256 {
                return Some(chunk.voxel(local_x, local_y, local_z).id);
            }
        }
        None
    }

    /// Update all furnaces in the world
    fn update_furnaces(&mut self, dt: f32) {
        let mut lit_changes: Vec<(IVec3, bool)> = Vec::new();

        for (pos, furnace) in &mut self.furnaces {
            let was_lit = furnace.is_lit;
            furnace.update(dt);
            if was_lit != furnace.is_lit {
                lit_changes.push((*pos, furnace.is_lit));
            }
        }

        // Update furnace block states (lit/unlit)
        for (pos, is_lit) in lit_changes {
            let chunk_x = pos.x.div_euclid(16);
            let chunk_z = pos.z.div_euclid(16);
            let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

            if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                let local_x = pos.x.rem_euclid(16) as usize;
                let local_y = pos.y as usize;
                let local_z = pos.z.rem_euclid(16) as usize;

                if local_y < 256 {
                    let new_id = if is_lit {
                        BLOCK_FURNACE_LIT
                    } else {
                        BLOCK_FURNACE
                    };
                    let mut voxel = chunk.voxel(local_x, local_y, local_z);
                    voxel.id = new_id;
                    chunk.set_voxel(local_x, local_y, local_z, voxel);
                }
            }
        }
    }

    /// Convert dropped item type to core item type
    fn convert_dropped_item_type(drop_type: DroppedItemType) -> Option<ItemType> {
        use mdminecraft_core::item::FoodType;

        // First check if it's a block item
        if let Some(block_id) = drop_type.to_block() {
            return Some(ItemType::Block(block_id));
        }

        // Handle non-block items
        match drop_type {
            DroppedItemType::RawPork | DroppedItemType::RawBeef => {
                Some(ItemType::Food(FoodType::RawMeat))
            }
            DroppedItemType::Apple => Some(ItemType::Food(FoodType::Apple)),
            DroppedItemType::Stick => Some(ItemType::Item(100)), // Arbitrary item ID
            DroppedItemType::Feather => Some(ItemType::Item(101)),
            DroppedItemType::Leather => Some(ItemType::Item(102)),
            DroppedItemType::Wool => Some(ItemType::Item(103)),
            DroppedItemType::Egg => Some(ItemType::Item(104)),
            DroppedItemType::Sapling => Some(ItemType::Item(105)),
            _ => None,
        }
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

/// Render the hunger bar (10 shanks on the right side)
fn render_hunger_bar(ctx: &egui::Context, health: &PlayerHealth) {
    egui::Area::new(egui::Id::new("hunger_bar"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-10.0, -70.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Show numerical hunger
                let hunger_text = format!("{:.1}/{:.0}", health.hunger, health.max_hunger);
                let text_color = if health.hunger < 6.0 {
                    egui::Color32::RED
                } else if health.hunger < 10.0 {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::WHITE
                };
                ui.label(
                    egui::RichText::new(hunger_text)
                        .size(14.0)
                        .color(text_color),
                );

                ui.add_space(10.0);

                // Draw shanks (drumsticks)
                let max_shanks = (health.max_hunger / 2.0) as i32; // 20 hunger = 10 shanks
                let current_shanks = health.hunger / 2.0;

                for i in 0..max_shanks {
                    let shank_value = current_shanks - i as f32;
                    let (symbol, color) = if shank_value >= 1.0 {
                        ("🍖", egui::Color32::from_rgb(200, 150, 100)) // Full shank - brown
                    } else if shank_value >= 0.5 {
                        ("🍖", egui::Color32::from_rgb(150, 100, 50)) // Half shank - darker
                    } else {
                        ("🦴", egui::Color32::from_rgb(100, 100, 100)) // Empty - bone/gray
                    };

                    ui.label(egui::RichText::new(symbol).size(18.0).color(color));
                }
            });
        });
}

/// Render the armor bar (above health bar)
fn render_armor_bar(ctx: &egui::Context, armor: &PlayerArmor) {
    let defense = armor.total_defense();

    // Only show if player has armor equipped
    if defense > 0 {
        egui::Area::new(egui::Id::new("armor_bar"))
            .anchor(egui::Align2::LEFT_BOTTOM, [10.0, -100.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Draw armor icons (10 max, each represents 2 defense)
                    let max_icons = 10;
                    let filled_icons = (defense as f32 / 2.0).ceil() as u32;

                    for i in 0..max_icons {
                        let icon_value = defense.saturating_sub(i * 2);
                        let (symbol, color) = if icon_value >= 2 {
                            ("🛡️", egui::Color32::from_rgb(220, 220, 220)) // Full armor
                        } else if icon_value >= 1 {
                            ("🛡️", egui::Color32::from_rgb(150, 150, 150)) // Half armor
                        } else {
                            ("🛡️", egui::Color32::from_rgb(60, 60, 60)) // Empty
                        };

                        if i < filled_icons || icon_value > 0 {
                            ui.label(egui::RichText::new(symbol).size(16.0).color(color));
                        }
                    }

                    ui.add_space(5.0);

                    // Show defense value
                    ui.label(
                        egui::RichText::new(format!("{} defense", defense))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                    );
                });

                // Show individual armor pieces with durability
                ui.horizontal(|ui| {
                    if let Some(piece) = armor.get(ArmorSlot::Helmet) {
                        render_armor_piece_icon(ui, "H", piece.durability_ratio());
                    }
                    if let Some(piece) = armor.get(ArmorSlot::Chestplate) {
                        render_armor_piece_icon(ui, "C", piece.durability_ratio());
                    }
                    if let Some(piece) = armor.get(ArmorSlot::Leggings) {
                        render_armor_piece_icon(ui, "L", piece.durability_ratio());
                    }
                    if let Some(piece) = armor.get(ArmorSlot::Boots) {
                        render_armor_piece_icon(ui, "B", piece.durability_ratio());
                    }
                });
            });
    }
}

/// Render a small armor piece icon with durability indicator
fn render_armor_piece_icon(ui: &mut egui::Ui, label: &str, durability_ratio: f32) {
    let color = if durability_ratio > 0.5 {
        egui::Color32::from_rgb(100, 200, 100) // Green
    } else if durability_ratio > 0.25 {
        egui::Color32::from_rgb(200, 200, 100) // Yellow
    } else {
        egui::Color32::from_rgb(200, 100, 100) // Red
    };

    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 180))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(2.0);

    frame.show(ui, |ui| {
        ui.label(egui::RichText::new(label).size(10.0).color(color));
    });
}

/// Render tool durability bar (only shows when a tool is selected)
fn render_tool_durability(ctx: &egui::Context, hotbar: &Hotbar) {
    // Check if selected item is a tool with durability
    if let Some(item) = hotbar.selected_item() {
        if let ItemType::Tool(tool_type, material) = item.item_type {
            if let Some(durability) = item.durability {
                let max_durability = item.max_durability().unwrap_or(1);
                let percent = (durability as f32 / max_durability as f32 * 100.0) as u32;

                egui::Area::new(egui::Id::new("tool_durability"))
                    .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -90.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            // Tool icon/name
                            let tool_name = match tool_type {
                                ToolType::Pickaxe => "⛏",
                                ToolType::Axe => "🪓",
                                ToolType::Shovel => "🪏",
                                ToolType::Sword => "⚔",
                                ToolType::Hoe => "🌾",
                            };
                            let material_name = match material {
                                ToolMaterial::Wood => "Wood",
                                ToolMaterial::Stone => "Stone",
                                ToolMaterial::Iron => "Iron",
                                ToolMaterial::Gold => "Gold",
                                ToolMaterial::Diamond => "Diamond",
                            };

                            ui.label(
                                egui::RichText::new(format!("{} {} ", tool_name, material_name))
                                    .size(12.0)
                                    .color(egui::Color32::WHITE),
                            );

                            // Durability bar
                            let bar_width = 80.0;
                            let bar_height = 8.0;
                            let bar_color = if percent > 50 {
                                egui::Color32::GREEN
                            } else if percent > 20 {
                                egui::Color32::YELLOW
                            } else {
                                egui::Color32::RED
                            };

                            let (response, painter) = ui.allocate_painter(
                                egui::vec2(bar_width, bar_height),
                                egui::Sense::hover(),
                            );
                            let rect = response.rect;

                            // Background
                            painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(40, 40, 40));

                            // Foreground (durability)
                            let fill_width = bar_width * (percent as f32 / 100.0);
                            painter.rect_filled(
                                egui::Rect::from_min_size(
                                    rect.min,
                                    egui::vec2(fill_width, bar_height),
                                ),
                                2.0,
                                bar_color,
                            );

                            // Border
                            painter.rect_stroke(
                                rect,
                                2.0,
                                egui::Stroke::new(1.0, egui::Color32::GRAY),
                            );

                            // Percentage text
                            ui.label(
                                egui::RichText::new(format!(" {}%", percent))
                                    .size(10.0)
                                    .color(bar_color),
                            );
                        });
                    });
            }
        }
    }
}

/// Render the XP bar above the hotbar
fn render_xp_bar(ctx: &egui::Context, xp: &PlayerXP) {
    egui::Area::new(egui::Id::new("xp_bar"))
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -85.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // XP bar background and foreground
                let bar_width = 182.0; // Match hotbar width roughly
                let bar_height = 5.0;

                let (response, painter) =
                    ui.allocate_painter(egui::vec2(bar_width, bar_height), egui::Sense::hover());
                let rect = response.rect;

                // Background (dark)
                painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(20, 20, 20));

                // Foreground (green XP progress)
                let fill_width = bar_width * xp.progress();
                if fill_width > 0.0 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, bar_height)),
                        2.0,
                        egui::Color32::from_rgb(128, 255, 32), // Bright green
                    );
                }

                // Border
                painter.rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60)),
                );
            });

            // Level number centered above the bar
            if xp.level > 0 {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}", xp.level))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(128, 255, 32))
                            .strong(),
                    );
                });
            }
        });
}

/// Render the death screen overlay
/// Returns (respawn_clicked, menu_clicked)
fn render_death_screen(ctx: &egui::Context, death_message: &str) -> (bool, bool) {
    let mut respawn_clicked = false;
    let mut menu_clicked = false;

    // Semi-transparent red overlay
    egui::Area::new(egui::Id::new("death_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(139, 0, 0, 180), // Dark red
            );
        });

    // Death screen content
    egui::Area::new(egui::Id::new("death_screen"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                // "You Died!" title
                ui.label(
                    egui::RichText::new("You Died!")
                        .size(64.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );

                ui.add_space(20.0);

                // Death message
                ui.label(
                    egui::RichText::new(death_message)
                        .size(24.0)
                        .color(egui::Color32::from_rgb(255, 200, 200)),
                );

                ui.add_space(40.0);

                // Respawn button
                let respawn_button = egui::Button::new(
                    egui::RichText::new("Respawn")
                        .size(24.0)
                        .color(egui::Color32::WHITE),
                )
                .min_size(egui::vec2(200.0, 50.0))
                .fill(egui::Color32::from_rgb(60, 120, 60));

                if ui.add(respawn_button).clicked() {
                    respawn_clicked = true;
                }

                ui.add_space(15.0);

                // Return to menu button
                let menu_button = egui::Button::new(
                    egui::RichText::new("Title Screen")
                        .size(18.0)
                        .color(egui::Color32::LIGHT_GRAY),
                )
                .min_size(egui::vec2(180.0, 40.0))
                .fill(egui::Color32::from_rgb(80, 80, 80));

                if ui.add(menu_button).clicked() {
                    menu_clicked = true;
                }

                ui.add_space(20.0);
            });
        });

    (respawn_clicked, menu_clicked)
}

/// Render the inventory UI
/// Returns true if the close button was clicked
fn render_inventory(ctx: &egui::Context, hotbar: &Hotbar, inventory: &Inventory) -> bool {
    let mut close_clicked = false;

    // Semi-transparent dark overlay
    egui::Area::new(egui::Id::new("inventory_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    // Inventory window
    egui::Window::new("Inventory")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(400.0);

            // Close button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Inventory").size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        close_clicked = true;
                    }
                });
            });

            ui.separator();

            // Main inventory (27 slots in 3 rows of 9)
            ui.label("Main Inventory");
            for row in 0..3 {
                ui.horizontal(|ui| {
                    for col in 0..9 {
                        let slot_idx = 9 + row * 9 + col; // Slots 9-35 are main inventory
                        render_inventory_slot_world(ui, inventory.get(slot_idx), false);
                    }
                });
            }

            ui.add_space(10.0);
            ui.separator();

            // Hotbar (9 slots)
            ui.label("Hotbar");
            ui.horizontal(|ui| {
                for i in 0..9 {
                    let is_selected = i == hotbar.selected;
                    render_inventory_slot_core(ui, hotbar.slots[i].as_ref(), is_selected);
                }
            });

            ui.add_space(5.0);
            ui.label(
                egui::RichText::new("Press E to close")
                    .size(12.0)
                    .color(egui::Color32::GRAY),
            );
        });

    close_clicked
}

/// Render a single inventory slot with core::ItemStack
fn render_inventory_slot_core(
    ui: &mut egui::Ui,
    item: Option<&mdminecraft_core::ItemStack>,
    is_selected: bool,
) {
    let frame = if is_selected {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200))
            .stroke(egui::Stroke::new(2.0, egui::Color32::WHITE))
            .inner_margin(4.0)
    } else {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 180))
            .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_GRAY))
            .inner_margin(4.0)
    };

    frame.show(ui, |ui| {
        ui.set_min_size(egui::vec2(36.0, 36.0));
        ui.set_max_size(egui::vec2(36.0, 36.0));

        if let Some(stack) = item {
            ui.vertical_centered(|ui| {
                // Item name (abbreviated)
                let name = match stack.item_type {
                    mdminecraft_core::ItemType::Tool(tool, _) => format!("{:?}", tool),
                    mdminecraft_core::ItemType::Block(id) => format!("B{}", id),
                    mdminecraft_core::ItemType::Food(food) => format!("{:?}", food),
                    mdminecraft_core::ItemType::Item(id) => format!("I{}", id),
                };
                ui.label(
                    egui::RichText::new(&name[..name.len().min(4)])
                        .size(9.0)
                        .color(egui::Color32::WHITE),
                );

                // Count
                if stack.count > 1 {
                    ui.label(
                        egui::RichText::new(format!("{}", stack.count))
                            .size(10.0)
                            .color(egui::Color32::YELLOW),
                    );
                }
            });
        }
    });
}

/// Render a single inventory slot with world::ItemStack
fn render_inventory_slot_world(
    ui: &mut egui::Ui,
    item: Option<&mdminecraft_world::ItemStack>,
    is_selected: bool,
) {
    let frame = if is_selected {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200))
            .stroke(egui::Stroke::new(2.0, egui::Color32::WHITE))
            .inner_margin(4.0)
    } else {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 180))
            .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_GRAY))
            .inner_margin(4.0)
    };

    frame.show(ui, |ui| {
        ui.set_min_size(egui::vec2(36.0, 36.0));
        ui.set_max_size(egui::vec2(36.0, 36.0));

        if let Some(stack) = item {
            ui.vertical_centered(|ui| {
                // Show item ID
                ui.label(
                    egui::RichText::new(format!("#{}", stack.item_id))
                        .size(9.0)
                        .color(egui::Color32::WHITE),
                );

                // Count
                if stack.count > 1 {
                    ui.label(
                        egui::RichText::new(format!("{}", stack.count))
                            .size(10.0)
                            .color(egui::Color32::YELLOW),
                    );
                }
            });
        }
    });
}

/// A crafting recipe: (required inputs, output item, output count)
type CraftingRecipe = (Vec<(ItemType, u32)>, ItemType, u32);

/// Get available crafting recipes as (inputs, output, output_count)
/// Inputs are a list of (ItemType, count) required
fn get_crafting_recipes() -> Vec<CraftingRecipe> {
    vec![
        // Furnace: 8 cobblestone → furnace
        (vec![(ItemType::Block(6), 8)], ItemType::Block(18), 1),
        // Planks: 1 log → 4 planks
        (vec![(ItemType::Block(3), 1)], ItemType::Block(7), 4),
        // Sticks: 2 planks → 4 sticks
        (vec![(ItemType::Block(7), 2)], ItemType::Item(3), 4), // Item(3) = Stick
        // Bow: 3 sticks + 3 string
        (
            vec![(ItemType::Item(3), 3), (ItemType::Item(4), 3)],
            ItemType::Item(1),
            1,
        ), // Item(4) = String
        // Arrow: 1 flint + 1 stick + 1 feather
        (
            vec![
                (ItemType::Item(5), 1),
                (ItemType::Item(3), 1),
                (ItemType::Item(6), 1),
            ],
            ItemType::Item(2),
            4,
        ), // Item(5) = Flint, Item(6) = Feather
        // Leather armor (Item(102) = Leather)
        // Leather Helmet: 5 leather
        (vec![(ItemType::Item(102), 5)], ItemType::Item(20), 1), // Item(20) = LeatherHelmet
        // Leather Chestplate: 8 leather
        (vec![(ItemType::Item(102), 8)], ItemType::Item(21), 1), // Item(21) = LeatherChestplate
        // Leather Leggings: 7 leather
        (vec![(ItemType::Item(102), 7)], ItemType::Item(22), 1), // Item(22) = LeatherLeggings
        // Leather Boots: 4 leather
        (vec![(ItemType::Item(102), 4)], ItemType::Item(23), 1), // Item(23) = LeatherBoots
        // Iron armor (Item(7) = IronIngot)
        // Iron Helmet: 5 iron ingots
        (vec![(ItemType::Item(7), 5)], ItemType::Item(10), 1), // Item(10) = IronHelmet
        // Iron Chestplate: 8 iron ingots
        (vec![(ItemType::Item(7), 8)], ItemType::Item(11), 1), // Item(11) = IronChestplate
        // Iron Leggings: 7 iron ingots
        (vec![(ItemType::Item(7), 7)], ItemType::Item(12), 1), // Item(12) = IronLeggings
        // Iron Boots: 4 iron ingots
        (vec![(ItemType::Item(7), 4)], ItemType::Item(13), 1), // Item(13) = IronBoots
        // Diamond armor (Item(14) = Diamond)
        // Diamond Helmet: 5 diamonds
        (vec![(ItemType::Item(14), 5)], ItemType::Item(30), 1), // Item(30) = DiamondHelmet
        // Diamond Chestplate: 8 diamonds
        (vec![(ItemType::Item(14), 8)], ItemType::Item(31), 1), // Item(31) = DiamondChestplate
        // Diamond Leggings: 7 diamonds
        (vec![(ItemType::Item(14), 7)], ItemType::Item(32), 1), // Item(32) = DiamondLeggings
        // Diamond Boots: 4 diamonds
        (vec![(ItemType::Item(14), 4)], ItemType::Item(33), 1), // Item(33) = DiamondBoots
    ]
}

/// Check if the crafting grid matches a recipe
fn check_crafting_recipe(crafting_grid: &[[Option<ItemStack>; 3]; 3]) -> Option<(ItemType, u32)> {
    // Gather items from grid
    let mut grid_items: std::collections::HashMap<ItemType, u32> = std::collections::HashMap::new();
    for row in crafting_grid {
        for stack in row.iter().flatten() {
            *grid_items.entry(stack.item_type).or_insert(0) += stack.count;
        }
    }

    // Check each recipe
    for (inputs, output, count) in get_crafting_recipes() {
        let mut matches = true;
        let mut required: std::collections::HashMap<ItemType, u32> =
            std::collections::HashMap::new();
        for (item_type, needed) in &inputs {
            *required.entry(*item_type).or_insert(0) += needed;
        }

        // Check if grid has exactly the required items
        for (item_type, needed) in &required {
            match grid_items.get(item_type) {
                Some(have) if *have >= *needed => {}
                _ => {
                    matches = false;
                    break;
                }
            }
        }

        // Also check no extra item types
        if matches {
            for item_type in grid_items.keys() {
                if !required.contains_key(item_type) {
                    matches = false;
                    break;
                }
            }
        }

        if matches {
            return Some((output, count));
        }
    }
    None
}

/// Render the crafting table UI
/// Returns (close_clicked, crafted_item)
fn render_crafting(
    ctx: &egui::Context,
    crafting_grid: &mut [[Option<ItemStack>; 3]; 3],
    hotbar: &mut Hotbar,
) -> (bool, Option<ItemStack>) {
    let mut close_clicked = false;
    let mut crafted_item = None;

    // Check for matching recipe
    let recipe_result = check_crafting_recipe(crafting_grid);

    // Semi-transparent dark overlay
    egui::Area::new(egui::Id::new("crafting_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    // Crafting window
    egui::Window::new("Crafting Table")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(400.0);

            // Close button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Crafting").size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        close_clicked = true;
                    }
                });
            });

            ui.separator();

            // Show hotbar for material selection
            ui.label("Hotbar (click to add to grid):");
            let mut clicked_slot: Option<usize> = None;
            ui.horizontal(|ui| {
                for i in 0..9 {
                    if let Some(stack) = &hotbar.slots[i] {
                        let btn_text = format!("{}", i + 1);
                        if ui.button(&btn_text).clicked() {
                            clicked_slot = Some(i);
                        }
                        ui.label(
                            format!("{:?}", stack.item_type)
                                .chars()
                                .take(6)
                                .collect::<String>(),
                        );
                    }
                }
            });

            // Handle adding item to crafting grid (after UI drawing to avoid borrow issues)
            if let Some(i) = clicked_slot {
                if let Some(stack) = &hotbar.slots[i] {
                    let item_type = stack.item_type;
                    // Find first empty grid slot
                    let mut added = false;
                    for row in crafting_grid.iter_mut() {
                        for slot in row.iter_mut() {
                            if slot.is_none() {
                                *slot = Some(ItemStack::new(item_type, 1));
                                added = true;
                                break;
                            }
                        }
                        if added {
                            break;
                        }
                    }
                    if added {
                        // Reduce hotbar count
                        if let Some(hotbar_stack) = &mut hotbar.slots[i] {
                            if hotbar_stack.count > 1 {
                                hotbar_stack.count -= 1;
                            } else {
                                hotbar.slots[i] = None;
                            }
                        }
                    }
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                // 3x3 Crafting grid with clickable slots
                ui.vertical(|ui| {
                    ui.label("Crafting Grid (click to remove)");
                    #[allow(clippy::needless_range_loop)]
                    for row_idx in 0..3 {
                        ui.horizontal(|ui| {
                            #[allow(clippy::needless_range_loop)]
                            for col_idx in 0..3 {
                                let slot = &crafting_grid[row_idx][col_idx];
                                if render_crafting_slot_clickable(ui, slot.as_ref()) {
                                    // Return item to hotbar
                                    if let Some(stack) = crafting_grid[row_idx][col_idx].take() {
                                        // Try to add back to hotbar
                                        for hotbar_slot in &mut hotbar.slots {
                                            if let Some(existing) = hotbar_slot {
                                                if existing.item_type == stack.item_type {
                                                    existing.count += stack.count;
                                                    break;
                                                }
                                            } else {
                                                *hotbar_slot = Some(stack);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                });

                ui.add_space(20.0);

                // Arrow and result
                ui.vertical(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("→")
                            .size(24.0)
                            .color(egui::Color32::WHITE),
                    );
                });

                ui.add_space(20.0);

                // Result slot with craft button
                ui.vertical(|ui| {
                    ui.label("Result");
                    ui.add_space(10.0);
                    if let Some((output, count)) = &recipe_result {
                        let result_stack = ItemStack::new(*output, *count);
                        render_crafting_slot(ui, Some(&result_stack));
                        if ui.button("Craft").clicked() {
                            // Clear crafting grid and produce output
                            for row in crafting_grid.iter_mut() {
                                for slot in row.iter_mut() {
                                    *slot = None;
                                }
                            }
                            crafted_item = Some(result_stack);
                        }
                    } else {
                        render_crafting_slot(ui, None);
                        ui.label(
                            egui::RichText::new("No recipe")
                                .size(10.0)
                                .color(egui::Color32::GRAY),
                        );
                    }
                });
            });

            ui.add_space(10.0);
            ui.separator();

            ui.label(
                egui::RichText::new("Click hotbar items to add, click grid slots to remove")
                    .size(12.0)
                    .color(egui::Color32::GRAY),
            );
        });

    (close_clicked, crafted_item)
}

/// Render a single crafting slot
fn render_crafting_slot(ui: &mut egui::Ui, item: Option<&ItemStack>) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 60, 200))
        .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
        .inner_margin(4.0);

    frame.show(ui, |ui| {
        ui.set_min_size(egui::vec2(40.0, 40.0));
        ui.set_max_size(egui::vec2(40.0, 40.0));

        if let Some(stack) = item {
            ui.vertical_centered(|ui| {
                let name = match stack.item_type {
                    mdminecraft_core::ItemType::Tool(tool, _) => format!("{:?}", tool),
                    mdminecraft_core::ItemType::Block(id) => format!("B{}", id),
                    mdminecraft_core::ItemType::Food(food) => format!("{:?}", food),
                    mdminecraft_core::ItemType::Item(id) => format!("I{}", id),
                };
                ui.label(
                    egui::RichText::new(&name[..name.len().min(4)])
                        .size(10.0)
                        .color(egui::Color32::WHITE),
                );

                if stack.count > 1 {
                    ui.label(
                        egui::RichText::new(format!("{}", stack.count))
                            .size(10.0)
                            .color(egui::Color32::YELLOW),
                    );
                }
            });
        }
    });
}

/// Render a clickable crafting slot, returns true if clicked
fn render_crafting_slot_clickable(ui: &mut egui::Ui, item: Option<&ItemStack>) -> bool {
    let mut clicked = false;
    let fill = if item.is_some() {
        egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200)
    } else {
        egui::Color32::from_rgba_unmultiplied(40, 40, 40, 200)
    };

    let response = ui.allocate_response(egui::vec2(40.0, 40.0), egui::Sense::click());
    let rect = response.rect;

    if response.clicked() && item.is_some() {
        clicked = true;
    }

    ui.painter()
        .rect(rect, 2.0, fill, egui::Stroke::new(1.0, egui::Color32::GRAY));

    if let Some(stack) = item {
        let name = match stack.item_type {
            mdminecraft_core::ItemType::Tool(tool, _) => format!("{:?}", tool),
            mdminecraft_core::ItemType::Block(id) => format!("B{}", id),
            mdminecraft_core::ItemType::Food(food) => format!("{:?}", food),
            mdminecraft_core::ItemType::Item(id) => format!("I{}", id),
        };

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &name[..name.len().min(4)],
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );

        if stack.count > 1 {
            ui.painter().text(
                rect.right_bottom() - egui::vec2(4.0, 4.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{}", stack.count),
                egui::FontId::proportional(9.0),
                egui::Color32::YELLOW,
            );
        }
    }

    clicked
}

/// Render the furnace UI
/// Returns true if the close button was clicked
fn render_furnace(ctx: &egui::Context, furnace: &mut FurnaceState) -> bool {
    let mut close_clicked = false;

    // Semi-transparent dark overlay
    egui::Area::new(egui::Id::new("furnace_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    // Furnace window
    egui::Window::new("Furnace")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(300.0);

            // Close button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Furnace").size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        close_clicked = true;
                    }
                });
            });

            ui.separator();

            ui.horizontal(|ui| {
                // Input and Fuel slots column
                ui.vertical(|ui| {
                    ui.label("Input (smeltable)");
                    render_furnace_slot(ui, furnace.input.as_ref(), "input");

                    ui.add_space(10.0);

                    ui.label("Fuel");
                    render_furnace_slot(ui, furnace.fuel.as_ref(), "fuel");
                });

                ui.add_space(20.0);

                // Arrow and progress
                ui.vertical(|ui| {
                    ui.add_space(20.0);

                    // Progress bar
                    let progress = furnace.smelt_progress;
                    let progress_bar = egui::ProgressBar::new(progress)
                        .desired_width(60.0)
                        .text(format!("{:.0}%", progress * 100.0));
                    ui.add(progress_bar);

                    ui.add_space(5.0);

                    // Fire indicator
                    let fire_color = if furnace.is_lit {
                        egui::Color32::from_rgb(255, 128, 0)
                    } else {
                        egui::Color32::DARK_GRAY
                    };
                    ui.label(egui::RichText::new("🔥").size(24.0).color(fire_color));

                    // Fuel remaining indicator
                    if furnace.fuel_remaining > 0.0 {
                        ui.label(
                            egui::RichText::new(format!("{:.1}", furnace.fuel_remaining))
                                .size(10.0)
                                .color(egui::Color32::YELLOW),
                        );
                    }
                });

                ui.add_space(20.0);

                // Output slot
                ui.vertical(|ui| {
                    ui.label("Output");
                    render_furnace_slot(ui, furnace.output.as_ref(), "output");
                });
            });

            ui.add_space(10.0);
            ui.separator();

            // Status text
            let status = if furnace.is_lit {
                "Smelting..."
            } else if furnace.input.is_some() && furnace.fuel.is_none() {
                "Need fuel"
            } else if furnace.input.is_none() {
                "Add smeltable item"
            } else {
                "Ready"
            };
            ui.label(
                egui::RichText::new(status)
                    .size(12.0)
                    .color(if furnace.is_lit {
                        egui::Color32::from_rgb(255, 200, 100)
                    } else {
                        egui::Color32::GRAY
                    }),
            );

            ui.add_space(5.0);

            // Quick-add buttons (temporary, for testing)
            ui.horizontal(|ui| {
                if ui.button("+ Iron Ore").clicked() {
                    furnace.add_input(DroppedItemType::IronOre, 1);
                }
                if ui.button("+ Coal").clicked() {
                    furnace.add_fuel(DroppedItemType::Coal, 1);
                }
                if ui.button("Take Output").clicked() {
                    let _ = furnace.take_output();
                }
            });

            ui.label(
                egui::RichText::new("Escape or X to close")
                    .size(11.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        });

    close_clicked
}

/// Render a single furnace slot
fn render_furnace_slot(ui: &mut egui::Ui, item: Option<&(DroppedItemType, u32)>, _slot_id: &str) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 60, 200))
        .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
        .inner_margin(4.0);

    frame.show(ui, |ui| {
        ui.set_min_size(egui::vec2(48.0, 48.0));
        ui.set_max_size(egui::vec2(48.0, 48.0));

        if let Some((item_type, count)) = item {
            ui.vertical_centered(|ui| {
                // Show item name
                let name = format!("{:?}", item_type);
                // Truncate to fit
                let display_name = if name.len() > 6 { &name[..6] } else { &name };
                ui.label(
                    egui::RichText::new(display_name)
                        .size(10.0)
                        .color(egui::Color32::WHITE),
                );

                // Count
                if *count > 1 {
                    ui.label(
                        egui::RichText::new(format!("{}", count))
                            .size(11.0)
                            .color(egui::Color32::YELLOW),
                    );
                }
            });
        }
    });
}

/// Result of enchanting table interaction
struct EnchantingResult {
    /// Whether close was requested
    close_requested: bool,
    /// Enchantment to apply (if any)
    enchantment_applied: Option<mdminecraft_core::Enchantment>,
    /// XP levels to consume (if enchantment applied)
    xp_to_consume: u32,
}

/// Render the enchanting table UI
/// Returns close state and any enchantment result
fn render_enchanting_table(
    ctx: &egui::Context,
    table: &mut EnchantingTableState,
    player_xp: &PlayerXP,
    selected_item_enchantable: bool,
) -> EnchantingResult {
    use mdminecraft_world::LAPIS_COSTS;

    let mut result = EnchantingResult {
        close_requested: false,
        enchantment_applied: None,
        xp_to_consume: 0,
    };

    // Semi-transparent dark overlay
    egui::Area::new(egui::Id::new("enchanting_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    // Enchanting table window
    egui::Window::new("Enchanting Table")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(350.0);

            // Close button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Enchanting Table").size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        result.close_requested = true;
                    }
                });
            });

            ui.separator();

            // Status info
            ui.horizontal(|ui| {
                ui.label(format!("Bookshelves: {}", table.bookshelf_count));
                ui.add_space(20.0);
                ui.label(format!("Lapis: {}", table.lapis_count));
                ui.add_space(20.0);
                ui.label(format!("Your Level: {}", player_xp.level));
            });

            ui.add_space(10.0);

            // Item status
            ui.horizontal(|ui| {
                if selected_item_enchantable {
                    ui.label(
                        egui::RichText::new("Selected hotbar item can be enchanted")
                            .color(egui::Color32::GREEN),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("Select an enchantable tool in your hotbar")
                            .color(egui::Color32::YELLOW),
                    );
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(5.0);

            // Enchantment options
            ui.label(egui::RichText::new("Enchantment Options:").size(14.0).strong());
            ui.add_space(5.0);

            // Track which slot was clicked (to apply after iteration)
            let mut apply_slot: Option<usize> = None;

            // Copy data we need to display (avoids borrow issues)
            let options_copy: [(Option<_>, u32); 3] = [
                (table.enchant_options[0], LAPIS_COSTS[0]),
                (table.enchant_options[1], LAPIS_COSTS[1]),
                (table.enchant_options[2], LAPIS_COSTS[2]),
            ];
            let lapis_count = table.lapis_count;

            for (slot_idx, (option, lapis_cost)) in options_copy.iter().enumerate() {
                if let Some(offer) = option {
                    // Can only enchant if player has enough XP, lapis, AND has an enchantable item
                    let can_afford = player_xp.level >= offer.level_cost
                        && lapis_count >= *lapis_cost
                        && selected_item_enchantable;

                    let text_color = if can_afford {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::DARK_GRAY
                    };

                    ui.horizontal(|ui| {
                        // Slot number
                        ui.label(
                            egui::RichText::new(format!("{}.", slot_idx + 1))
                                .size(14.0)
                                .color(text_color),
                        );

                        // Enchantment name and level
                        let enchant_name = format!(
                            "{:?} {}",
                            offer.enchantment.enchantment_type, offer.enchantment.level
                        );
                        ui.label(
                            egui::RichText::new(enchant_name)
                                .size(13.0)
                                .color(text_color),
                        );

                        ui.add_space(10.0);

                        // Cost info
                        ui.label(
                            egui::RichText::new(format!(
                                "Cost: {} levels, {} lapis",
                                offer.level_cost, lapis_cost
                            ))
                            .size(11.0)
                            .color(if can_afford {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::RED
                            }),
                        );

                        // Enchant button (disabled if can't afford or no enchantable item)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let button = egui::Button::new("Enchant").sense(if can_afford {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            });
                            if ui.add(button).clicked() && can_afford {
                                apply_slot = Some(slot_idx);
                            }
                        });
                    });
                } else {
                    ui.label(
                        egui::RichText::new(format!("{}. ---", slot_idx + 1))
                            .size(14.0)
                            .color(egui::Color32::DARK_GRAY),
                    );
                }

                ui.add_space(3.0);
            }

            // Apply enchantment after the loop (now we can mutably borrow table)
            if let Some(slot_idx) = apply_slot {
                if let Some((enchantment, levels_consumed)) = table.apply_enchantment(slot_idx) {
                    result.enchantment_applied = Some(enchantment);
                    result.xp_to_consume = levels_consumed;
                    tracing::info!(
                        "Enchanting: {:?} level {} (costs {} XP levels)",
                        enchantment.enchantment_type,
                        enchantment.level,
                        levels_consumed
                    );
                }
            }

            ui.add_space(10.0);
            ui.separator();

            // Test buttons for lapis (keep for testing)
            ui.horizontal(|ui| {
                if ui.button("+ Lapis").clicked() {
                    table.add_lapis(3);
                }
            });

            ui.label(
                egui::RichText::new("Escape or X to close")
                    .size(11.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        });

    result
}

/// Convert an ItemType to DroppedItemType for armor pieces
/// Returns the DroppedItemType if the item is armor, None otherwise
fn item_type_to_armor_dropped(item_type: ItemType) -> Option<DroppedItemType> {
    if let ItemType::Item(id) = item_type {
        match id {
            // Leather armor
            20 => Some(DroppedItemType::LeatherHelmet),
            21 => Some(DroppedItemType::LeatherChestplate),
            22 => Some(DroppedItemType::LeatherLeggings),
            23 => Some(DroppedItemType::LeatherBoots),
            // Iron armor
            10 => Some(DroppedItemType::IronHelmet),
            11 => Some(DroppedItemType::IronChestplate),
            12 => Some(DroppedItemType::IronLeggings),
            13 => Some(DroppedItemType::IronBoots),
            // Diamond armor
            30 => Some(DroppedItemType::DiamondHelmet),
            31 => Some(DroppedItemType::DiamondChestplate),
            32 => Some(DroppedItemType::DiamondLeggings),
            33 => Some(DroppedItemType::DiamondBoots),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::frames_to_complete;

    #[test]
    fn mining_completion_is_fps_independent() {
        let required = 1.2_f32; // seconds
        let frames_60hz: Vec<f32> = std::iter::repeat_n(1.0 / 60.0, 200).collect();
        let frames_30hz: Vec<f32> = std::iter::repeat_n(1.0 / 30.0, 200).collect();

        let f60 = frames_to_complete(required, &frames_60hz);
        let f30 = frames_to_complete(required, &frames_30hz);

        let t60 = (f60 as f32) * (1.0 / 60.0);
        let t30 = (f30 as f32) * (1.0 / 30.0);

        assert!(t60 >= required - 1e-3);
        assert!(t30 >= required - 1e-3);
    }
}
