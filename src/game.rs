//! Game world state - the actual 3D voxel game

use crate::{
    config::{load_block_registry, ControlsConfig},
    input::{ActionState, InputProcessor},
    scripted_input::ScriptedInputPlayer,
};
use anyhow::Result;
use glam::IVec3;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_audio::{AudioManager, AudioSettings, SoundId};
use mdminecraft_core::{
    item::{item_ids, potion_ids},
    DimensionId, Enchantment, EnchantmentType, ItemStack, ItemType, SimTick, ToolMaterial,
    ToolType,
};
use mdminecraft_render::{
    mesh_chunk_with_voxel_at, raycast, ChunkManager, ControlMode, DebugHud, Frustum, InputContext,
    InputState, ParticleEmitter, ParticleSystem, ParticleVertex, RaycastHit, Renderer,
    RendererConfig, TimeOfDay, UiRenderContext, WindowConfig, WindowManager,
};
#[cfg(feature = "ui3d_billboards")]
use mdminecraft_ui3d::render::{
    BillboardEmitter, BillboardFlags, BillboardInstance, BillboardRenderer,
};
use mdminecraft_world::{
    get_fluid_type, interactive_blocks,
    lighting::{init_skylight, stitch_light_seams, LightType},
    ArmorPiece, ArmorSlot, BlockEntitiesState, BlockEntityKey, BlockId, BlockPropertiesRegistry,
    BlockState, BrewingStandState, ChestState, Chunk, ChunkPos, CropGrowthSystem, CropPosition,
    EnchantingTableState, FluidPos, FluidSimulator, FluidType, FurnaceState, InteractionManager,
    Inventory, ItemManager, ItemType as DroppedItemType, Mob, MobSpawner, MobType, PlayerArmor,
    PlayerSave, PlayerTransform, PotionType, Projectile, ProjectileManager, RedstonePos,
    RedstoneSimulator, RegionStore, SimTime, StatusEffect, StatusEffectType, StatusEffects,
    SugarCaneGrowthSystem, SugarCanePosition, TerrainGenerator, Voxel, WeatherState, WeatherToggle,
    WorldEntitiesState, WorldMeta, WorldPoint, WorldState, BLOCK_AIR, BLOCK_BOOKSHELF,
    BLOCK_BREWING_STAND, BLOCK_BROWN_MUSHROOM, BLOCK_COBBLESTONE, BLOCK_CRAFTING_TABLE,
    BLOCK_ENCHANTING_TABLE, BLOCK_FURNACE, BLOCK_FURNACE_LIT, BLOCK_OAK_LOG, BLOCK_OAK_PLANKS,
    BLOCK_OBSIDIAN, BLOCK_SUGAR_CANE, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::Instant;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

const MAX_PARTICLES: usize = 8_192;
const PRECIPITATION_SPAWN_RATE: f32 = 480.0;
const PRECIPITATION_RADIUS: f32 = 18.0;
const PRECIPITATION_CEILING_OFFSET: f32 = 12.0;
/// Fixed simulation tick rate (20 TPS).
const TICK_RATE: f64 = 1.0 / 20.0;

// Core `ItemType::Item` IDs used by the client for non-legacy items.
// These are intentionally kept separate from the world brewing ingredient IDs
// (see `mdminecraft_core::item::item_ids`) to avoid collisions with legacy
// saves and other in-game item IDs.
const CORE_ITEM_GLASS_BOTTLE: u16 = 2000;
const CORE_ITEM_WATER_BOTTLE: u16 = 2001;
const CORE_ITEM_NETHER_WART: u16 = 2002;
const CORE_ITEM_BLAZE_POWDER: u16 = 2003;
const CORE_ITEM_GUNPOWDER: u16 = 2004;
const CORE_ITEM_WHEAT_SEEDS: u16 = 2005;
const CORE_ITEM_WHEAT: u16 = 2006;
const CORE_ITEM_SPIDER_EYE: u16 = 2007;
const CORE_ITEM_SUGAR: u16 = 2008;
const CORE_ITEM_PAPER: u16 = 2009;
const CORE_ITEM_BOOK: u16 = 2010;
const CORE_ITEM_FERMENTED_SPIDER_EYE: u16 = 2011;
const CORE_ITEM_MAGMA_CREAM: u16 = 2012;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseMenuView {
    Main,
    Options,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseMenuAction {
    None,
    Resume,
    ReturnToMenu,
    Quit,
}

/// Hotbar for item selection
struct Hotbar {
    slots: [Option<ItemStack>; 9],
    selected: usize,
}

/// Main inventory storage (27 slots; excludes hotbar).
struct MainInventory {
    slots: [Option<ItemStack>; 27],
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
                Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 64)), // Cobblestone
                Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 64)), // Oak Planks
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

    /// Get the potion ID if selected item is a potion
    fn selected_potion(&self) -> Option<u16> {
        if let Some(item) = self.selected_item() {
            match item.item_type {
                ItemType::Potion(potion_id) => Some(potion_id),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Get the splash potion ID if the selected item is a splash potion
    fn selected_splash_potion(&self) -> Option<u16> {
        if let Some(item) = self.selected_item() {
            match item.item_type {
                ItemType::SplashPotion(potion_id) => Some(potion_id),
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

    fn add_stack(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        if stack.count == 0 {
            return None;
        }

        // Merge into existing stacks first.
        for existing in self.slots.iter_mut().flatten() {
            if !stacks_match_for_merge(existing, &stack) {
                continue;
            }

            let max = existing.max_stack_size();
            if existing.count >= max {
                continue;
            }

            let space = max - existing.count;
            let to_add = space.min(stack.count);
            existing.count += to_add;
            stack.count -= to_add;

            if stack.count == 0 {
                return None;
            }
        }

        // Then fill empty slots, splitting if needed.
        for slot in &mut self.slots {
            if stack.count == 0 {
                return None;
            }
            if slot.is_some() {
                continue;
            }

            let max = stack.max_stack_size();
            if stack.count <= max {
                *slot = Some(stack);
                return None;
            }

            let mut placed = stack.clone();
            placed.count = max;
            *slot = Some(placed);
            stack.count -= max;
        }

        Some(stack)
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

    fn item_name(&self, item_stack: Option<&ItemStack>, registry: &BlockRegistry) -> String {
        if let Some(stack) = item_stack {
            match stack.item_type {
                ItemType::Tool(tool_type, material) => {
                    format!("{:?} {:?}", material, tool_type)
                }
                ItemType::Block(block_id) => registry
                    .descriptor(block_id)
                    .map(|desc| desc.name.clone())
                    .unwrap_or_else(|| format!("Block({})", block_id)),
                ItemType::Food(food_type) => format!("{:?}", food_type),
                ItemType::Item(id) => match id {
                    1 => "Bow".to_string(),
                    2 => "Arrow".to_string(),
                    3 => "Stick".to_string(),
                    4 => "String".to_string(),
                    5 => "Flint".to_string(),
                    6 => "Feather".to_string(),
                    7 => "Iron Ingot".to_string(),
                    8 => "Coal".to_string(),
                    9 => "Gold Ingot".to_string(),
                    // Iron armor
                    10 => "Iron Helmet".to_string(),
                    11 => "Iron Chestplate".to_string(),
                    12 => "Iron Leggings".to_string(),
                    13 => "Iron Boots".to_string(),
                    14 => "Diamond".to_string(),
                    15 => "Lapis Lazuli".to_string(),
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
                    CORE_ITEM_GLASS_BOTTLE => "Glass Bottle".to_string(),
                    CORE_ITEM_WATER_BOTTLE => "Water Bottle".to_string(),
                    CORE_ITEM_NETHER_WART => "Nether Wart".to_string(),
                    CORE_ITEM_BLAZE_POWDER => "Blaze Powder".to_string(),
                    CORE_ITEM_GUNPOWDER => "Gunpowder".to_string(),
                    CORE_ITEM_WHEAT_SEEDS => "Wheat Seeds".to_string(),
                    CORE_ITEM_WHEAT => "Wheat".to_string(),
                    CORE_ITEM_SPIDER_EYE => "Spider Eye".to_string(),
                    CORE_ITEM_FERMENTED_SPIDER_EYE => "Fermented Spider Eye".to_string(),
                    CORE_ITEM_MAGMA_CREAM => "Magma Cream".to_string(),
                    CORE_ITEM_SUGAR => "Sugar".to_string(),
                    CORE_ITEM_PAPER => "Paper".to_string(),
                    CORE_ITEM_BOOK => "Book".to_string(),
                    _ => format!("Item({})", id),
                },
                ItemType::Potion(id) => potion_name(id),
                ItemType::SplashPotion(id) => format!("Splash {}", potion_name(id)),
            }
        } else {
            "Empty".to_string()
        }
    }
}

impl MainInventory {
    fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }

    fn add_stack(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        if stack.count == 0 {
            return None;
        }

        // Merge into existing stacks first.
        for existing in self.slots.iter_mut().flatten() {
            if !stacks_match_for_merge(existing, &stack) {
                continue;
            }

            let max = existing.max_stack_size();
            if existing.count >= max {
                continue;
            }

            let space = max - existing.count;
            let to_add = space.min(stack.count);
            existing.count += to_add;
            stack.count -= to_add;

            if stack.count == 0 {
                return None;
            }
        }

        // Then fill empty slots, splitting if needed.
        for slot in &mut self.slots {
            if stack.count == 0 {
                return None;
            }
            if slot.is_some() {
                continue;
            }

            let max = stack.max_stack_size();
            if stack.count <= max {
                *slot = Some(stack);
                return None;
            }

            let mut placed = stack.clone();
            placed.count = max;
            *slot = Some(placed);
            stack.count -= max;
        }

        Some(stack)
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

    /// Check if this AABB intersects with another
    fn intersects(&self, other: &AABB) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }

    /// Offset the AABB by a vector
    fn offset(&self, offset: glam::Vec3) -> Self {
        Self {
            min: self.min + offset,
            max: self.max + offset,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AabbSet<const N: usize> {
    items: [Option<AABB>; N],
    len: usize,
}

impl<const N: usize> AabbSet<N> {
    fn empty() -> Self {
        Self {
            items: [None; N],
            len: 0,
        }
    }

    fn single(aabb: AABB) -> Self {
        let mut set = Self::empty();
        set.push(aabb);
        set
    }

    fn push(&mut self, aabb: AABB) {
        debug_assert!(self.len < N);
        if self.len >= N {
            return;
        }
        self.items[self.len] = Some(aabb);
        self.len += 1;
    }

    fn iter(&self) -> impl Iterator<Item = &AABB> {
        self.items[..self.len].iter().flatten()
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
    /// Time since last jump press (for double-jump flight toggle)
    last_jump_press_time: f32,
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
    /// Remaining air while underwater (ticks, 20 TPS). Vanilla: 300 ticks (~15s).
    air_ticks: u16,
    /// Ticks accumulated since last drowning damage once out of air.
    drowning_timer_ticks: u8,
    /// Remaining on-fire ticks (20 TPS).
    burning_ticks: u16,
    /// Ticks accumulated since last fire damage while burning.
    burning_damage_timer_ticks: u8,
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
            air_ticks: 300,
            drowning_timer_ticks: 0,
            burning_ticks: 0,
            burning_damage_timer_ticks: 0,
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

    fn tick_air(&mut self, underwater: bool, has_water_breathing: bool) -> bool {
        const MAX_AIR_TICKS: u16 = 300;
        const AIR_REGEN_PER_TICK: u16 = 4;
        const DROWNING_DAMAGE_INTERVAL_TICKS: u8 = 20;

        if has_water_breathing {
            self.air_ticks = MAX_AIR_TICKS;
            self.drowning_timer_ticks = 0;
            return false;
        }

        if underwater {
            if self.air_ticks > 0 {
                self.air_ticks = self.air_ticks.saturating_sub(1);
                self.drowning_timer_ticks = 0;
                return false;
            }

            self.drowning_timer_ticks = self
                .drowning_timer_ticks
                .saturating_add(1)
                .min(DROWNING_DAMAGE_INTERVAL_TICKS);
            if self.drowning_timer_ticks >= DROWNING_DAMAGE_INTERVAL_TICKS {
                self.drowning_timer_ticks = 0;
                return true;
            }
            return false;
        }

        self.air_ticks = (self.air_ticks + AIR_REGEN_PER_TICK).min(MAX_AIR_TICKS);
        self.drowning_timer_ticks = 0;
        false
    }

    fn ignite(&mut self, ticks: u16) {
        self.burning_ticks = self.burning_ticks.max(ticks);
    }

    fn tick_burning(&mut self, extinguish: bool, has_fire_resistance: bool) -> bool {
        const BURNING_DAMAGE_INTERVAL_TICKS: u8 = 20;

        if extinguish {
            self.burning_ticks = 0;
            self.burning_damage_timer_ticks = 0;
            return false;
        }

        if self.burning_ticks == 0 {
            self.burning_damage_timer_ticks = 0;
            return false;
        }

        self.burning_ticks = self.burning_ticks.saturating_sub(1);

        if has_fire_resistance {
            self.burning_damage_timer_ticks = 0;
            return false;
        }

        self.burning_damage_timer_ticks = self
            .burning_damage_timer_ticks
            .saturating_add(1)
            .min(BURNING_DAMAGE_INTERVAL_TICKS);
        if self.burning_damage_timer_ticks >= BURNING_DAMAGE_INTERVAL_TICKS {
            self.burning_damage_timer_ticks = 0;
            return true;
        }

        false
    }

    /// Reset health and hunger to full (for respawn)
    fn reset(&mut self) {
        self.current = self.max;
        self.hunger = self.max_hunger;
        self.time_since_damage = 0.0;
        self.invulnerability_time = 0.0;
        self.hunger_timer = 0.0;
        self.starvation_timer = 0.0;
        self.air_ticks = 300;
        self.drowning_timer_ticks = 0;
        self.burning_ticks = 0;
        self.burning_damage_timer_ticks = 0;
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
        FoodType::Carrot => 3.0,
        FoodType::Potato => 1.0,
        FoodType::BakedPotato => 5.0,
        FoodType::GoldenCarrot => 6.0,
    }
}

/// Get the display name for a potion ID
fn potion_name(potion_id: u16) -> String {
    match potion_id {
        potion_ids::AWKWARD => "Awkward Potion".to_string(),
        potion_ids::NIGHT_VISION => "Potion of Night Vision".to_string(),
        potion_ids::INVISIBILITY => "Potion of Invisibility".to_string(),
        potion_ids::LEAPING => "Potion of Leaping".to_string(),
        potion_ids::FIRE_RESISTANCE => "Potion of Fire Resistance".to_string(),
        potion_ids::SWIFTNESS => "Potion of Swiftness".to_string(),
        potion_ids::SLOWNESS => "Potion of Slowness".to_string(),
        potion_ids::WATER_BREATHING => "Potion of Water Breathing".to_string(),
        potion_ids::HEALING => "Potion of Healing".to_string(),
        potion_ids::HARMING => "Potion of Harming".to_string(),
        potion_ids::POISON => "Potion of Poison".to_string(),
        potion_ids::REGENERATION => "Potion of Regeneration".to_string(),
        potion_ids::STRENGTH => "Potion of Strength".to_string(),
        potion_ids::WEAKNESS => "Potion of Weakness".to_string(),
        _ => format!("Potion({})", potion_id),
    }
}

fn brew_ingredient_label(ingredient_id: u16) -> &'static str {
    match ingredient_id {
        item_ids::NETHER_WART => "Nether Wart",
        item_ids::BLAZE_POWDER => "Blaze Powder",
        item_ids::GHAST_TEAR => "Ghast Tear",
        item_ids::MAGMA_CREAM => "Magma Cream",
        item_ids::SPIDER_EYE => "Spider Eye",
        item_ids::FERMENTED_SPIDER_EYE => "Fermented Spider Eye",
        item_ids::GLISTERING_MELON => "Glistering Melon",
        item_ids::GOLDEN_CARROT => "Golden Carrot",
        item_ids::RABBIT_FOOT => "Rabbit Foot",
        item_ids::PHANTOM_MEMBRANE => "Phantom Membrane",
        item_ids::REDSTONE_DUST => "Redstone Dust",
        item_ids::GLOWSTONE_DUST => "Glowstone Dust",
        item_ids::GUNPOWDER => "Gunpowder",
        item_ids::DRAGON_BREATH => "Dragon Breath",
        item_ids::SUGAR => "Sugar",
        _ => "Unknown",
    }
}

fn core_item_type_to_brew_ingredient_id(item_type: ItemType) -> Option<u16> {
    match item_type {
        ItemType::Item(CORE_ITEM_NETHER_WART) => Some(item_ids::NETHER_WART),
        ItemType::Item(CORE_ITEM_BLAZE_POWDER) => Some(item_ids::BLAZE_POWDER),
        ItemType::Item(CORE_ITEM_GUNPOWDER) => Some(item_ids::GUNPOWDER),
        ItemType::Item(CORE_ITEM_SPIDER_EYE) => Some(item_ids::SPIDER_EYE),
        ItemType::Item(CORE_ITEM_FERMENTED_SPIDER_EYE) => Some(item_ids::FERMENTED_SPIDER_EYE),
        ItemType::Item(CORE_ITEM_SUGAR) => Some(item_ids::SUGAR),
        ItemType::Item(CORE_ITEM_MAGMA_CREAM) => Some(item_ids::MAGMA_CREAM),
        ItemType::Food(mdminecraft_core::item::FoodType::GoldenCarrot) => {
            Some(item_ids::GOLDEN_CARROT)
        }
        _ => None,
    }
}

fn brew_ingredient_id_to_core_item_type(ingredient_id: u16) -> Option<ItemType> {
    match ingredient_id {
        item_ids::NETHER_WART => Some(ItemType::Item(CORE_ITEM_NETHER_WART)),
        item_ids::BLAZE_POWDER => Some(ItemType::Item(CORE_ITEM_BLAZE_POWDER)),
        item_ids::GUNPOWDER => Some(ItemType::Item(CORE_ITEM_GUNPOWDER)),
        item_ids::SPIDER_EYE => Some(ItemType::Item(CORE_ITEM_SPIDER_EYE)),
        item_ids::FERMENTED_SPIDER_EYE => Some(ItemType::Item(CORE_ITEM_FERMENTED_SPIDER_EYE)),
        item_ids::SUGAR => Some(ItemType::Item(CORE_ITEM_SUGAR)),
        item_ids::MAGMA_CREAM => Some(ItemType::Item(CORE_ITEM_MAGMA_CREAM)),
        item_ids::GOLDEN_CARROT => Some(ItemType::Food(
            mdminecraft_core::item::FoodType::GoldenCarrot,
        )),
        _ => None,
    }
}

fn core_item_stack_to_bottle(stack: &ItemStack) -> Option<(PotionType, bool)> {
    match stack.item_type {
        ItemType::Item(CORE_ITEM_WATER_BOTTLE) => Some((PotionType::Water, false)),
        ItemType::Potion(id) => {
            let potion = match id {
                potion_ids::AWKWARD => PotionType::Awkward,
                potion_ids::NIGHT_VISION => PotionType::NightVision,
                potion_ids::INVISIBILITY => PotionType::Invisibility,
                potion_ids::LEAPING => PotionType::Leaping,
                potion_ids::FIRE_RESISTANCE => PotionType::FireResistance,
                potion_ids::SWIFTNESS => PotionType::Swiftness,
                potion_ids::SLOWNESS => PotionType::Slowness,
                potion_ids::WATER_BREATHING => PotionType::WaterBreathing,
                potion_ids::HEALING => PotionType::Healing,
                potion_ids::HARMING => PotionType::Harming,
                potion_ids::POISON => PotionType::Poison,
                potion_ids::REGENERATION => PotionType::Regeneration,
                potion_ids::STRENGTH => PotionType::Strength,
                potion_ids::WEAKNESS => PotionType::Weakness,
                _ => return None,
            };
            Some((potion, false))
        }
        ItemType::SplashPotion(id) => {
            let potion = match id {
                potion_ids::AWKWARD => PotionType::Awkward,
                potion_ids::NIGHT_VISION => PotionType::NightVision,
                potion_ids::INVISIBILITY => PotionType::Invisibility,
                potion_ids::LEAPING => PotionType::Leaping,
                potion_ids::FIRE_RESISTANCE => PotionType::FireResistance,
                potion_ids::SWIFTNESS => PotionType::Swiftness,
                potion_ids::SLOWNESS => PotionType::Slowness,
                potion_ids::WATER_BREATHING => PotionType::WaterBreathing,
                potion_ids::HEALING => PotionType::Healing,
                potion_ids::HARMING => PotionType::Harming,
                potion_ids::POISON => PotionType::Poison,
                potion_ids::REGENERATION => PotionType::Regeneration,
                potion_ids::STRENGTH => PotionType::Strength,
                potion_ids::WEAKNESS => PotionType::Weakness,
                _ => return None,
            };
            Some((potion, true))
        }
        _ => None,
    }
}

fn bottle_to_core_item_stack(potion: PotionType, is_splash: bool) -> ItemStack {
    let potion_item = |id: u16| {
        if is_splash {
            ItemType::SplashPotion(id)
        } else {
            ItemType::Potion(id)
        }
    };

    match potion {
        PotionType::Water => ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 1),
        PotionType::Awkward => ItemStack::new(potion_item(potion_ids::AWKWARD), 1),
        PotionType::NightVision => ItemStack::new(potion_item(potion_ids::NIGHT_VISION), 1),
        PotionType::Invisibility => ItemStack::new(potion_item(potion_ids::INVISIBILITY), 1),
        PotionType::Leaping => ItemStack::new(potion_item(potion_ids::LEAPING), 1),
        PotionType::FireResistance => ItemStack::new(potion_item(potion_ids::FIRE_RESISTANCE), 1),
        PotionType::Swiftness => ItemStack::new(potion_item(potion_ids::SWIFTNESS), 1),
        PotionType::Slowness => ItemStack::new(potion_item(potion_ids::SLOWNESS), 1),
        PotionType::WaterBreathing => ItemStack::new(potion_item(potion_ids::WATER_BREATHING), 1),
        PotionType::Healing => ItemStack::new(potion_item(potion_ids::HEALING), 1),
        PotionType::Harming => ItemStack::new(potion_item(potion_ids::HARMING), 1),
        PotionType::Poison => ItemStack::new(potion_item(potion_ids::POISON), 1),
        PotionType::Regeneration => ItemStack::new(potion_item(potion_ids::REGENERATION), 1),
        PotionType::Strength => ItemStack::new(potion_item(potion_ids::STRENGTH), 1),
        PotionType::Weakness => ItemStack::new(potion_item(potion_ids::WEAKNESS), 1),

        // Base/unsupported potions don't have a client-side representation yet.
        PotionType::Mundane | PotionType::Thick | PotionType::Luck | PotionType::SlowFalling => {
            ItemStack::new(ItemType::Item(9999), 1)
        }
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
    fn new(pos: glam::Vec3, value: u32, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        // Small deterministic upward and outward velocity for visual scatter.
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(0.1..0.2);
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
    /// Vanilla-ish step height. Uses a power-of-two fraction for determinism.
    const STEP_HEIGHT: f32 = 19.0 / 32.0;

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
            last_jump_press_time: 10.0,
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
    audio: AudioManager,
    chunk_manager: ChunkManager,
    chunks: HashMap<ChunkPos, Chunk>,
    registry: BlockRegistry,
    block_properties: BlockPropertiesRegistry,
    input: InputState,
    last_frame: Instant,
    debug_hud: DebugHud,
    time_of_day: TimeOfDay,
    sim_time: SimTime,
    sim_time_paused: bool,
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
    weather_next_change_tick: SimTick,
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
    /// Temporary cursor-held stack for UI drag/drop interactions.
    ui_cursor_stack: Option<ItemStack>,
    /// UI drag state for click-drag stack distribution.
    ui_drag_state: UiDragState,
    /// Main inventory storage (27 slots; excludes hotbar).
    main_inventory: MainInventory,
    /// Mob spawner for passive mobs
    #[allow(dead_code)]
    mob_spawner: MobSpawner,
    /// Active mobs in the world
    mobs: Vec<Mob>,
    /// Fluid simulation
    fluid_sim: FluidSimulator,
    /// Redstone simulation
    redstone_sim: RedstoneSimulator,
    /// Farming simulation (crop growth + farmland hydration).
    crop_growth: CropGrowthSystem,
    /// Sugar cane growth simulation.
    sugar_cane_growth: SugarCaneGrowthSystem,
    /// Block interaction manager (doors/trapdoors/etc)
    interaction_manager: InteractionManager,
    /// Pressure plate currently pressed by the player (if any)
    pressed_pressure_plate: Option<RedstonePos>,
    /// Simulation tick counter
    sim_tick: SimTick,
    /// Time accumulator for fixed timestep loop
    accumulator: f64,
    /// Region store for persistence
    region_store: RegionStore,
    /// World seed used for deterministic world generation.
    world_seed: u64,
    /// Terrain generator
    terrain_generator: TerrainGenerator,
    /// Render distance (chunks radius)
    render_distance: i32,
    /// Whether the crafting UI is open
    crafting_open: bool,
    /// Crafting grid (3x3)
    crafting_grid: [[Option<ItemStack>; 3]; 3],
    /// Personal crafting grid (2x2) shown in the inventory UI.
    personal_crafting_grid: [[Option<ItemStack>; 2]; 2],
    /// Whether the furnace UI is open
    furnace_open: bool,
    /// Currently open furnace position (if any)
    open_furnace_pos: Option<BlockEntityKey>,
    /// Furnace states by position
    furnaces: BTreeMap<BlockEntityKey, FurnaceState>,
    /// Whether enchanting table UI is open
    enchanting_open: bool,
    /// Currently open enchanting table position (if any)
    open_enchanting_pos: Option<BlockEntityKey>,
    /// Enchanting table states by position
    enchanting_tables: BTreeMap<BlockEntityKey, EnchantingTableState>,
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
    /// Player status effects (potions, etc.)
    status_effects: StatusEffects,
    /// Brewing stand states by position
    brewing_stands: BTreeMap<BlockEntityKey, BrewingStandState>,
    /// Whether brewing stand UI is open
    brewing_open: bool,
    /// Currently open brewing stand position (if any)
    open_brewing_pos: Option<BlockEntityKey>,
    /// Chest inventories by position
    chests: BTreeMap<BlockEntityKey, ChestState>,
    /// Whether chest UI is open
    chest_open: bool,
    /// Currently open chest position (if any)
    open_chest_pos: Option<BlockEntityKey>,

    /// Whether the in-game pause menu is open.
    pause_menu_open: bool,
    pause_menu_view: PauseMenuView,
    pause_controls_dirty: bool,
    pending_action: Option<GameAction>,
}

impl GameWorld {
    #[inline(always)]
    fn flat_directions(camera: &mdminecraft_render::Camera) -> (glam::Vec3, glam::Vec3) {
        let yaw = camera.yaw;
        let forward = glam::Vec3::new(yaw.cos(), 0.0, yaw.sin()).normalize_or_zero();
        let right = glam::Vec3::new(-forward.z, 0.0, forward.x).normalize_or_zero();
        (forward, right)
    }

    fn audio_settings_from_controls(controls: &ControlsConfig) -> AudioSettings {
        AudioSettings {
            master: controls.master_volume.clamp(0.0, 1.0),
            music: controls.music_volume.clamp(0.0, 1.0),
            sfx: controls.sfx_volume.clamp(0.0, 1.0),
            ambient: controls.ambient_volume.clamp(0.0, 1.0),
            muted: controls.audio_muted,
        }
    }

    fn overworld_block_entity_key(block_pos: IVec3) -> BlockEntityKey {
        BlockEntityKey {
            dimension: DimensionId::Overworld,
            x: block_pos.x,
            y: block_pos.y,
            z: block_pos.z,
        }
    }

    fn neighbor_chunk_positions(center: ChunkPos) -> [ChunkPos; 4] {
        [
            ChunkPos::new(center.x - 1, center.z),
            ChunkPos::new(center.x + 1, center.z),
            ChunkPos::new(center.x, center.z - 1),
            ChunkPos::new(center.x, center.z + 1),
        ]
    }

    fn mesh_for_chunk(&self, chunk: &Chunk) -> mdminecraft_render::MeshBuffers {
        let chunks = &self.chunks;
        let meshing_pos = chunk.position();
        let origin_x = meshing_pos.x * CHUNK_SIZE_X as i32;
        let origin_z = meshing_pos.z * CHUNK_SIZE_Z as i32;

        mesh_chunk_with_voxel_at(
            chunk,
            &self.registry,
            self.renderer.atlas_metadata(),
            |wx, wy, wz| {
                if wy < 0 || wy >= CHUNK_SIZE_Y as i32 {
                    return None;
                }

                let chunk_x = wx.div_euclid(CHUNK_SIZE_X as i32);
                let chunk_z = wz.div_euclid(CHUNK_SIZE_Z as i32);
                let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

                if chunk_pos == meshing_pos {
                    let local_x = wx - origin_x;
                    let local_z = wz - origin_z;
                    if !(0..CHUNK_SIZE_X as i32).contains(&local_x)
                        || !(0..CHUNK_SIZE_Z as i32).contains(&local_z)
                    {
                        return None;
                    }
                    return Some(chunk.voxel(local_x as usize, wy as usize, local_z as usize));
                }

                let chunk = chunks.get(&chunk_pos)?;
                let local_x = wx.rem_euclid(CHUNK_SIZE_X as i32) as usize;
                let local_z = wz.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
                Some(chunk.voxel(local_x, wy as usize, local_z))
            },
        )
    }

    fn upload_chunk_mesh(&mut self, chunk_pos: ChunkPos) -> bool {
        let Some(resources) = self.renderer.render_resources() else {
            return false;
        };
        let Some(chunk) = self.chunks.get(&chunk_pos) else {
            return false;
        };

        let mesh = self.mesh_for_chunk(chunk);
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return false;
        }

        let chunk_bind_group = resources
            .pipeline
            .create_chunk_bind_group(resources.device, chunk_pos);
        self.chunk_manager.add_chunk(
            resources.device,
            resources.queue,
            &mesh,
            chunk_pos,
            chunk_bind_group,
        );
        true
    }

    fn upload_chunk_mesh_and_neighbors(&mut self, chunk_pos: ChunkPos) -> u32 {
        let mut uploads = 0u32;
        if self.upload_chunk_mesh(chunk_pos) {
            uploads += 1;
        }
        for neighbor in Self::neighbor_chunk_positions(chunk_pos) {
            if self.upload_chunk_mesh(neighbor) {
                uploads += 1;
            }
        }
        uploads
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

        // Load block registry
        let registry = load_block_registry();

        // Setup persistence and generator
        let save_path = PathBuf::from("saves/default");
        let region_store = RegionStore::new(&save_path).unwrap_or_else(|_| {
            tracing::warn!("Failed to create save directory, using temporary");
            RegionStore::new(std::env::temp_dir().join("mdminecraft_save")).unwrap()
        });

        let (world_seed, loaded_state) = {
            let meta = if region_store.world_meta_exists() {
                match region_store.load_world_meta() {
                    Ok(meta) => meta,
                    Err(err) => {
                        tracing::warn!(?err, "Failed to load world meta; generating new seed");
                        WorldMeta {
                            world_seed: rand::random(),
                        }
                    }
                }
            } else {
                let world_seed = std::env::var("MDM_WORLD_SEED")
                    .ok()
                    .and_then(|raw| raw.parse::<u64>().ok())
                    .unwrap_or_else(rand::random);

                let meta = WorldMeta { world_seed };
                if let Err(err) = region_store.save_world_meta(&meta) {
                    tracing::warn!(?err, "Failed to save world meta");
                }
                meta
            };

            let state = if region_store.world_state_exists() {
                match region_store.load_world_state() {
                    Ok(state) => Some(state),
                    Err(err) => {
                        tracing::warn!(?err, "Failed to load world state; starting fresh");
                        None
                    }
                }
            } else {
                None
            };

            (meta.world_seed, state)
        };

        tracing::info!("World Seed: {}", world_seed);
        let terrain_generator = TerrainGenerator::new(world_seed);
        let render_distance = controls.render_distance.clamp(2, 16);

        let chunk_manager = ChunkManager::new();
        let chunks = HashMap::new();
        let rng = StdRng::seed_from_u64(world_seed ^ 0x5eed_a11c);

        let mut sim_tick = SimTick::ZERO;
        let mut sim_time = SimTime::default();
        let mut weather = WeatherToggle::new();
        let mut loaded_player: Option<PlayerSave> = None;
        let mut loaded_entities = WorldEntitiesState::default();
        let mut loaded_block_entities = BlockEntitiesState::default();
        let weather_next_change_tick = if let Some(state) = loaded_state {
            sim_tick = state.tick;
            sim_time = state.sim_time;
            weather = state.weather;
            loaded_player = state.player;
            loaded_entities = state.entities;
            loaded_block_entities = state.block_entities;
            state.weather_next_change_tick
        } else {
            // Deterministic initial schedule (based on seed + tick).
            let delay_ticks = Self::weather_delay_ticks(
                world_seed,
                sim_tick,
                900..2400, // 45..120 seconds at 20 TPS
                0xC0FFEE_u64,
            );
            sim_tick.advance(delay_ticks)
        };

        if let Some(player) = &loaded_player {
            renderer.camera_mut().position = glam::Vec3::new(
                player.transform.x as f32,
                player.transform.y as f32,
                player.transform.z as f32,
            );
            renderer.camera_mut().yaw = player.transform.yaw;
            renderer.camera_mut().pitch = player.transform.pitch;
        } else {
            renderer.camera_mut().position = glam::Vec3::new(0.0, 100.0, 0.0);
            renderer.camera_mut().yaw = 0.0;
            renderer.camera_mut().pitch = -0.3;
        }

        renderer.camera_mut().fov = controls.fov_degrees.clamp(30.0, 150.0).to_radians();

        let mut audio = AudioManager::new()?;
        audio.update_settings(Self::audio_settings_from_controls(controls.as_ref()));
        let camera_pos = renderer.camera().position;
        audio.set_listener_position([camera_pos.x, camera_pos.y, camera_pos.z]);

        let mob_spawner = MobSpawner::new(world_seed);
        let WorldEntitiesState {
            mobs,
            dropped_items,
            projectiles,
        } = loaded_entities;
        let furnaces = loaded_block_entities.furnaces;
        let enchanting_tables = loaded_block_entities.enchanting_tables;
        let brewing_stands = loaded_block_entities.brewing_stands;
        let chests = loaded_block_entities.chests;

        // Setup state
        let debug_hud = DebugHud::new(); // Zeroed by default

        let input = InputState::new();
        let input_processor = InputProcessor::new(&controls);
        let scripted_input = scripted_input_path
            .as_ref()
            .map(|path| ScriptedInputPlayer::from_path(path))
            .transpose()?;

        let mut world = Self {
            window,
            renderer,
            audio,
            chunk_manager,
            chunks,
            registry,
            block_properties: BlockPropertiesRegistry::new(),
            input,
            last_frame: Instant::now(),
            debug_hud,
            time_of_day: TimeOfDay::new(),
            sim_time,
            sim_time_paused: false,
            selected_block: None,
            hotbar: Hotbar::new(),
            player_physics: PlayerPhysics::new(),
            player_health: PlayerHealth::new(),
            chunks_visible: 0,
            mining_progress: None,
            spawn_point: glam::Vec3::ZERO, // Temp
            controls,
            input_processor,
            actions: ActionState::default(),
            scripted_input,
            particle_emitter: ParticleEmitter::new(),
            particles: Vec::new(),
            weather,
            weather_next_change_tick,
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
            item_manager: dropped_items,
            inventory_open: false,
            ui_cursor_stack: None,
            ui_drag_state: UiDragState::default(),
            main_inventory: MainInventory::new(),
            mob_spawner,
            mobs,
            fluid_sim: FluidSimulator::new(),
            redstone_sim: RedstoneSimulator::new(),
            crop_growth: CropGrowthSystem::new(world_seed),
            sugar_cane_growth: SugarCaneGrowthSystem::new(world_seed),
            interaction_manager: InteractionManager::new(),
            pressed_pressure_plate: None,
            sim_tick,
            accumulator: 0.0,
            crafting_open: false,
            crafting_grid: Default::default(),
            personal_crafting_grid: Default::default(),
            furnace_open: false,
            open_furnace_pos: None,
            furnaces,
            enchanting_open: false,
            open_enchanting_pos: None,
            enchanting_tables,
            player_armor: PlayerArmor::new(),
            projectiles,
            bow_charge: 0.0,
            bow_drawing: false,
            attack_cooldown: 0.0,
            player_xp: PlayerXP::new(),
            xp_orbs: Vec::new(),
            status_effects: StatusEffects::new(),
            brewing_stands,
            brewing_open: false,
            open_brewing_pos: None,
            chests,
            chest_open: false,
            open_chest_pos: None,
            pause_menu_open: false,
            pause_menu_view: PauseMenuView::Main,
            pause_controls_dirty: false,
            pending_action: None,

            // New fields
            region_store,
            world_seed,
            terrain_generator,
            render_distance,
        };

        world
            .time_of_day
            .set_time(world.sim_time.time_of_day() as f32);

        // Initial chunk load (blocking, load all initial chunks)
        world.update_chunks(usize::MAX);

        let has_loaded_player = loaded_player.is_some();
        let spawn_test_mobs = std::env::var("MDM_DEBUG_SPAWN_TEST_MOBS")
            .ok()
            .is_some_and(|value| value == "1");

        // Spawn passive mobs only for brand-new worlds.
        if !has_loaded_player && world.mobs.is_empty() {
            let mut positions: Vec<_> = world.chunks.keys().copied().collect();
            positions.sort();

            for pos in positions {
                let Some(chunk) = world.chunks.get(&pos) else {
                    continue;
                };
                let chunk_center_x = pos.x * CHUNK_SIZE_X as i32 + CHUNK_SIZE_X as i32 / 2;
                let chunk_center_z = pos.z * CHUNK_SIZE_Z as i32 + CHUNK_SIZE_Z as i32 / 2;
                let biome = world
                    .terrain_generator
                    .biome_assigner()
                    .get_biome(chunk_center_x, chunk_center_z);

                let mut surface_heights = [[0i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];
                for (local_z, row) in surface_heights.iter_mut().enumerate() {
                    for (local_x, height) in row.iter_mut().enumerate() {
                        for y in (0..CHUNK_SIZE_Y).rev() {
                            let voxel = chunk.voxel(local_x, y, local_z);
                            if voxel.id != BLOCK_AIR {
                                *height = y as i32;
                                break;
                            }
                        }
                    }
                }

                let mut new_mobs =
                    world
                        .mob_spawner
                        .generate_spawns(pos.x, pos.z, biome, &surface_heights);
                world.mobs.append(&mut new_mobs);
            }
        }

        if let Some(save) = loaded_player {
            world.apply_player_save(save);
        } else {
            // Determine spawn point
            let spawn_feet = Self::determine_spawn_point(&world.chunks, &world.block_properties)
                .unwrap_or_else(|| glam::Vec3::new(0.0, 100.0, 0.0));
            world.spawn_point = spawn_feet;

            // Setup camera
            world.renderer.camera_mut().position =
                spawn_feet + glam::Vec3::new(0.0, PlayerPhysics::new().eye_height, 0.0);
            world.renderer.camera_mut().yaw = 0.0;
            world.renderer.camera_mut().pitch = -0.3;
            world.player_physics.last_ground_y = spawn_feet.y;

            // Optional debug mob spawn for visibility testing.
            if spawn_test_mobs {
                let test_mob_types = [
                    MobType::Pig,
                    MobType::Cow,
                    MobType::Sheep,
                    MobType::Chicken,
                    MobType::Villager,
                ];
                for (i, mob_type) in test_mob_types.iter().enumerate() {
                    let angle = (i as f32) * std::f32::consts::TAU / test_mob_types.len() as f32;
                    let distance = 8.0; // 8 blocks away from player
                    let mob_x = spawn_feet.x as f64 + (angle.cos() * distance) as f64;
                    let mob_z = spawn_feet.z as f64 + (angle.sin() * distance) as f64;
                    let mob_y = spawn_feet.y as f64 + 1.0; // Spawn at player's ground level + 1

                    let mob = Mob::new(mob_x, mob_y, mob_z, *mob_type);
                    world.mobs.push(mob);
                }
            }
        }

        let _ = world.input.enter_gameplay(&world.window);

        Ok(world)
    }

    fn column_ground_height(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
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
                if let Some(top_offset) = Self::voxel_collision_top_offset(block_properties, &voxel)
                {
                    return y as f32 + top_offset;
                }
            }
        }
        50.0
    }

    fn voxel_collision_top_offset(
        block_properties: &BlockPropertiesRegistry,
        voxel: &Voxel,
    ) -> Option<f32> {
        if !block_properties.get(voxel.id).is_solid {
            return None;
        }

        match mdminecraft_world::get_collision_type(voxel.id, voxel.state) {
            mdminecraft_world::CollisionType::None | mdminecraft_world::CollisionType::Ladder => {
                None
            }
            mdminecraft_world::CollisionType::Door { .. } => None,
            mdminecraft_world::CollisionType::Full => Some(1.0),
            mdminecraft_world::CollisionType::Partial { max_y, .. } => Some(max_y),
            mdminecraft_world::CollisionType::Fence => Some(1.5),
        }
    }

    fn collision_aabbs_for_voxel(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        voxel: &Voxel,
    ) -> AabbSet<8> {
        if !block_properties.get(voxel.id).is_solid {
            return AabbSet::empty();
        }

        let full = || AABB {
            min: glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
            max: glam::Vec3::new(
                block_x as f32 + 1.0,
                block_y as f32 + 1.0,
                block_z as f32 + 1.0,
            ),
        };

        let voxel_at = |x: i32, y: i32, z: i32| -> Option<Voxel> {
            if y < 0 || y >= CHUNK_SIZE_Y as i32 {
                return None;
            }
            let chunk_x = x.div_euclid(CHUNK_SIZE_X as i32);
            let chunk_z = z.div_euclid(CHUNK_SIZE_Z as i32);
            let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
            let chunk = chunks.get(&chunk_pos)?;
            let local_x = x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
            let local_z = z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
            Some(chunk.voxel(local_x, y as usize, local_z))
        };

        match mdminecraft_world::get_collision_type(voxel.id, voxel.state) {
            mdminecraft_world::CollisionType::None | mdminecraft_world::CollisionType::Ladder => {
                if mdminecraft_world::is_fence_gate(voxel.id)
                    && mdminecraft_world::is_fence_gate_open(voxel.state)
                {
                    let thickness = 3.0 / 16.0;
                    let facing = mdminecraft_world::Facing::from_state(voxel.state);

                    // Simplified hinge: gates always swing "left" from their facing direction.
                    let (min, max) = match facing {
                        mdminecraft_world::Facing::North => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + thickness,
                                block_y as f32 + 1.5,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::South => (
                            glam::Vec3::new(
                                block_x as f32 + 1.0 - thickness,
                                block_y as f32,
                                block_z as f32,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.5,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::East => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.5,
                                block_z as f32 + thickness,
                            ),
                        ),
                        mdminecraft_world::Facing::West => (
                            glam::Vec3::new(
                                block_x as f32,
                                block_y as f32,
                                block_z as f32 + 1.0 - thickness,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.5,
                                block_z as f32 + 1.0,
                            ),
                        ),
                    };

                    return AabbSet::single(AABB { min, max });
                }

                if mdminecraft_world::is_trapdoor(voxel.id)
                    && mdminecraft_world::is_trapdoor_open(voxel.state)
                {
                    let thickness = 3.0 / 16.0;
                    let facing = mdminecraft_world::Facing::from_state(voxel.state);

                    let (min, max) = match facing {
                        mdminecraft_world::Facing::North => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + thickness,
                            ),
                        ),
                        mdminecraft_world::Facing::South => (
                            glam::Vec3::new(
                                block_x as f32,
                                block_y as f32,
                                block_z as f32 + 1.0 - thickness,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::East => (
                            glam::Vec3::new(
                                block_x as f32 + 1.0 - thickness,
                                block_y as f32,
                                block_z as f32,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::West => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + thickness,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                    };

                    return AabbSet::single(AABB { min, max });
                }

                AabbSet::empty()
            }
            mdminecraft_world::CollisionType::Door { open } => {
                let thickness = 3.0 / 16.0;
                let facing = mdminecraft_world::Facing::from_state(voxel.state);

                let (min, max) = if open {
                    // Simplified hinge: doors always swing "left" from their facing direction.
                    match facing {
                        mdminecraft_world::Facing::North => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + thickness,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::South => (
                            glam::Vec3::new(
                                block_x as f32 + 1.0 - thickness,
                                block_y as f32,
                                block_z as f32,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::East => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + thickness,
                            ),
                        ),
                        mdminecraft_world::Facing::West => (
                            glam::Vec3::new(
                                block_x as f32,
                                block_y as f32,
                                block_z as f32 + 1.0 - thickness,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                    }
                } else {
                    match facing {
                        mdminecraft_world::Facing::North => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + thickness,
                            ),
                        ),
                        mdminecraft_world::Facing::South => (
                            glam::Vec3::new(
                                block_x as f32,
                                block_y as f32,
                                block_z as f32 + 1.0 - thickness,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::East => (
                            glam::Vec3::new(
                                block_x as f32 + 1.0 - thickness,
                                block_y as f32,
                                block_z as f32,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                        mdminecraft_world::Facing::West => (
                            glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                            glam::Vec3::new(
                                block_x as f32 + thickness,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        ),
                    }
                };

                AabbSet::single(AABB { min, max })
            }
            mdminecraft_world::CollisionType::Full => {
                if voxel.id == mdminecraft_world::interactive_blocks::GLASS_PANE {
                    // Glass pane collision: a thin post, optionally with connecting arms.
                    let thickness = 2.0 / 16.0;
                    let half = thickness * 0.5;

                    let connects_to = |neighbor: Voxel| -> bool {
                        neighbor.id == mdminecraft_world::interactive_blocks::GLASS_PANE
                            || neighbor.id == mdminecraft_world::interactive_blocks::GLASS
                            || block_properties.get(neighbor.id).is_solid
                    };

                    let connect_west =
                        voxel_at(block_x - 1, block_y, block_z).is_some_and(connects_to);
                    let connect_east =
                        voxel_at(block_x + 1, block_y, block_z).is_some_and(connects_to);
                    let connect_north =
                        voxel_at(block_x, block_y, block_z - 1).is_some_and(connects_to);
                    let connect_south =
                        voxel_at(block_x, block_y, block_z + 1).is_some_and(connects_to);

                    let post_min_x = block_x as f32 + 0.5 - half;
                    let post_max_x = block_x as f32 + 0.5 + half;
                    let post_min_z = block_z as f32 + 0.5 - half;
                    let post_max_z = block_z as f32 + 0.5 + half;

                    let any_x = connect_west || connect_east;
                    let any_z = connect_north || connect_south;
                    if !any_x && !any_z {
                        return AabbSet::single(AABB {
                            min: glam::Vec3::new(post_min_x, block_y as f32, post_min_z),
                            max: glam::Vec3::new(post_max_x, block_y as f32 + 1.0, post_max_z),
                        });
                    }

                    let mut set = AabbSet::empty();
                    if any_x {
                        let min_x = if connect_west {
                            block_x as f32
                        } else {
                            post_min_x
                        };
                        let max_x = if connect_east {
                            block_x as f32 + 1.0
                        } else {
                            post_max_x
                        };
                        set.push(AABB {
                            min: glam::Vec3::new(min_x, block_y as f32, post_min_z),
                            max: glam::Vec3::new(max_x, block_y as f32 + 1.0, post_max_z),
                        });
                    }
                    if any_z {
                        let min_z = if connect_north {
                            block_z as f32
                        } else {
                            post_min_z
                        };
                        let max_z = if connect_south {
                            block_z as f32 + 1.0
                        } else {
                            post_max_z
                        };
                        set.push(AABB {
                            min: glam::Vec3::new(post_min_x, block_y as f32, min_z),
                            max: glam::Vec3::new(post_max_x, block_y as f32 + 1.0, max_z),
                        });
                    }
                    return set;
                }

                AabbSet::single(full())
            }
            mdminecraft_world::CollisionType::Partial { min_y, max_y } => {
                if voxel.id == mdminecraft_world::BLOCK_BREWING_STAND {
                    let pad = 4.0 / 16.0;
                    return AabbSet::single(AABB {
                        min: glam::Vec3::new(
                            block_x as f32 + pad,
                            block_y as f32 + min_y,
                            block_z as f32 + pad,
                        ),
                        max: glam::Vec3::new(
                            block_x as f32 + 1.0 - pad,
                            block_y as f32 + max_y,
                            block_z as f32 + 1.0 - pad,
                        ),
                    });
                }

                if mdminecraft_world::is_stairs(voxel.id) {
                    let facing = mdminecraft_world::Facing::from_state(voxel.state);
                    let top = (voxel.state & 0x04) != 0;

                    let mut set = AabbSet::empty();
                    if top {
                        // Upside-down stairs: full top half + a lower half-footprint step.
                        let upper_min_y = block_y as f32 + max_y;
                        set.push(AABB {
                            min: glam::Vec3::new(block_x as f32, upper_min_y, block_z as f32),
                            max: glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.0,
                                block_z as f32 + 1.0,
                            ),
                        });

                        let lower_min_y = block_y as f32 + min_y;
                        let lower_max_y = block_y as f32 + max_y;
                        let (min, max) = match facing {
                            mdminecraft_world::Facing::North => (
                                glam::Vec3::new(block_x as f32, lower_min_y, block_z as f32),
                                glam::Vec3::new(
                                    block_x as f32 + 1.0,
                                    lower_max_y,
                                    block_z as f32 + 0.5,
                                ),
                            ),
                            mdminecraft_world::Facing::South => (
                                glam::Vec3::new(block_x as f32, lower_min_y, block_z as f32 + 0.5),
                                glam::Vec3::new(
                                    block_x as f32 + 1.0,
                                    lower_max_y,
                                    block_z as f32 + 1.0,
                                ),
                            ),
                            mdminecraft_world::Facing::East => (
                                glam::Vec3::new(block_x as f32 + 0.5, lower_min_y, block_z as f32),
                                glam::Vec3::new(
                                    block_x as f32 + 1.0,
                                    lower_max_y,
                                    block_z as f32 + 1.0,
                                ),
                            ),
                            mdminecraft_world::Facing::West => (
                                glam::Vec3::new(block_x as f32, lower_min_y, block_z as f32),
                                glam::Vec3::new(
                                    block_x as f32 + 0.5,
                                    lower_max_y,
                                    block_z as f32 + 1.0,
                                ),
                            ),
                        };
                        set.push(AABB { min, max });
                    } else {
                        // Normal stairs: full bottom half + an upper half-footprint step.
                        set.push(AABB {
                            min: glam::Vec3::new(
                                block_x as f32,
                                block_y as f32 + min_y,
                                block_z as f32,
                            ),
                            max: glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + max_y,
                                block_z as f32 + 1.0,
                            ),
                        });

                        let upper_min_y = block_y as f32 + max_y;
                        let (min, max) = match facing {
                            mdminecraft_world::Facing::North => (
                                glam::Vec3::new(block_x as f32, upper_min_y, block_z as f32),
                                glam::Vec3::new(
                                    block_x as f32 + 1.0,
                                    block_y as f32 + 1.0,
                                    block_z as f32 + 0.5,
                                ),
                            ),
                            mdminecraft_world::Facing::South => (
                                glam::Vec3::new(block_x as f32, upper_min_y, block_z as f32 + 0.5),
                                glam::Vec3::new(
                                    block_x as f32 + 1.0,
                                    block_y as f32 + 1.0,
                                    block_z as f32 + 1.0,
                                ),
                            ),
                            mdminecraft_world::Facing::East => (
                                glam::Vec3::new(block_x as f32 + 0.5, upper_min_y, block_z as f32),
                                glam::Vec3::new(
                                    block_x as f32 + 1.0,
                                    block_y as f32 + 1.0,
                                    block_z as f32 + 1.0,
                                ),
                            ),
                            mdminecraft_world::Facing::West => (
                                glam::Vec3::new(block_x as f32, upper_min_y, block_z as f32),
                                glam::Vec3::new(
                                    block_x as f32 + 0.5,
                                    block_y as f32 + 1.0,
                                    block_z as f32 + 1.0,
                                ),
                            ),
                        };
                        set.push(AABB { min, max });
                    }
                    return set;
                }

                AabbSet::single(AABB {
                    min: glam::Vec3::new(block_x as f32, block_y as f32 + min_y, block_z as f32),
                    max: glam::Vec3::new(
                        block_x as f32 + 1.0,
                        block_y as f32 + max_y,
                        block_z as f32 + 1.0,
                    ),
                })
            }
            mdminecraft_world::CollisionType::Fence => {
                if mdminecraft_world::is_fence_gate(voxel.id) {
                    let thickness = 3.0 / 16.0;
                    let half = thickness * 0.5;
                    let facing = mdminecraft_world::Facing::from_state(voxel.state);

                    let (min, max) = match facing {
                        mdminecraft_world::Facing::North | mdminecraft_world::Facing::South => (
                            glam::Vec3::new(
                                block_x as f32,
                                block_y as f32,
                                block_z as f32 + 0.5 - half,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 1.0,
                                block_y as f32 + 1.5,
                                block_z as f32 + 0.5 + half,
                            ),
                        ),
                        mdminecraft_world::Facing::East | mdminecraft_world::Facing::West => (
                            glam::Vec3::new(
                                block_x as f32 + 0.5 - half,
                                block_y as f32,
                                block_z as f32,
                            ),
                            glam::Vec3::new(
                                block_x as f32 + 0.5 + half,
                                block_y as f32 + 1.5,
                                block_z as f32 + 1.0,
                            ),
                        ),
                    };

                    return AabbSet::single(AABB { min, max });
                }

                let connects_to = |neighbor: Voxel| -> bool {
                    mdminecraft_world::is_fence(neighbor.id)
                        || mdminecraft_world::is_fence_gate(neighbor.id)
                        || block_properties.get(neighbor.id).is_solid
                };

                let connect_west = voxel_at(block_x - 1, block_y, block_z).is_some_and(connects_to);
                let connect_east = voxel_at(block_x + 1, block_y, block_z).is_some_and(connects_to);
                let connect_north =
                    voxel_at(block_x, block_y, block_z - 1).is_some_and(connects_to);
                let connect_south =
                    voxel_at(block_x, block_y, block_z + 1).is_some_and(connects_to);

                // Fence collision: center post + optional connecting arms (multi-AABB), avoiding
                // over-colliding corners when connected in multiple directions.
                let post_min_x = block_x as f32 + 0.375;
                let post_max_x = block_x as f32 + 0.625;
                let post_min_z = block_z as f32 + 0.375;
                let post_max_z = block_z as f32 + 0.625;

                let arm_thickness = 2.0 / 16.0;
                let arm_half = arm_thickness * 0.5;
                let arm_min_x = block_x as f32 + 0.5 - arm_half;
                let arm_max_x = block_x as f32 + 0.5 + arm_half;
                let arm_min_z = block_z as f32 + 0.5 - arm_half;
                let arm_max_z = block_z as f32 + 0.5 + arm_half;

                let mut set = AabbSet::empty();
                set.push(AABB {
                    min: glam::Vec3::new(post_min_x, block_y as f32, post_min_z),
                    max: glam::Vec3::new(post_max_x, block_y as f32 + 1.5, post_max_z),
                });

                if connect_west {
                    set.push(AABB {
                        min: glam::Vec3::new(block_x as f32, block_y as f32, arm_min_z),
                        max: glam::Vec3::new(block_x as f32 + 0.5, block_y as f32 + 1.5, arm_max_z),
                    });
                }
                if connect_east {
                    set.push(AABB {
                        min: glam::Vec3::new(block_x as f32 + 0.5, block_y as f32, arm_min_z),
                        max: glam::Vec3::new(block_x as f32 + 1.0, block_y as f32 + 1.5, arm_max_z),
                    });
                }
                if connect_north {
                    set.push(AABB {
                        min: glam::Vec3::new(arm_min_x, block_y as f32, block_z as f32),
                        max: glam::Vec3::new(arm_max_x, block_y as f32 + 1.5, block_z as f32 + 0.5),
                    });
                }
                if connect_south {
                    set.push(AABB {
                        min: glam::Vec3::new(arm_min_x, block_y as f32, block_z as f32 + 0.5),
                        max: glam::Vec3::new(arm_max_x, block_y as f32 + 1.5, block_z as f32 + 1.0),
                    });
                }

                set
            }
        }
    }

    fn block_collision_aabbs_at(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
        block_x: i32,
        block_y: i32,
        block_z: i32,
    ) -> AabbSet<8> {
        if block_y < 0 {
            return AabbSet::single(AABB {
                min: glam::Vec3::new(block_x as f32, block_y as f32, block_z as f32),
                max: glam::Vec3::new(
                    block_x as f32 + 1.0,
                    block_y as f32 + 1.0,
                    block_z as f32 + 1.0,
                ),
            });
        }
        if block_y >= CHUNK_SIZE_Y as i32 {
            return AabbSet::empty();
        }

        let chunk_x = block_x.div_euclid(CHUNK_SIZE_X as i32);
        let chunk_z = block_z.div_euclid(CHUNK_SIZE_Z as i32);
        let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
        let Some(chunk) = chunks.get(&chunk_pos) else {
            return AabbSet::empty();
        };
        let local_x = block_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_z = block_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        let voxel = chunk.voxel(local_x, block_y as usize, local_z);
        Self::collision_aabbs_for_voxel(chunks, block_properties, block_x, block_y, block_z, &voxel)
    }

    /// Check if an AABB collides with any solid blocks in the world
    fn aabb_collides_with_world(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
        aabb: &AABB,
    ) -> bool {
        // Get the range of blocks the AABB might intersect
        let min_x = aabb.min.x.floor() as i32;
        let min_y = aabb.min.y.floor() as i32;
        let min_z = aabb.min.z.floor() as i32;
        let max_x = aabb.max.x.ceil() as i32;
        let max_y = aabb.max.y.ceil() as i32;
        let max_z = aabb.max.z.ceil() as i32;

        for bx in min_x..max_x {
            for by in min_y..max_y {
                for bz in min_z..max_z {
                    let block_aabbs =
                        Self::block_collision_aabbs_at(chunks, block_properties, bx, by, bz);
                    for block_aabb in block_aabbs.iter() {
                        if aabb.intersects(block_aabb) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn aabb_touches_ladder(chunks: &HashMap<ChunkPos, Chunk>, aabb: &AABB) -> bool {
        let min_x = aabb.min.x.floor() as i32;
        let min_y = aabb.min.y.floor() as i32;
        let min_z = aabb.min.z.floor() as i32;
        let max_x = aabb.max.x.ceil() as i32;
        let max_y = aabb.max.y.ceil() as i32;
        let max_z = aabb.max.z.ceil() as i32;

        for bx in min_x..max_x {
            for by in min_y..max_y {
                if by < 0 || by >= CHUNK_SIZE_Y as i32 {
                    continue;
                }
                for bz in min_z..max_z {
                    let chunk_x = bx.div_euclid(CHUNK_SIZE_X as i32);
                    let chunk_z = bz.div_euclid(CHUNK_SIZE_Z as i32);
                    let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
                    let Some(chunk) = chunks.get(&chunk_pos) else {
                        continue;
                    };
                    let local_x = bx.rem_euclid(CHUNK_SIZE_X as i32) as usize;
                    let local_z = bz.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
                    let voxel = chunk.voxel(local_x, by as usize, local_z);
                    if matches!(
                        mdminecraft_world::get_collision_type(voxel.id, voxel.state),
                        mdminecraft_world::CollisionType::Ladder
                    ) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Move with collision detection, returning the actual position after collision resolution.
    /// Uses sweep testing along each axis separately for wall sliding.
    fn move_with_collision_axis_separated(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
        current_aabb: &AABB,
        velocity: glam::Vec3,
    ) -> (glam::Vec3, glam::Vec3) {
        let mut result_offset = glam::Vec3::ZERO;
        let mut result_velocity = velocity;

        // Move along X axis
        if velocity.x != 0.0 {
            let test_aabb =
                current_aabb.offset(glam::Vec3::new(velocity.x, 0.0, 0.0) + result_offset);
            if !Self::aabb_collides_with_world(chunks, block_properties, &test_aabb) {
                result_offset.x += velocity.x;
            } else {
                result_velocity.x = 0.0;
            }
        }

        // Move along Y axis
        if velocity.y != 0.0 {
            let test_aabb =
                current_aabb.offset(glam::Vec3::new(0.0, velocity.y, 0.0) + result_offset);
            if !Self::aabb_collides_with_world(chunks, block_properties, &test_aabb) {
                result_offset.y += velocity.y;
            } else {
                result_velocity.y = 0.0;
            }
        }

        // Move along Z axis
        if velocity.z != 0.0 {
            let test_aabb =
                current_aabb.offset(glam::Vec3::new(0.0, 0.0, velocity.z) + result_offset);
            if !Self::aabb_collides_with_world(chunks, block_properties, &test_aabb) {
                result_offset.z += velocity.z;
            } else {
                result_velocity.z = 0.0;
            }
        }

        (result_offset, result_velocity)
    }

    fn step_down_offset(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
        base_aabb: &AABB,
        max_down: f32,
    ) -> f32 {
        const STEP: f32 = 1.0 / 64.0;
        if max_down <= 0.0 {
            return 0.0;
        }

        let mut down = 0.0;
        while down < max_down {
            let next = (down + STEP).min(max_down);
            let test_aabb = base_aabb.offset(glam::Vec3::new(0.0, -next, 0.0));
            if Self::aabb_collides_with_world(chunks, block_properties, &test_aabb) {
                break;
            }
            down = next;
        }
        -down
    }

    /// Move with collision detection, returning the actual position after collision resolution.
    /// Uses axis-separated testing for wall sliding, plus an optional step-up for walking.
    fn move_with_collision(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
        current_aabb: &AABB,
        velocity: glam::Vec3,
        step_height: f32,
    ) -> (glam::Vec3, glam::Vec3) {
        let (base_offset, base_velocity) = Self::move_with_collision_axis_separated(
            chunks,
            block_properties,
            current_aabb,
            velocity,
        );

        if step_height <= 0.0 || velocity.y > 0.0 {
            return (base_offset, base_velocity);
        }

        let blocked_x = velocity.x != 0.0 && base_velocity.x == 0.0;
        let blocked_z = velocity.z != 0.0 && base_velocity.z == 0.0;
        if !(blocked_x || blocked_z) {
            return (base_offset, base_velocity);
        }

        // Only step when we're on (or very near) the ground.
        let ground_probe = current_aabb.offset(glam::Vec3::new(0.0, -0.1, 0.0));
        if !Self::aabb_collides_with_world(chunks, block_properties, &ground_probe) {
            return (base_offset, base_velocity);
        }

        let step_up = glam::Vec3::new(0.0, step_height, 0.0);
        let stepped_aabb = current_aabb.offset(step_up);
        if Self::aabb_collides_with_world(chunks, block_properties, &stepped_aabb) {
            return (base_offset, base_velocity);
        }

        // Try the horizontal move from the stepped position (no vertical move in the step attempt).
        let horizontal_velocity = glam::Vec3::new(velocity.x, 0.0, velocity.z);
        let (step_horizontal_offset, step_horizontal_velocity) =
            Self::move_with_collision_axis_separated(
                chunks,
                block_properties,
                &stepped_aabb,
                horizontal_velocity,
            );
        let stepped_after_horizontal = stepped_aabb.offset(step_horizontal_offset);

        // Drop down by up to step height to land on the surface.
        let step_down = Self::step_down_offset(
            chunks,
            block_properties,
            &stepped_after_horizontal,
            step_height,
        );
        let step_offset = step_up + step_horizontal_offset + glam::Vec3::new(0.0, step_down, 0.0);

        let base_h = glam::Vec2::new(base_offset.x, base_offset.z).length_squared();
        let step_h = glam::Vec2::new(step_offset.x, step_offset.z).length_squared();
        if step_h <= base_h {
            return (base_offset, base_velocity);
        }

        let mut result_velocity = base_velocity;
        result_velocity.x = step_horizontal_velocity.x;
        result_velocity.z = step_horizontal_velocity.z;
        (step_offset, result_velocity)
    }

    fn determine_spawn_point(
        chunks: &HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
    ) -> Option<glam::Vec3> {
        let origin = ChunkPos::new(0, 0);
        let chunk = chunks.get(&origin)?;
        let base_x = origin.x * CHUNK_SIZE_X as i32;
        let base_z = origin.z * CHUNK_SIZE_Z as i32;
        let mut best: Option<(f32, i32, i32)> = None;

        for local_z in 0..CHUNK_SIZE_Z {
            for local_x in 0..CHUNK_SIZE_X {
                for y in (0..CHUNK_SIZE_Y).rev() {
                    let voxel = chunk.voxel(local_x, y, local_z);
                    let Some(top_offset) =
                        Self::voxel_collision_top_offset(block_properties, &voxel)
                    else {
                        continue;
                    };
                    let top_world_y = y as f32 + top_offset;
                    let world_x = base_x + local_x as i32;
                    let world_z = base_z + local_z as i32;
                    if best.is_none_or(|(best_top_y, _, _)| top_world_y > best_top_y) {
                        best = Some((top_world_y, world_x, world_z));
                    }
                    break;
                }
            }
        }

        best.map(|(top_world_y, world_x, world_z)| {
            // Feet rest slightly above block top to avoid initial intersection.
            glam::Vec3::new(
                world_x as f32 + 0.5,
                top_world_y + PlayerPhysics::GROUND_EPS,
                world_z as f32 + 0.5,
            )
        })
    }

    fn player_save(&self) -> PlayerSave {
        let camera = self.renderer.camera();
        PlayerSave {
            transform: PlayerTransform {
                dimension: DimensionId::DEFAULT,
                x: camera.position.x as f64,
                y: camera.position.y as f64,
                z: camera.position.z as f64,
                yaw: camera.yaw,
                pitch: camera.pitch,
            },
            spawn_point: WorldPoint {
                dimension: DimensionId::DEFAULT,
                x: self.spawn_point.x as f64,
                y: self.spawn_point.y as f64,
                z: self.spawn_point.z as f64,
            },
            hotbar: self.hotbar.slots.clone(),
            hotbar_selected: self.hotbar.selected,
            inventory: Self::persisted_inventory_from_main_inventory(&self.main_inventory),
            health: self.player_health.current,
            hunger: self.player_health.hunger,
            xp_level: self.player_xp.level,
            xp_current: self.player_xp.current,
            xp_next_level_xp: self.player_xp.next_level_xp,
            armor: self.player_armor.clone(),
            status_effects: self.status_effects.clone(),
        }
    }

    /// Persistent inventory item-id used for "full core stack encoded in metadata" fallback.
    const PERSISTED_CORE_STACK_SENTINEL_ID: u16 = u16::MAX;

    fn persisted_inventory_from_main_inventory(main_inventory: &MainInventory) -> Inventory {
        let mut inventory = Inventory::new();
        for (idx, slot) in main_inventory.slots.iter().enumerate() {
            let Some(stack) = slot.as_ref() else {
                continue;
            };

            let world_stack = Self::persisted_world_stack_from_core(stack);
            let _ = inventory.set(9 + idx, Some(world_stack));
        }
        inventory
    }

    fn main_inventory_from_persisted_inventory(mut inventory: Inventory) -> MainInventory {
        let mut main = MainInventory::new();

        // Prefer canonical layout: slots 9-35.
        let has_any_main = (9..36).any(|slot| inventory.get(slot).is_some());
        if has_any_main {
            for idx in 0..27 {
                if let Some(world_stack) = inventory.take(9 + idx) {
                    main.slots[idx] = Self::core_stack_from_persisted_world_stack(world_stack);
                }
            }
        } else {
            // Legacy layout fallback: slots 0-26.
            for idx in 0..27 {
                if let Some(world_stack) = inventory.take(idx) {
                    main.slots[idx] = Self::core_stack_from_persisted_world_stack(world_stack);
                }
            }
        }

        main
    }

    fn persisted_world_stack_from_core(stack: &ItemStack) -> mdminecraft_world::ItemStack {
        if stack.count > u8::MAX as u32 {
            return Self::persisted_fallback_world_stack(stack);
        }

        let Some(drop_type) = Self::convert_core_item_type_to_dropped(stack.item_type) else {
            return Self::persisted_fallback_world_stack(stack);
        };

        let mut world_stack = mdminecraft_world::ItemStack::new(drop_type.id(), stack.count as u8);
        world_stack.metadata = encode_inventory_stack_metadata(stack);
        world_stack
    }

    fn persisted_fallback_world_stack(stack: &ItemStack) -> mdminecraft_world::ItemStack {
        let metadata = serde_json::to_vec(stack).unwrap_or_default();
        mdminecraft_world::ItemStack::with_metadata(
            Self::PERSISTED_CORE_STACK_SENTINEL_ID,
            1,
            metadata,
        )
    }

    fn core_stack_from_persisted_world_stack(
        stack: mdminecraft_world::ItemStack,
    ) -> Option<ItemStack> {
        if stack.item_id == Self::PERSISTED_CORE_STACK_SENTINEL_ID {
            let bytes = stack.metadata.as_ref()?;
            return serde_json::from_slice::<ItemStack>(bytes).ok();
        }

        let drop_type = DroppedItemType::from_id(stack.item_id)?;
        let core_item_type = Self::convert_dropped_item_type(drop_type)?;
        let mut core_stack = ItemStack::new(core_item_type, stack.count as u32);

        if let Some(metadata) = stack.metadata.as_ref() {
            apply_inventory_stack_metadata(&mut core_stack, metadata);
        }

        Some(core_stack)
    }

    fn apply_player_save(&mut self, save: PlayerSave) {
        self.spawn_point = glam::Vec3::new(
            save.spawn_point.x as f32,
            save.spawn_point.y as f32,
            save.spawn_point.z as f32,
        );

        self.hotbar.slots = save.hotbar;
        self.hotbar.selected = save.hotbar_selected.min(8);

        // Migrate legacy item IDs that were previously used for dropped-item conversions.
        for slot in self.hotbar.slots.iter_mut().flatten() {
            if let ItemType::Item(id) = slot.item_type {
                match id {
                    100 => slot.item_type = ItemType::Item(3), // Stick
                    101 => slot.item_type = ItemType::Item(6), // Feather
                    _ => {}
                }
            }
        }
        self.main_inventory = Self::main_inventory_from_persisted_inventory(save.inventory);

        self.player_health.current = save.health.clamp(0.0, self.player_health.max);
        self.player_health.hunger = save.hunger.clamp(0.0, self.player_health.max_hunger);
        self.player_health.time_since_damage = 0.0;
        self.player_health.invulnerability_time = 0.0;
        self.player_health.hunger_timer = 0.0;
        self.player_health.starvation_timer = 0.0;
        self.player_health.is_active = false;

        if self.player_health.is_dead() {
            self.player_health.reset();
        }

        self.player_xp.level = save.xp_level;
        self.player_xp.current = save.xp_current;
        self.player_xp.next_level_xp = save.xp_next_level_xp;

        self.player_armor = save.armor;
        self.status_effects = save.status_effects;

        self.renderer.camera_mut().position = glam::Vec3::new(
            save.transform.x as f32,
            save.transform.y as f32,
            save.transform.z as f32,
        );
        self.renderer.camera_mut().yaw = save.transform.yaw;
        self.renderer.camera_mut().pitch = save.transform.pitch;

        self.player_physics.velocity = glam::Vec3::ZERO;
        self.player_physics.on_ground = false;
        self.player_physics.last_ground_y = self.spawn_point.y;

        self.player_state = PlayerState::Alive;
        self.death_message.clear();
        self.respawn_requested = false;
        self.menu_requested = false;
    }

    fn world_entities_state(&self) -> WorldEntitiesState {
        WorldEntitiesState {
            mobs: self.mobs.clone(),
            dropped_items: self.item_manager.clone(),
            projectiles: self.projectiles.clone(),
        }
    }

    fn block_entities_state(&self) -> BlockEntitiesState {
        BlockEntitiesState {
            furnaces: self.furnaces.clone(),
            enchanting_tables: self.enchanting_tables.clone(),
            brewing_stands: self.brewing_stands.clone(),
            chests: self.chests.clone(),
        }
    }

    fn stash_ui_items_for_save(&mut self) {
        let mut returned: Vec<ItemStack> = Vec::new();

        if let Some(stack) = self.ui_cursor_stack.take() {
            returned.push(stack);
        }

        for row in &mut self.personal_crafting_grid {
            for slot in row.iter_mut() {
                if let Some(stack) = slot.take() {
                    returned.push(stack);
                }
            }
        }

        for row in &mut self.crafting_grid {
            for slot in row.iter_mut() {
                if let Some(stack) = slot.take() {
                    returned.push(stack);
                }
            }
        }

        for stack in returned {
            self.return_stack_to_storage_or_spill(stack);
        }
    }

    fn persist_world(&mut self) {
        self.stash_ui_items_for_save();
        self.persist_loaded_chunks();

        let meta = WorldMeta {
            world_seed: self.world_seed,
        };
        if let Err(err) = self.region_store.save_world_meta(&meta) {
            tracing::warn!(?err, "Failed to save world meta");
        }

        let state = WorldState {
            tick: self.sim_tick,
            sim_time: self.sim_time,
            weather: self.weather,
            weather_next_change_tick: self.weather_next_change_tick,
            player: Some(self.player_save()),
            entities: self.world_entities_state(),
            block_entities: self.block_entities_state(),
        };
        if let Err(err) = self.region_store.save_world_state(&state) {
            tracing::warn!(?err, "Failed to save world state");
        }
    }

    fn persist_loaded_chunks(&mut self) {
        for (pos, chunk) in &self.chunks {
            if let Err(err) = self.region_store.save_chunk(chunk) {
                tracing::error!(?pos, ?err, "Failed to save chunk");
            }
        }
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
                        self.persist_world();
                        return GameAction::Quit;
                    }
                    WindowEvent::Focused(focused) => {
                        if *focused {
                            // Regained focus - recapture cursor if we were in gameplay mode
                            let _ = self.input.handle_focus_regained(&self.window);
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        if let winit::keyboard::PhysicalKey::Code(KeyCode::Escape) =
                            event.physical_key
                        {
                            if event.state.is_pressed() {
                                self.handle_escape_pressed();
                                return GameAction::Continue;
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
                            if matches!(action, GameAction::ReturnToMenu | GameAction::Quit) {
                                self.persist_world();
                            }
                            return action;
                        }

                        if let Some(action) = self.pending_action.take() {
                            if matches!(action, GameAction::ReturnToMenu | GameAction::Quit) {
                                self.persist_world();
                            }
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

        if self.pause_menu_open {
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
                self.sim_time_paused = !self.sim_time_paused;
                tracing::info!(
                    paused = self.sim_time_paused,
                    "Simulation day/night progression toggled"
                );
            }
            PhysicalKey::Code(KeyCode::BracketLeft) => {
                // Increase ticks per day to slow down the day/night cycle.
                self.sim_time.ticks_per_day =
                    (self.sim_time.ticks_per_day.saturating_mul(3) / 2).min(240_000);
                tracing::info!(
                    ticks_per_day = self.sim_time.ticks_per_day,
                    "Simulation day length increased"
                );
            }
            PhysicalKey::Code(KeyCode::BracketRight) => {
                // Decrease ticks per day to speed up the day/night cycle.
                self.sim_time.ticks_per_day =
                    ((self.sim_time.ticks_per_day.saturating_mul(2)) / 3).max(2_400);
                tracing::info!(
                    ticks_per_day = self.sim_time.ticks_per_day,
                    "Simulation day length decreased"
                );
            }
            PhysicalKey::Code(KeyCode::KeyO) => {
                self.weather.toggle();
                let delay_ticks = Self::weather_delay_ticks(
                    self.world_seed,
                    self.sim_tick,
                    900..2400, // 45..120 seconds at 20 TPS
                    0xB16B_00B5_u64,
                );
                self.weather_next_change_tick = self.sim_tick.advance(delay_ticks);
                tracing::info!(state = ?self.weather.state, "Weather toggled");
            }
            PhysicalKey::Code(KeyCode::KeyE) => {
                if self.chest_open {
                    self.close_chest();
                } else if self.brewing_open {
                    self.close_brewing_stand();
                } else if self.enchanting_open {
                    self.close_enchanting_table();
                } else if self.furnace_open {
                    self.close_furnace();
                } else if self.crafting_open {
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
                } else if self.brewing_open {
                    self.close_brewing_stand();
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

        if actions.drop_item {
            self.drop_selected_hotbar_item(actions.drop_stack);
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

    fn weather_delay_ticks(
        world_seed: u64,
        tick: SimTick,
        range: std::ops::Range<u64>,
        salt: u64,
    ) -> u64 {
        let seed = world_seed ^ tick.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt;
        let mut rng = StdRng::seed_from_u64(seed);
        rng.gen_range(range)
    }

    fn tick_weather(&mut self) {
        if self.sim_tick < self.weather_next_change_tick {
            return;
        }

        let seed = self.world_seed
            ^ self.sim_tick.0.wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ 0x0057_4541_5448_4552_u64;
        let mut rng = StdRng::seed_from_u64(seed);

        let delay_ticks = rng.gen_range(1200..3000); // 60..150 seconds at 20 TPS
        self.weather_next_change_tick = self.sim_tick.advance(delay_ticks);

        let target_state = if self.weather.is_precipitating() {
            if rng.gen_bool(0.7) {
                WeatherState::Clear
            } else {
                WeatherState::Precipitation
            }
        } else if rng.gen_bool(0.55) {
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

    fn update_weather(&mut self, dt: f32) {
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

    /// Apply Mending enchantment effect: use XP to repair tools with Mending.
    ///
    /// Returns the amount of XP that should go to the player (after Mending uses some).
    /// In vanilla Minecraft, 1 XP restores 2 durability.
    fn apply_mending(&mut self, xp_amount: u32) -> u32 {
        let mut remaining_xp = xp_amount;

        // Check each hotbar slot for tools with Mending that need repair
        for slot in &mut self.hotbar.slots {
            if remaining_xp == 0 {
                break;
            }

            if let Some(stack) = slot {
                // Only process tools with durability and Mending enchantment
                if let Some(current_dur) = stack.durability {
                    if stack.has_enchantment(EnchantmentType::Mending) {
                        // Calculate max durability for this tool
                        let max_dur = stack.max_durability().unwrap_or(0);

                        if current_dur < max_dur {
                            // Each XP restores 2 durability
                            let missing_dur = max_dur - current_dur;
                            let xp_needed = missing_dur.div_ceil(2); // Ceiling division
                            let xp_to_use = remaining_xp.min(xp_needed);
                            let dur_restored = xp_to_use * 2;

                            // Apply repair
                            stack.durability = Some((current_dur + dur_restored).min(max_dur));
                            remaining_xp -= xp_to_use;

                            tracing::debug!(
                                "Mending restored {} durability to {:?} (used {} XP)",
                                dur_restored,
                                stack.item_type,
                                xp_to_use
                            );
                        }
                    }
                }
            }
        }

        remaining_xp
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

        // Render mobs
        for mob in &self.mobs {
            if mob.dead {
                continue;
            }

            let height = match mob.mob_type {
                MobType::Chicken | MobType::Spider => 0.5,
                _ => 1.8,
            };

            let center_pos =
                glam::Vec3::new(mob.x as f32, mob.y as f32 + height * 0.5, mob.z as f32);

            let color = match mob.mob_type {
                MobType::Zombie => [0.0, 0.5, 0.0, 1.0],
                MobType::Skeleton => [0.8, 0.8, 0.8, 1.0],
                MobType::Creeper => [0.0, 0.8, 0.0, 1.0],
                MobType::Spider => [0.1, 0.1, 0.1, 1.0],
                MobType::Pig => [1.0, 0.6, 0.6, 1.0],
                MobType::Cow => [0.3, 0.2, 0.1, 1.0],
                MobType::Sheep => [0.9, 0.9, 0.9, 1.0],
                MobType::Chicken => [1.0, 1.0, 1.0, 1.0],
            };

            let size = match mob.mob_type {
                MobType::Chicken => [0.5, 0.5],
                MobType::Spider => [1.0, 0.5],
                _ => [0.6, 1.8],
            };

            self.billboard_emitter.submit(
                0,
                BillboardInstance {
                    position: center_pos.to_array(),
                    size,
                    color,
                    flags: 0,
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

            // Handle double-jump to toggle fly mode
            if actions.jump_pressed {
                if physics.last_jump_press_time < 0.3 {
                    physics.toggle_physics();
                    physics.last_jump_press_time = 10.0; // Reset
                    tracing::info!(
                        "Physics mode: {}",
                        if physics.physics_enabled {
                            "ENABLED"
                        } else {
                            "DISABLED (fly mode)"
                        }
                    );
                } else {
                    physics.last_jump_press_time = 0.0;
                }
            } else {
                physics.last_jump_press_time += dt;
            }

            let on_ladder = Self::aabb_touches_ladder(&self.chunks, &physics.get_aabb(camera_pos));
            if on_ladder {
                // Vanilla-ish: ladders cancel gravity and clamp vertical speed.
                let climb_speed = 3.0;
                if actions.jump {
                    physics.velocity.y = climb_speed;
                } else if actions.crouch {
                    physics.velocity.y = -climb_speed;
                } else {
                    physics.velocity.y = physics.velocity.y.clamp(-1.0, 0.0);
                }
            } else {
                // Apply gravity
                physics.velocity.y += physics.gravity * dt;
                if physics.velocity.y < physics.terminal_velocity {
                    physics.velocity.y = physics.terminal_velocity;
                }
            }

            // Calculate horizontal movement
            let mut axis = glam::Vec2::new(actions.move_x, actions.move_y);
            if axis.length_squared() > 1.0 {
                axis = axis.normalize();
            }

            let mut move_speed = if actions.sprint { 6.0 } else { 4.3 };
            if actions.crouch {
                move_speed *= 0.5;
            }

            // Build movement velocity vector
            let mut move_velocity = glam::Vec3::ZERO;
            if axis.length_squared() > 0.0 {
                let move_dir = forward_h * axis.y + right_h * axis.x;
                move_velocity = move_dir * move_speed * dt;
            }
            move_velocity.y = physics.velocity.y * dt;

            // Get current AABB and apply movement with collision
            let current_aabb = physics.get_aabb(camera_pos);
            let (offset, new_velocity) = Self::move_with_collision(
                &self.chunks,
                &self.block_properties,
                &current_aabb,
                move_velocity,
                PlayerPhysics::STEP_HEIGHT,
            );

            camera_pos += offset;

            // Update velocity based on collision results (for Y axis mainly)
            let was_on_ground = physics.on_ground;
            let was_falling = physics.velocity.y < 0.0;

            if new_velocity.y == 0.0 && physics.velocity.y != 0.0 {
                // We hit something vertically
                if was_falling {
                    // Hit ground
                    let player_aabb = physics.get_aabb(camera_pos);
                    let ground_y = player_aabb.min.y;

                    if !was_on_ground {
                        fall_damage = Some(physics.last_ground_y - ground_y);
                    }

                    physics.on_ground = true;
                    physics.last_ground_y = ground_y;
                }
                physics.velocity.y = 0.0;
            } else {
                physics.on_ground = false;
            }

            // Check if standing on ground (for jump detection)
            let feet_check_aabb = physics
                .get_aabb(camera_pos)
                .offset(glam::Vec3::new(0.0, -0.1, 0.0));
            if Self::aabb_collides_with_world(
                &self.chunks,
                &self.block_properties,
                &feet_check_aabb,
            ) {
                physics.on_ground = true;
                physics.last_ground_y = physics.get_aabb(camera_pos).min.y;
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
        // Handle double-jump to toggle fly mode (exit fly)
        if actions.jump_pressed {
            let physics = &mut self.player_physics;
            if physics.last_jump_press_time < 0.3 {
                physics.toggle_physics();
                physics.last_jump_press_time = 10.0; // Reset
                tracing::info!("Physics mode: ENABLED");
            } else {
                physics.last_jump_press_time = 0.0;
            }
        } else {
            self.player_physics.last_jump_press_time += dt;
        }

        let (forward, right, position) = {
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
            let velocity = movement.normalize() * speed * dt;

            // Apply collision detection for fly mode (like original Minecraft)
            let current_aabb = self.player_physics.get_aabb(position);
            let (offset, _) = Self::move_with_collision(
                &self.chunks,
                &self.block_properties,
                &current_aabb,
                velocity,
                0.0,
            );

            self.renderer.camera_mut().position = position + offset;
        }
    }

    fn fixed_update(&mut self) {
        // Update world chunks
        self.update_chunks(1);

        // Increment tick
        self.sim_tick = self.sim_tick.advance(1);
        if !self.sim_time_paused {
            self.sim_time.advance();
        }
        self.tick_weather();
        let dt = TICK_RATE as f32;

        // Tick player survival systems (deterministic, once per sim tick).
        self.tick_player_survival(dt);

        // Update dropped items and handle pickup
        self.update_dropped_items();

        // Update furnaces
        self.update_furnaces(dt);

        // Update brewing stands
        self.update_brewing_stands(dt);

        // Update mobs
        self.update_mobs(dt);

        let mut mesh_refresh = std::collections::BTreeSet::new();

        // Update farming (crops + farmland hydration).
        self.crop_growth.tick(self.sim_tick.0, &mut self.chunks);
        for chunk_pos in self.crop_growth.take_dirty_chunks() {
            mesh_refresh.insert(chunk_pos);
            for neighbor in Self::neighbor_chunk_positions(chunk_pos) {
                mesh_refresh.insert(neighbor);
            }
        }

        // Update sugar cane growth.
        self.sugar_cane_growth
            .tick(self.sim_tick.0, &mut self.chunks);
        for chunk_pos in self.sugar_cane_growth.take_dirty_chunks() {
            mesh_refresh.insert(chunk_pos);
            for neighbor in Self::neighbor_chunk_positions(chunk_pos) {
                mesh_refresh.insert(neighbor);
            }
        }

        // Update fluids
        self.fluid_sim.tick(&mut self.chunks);
        let dirty_fluids = self.fluid_sim.take_dirty_chunks();
        let dirty_fluid_lighting = self.fluid_sim.take_dirty_light_chunks();
        for chunk_pos in dirty_fluids {
            self.recompute_chunk_lighting(chunk_pos);
            mesh_refresh.insert(chunk_pos);
            for neighbor in Self::neighbor_chunk_positions(chunk_pos) {
                mesh_refresh.insert(neighbor);
            }
        }
        for chunk_pos in dirty_fluid_lighting {
            let affected = mdminecraft_world::recompute_block_light_local(
                &mut self.chunks,
                &self.registry,
                chunk_pos,
            );
            mesh_refresh.extend(affected);
        }

        // Update redstone
        self.update_player_pressure_plate();
        self.redstone_sim.tick(&mut self.chunks);
        let dirty_redstone = self.redstone_sim.take_dirty_chunks();
        let dirty_redstone_lighting = self.redstone_sim.take_dirty_light_chunks();
        for chunk_pos in dirty_redstone {
            mesh_refresh.insert(chunk_pos);
            for neighbor in Self::neighbor_chunk_positions(chunk_pos) {
                mesh_refresh.insert(neighbor);
            }
        }
        for chunk_pos in dirty_redstone_lighting {
            let affected = mdminecraft_world::recompute_block_light_local(
                &mut self.chunks,
                &self.registry,
                chunk_pos,
            );
            mesh_refresh.extend(affected);
        }
        for chunk_pos in mesh_refresh {
            let _ = self.upload_chunk_mesh(chunk_pos);
        }

        // Update projectiles (arrows)
        self.update_projectiles();

        // Check for death
        if self.player_health.is_dead() && self.player_state != PlayerState::Dead {
            self.handle_death("You died!");
        }
    }

    fn tick_player_survival(&mut self, dt: f32) {
        if self.player_state != PlayerState::Alive {
            return;
        }

        // Core survival stats (hunger/regen) and timers. Use fixed dt to avoid FPS dependence.
        self.player_health.update(dt);

        // Tick status effects in sim-time (20 TPS).
        self.update_status_effects(dt);

        let camera_pos = self.renderer.camera().position;
        let feet_pos = camera_pos - glam::Vec3::new(0.0, self.player_physics.eye_height, 0.0);
        let feet_sample = feet_pos + glam::Vec3::new(0.0, 0.1, 0.0);

        let fluid_at = |pos: glam::Vec3| -> Option<FluidType> {
            self.get_block_at(IVec3::new(
                pos.x.floor() as i32,
                pos.y.floor() as i32,
                pos.z.floor() as i32,
            ))
            .and_then(get_fluid_type)
        };

        let eye_fluid = fluid_at(camera_pos);
        let feet_fluid = fluid_at(feet_sample);

        let has_water_breathing = self.status_effects.has(StatusEffectType::WaterBreathing);
        let has_fire_resistance = self.status_effects.has(StatusEffectType::FireResistance);

        let eye_in_water = eye_fluid == Some(FluidType::Water);
        let in_water = eye_in_water || feet_fluid == Some(FluidType::Water);
        let in_lava = eye_fluid == Some(FluidType::Lava) || feet_fluid == Some(FluidType::Lava);

        // Drowning (vanilla-ish): 15s air, then periodic damage while underwater.
        if self
            .player_health
            .tick_air(eye_in_water, has_water_breathing)
        {
            self.player_health.damage(2.0);
        }

        // Lava contact + ignition (vanilla-ish; simplified).
        if in_lava && !has_fire_resistance {
            self.player_health.damage(4.0);
            self.player_health.ignite(300);
        }

        // Burning DOT.
        if self
            .player_health
            .tick_burning(in_water, has_fire_resistance)
        {
            self.player_health.damage(1.0);
        }

        // XP orbs (physics + collection).
        let mut collected_xp = 0u32;
        self.xp_orbs.retain_mut(|orb| {
            if orb.update(dt, camera_pos) {
                return false;
            }
            if orb.should_collect(camera_pos) {
                collected_xp = collected_xp.saturating_add(orb.value);
                return false;
            }
            true
        });
        if collected_xp > 0 {
            let remaining_xp = self.apply_mending(collected_xp);
            if remaining_xp > 0 {
                self.player_xp.add_xp(remaining_xp);
            }
        }
    }

    fn update_chunks(&mut self, max_load: usize) {
        let camera_pos = self.renderer.camera().position;
        let center_chunk_x = (camera_pos.x / 16.0).floor() as i32;
        let center_chunk_z = (camera_pos.z / 16.0).floor() as i32;
        let radius = self.render_distance;

        // Unload chunks
        let mut chunks_to_unload = Vec::new();
        for pos in self.chunks.keys() {
            let dx = pos.x - center_chunk_x;
            let dz = pos.z - center_chunk_z;
            if dx * dx + dz * dz > (radius + 2) * (radius + 2) {
                chunks_to_unload.push(*pos);
            }
        }

        let mut neighbor_mesh_refresh = std::collections::BTreeSet::new();
        for pos in chunks_to_unload {
            self.crop_growth.unregister_chunk(pos);
            self.sugar_cane_growth.unregister_chunk(pos);
            if let Some(chunk) = self.chunks.remove(&pos) {
                if let Err(e) = self.region_store.save_chunk(&chunk) {
                    tracing::error!("Failed to save chunk {:?}: {}", pos, e);
                }
            }
            self.chunk_manager.remove_chunk(&pos);
            for neighbor in Self::neighbor_chunk_positions(pos) {
                neighbor_mesh_refresh.insert(neighbor);
            }
        }
        for pos in neighbor_mesh_refresh {
            let _ = self.upload_chunk_mesh(pos);
        }

        // Load chunks
        let mut chunks_to_load = Vec::new();
        for x in -radius..=radius {
            for z in -radius..=radius {
                if x * x + z * z > radius * radius {
                    continue;
                }
                let chunk_pos = ChunkPos::new(center_chunk_x + x, center_chunk_z + z);
                if !self.chunks.contains_key(&chunk_pos) {
                    chunks_to_load.push(chunk_pos);
                }
            }
        }

        // Sort by distance to center to load nearest first
        chunks_to_load.sort_by_key(|pos| {
            let dx = pos.x - center_chunk_x;
            let dz = pos.z - center_chunk_z;
            dx * dx + dz * dz
        });

        // Apply limit
        if chunks_to_load.len() > max_load {
            chunks_to_load.truncate(max_load);
        }

        if self.renderer.render_resources().is_none() {
            return;
        }

        // Load limited number per frame to avoid lag (e.g. 2 chunks)
        // But for initial load we might want more.
        // For now, load all to ensure correctness, optimization later.
        for pos in chunks_to_load {
            let chunk = if let Ok(loaded) = self.region_store.load_chunk(pos) {
                loaded
            } else {
                self.terrain_generator.generate_chunk(pos)
            };

            let mut crops_to_register = Vec::new();
            let mut sugar_cane_bases_to_register = Vec::new();
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    for x in 0..CHUNK_SIZE_X {
                        let voxel = chunk.voxel(x, y, z);
                        if !mdminecraft_world::CropType::is_crop(voxel.id) {
                            if voxel.id == mdminecraft_world::BLOCK_SUGAR_CANE {
                                if y == 0 {
                                    continue;
                                }
                                let below = chunk.voxel(x, y - 1, z);
                                if below.id != mdminecraft_world::BLOCK_SUGAR_CANE {
                                    sugar_cane_bases_to_register.push(SugarCanePosition {
                                        chunk: pos,
                                        x: x as u8,
                                        y: y as u8,
                                        z: z as u8,
                                    });
                                }
                            }
                            continue;
                        }

                        crops_to_register.push(CropPosition {
                            chunk: pos,
                            x: x as u8,
                            y: y as u8,
                            z: z as u8,
                        });
                    }
                }
            }

            self.chunks.insert(pos, chunk);
            for crop in crops_to_register {
                self.crop_growth.register_crop(crop);
            }
            for base in sugar_cane_bases_to_register {
                self.sugar_cane_growth.register_base(base);
            }
            self.recompute_chunk_lighting(pos);
            let affected = mdminecraft_world::recompute_block_light_local(
                &mut self.chunks,
                &self.registry,
                pos,
            );
            let mut mesh_refresh = std::collections::BTreeSet::new();
            mesh_refresh.insert(pos);
            mesh_refresh.extend(Self::neighbor_chunk_positions(pos));
            mesh_refresh.extend(affected);
            for chunk_pos in mesh_refresh {
                let _ = self.upload_chunk_mesh(chunk_pos);
            }

            // Spawn mobs in new chunk
            if let Some(chunk) = self.chunks.get(&pos) {
                let chunk_center_x = pos.x * CHUNK_SIZE_X as i32 + CHUNK_SIZE_X as i32 / 2;
                let chunk_center_z = pos.z * CHUNK_SIZE_Z as i32 + CHUNK_SIZE_Z as i32 / 2;
                let biome = self
                    .terrain_generator
                    .biome_assigner()
                    .get_biome(chunk_center_x, chunk_center_z);

                let mut surface_heights = [[0i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];
                for (local_z, row) in surface_heights.iter_mut().enumerate() {
                    for (local_x, height) in row.iter_mut().enumerate() {
                        for y in (0..CHUNK_SIZE_Y).rev() {
                            let voxel = chunk.voxel(local_x, y, local_z);
                            if voxel.id != BLOCK_AIR {
                                *height = y as i32;
                                break;
                            }
                        }
                    }
                }

                let mut new_mobs =
                    self.mob_spawner
                        .generate_spawns(pos.x, pos.z, biome, &surface_heights);
                if !new_mobs.is_empty() {
                    tracing::info!(
                        chunk_x = pos.x,
                        chunk_z = pos.z,
                        new_mob_count = new_mobs.len(),
                        total_mobs = self.mobs.len() + new_mobs.len(),
                        "Adding mobs to world"
                    );
                }
                self.mobs.append(&mut new_mobs);
            }
        }
    }

    fn update_and_render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f64();
        self.last_frame = now;
        self.frame_dt = dt as f32;
        self.debug_hud.chunk_uploads_last_frame = 0;

        // Cap dt to avoid spiral of death
        let dt = dt.min(0.25);
        if self.pause_menu_open {
            // Prevent accumulator buildup while paused so unpausing doesn't "fast-forward".
            self.accumulator = 0.0;
        } else {
            self.accumulator += dt;

            while self.accumulator >= TICK_RATE {
                self.fixed_update();
                self.accumulator -= TICK_RATE;
            }
        }

        // Process input and camera every frame for responsiveness
        self.process_actions(self.frame_dt);

        // Sync visual time-of-day from deterministic simulation time.
        self.time_of_day
            .set_time(self.sim_time.time_of_day() as f32);

        // Update environment and effects (visual)
        self.update_weather(self.frame_dt);
        self.update_particles(self.frame_dt);
        self.debug_hud.particle_count = self.particles.len();

        // Update debug HUD
        self.debug_hud.update_fps(self.frame_dt);
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
            self.update_camera(self.frame_dt);
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
            self.handle_block_interaction(self.frame_dt);
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

    fn player_has_arrows(&self) -> bool {
        if self.hotbar.has_arrows() {
            return true;
        }

        self.main_inventory.slots.iter().any(|slot| {
            slot.as_ref()
                .is_some_and(|item| matches!(item.item_type, ItemType::Item(2)))
        })
    }

    fn player_consume_arrow(&mut self) -> bool {
        if self.hotbar.consume_arrow() {
            return true;
        }

        for slot in &mut self.main_inventory.slots {
            let Some(item) = slot.as_mut() else {
                continue;
            };

            if !matches!(item.item_type, ItemType::Item(2)) {
                continue;
            }

            if item.count > 1 {
                item.count -= 1;
            } else {
                *slot = None;
            }
            return true;
        }

        false
    }

    fn handle_block_interaction(&mut self, dt: f32) {
        // Handle bow charging and shooting (before other interactions)
        if self.hotbar.has_bow_selected() && self.player_has_arrows() {
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
                    if self.player_consume_arrow() {
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
        if self.input.is_mouse_clicked(MouseButton::Left)
            && self.attack_cooldown <= 0.0
            && self.try_attack_mob()
        {
            // Attacked a mob successfully - set cooldown to 0.6 seconds
            self.attack_cooldown = 0.6;
            // Don't mine
            self.mining_progress = None;
            return;
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
                if self.try_interact_with_target_block(hit) {
                    return;
                }

                // First, check if we're holding armor and try to equip it
                let mut equipped_armor = false;
                if let Some(stack) = self.hotbar.slots[self.hotbar.selected].clone() {
                    if let Some(armor_piece) = armor_piece_from_core_stack(&stack) {
                        // Equip the armor piece.
                        let old_piece = self.player_armor.equip(armor_piece);

                        // Consume the item from hotbar.
                        let _ = self.hotbar.consume_selected();
                        equipped_armor = true;
                        tracing::info!("Equipped armor");

                        // Return any replaced armor back to player storage (or spill to world).
                        if let Some(old_piece) = old_piece {
                            if let Some(old_stack) = armor_piece_to_core_stack(&old_piece) {
                                self.return_stack_to_storage_or_spill(old_stack);
                            } else {
                                tracing::warn!(
                                    item = ?old_piece.item_type,
                                    "Replaced armor could not be represented as a core stack"
                                );
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
                } else if let Some(potion_id) = self.hotbar.selected_potion() {
                    // Check if we're holding a potion and try to drink it
                    if self.drink_potion(potion_id) {
                        self.hotbar.consume_selected();
                        // Skip other interactions when drinking
                    }
                } else if let Some(potion_id) = self.hotbar.selected_splash_potion() {
                    // Check if we're holding a splash potion and throw it
                    self.throw_splash_potion(potion_id);
                    self.hotbar.consume_selected();
                    // Skip other interactions when throwing
                } else {
                    self.handle_block_placement(hit);
                }
            }
        } else {
            // No block selected, reset mining progress
            self.mining_progress = None;
        }
    }

    fn try_sleep_in_bed(&mut self, bed_pos: IVec3) {
        if self.player_state != PlayerState::Alive {
            return;
        }

        // Time: 0.0-0.25 Night→Dawn, 0.75-1.0 Dusk→Night
        let time = self.sim_time.time_of_day() as f32;
        let is_night = !(0.25..=0.75).contains(&time);
        if !is_night {
            tracing::info!("Tried to sleep, but it's not night");
            return;
        }

        let bed_center_x = bed_pos.x as f64 + 0.5;
        let bed_center_y = bed_pos.y as f64 + 0.5;
        let bed_center_z = bed_pos.z as f64 + 0.5;

        // Vanilla checks a radius around the bed. Keep simple: any living hostile within 8 blocks.
        let monsters_nearby = self.mobs.iter().any(|mob| {
            !mob.dead
                && mob.is_hostile()
                && mob.distance_to(bed_center_x, bed_center_y, bed_center_z) <= 8.0
        });
        if monsters_nearby {
            tracing::info!("Tried to sleep, but monsters are nearby");
            return;
        }

        // Set spawn point to the block above the bed (feet position).
        self.spawn_point = glam::Vec3::new(
            bed_pos.x as f32 + 0.5,
            bed_pos.y as f32 + 1.0,
            bed_pos.z as f32 + 0.5,
        );
        self.player_physics.last_ground_y = self.spawn_point.y;

        // Advance simulation time to sunrise (time_of_day = 0.25).
        let ticks_per_day = self.sim_time.ticks_per_day.max(1);
        let target_tick_in_day = (ticks_per_day as f64 * 0.25).round() as u64;
        let tick_in_day = self.sim_time.tick.0 % ticks_per_day;
        let advance = if tick_in_day < target_tick_in_day {
            target_tick_in_day - tick_in_day
        } else {
            (ticks_per_day - tick_in_day) + target_tick_in_day
        };
        self.sim_time.tick = self.sim_time.tick.advance(advance);

        // Vanilla clears weather after sleeping; keep it simple.
        self.weather.set_state(WeatherState::Clear);

        tracing::info!(
            "Slept in bed at {:?} (spawn set), advanced time by {} ticks",
            bed_pos,
            advance
        );
    }

    fn try_fill_glass_bottle_from_water(&mut self) -> bool {
        let selected = self.hotbar.selected;
        let Some(stack) = self.hotbar.slots[selected].as_ref() else {
            return false;
        };

        if stack.item_type != ItemType::Item(CORE_ITEM_GLASS_BOTTLE) || stack.count == 0 {
            return false;
        }

        // Vanilla-ish behavior: after filling, the player holds the filled bottle and any
        // remaining empty bottles move to inventory (or spill if full).
        let remaining_empty = stack.count.saturating_sub(1);
        self.hotbar.slots[selected] =
            Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 1));

        if remaining_empty > 0 {
            self.return_stack_to_storage_or_spill(ItemStack::new(
                ItemType::Item(CORE_ITEM_GLASS_BOTTLE),
                remaining_empty,
            ));
        }

        true
    }

    fn try_till_farmland(
        &mut self,
        block_id: BlockId,
        chunk_pos: ChunkPos,
        local_x: usize,
        local_y: usize,
        local_z: usize,
    ) -> bool {
        let Some((tool, _material)) = self.hotbar.selected_tool() else {
            return false;
        };
        if tool != ToolType::Hoe {
            return false;
        }

        if !mdminecraft_world::can_till(block_id) {
            return false;
        }

        if local_y + 1 >= CHUNK_SIZE_Y {
            return false;
        }

        let Some(chunk) = self.chunks.get_mut(&chunk_pos) else {
            return false;
        };

        if chunk.voxel(local_x, local_y + 1, local_z).id != BLOCK_AIR {
            return false;
        }

        let mut voxel = chunk.voxel(local_x, local_y, local_z);
        voxel.id = mdminecraft_world::farming_blocks::FARMLAND;
        voxel.state = 0;
        chunk.set_voxel(local_x, local_y, local_z, voxel);

        // Using a hoe consumes durability.
        if let Some(item) = self.hotbar.selected_item_mut() {
            if matches!(item.item_type, ItemType::Tool(ToolType::Hoe, _)) {
                item.damage_durability(1);
                if item.is_broken() {
                    self.hotbar.slots[self.hotbar.selected] = None;
                }
            }
        }

        self.debug_hud.chunk_uploads_last_frame += self.upload_chunk_mesh_and_neighbors(chunk_pos);
        true
    }

    fn try_plant_crop(
        &mut self,
        block_id: BlockId,
        chunk_pos: ChunkPos,
        local_x: usize,
        local_y: usize,
        local_z: usize,
    ) -> bool {
        let Some(stack) = self.hotbar.selected_item() else {
            return false;
        };
        if stack.count == 0 {
            return false;
        }

        let crop_type = match stack.item_type {
            ItemType::Item(CORE_ITEM_WHEAT_SEEDS) => mdminecraft_world::CropType::Wheat,
            ItemType::Food(mdminecraft_core::item::FoodType::Carrot) => {
                mdminecraft_world::CropType::Carrots
            }
            ItemType::Food(mdminecraft_core::item::FoodType::Potato) => {
                mdminecraft_world::CropType::Potatoes
            }
            _ => return false,
        };

        if !mdminecraft_world::is_farmland(block_id) {
            return false;
        }

        if local_y + 1 >= CHUNK_SIZE_Y {
            return true;
        }

        let Some(chunk) = self.chunks.get_mut(&chunk_pos) else {
            return true;
        };

        let above = chunk.voxel(local_x, local_y + 1, local_z);
        if above.id != BLOCK_AIR {
            return true;
        }

        chunk.set_voxel(
            local_x,
            local_y + 1,
            local_z,
            Voxel {
                id: crop_type.base_block_id(),
                state: 0,
                light_sky: above.light_sky,
                light_block: above.light_block,
            },
        );

        self.crop_growth.register_crop(CropPosition {
            chunk: chunk_pos,
            x: local_x as u8,
            y: (local_y + 1) as u8,
            z: local_z as u8,
        });

        let _ = self.hotbar.consume_selected();

        self.debug_hud.chunk_uploads_last_frame += self.upload_chunk_mesh_and_neighbors(chunk_pos);
        true
    }

    fn try_interact_with_target_block(&mut self, hit: RaycastHit) -> bool {
        let chunk_x = hit.block_pos.x.div_euclid(CHUNK_SIZE_X as i32);
        let chunk_z = hit.block_pos.z.div_euclid(CHUNK_SIZE_Z as i32);
        let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
        let local_x = hit.block_pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_y = hit.block_pos.y as usize;
        let local_z = hit.block_pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

        let block_id = self.chunks.get(&chunk_pos).and_then(|chunk| {
            if local_y < CHUNK_SIZE_Y {
                Some(chunk.voxel(local_x, local_y, local_z).id)
            } else {
                None
            }
        });

        if let Some(id) = block_id {
            if self.try_till_farmland(id, chunk_pos, local_x, local_y, local_z) {
                return true;
            }
            if self.try_plant_crop(id, chunk_pos, local_x, local_y, local_z) {
                return true;
            }
        }

        match block_id {
            Some(BLOCK_CRAFTING_TABLE) => {
                self.open_crafting();
                true
            }
            Some(BLOCK_FURNACE) | Some(BLOCK_FURNACE_LIT) => {
                self.open_furnace(hit.block_pos);
                true
            }
            Some(BLOCK_ENCHANTING_TABLE) => {
                self.open_enchanting_table(hit.block_pos);
                true
            }
            Some(BLOCK_BREWING_STAND) => {
                self.open_brewing_stand(hit.block_pos);
                true
            }
            Some(interactive_blocks::CHEST) => {
                self.open_chest(hit.block_pos);
                true
            }
            Some(interactive_blocks::BED_HEAD) | Some(interactive_blocks::BED_FOOT) => {
                self.try_sleep_in_bed(hit.block_pos);
                true
            }
            Some(mdminecraft_world::BLOCK_WATER) => self.try_fill_glass_bottle_from_water(),
            Some(id)
                if mdminecraft_world::is_door(id)
                    || mdminecraft_world::is_trapdoor(id)
                    || mdminecraft_world::is_fence_gate(id) =>
            {
                let result = self.interaction_manager.interact(
                    chunk_pos,
                    local_x,
                    local_y,
                    local_z,
                    &mut self.chunks,
                );
                if result == mdminecraft_world::InteractionResult::None {
                    return false;
                }

                let mut mesh_refresh = std::collections::BTreeSet::new();
                for dirty in self.interaction_manager.take_dirty_chunks() {
                    mesh_refresh.insert(dirty);
                    for neighbor in Self::neighbor_chunk_positions(dirty) {
                        mesh_refresh.insert(neighbor);
                    }
                }
                for dirty_chunk in mesh_refresh {
                    self.debug_hud.chunk_uploads_last_frame +=
                        self.upload_chunk_mesh(dirty_chunk) as u32;
                }
                true
            }
            Some(mdminecraft_world::redstone_blocks::LEVER) => {
                self.redstone_sim.toggle_lever(
                    RedstonePos::new(hit.block_pos.x, hit.block_pos.y, hit.block_pos.z),
                    &mut self.chunks,
                );
                self.debug_hud.chunk_uploads_last_frame +=
                    self.upload_chunk_mesh_and_neighbors(chunk_pos);
                true
            }
            Some(mdminecraft_world::redstone_blocks::STONE_BUTTON)
            | Some(mdminecraft_world::redstone_blocks::OAK_BUTTON) => {
                self.redstone_sim.activate_button(
                    RedstonePos::new(hit.block_pos.x, hit.block_pos.y, hit.block_pos.z),
                    &mut self.chunks,
                );
                self.debug_hud.chunk_uploads_last_frame +=
                    self.upload_chunk_mesh_and_neighbors(chunk_pos);
                true
            }
            _ => false,
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
            let mut mining_time = block_props.calculate_mining_time(tool, false);

            // Apply Efficiency enchantment bonus
            // Each level of Efficiency adds 26% mining speed (Minecraft formula: (level^2 + 1) bonus)
            // Simplified: multiply speed by 1 + (0.26 * level)
            if let Some(item) = self.hotbar.selected_item() {
                let efficiency_level = item.enchantment_level(EnchantmentType::Efficiency);
                if efficiency_level > 0 {
                    let speed_bonus = 1.0 + 0.26 * efficiency_level as f32;
                    mining_time /= speed_bonus;
                    tracing::debug!(
                        "Efficiency {} applied: speed bonus {:.0}%",
                        efficiency_level,
                        (speed_bonus - 1.0) * 100.0
                    );
                }
            }

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
                let mut removed_extra: Option<IVec3> = None;
                let mut mined_block_state: Option<BlockState> = None;

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

                    mined_block_state = Some(chunk.voxel(local_x, local_y, local_z).state);

                    // Remove the block
                    chunk.set_voxel(local_x, local_y, local_z, Voxel::default());

                    if mdminecraft_world::CropType::is_crop(block_id) {
                        self.crop_growth.unregister_crop(CropPosition {
                            chunk: chunk_pos,
                            x: local_x as u8,
                            y: local_y as u8,
                            z: local_z as u8,
                        });
                    }

                    removed_extra = Self::try_remove_other_door_half(
                        chunk, local_x, local_y, local_z, block_id,
                    )
                    .map(|other_local_y| {
                        IVec3::new(hit.block_pos.x, other_local_y as i32, hit.block_pos.z)
                    });
                    spawn_particles_at = Some(glam::Vec3::new(
                        hit.block_pos.x as f32 + 0.5,
                        hit.block_pos.y as f32 + 0.5,
                        hit.block_pos.z as f32 + 0.5,
                    ));
                    mined = true;
                }

                if mined && removed_extra.is_none() && mdminecraft_world::is_bed(block_id) {
                    if let Some(state) = mined_block_state {
                        removed_extra = Self::try_remove_other_bed_half(
                            &mut self.chunks,
                            hit.block_pos,
                            block_id,
                            state,
                        );
                    }
                }

                if mined {
                    // Notify fluid sim to update neighbors.
                    self.fluid_sim.on_fluid_removed(
                        FluidPos::new(hit.block_pos.x, hit.block_pos.y, hit.block_pos.z),
                        &self.chunks,
                    );
                    if let Some(extra) = removed_extra {
                        self.fluid_sim.on_fluid_removed(
                            FluidPos::new(extra.x, extra.y, extra.z),
                            &self.chunks,
                        );
                    }

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
                    self.on_block_entity_removed(hit.block_pos, block_id);

                    let mut changed_positions = vec![hit.block_pos];
                    if let Some(extra) = removed_extra {
                        changed_positions.push(extra);
                    }

                    let removed_support = Self::remove_unsupported_blocks(
                        &mut self.chunks,
                        &self.block_properties,
                        changed_positions,
                    );

                    for (pos, removed_block_id) in &removed_support {
                        if !mdminecraft_world::CropType::is_crop(*removed_block_id) {
                            continue;
                        }

                        let removed_chunk = ChunkPos::new(
                            pos.x.div_euclid(CHUNK_SIZE_X as i32),
                            pos.z.div_euclid(CHUNK_SIZE_Z as i32),
                        );
                        self.crop_growth.unregister_crop(CropPosition {
                            chunk: removed_chunk,
                            x: pos.x.rem_euclid(CHUNK_SIZE_X as i32) as u8,
                            y: pos.y as u8,
                            z: pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as u8,
                        });
                    }

                    // Notify redstone sim for any neighbor-dependent updates.
                    self.schedule_redstone_updates_around(hit.block_pos);
                    if let Some(extra) = removed_extra {
                        self.schedule_redstone_updates_around(extra);
                    }
                    for (pos, _) in &removed_support {
                        self.schedule_redstone_updates_around(*pos);
                    }

                    let mut affected_chunks = std::collections::BTreeSet::new();
                    affected_chunks.insert(chunk_pos);
                    if let Some(extra) = removed_extra {
                        affected_chunks.insert(ChunkPos::new(
                            extra.x.div_euclid(CHUNK_SIZE_X as i32),
                            extra.z.div_euclid(CHUNK_SIZE_Z as i32),
                        ));
                    }
                    for (pos, removed_block_id) in &removed_support {
                        self.on_block_entity_removed(*pos, *removed_block_id);

                        affected_chunks.insert(ChunkPos::new(
                            pos.x.div_euclid(CHUNK_SIZE_X as i32),
                            pos.z.div_euclid(CHUNK_SIZE_Z as i32),
                        ));

                        self.fluid_sim
                            .on_fluid_removed(FluidPos::new(pos.x, pos.y, pos.z), &self.chunks);

                        let should_drop = if mdminecraft_world::is_door_upper(*removed_block_id) {
                            let lower_pos = IVec3::new(pos.x, pos.y - 1, pos.z);
                            !removed_support.iter().any(|(other_pos, other_id)| {
                                *other_pos == lower_pos
                                    && mdminecraft_world::is_door_lower(*other_id)
                            })
                        } else {
                            *removed_block_id != interactive_blocks::BED_HEAD
                        };

                        if should_drop {
                            if let Some((drop_type, count)) =
                                DroppedItemType::from_block(*removed_block_id)
                            {
                                let drop_x = pos.x as f64 + 0.5;
                                let drop_y = pos.y as f64 + 0.5;
                                let drop_z = pos.z as f64 + 0.5;
                                self.item_manager
                                    .spawn_item(drop_x, drop_y, drop_z, drop_type, count);
                            }
                        }
                    }

                    let mut mesh_refresh = std::collections::BTreeSet::new();
                    for dirty_chunk in affected_chunks {
                        self.recompute_chunk_lighting(dirty_chunk);
                        mesh_refresh.insert(dirty_chunk);
                        mesh_refresh.extend(Self::neighbor_chunk_positions(dirty_chunk));

                        let affected = mdminecraft_world::recompute_block_light_local(
                            &mut self.chunks,
                            &self.registry,
                            dirty_chunk,
                        );
                        mesh_refresh.extend(affected);
                    }

                    for chunk_pos in mesh_refresh {
                        if self.upload_chunk_mesh(chunk_pos) {
                            self.debug_hud.chunk_uploads_last_frame += 1;
                        }
                    }

                    // Spawn dropped item if harvested successfully
                    let tool = self.hotbar.selected_tool();
                    let can_harvest = self.block_properties.get(block_id).can_harvest(tool);
                    if can_harvest {
                        // Check for Silk Touch and Fortune enchantments on the tool
                        let (has_silk_touch, fortune_level) =
                            if let Some(stack) = &self.hotbar.slots[self.hotbar.selected] {
                                let silk_touch = stack.has_enchantment(EnchantmentType::SilkTouch);
                                let fortune = stack.enchantment_level(EnchantmentType::Fortune);
                                (silk_touch, fortune)
                            } else {
                                (false, 0)
                            };

                        let mut rng = {
                            let pos_seed = (hit.block_pos.x as u64)
                                ^ ((hit.block_pos.y as u64).rotate_left(21))
                                ^ ((hit.block_pos.z as u64).rotate_left(42));
                            let seed = self.world_seed
                                ^ self.sim_tick.0.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                ^ pos_seed.wrapping_mul(0xD6E8_FEB8_6659_FD93)
                                ^ 0xDBA0_11A5_115D_1EAF_u64;
                            StdRng::seed_from_u64(seed)
                        };
                        let random = (rng.gen::<u32>() as f64) / (u32::MAX as f64);

                        let is_leaf_block = matches!(
                            block_id,
                            mdminecraft_world::tree_blocks::LEAVES
                                | mdminecraft_world::tree_blocks::BIRCH_LEAVES
                                | mdminecraft_world::tree_blocks::PINE_LEAVES
                        );

                        // Determine what to drop based on enchantments.
                        let drop = if is_leaf_block {
                            if has_silk_touch {
                                DroppedItemType::silk_touch_drop(block_id)
                            } else {
                                DroppedItemType::from_leaves_random(block_id, random)
                            }
                        } else if has_silk_touch {
                            DroppedItemType::silk_touch_drop(block_id)
                        } else if fortune_level > 0 {
                            DroppedItemType::fortune_drop(block_id, fortune_level, random)
                        } else {
                            DroppedItemType::from_block(block_id)
                        };

                        if let Some((drop_type, count)) = drop {
                            let drop_x = hit.block_pos.x as f64 + 0.5;
                            let drop_y = hit.block_pos.y as f64 + 0.5;
                            let drop_z = hit.block_pos.z as f64 + 0.5;
                            self.item_manager
                                .spawn_item(drop_x, drop_y, drop_z, drop_type, count);
                            tracing::debug!(
                                "Dropped {:?} x{} at ({:.1}, {:.1}, {:.1}){}",
                                drop_type,
                                count,
                                drop_x,
                                drop_y,
                                drop_z,
                                if has_silk_touch {
                                    " (Silk Touch)"
                                } else if fortune_level > 0 {
                                    " (Fortune)"
                                } else {
                                    ""
                                }
                            );

                            // Vanilla-ish: breaking grass can drop seeds.
                            if block_id == mdminecraft_world::BLOCK_GRASS && !has_silk_touch {
                                // Keep a simple 1/8 chance; deterministic via the per-block RNG.
                                if random < 0.125 {
                                    self.item_manager.spawn_item(
                                        drop_x,
                                        drop_y,
                                        drop_z,
                                        DroppedItemType::WheatSeeds,
                                        1,
                                    );
                                }
                            }

                            // Vanilla-ish: mature wheat drops extra seeds in addition to wheat.
                            if block_id == mdminecraft_world::farming_blocks::WHEAT_7 {
                                let extra_seeds = ((random * 3.0).floor() as u32).min(2);
                                let seeds = 1 + extra_seeds;
                                self.item_manager.spawn_item(
                                    drop_x,
                                    drop_y,
                                    drop_z,
                                    DroppedItemType::WheatSeeds,
                                    seeds,
                                );
                            }

                            // Vanilla-ish: mature carrots/potatoes drop extra produce in addition to the base drop.
                            if block_id == mdminecraft_world::farming_blocks::CARROTS_3 {
                                let extra = ((random * 4.0).floor() as u32).min(3);
                                if extra > 0 {
                                    self.item_manager.spawn_item(
                                        drop_x,
                                        drop_y,
                                        drop_z,
                                        DroppedItemType::Carrot,
                                        extra,
                                    );
                                }
                            }
                            if block_id == mdminecraft_world::farming_blocks::POTATOES_3 {
                                let extra = ((random * 4.0).floor() as u32).min(3);
                                if extra > 0 {
                                    self.item_manager.spawn_item(
                                        drop_x,
                                        drop_y,
                                        drop_z,
                                        DroppedItemType::Potato,
                                        extra,
                                    );
                                }
                            }
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
            let Some(place_state) = Self::placement_state_for_block(
                block_id,
                self.renderer.camera().yaw,
                hit.face_normal,
                (hit.hit_pos.y - hit.block_pos.y as f32).clamp(0.0, 1.0),
            ) else {
                return;
            };

            let place_pos = IVec3::new(
                hit.block_pos.x + hit.face_normal.x,
                hit.block_pos.y + hit.face_normal.y,
                hit.block_pos.z + hit.face_normal.z,
            );

            if matches!(
                block_id,
                interactive_blocks::LADDER
                    | interactive_blocks::TORCH
                    | mdminecraft_world::redstone_blocks::REDSTONE_TORCH
                    | mdminecraft_world::redstone_blocks::LEVER
                    | mdminecraft_world::redstone_blocks::STONE_BUTTON
                    | mdminecraft_world::redstone_blocks::OAK_BUTTON
                    | mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE
                    | mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE
                    | mdminecraft_world::redstone_blocks::REDSTONE_WIRE
            ) {
                let support_chunk_pos = ChunkPos::new(
                    hit.block_pos.x.div_euclid(CHUNK_SIZE_X as i32),
                    hit.block_pos.z.div_euclid(CHUNK_SIZE_Z as i32),
                );
                let Some(chunk) = self.chunks.get(&support_chunk_pos) else {
                    return;
                };

                let local_x = hit.block_pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
                let local_y = hit.block_pos.y as usize;
                let local_z = hit.block_pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
                if local_y >= CHUNK_SIZE_Y {
                    return;
                }

                let support_voxel = chunk.voxel(local_x, local_y, local_z);
                if !self.block_properties.get(support_voxel.id).is_solid {
                    return;
                }
            }

            if block_id == mdminecraft_world::BLOCK_SUGAR_CANE {
                // Vanilla-ish: sugar cane must be placed on top of a valid support block.
                if hit.face_normal != IVec3::new(0, 1, 0) {
                    return;
                }

                let support_pos = hit.block_pos;
                let Some(support_id) = self.get_block_at(support_pos) else {
                    return;
                };

                // Sugar cane can always be stacked on sugar cane.
                if support_id != mdminecraft_world::BLOCK_SUGAR_CANE {
                    // Otherwise the base must be on dirt/grass/sand and adjacent to water.
                    if !matches!(
                        support_id,
                        mdminecraft_world::BLOCK_DIRT
                            | mdminecraft_world::BLOCK_GRASS
                            | mdminecraft_world::BLOCK_SAND
                    ) {
                        return;
                    }

                    let has_adjacent_water = [
                        IVec3::new(1, 0, 0),
                        IVec3::new(-1, 0, 0),
                        IVec3::new(0, 0, 1),
                        IVec3::new(0, 0, -1),
                    ]
                    .into_iter()
                    .any(|offset| {
                        matches!(
                            self.get_block_at(support_pos + offset),
                            Some(
                                mdminecraft_world::BLOCK_WATER
                                    | mdminecraft_world::BLOCK_WATER_FLOWING
                            )
                        )
                    });

                    if !has_adjacent_water {
                        return;
                    }
                }
            }

            if block_id == mdminecraft_world::BLOCK_BROWN_MUSHROOM {
                // Vanilla-ish: mushrooms are placed on the top face of a solid block.
                if hit.face_normal != IVec3::new(0, 1, 0) {
                    return;
                }

                let support_pos = hit.block_pos;
                let Some(support_id) = self.get_block_at(support_pos) else {
                    return;
                };

                if !self.block_properties.get(support_id).is_solid {
                    return;
                }
            }

            let chunk_x = place_pos.x.div_euclid(16);
            let chunk_z = place_pos.z.div_euclid(16);
            let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
            let mut spawn_particles_at: Option<glam::Vec3> = None;
            let mut placed = false;
            let mut placed_extra: Option<IVec3> = None;

            if block_id == interactive_blocks::BED_FOOT {
                if let Some(extra) = self.try_place_bed(place_pos, place_state) {
                    placed_extra = Some(extra);
                    spawn_particles_at = Some(glam::Vec3::new(
                        place_pos.x as f32 + 0.5,
                        place_pos.y as f32 + 0.5,
                        place_pos.z as f32 + 0.5,
                    ));
                    placed = true;
                }
            } else if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                let local_x = place_pos.x.rem_euclid(16) as usize;
                let local_y = place_pos.y as usize;
                let local_z = place_pos.z.rem_euclid(16) as usize;

                if local_y < 256 {
                    let current = chunk.voxel(local_x, local_y, local_z);
                    if current.id == BLOCK_AIR {
                        if mdminecraft_world::is_door_lower(block_id) {
                            if Self::try_place_door(
                                chunk,
                                local_x,
                                local_y,
                                local_z,
                                block_id,
                                place_state,
                            ) {
                                placed_extra =
                                    Some(IVec3::new(place_pos.x, place_pos.y + 1, place_pos.z));
                                spawn_particles_at = Some(glam::Vec3::new(
                                    place_pos.x as f32 + 0.5,
                                    place_pos.y as f32 + 0.5,
                                    place_pos.z as f32 + 0.5,
                                ));
                                placed = true;
                            }
                        } else {
                            let new_voxel = Voxel {
                                id: block_id,
                                state: place_state,
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

                            if block_id == mdminecraft_world::BLOCK_SUGAR_CANE {
                                // Register the base of the column so it can grow deterministically.
                                let mut base_y = local_y;
                                while base_y > 0
                                    && chunk.voxel(local_x, base_y - 1, local_z).id
                                        == mdminecraft_world::BLOCK_SUGAR_CANE
                                {
                                    base_y -= 1;
                                }

                                self.sugar_cane_growth.register_base(SugarCanePosition {
                                    chunk: chunk_pos,
                                    x: local_x as u8,
                                    y: base_y as u8,
                                    z: local_z as u8,
                                });
                            }
                        }
                    }
                }
            }

            if placed {
                // Notify fluid sim (treat as removal to wake neighbors who might be flowing here).
                self.fluid_sim.on_fluid_removed(
                    FluidPos::new(place_pos.x, place_pos.y, place_pos.z),
                    &self.chunks,
                );
                if let Some(extra) = placed_extra {
                    self.fluid_sim
                        .on_fluid_removed(FluidPos::new(extra.x, extra.y, extra.z), &self.chunks);
                }

                // Notify redstone sim for any neighbor-dependent updates.
                self.schedule_redstone_updates_around(place_pos);
                if let Some(extra) = placed_extra {
                    self.schedule_redstone_updates_around(extra);
                }

                // Decrease block count.
                if let Some(item) = self.hotbar.selected_item_mut() {
                    if item.count > 0 {
                        item.count -= 1;
                        if item.count == 0 {
                            self.hotbar.slots[self.hotbar.selected] = None;
                        }
                    }
                }
            }

            if placed {
                let mut dirty_chunks = std::collections::BTreeSet::new();
                dirty_chunks.insert(chunk_pos);
                if let Some(extra) = placed_extra {
                    dirty_chunks.insert(ChunkPos::new(
                        extra.x.div_euclid(16),
                        extra.z.div_euclid(16),
                    ));
                }

                // Update lighting (skylight + seams) for any affected chunks (including
                // cross-chunk multi-block placements like beds).
                for pos in &dirty_chunks {
                    self.recompute_chunk_lighting(*pos);
                }

                // Update blocklight and refresh affected meshes.
                let mut affected = std::collections::BTreeSet::new();
                for pos in &dirty_chunks {
                    affected.extend(mdminecraft_world::recompute_block_light_local(
                        &mut self.chunks,
                        &self.registry,
                        *pos,
                    ));
                }

                // Refresh the changed chunk + neighbors (geometry/connectivity) and any chunks
                // touched by lighting updates (includes diagonals).
                let mut mesh_refresh = std::collections::BTreeSet::new();
                for pos in &dirty_chunks {
                    mesh_refresh.insert(*pos);
                    mesh_refresh.extend(Self::neighbor_chunk_positions(*pos));
                }
                mesh_refresh.extend(affected);
                for pos in mesh_refresh {
                    if self.upload_chunk_mesh(pos) {
                        self.debug_hud.chunk_uploads_last_frame += 1;
                    }
                }
            }

            if let Some(center) = spawn_particles_at {
                self.spawn_block_break_particles(center, block_id);
            }
        }
    }

    fn placement_state_for_block(
        block_id: BlockId,
        camera_yaw: f32,
        face_normal: IVec3,
        hit_local_y: f32,
    ) -> Option<BlockState> {
        if matches!(
            block_id,
            interactive_blocks::TORCH | mdminecraft_world::redstone_blocks::REDSTONE_TORCH
        ) {
            if face_normal.y == 1 {
                return Some(0);
            }
            // Torches cannot be mounted on ceilings.
            if face_normal.y == -1 {
                return None;
            }

            let facing = match (face_normal.x, face_normal.z) {
                (1, 0) => mdminecraft_world::Facing::East,
                (-1, 0) => mdminecraft_world::Facing::West,
                (0, 1) => mdminecraft_world::Facing::South,
                (0, -1) => mdminecraft_world::Facing::North,
                _ => return None,
            };
            return Some(mdminecraft_world::torch_wall_state(facing));
        }

        if matches!(
            block_id,
            mdminecraft_world::redstone_blocks::LEVER
                | mdminecraft_world::redstone_blocks::STONE_BUTTON
                | mdminecraft_world::redstone_blocks::OAK_BUTTON
        ) {
            if face_normal.y == 1 {
                return Some(0);
            }
            if face_normal.y == -1 {
                return Some(mdminecraft_world::ceiling_mount_state());
            }

            let facing = match (face_normal.x, face_normal.z) {
                (1, 0) => mdminecraft_world::Facing::East,
                (-1, 0) => mdminecraft_world::Facing::West,
                (0, 1) => mdminecraft_world::Facing::South,
                (0, -1) => mdminecraft_world::Facing::North,
                _ => return None,
            };
            return Some(mdminecraft_world::wall_mount_state(facing));
        }

        if matches!(
            block_id,
            mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE
                | mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE
                | mdminecraft_world::redstone_blocks::REDSTONE_WIRE
        ) {
            if face_normal.y != 1 {
                return None;
            }
            return Some(0);
        }

        if mdminecraft_world::is_slab(block_id) {
            let top = if face_normal.y < 0 {
                true
            } else if face_normal.y > 0 {
                false
            } else {
                hit_local_y >= 0.5
            };

            let pos = if top {
                mdminecraft_world::SlabPosition::Top
            } else {
                mdminecraft_world::SlabPosition::Bottom
            };
            return Some(pos.to_state(0));
        }

        if mdminecraft_world::is_ladder(block_id) {
            let facing = match (face_normal.x, face_normal.z) {
                (-1, 0) => mdminecraft_world::Facing::East,
                (1, 0) => mdminecraft_world::Facing::West,
                (0, -1) => mdminecraft_world::Facing::South,
                (0, 1) => mdminecraft_world::Facing::North,
                _ => return None,
            };
            return Some(facing.to_state());
        }

        if mdminecraft_world::is_trapdoor(block_id) {
            let top = if face_normal.y < 0 {
                true
            } else if face_normal.y > 0 {
                false
            } else {
                hit_local_y >= 0.5
            };
            let mut state = mdminecraft_world::Facing::from_yaw(camera_yaw).to_state();
            state = mdminecraft_world::set_trapdoor_top(state, top);
            return Some(state);
        }

        if mdminecraft_world::is_stairs(block_id) {
            let top = if face_normal.y < 0 {
                true
            } else if face_normal.y > 0 {
                false
            } else {
                hit_local_y >= 0.5
            };
            let mut state = mdminecraft_world::Facing::from_yaw(camera_yaw).to_state();
            if top {
                state |= 0x04;
            } else {
                state &= !0x04;
            }
            return Some(state);
        }

        if block_id == interactive_blocks::BED_HEAD {
            // Beds should always be placed via their foot block, which spawns both halves.
            return None;
        }

        if block_id == interactive_blocks::BED_FOOT {
            return Some(mdminecraft_world::Facing::from_yaw(camera_yaw).to_state());
        }

        if mdminecraft_world::is_door(block_id) {
            // Only lower halves are placeable; doors must be placed on a top face.
            if !mdminecraft_world::is_door_lower(block_id) {
                return None;
            }
            if face_normal.y != 1 {
                return None;
            }
            return Some(mdminecraft_world::Facing::from_yaw(camera_yaw).to_state());
        }

        if mdminecraft_world::is_fence_gate(block_id) {
            return Some(mdminecraft_world::Facing::from_yaw(camera_yaw).to_state());
        }

        Some(0)
    }

    fn schedule_redstone_updates_around(&mut self, pos: IVec3) {
        let center = RedstonePos::new(pos.x, pos.y, pos.z);
        self.redstone_sim.schedule_update(center);
        for neighbor in center.neighbors() {
            self.redstone_sim.schedule_update(neighbor);
        }
    }

    fn update_player_pressure_plate(&mut self) {
        let mut new_plate = None;
        if self.player_state == PlayerState::Alive {
            let camera_pos = self.renderer.camera().position;
            let feet_pos = camera_pos - glam::Vec3::new(0.0, self.player_physics.eye_height, 0.0);
            let sample = feet_pos - glam::Vec3::new(0.0, 0.01, 0.0);
            let x = sample.x.floor() as i32;
            let y = sample.y.floor() as i32;
            let z = sample.z.floor() as i32;
            let pos = IVec3::new(x, y, z);
            if matches!(
                self.get_block_at(pos),
                Some(
                    mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE
                        | mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE
                )
            ) {
                new_plate = Some(RedstonePos::new(x, y, z));
            }
        }

        if new_plate == self.pressed_pressure_plate {
            return;
        }

        if let Some(old) = self.pressed_pressure_plate {
            self.redstone_sim
                .update_pressure_plate(old, false, &mut self.chunks);
        }
        if let Some(new) = new_plate {
            self.redstone_sim
                .update_pressure_plate(new, true, &mut self.chunks);
        }

        self.pressed_pressure_plate = new_plate;
    }

    fn door_upper_id(lower_id: BlockId) -> Option<BlockId> {
        match lower_id {
            mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER => {
                Some(mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER)
            }
            mdminecraft_world::interactive_blocks::IRON_DOOR_LOWER => {
                Some(mdminecraft_world::interactive_blocks::IRON_DOOR_UPPER)
            }
            _ => None,
        }
    }

    fn try_place_door(
        chunk: &mut Chunk,
        local_x: usize,
        local_y: usize,
        local_z: usize,
        lower_id: BlockId,
        state: BlockState,
    ) -> bool {
        let Some(upper_id) = Self::door_upper_id(lower_id) else {
            return false;
        };
        if local_y + 1 >= CHUNK_SIZE_Y {
            return false;
        }
        if chunk.voxel(local_x, local_y, local_z).id != BLOCK_AIR {
            return false;
        }
        if chunk.voxel(local_x, local_y + 1, local_z).id != BLOCK_AIR {
            return false;
        }

        chunk.set_voxel(
            local_x,
            local_y,
            local_z,
            Voxel {
                id: lower_id,
                state,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            local_x,
            local_y + 1,
            local_z,
            Voxel {
                id: upper_id,
                state,
                light_sky: 0,
                light_block: 0,
            },
        );
        true
    }

    fn bed_other_half_pos(
        pos: IVec3,
        bed_id: BlockId,
        state: BlockState,
    ) -> Option<(IVec3, BlockId)> {
        if !mdminecraft_world::is_bed(bed_id) {
            return None;
        }

        let facing = mdminecraft_world::Facing::from_state(state);
        let (dx, dz) = facing.offset();

        match bed_id {
            interactive_blocks::BED_FOOT => Some((
                IVec3::new(pos.x + dx, pos.y, pos.z + dz),
                interactive_blocks::BED_HEAD,
            )),
            interactive_blocks::BED_HEAD => Some((
                IVec3::new(pos.x - dx, pos.y, pos.z - dz),
                interactive_blocks::BED_FOOT,
            )),
            _ => None,
        }
    }

    fn try_place_bed(&mut self, foot_pos: IVec3, state: BlockState) -> Option<IVec3> {
        if foot_pos.y < 0 || foot_pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        let (head_pos, _) =
            Self::bed_other_half_pos(foot_pos, interactive_blocks::BED_FOOT, state)?;

        if head_pos.y < 0 || head_pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        // Require space for both halves.
        if self.get_block_at(foot_pos) != Some(BLOCK_AIR) {
            return None;
        }
        if self.get_block_at(head_pos) != Some(BLOCK_AIR) {
            return None;
        }

        // Require solid support below both halves.
        for pos in [foot_pos, head_pos] {
            let support_pos = IVec3::new(pos.x, pos.y - 1, pos.z);
            let support_id = self.get_block_at(support_pos)?;
            if !self.block_properties.get(support_id).is_solid {
                return None;
            }
        }

        let foot_chunk_pos = ChunkPos::new(
            foot_pos.x.div_euclid(CHUNK_SIZE_X as i32),
            foot_pos.z.div_euclid(CHUNK_SIZE_Z as i32),
        );
        let head_chunk_pos = ChunkPos::new(
            head_pos.x.div_euclid(CHUNK_SIZE_X as i32),
            head_pos.z.div_euclid(CHUNK_SIZE_Z as i32),
        );

        let foot_local_x = foot_pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let foot_local_y = foot_pos.y as usize;
        let foot_local_z = foot_pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

        let head_local_x = head_pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let head_local_y = head_pos.y as usize;
        let head_local_z = head_pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

        if foot_chunk_pos == head_chunk_pos {
            let chunk = self.chunks.get_mut(&foot_chunk_pos)?;
            chunk.set_voxel(
                foot_local_x,
                foot_local_y,
                foot_local_z,
                Voxel {
                    id: interactive_blocks::BED_FOOT,
                    state,
                    light_sky: 0,
                    light_block: 0,
                },
            );
            chunk.set_voxel(
                head_local_x,
                head_local_y,
                head_local_z,
                Voxel {
                    id: interactive_blocks::BED_HEAD,
                    state,
                    light_sky: 0,
                    light_block: 0,
                },
            );
        } else {
            {
                let foot_chunk = self.chunks.get_mut(&foot_chunk_pos)?;
                foot_chunk.set_voxel(
                    foot_local_x,
                    foot_local_y,
                    foot_local_z,
                    Voxel {
                        id: interactive_blocks::BED_FOOT,
                        state,
                        light_sky: 0,
                        light_block: 0,
                    },
                );
            }
            {
                let head_chunk = self.chunks.get_mut(&head_chunk_pos)?;
                head_chunk.set_voxel(
                    head_local_x,
                    head_local_y,
                    head_local_z,
                    Voxel {
                        id: interactive_blocks::BED_HEAD,
                        state,
                        light_sky: 0,
                        light_block: 0,
                    },
                );
            }
        }

        Some(head_pos)
    }

    fn try_remove_other_door_half(
        chunk: &mut Chunk,
        local_x: usize,
        local_y: usize,
        local_z: usize,
        door_id: BlockId,
    ) -> Option<usize> {
        if !mdminecraft_world::is_door(door_id) {
            return None;
        }

        let other_local_y = if mdminecraft_world::is_door_lower(door_id) {
            local_y.checked_add(1)?
        } else {
            local_y.checked_sub(1)?
        };
        if other_local_y >= CHUNK_SIZE_Y {
            return None;
        }

        let other_voxel = chunk.voxel(local_x, other_local_y, local_z);
        if mdminecraft_world::is_door(other_voxel.id) {
            chunk.set_voxel(local_x, other_local_y, local_z, Voxel::default());
            Some(other_local_y)
        } else {
            None
        }
    }

    fn try_remove_other_bed_half(
        chunks: &mut HashMap<ChunkPos, Chunk>,
        bed_pos: IVec3,
        bed_id: BlockId,
        bed_state: BlockState,
    ) -> Option<IVec3> {
        let (other_pos, expected_other_id) = Self::bed_other_half_pos(bed_pos, bed_id, bed_state)?;

        if other_pos.y < 0 || other_pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        let other_chunk_pos = ChunkPos::new(
            other_pos.x.div_euclid(CHUNK_SIZE_X as i32),
            other_pos.z.div_euclid(CHUNK_SIZE_Z as i32),
        );
        let chunk = chunks.get_mut(&other_chunk_pos)?;

        let local_x = other_pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_y = other_pos.y as usize;
        let local_z = other_pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

        if local_y >= CHUNK_SIZE_Y {
            return None;
        }

        let other_voxel = chunk.voxel(local_x, local_y, local_z);
        if other_voxel.id != expected_other_id {
            return None;
        }

        chunk.set_voxel(local_x, local_y, local_z, Voxel::default());
        Some(other_pos)
    }

    fn remove_unsupported_blocks(
        chunks: &mut HashMap<ChunkPos, Chunk>,
        block_properties: &BlockPropertiesRegistry,
        changed_positions: impl IntoIterator<Item = IVec3>,
    ) -> Vec<(IVec3, BlockId)> {
        let mut queue: std::collections::VecDeque<IVec3> = changed_positions.into_iter().collect();
        let mut removed = Vec::new();

        let voxel_at = |chunks: &HashMap<ChunkPos, Chunk>, pos: IVec3| -> Option<Voxel> {
            if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
                return None;
            }

            let chunk_pos = ChunkPos::new(
                pos.x.div_euclid(CHUNK_SIZE_X as i32),
                pos.z.div_euclid(CHUNK_SIZE_Z as i32),
            );
            let chunk = chunks.get(&chunk_pos)?;
            let local_x = pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
            let local_z = pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
            Some(chunk.voxel(local_x, pos.y as usize, local_z))
        };

        let is_solid_at = |chunks: &HashMap<ChunkPos, Chunk>, pos: IVec3| -> bool {
            voxel_at(chunks, pos).is_some_and(|voxel| block_properties.get(voxel.id).is_solid)
        };

        let neighbor_offsets = [
            IVec3::new(0, 1, 0),
            IVec3::new(0, -1, 0),
            IVec3::new(1, 0, 0),
            IVec3::new(-1, 0, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(0, 0, -1),
        ];

        while let Some(pos) = queue.pop_front() {
            for offset in neighbor_offsets {
                let candidate = pos + offset;
                let Some(voxel) = voxel_at(chunks, candidate) else {
                    continue;
                };
                if voxel.id == BLOCK_AIR {
                    continue;
                }

                let unsupported = match voxel.id {
                    interactive_blocks::LADDER => {
                        let facing = mdminecraft_world::Facing::from_state(voxel.state);
                        let (dx, dz) = facing.offset();
                        let support_pos =
                            IVec3::new(candidate.x + dx, candidate.y, candidate.z + dz);
                        !is_solid_at(chunks, support_pos)
                    }
                    id if mdminecraft_world::is_door(id) => {
                        if mdminecraft_world::is_door_lower(id) {
                            let support_pos = IVec3::new(candidate.x, candidate.y - 1, candidate.z);
                            if !is_solid_at(chunks, support_pos) {
                                true
                            } else {
                                let upper_pos =
                                    IVec3::new(candidate.x, candidate.y + 1, candidate.z);
                                let expected_upper = match id {
                                    mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER => {
                                        Some(mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER)
                                    }
                                    mdminecraft_world::interactive_blocks::IRON_DOOR_LOWER => {
                                        Some(mdminecraft_world::interactive_blocks::IRON_DOOR_UPPER)
                                    }
                                    _ => None,
                                };
                                match expected_upper {
                                    Some(expected_upper) => voxel_at(chunks, upper_pos)
                                        .is_none_or(|upper| upper.id != expected_upper),
                                    None => true,
                                }
                            }
                        } else {
                            let lower_pos = IVec3::new(candidate.x, candidate.y - 1, candidate.z);
                            let expected_lower = match id {
                                mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER => {
                                    Some(mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER)
                                }
                                mdminecraft_world::interactive_blocks::IRON_DOOR_UPPER => {
                                    Some(mdminecraft_world::interactive_blocks::IRON_DOOR_LOWER)
                                }
                                _ => None,
                            };
                            match expected_lower {
                                Some(expected_lower) => voxel_at(chunks, lower_pos)
                                    .is_none_or(|lower| lower.id != expected_lower),
                                None => true,
                            }
                        }
                    }
                    id if mdminecraft_world::is_bed(id) => {
                        let support_pos = IVec3::new(candidate.x, candidate.y - 1, candidate.z);
                        if !is_solid_at(chunks, support_pos) {
                            true
                        } else {
                            match Self::bed_other_half_pos(candidate, id, voxel.state) {
                                Some((other_pos, expected_other_id)) => voxel_at(chunks, other_pos)
                                    .is_none_or(|other| other.id != expected_other_id),
                                None => true,
                            }
                        }
                    }
                    id if mdminecraft_world::CropType::is_crop(id) => {
                        let below_pos = IVec3::new(candidate.x, candidate.y - 1, candidate.z);
                        voxel_at(chunks, below_pos)
                            .is_none_or(|below| !mdminecraft_world::is_farmland(below.id))
                    }
                    id if id == mdminecraft_world::BLOCK_SUGAR_CANE => {
                        let below_pos = IVec3::new(candidate.x, candidate.y - 1, candidate.z);
                        voxel_at(chunks, below_pos).is_none_or(|below| {
                            below.id != mdminecraft_world::BLOCK_SUGAR_CANE
                                && !matches!(
                                    below.id,
                                    mdminecraft_world::BLOCK_DIRT
                                        | mdminecraft_world::BLOCK_GRASS
                                        | mdminecraft_world::BLOCK_SAND
                                )
                        })
                    }
                    id if id == mdminecraft_world::BLOCK_BROWN_MUSHROOM => {
                        let below_pos = IVec3::new(candidate.x, candidate.y - 1, candidate.z);
                        !is_solid_at(chunks, below_pos)
                    }
                    interactive_blocks::TORCH
                    | mdminecraft_world::redstone_blocks::REDSTONE_TORCH => {
                        let support_pos = if mdminecraft_world::is_torch_wall(voxel.state) {
                            let facing = mdminecraft_world::torch_facing(voxel.state);
                            let (dx, dz) = facing.offset();
                            IVec3::new(candidate.x - dx, candidate.y, candidate.z - dz)
                        } else {
                            IVec3::new(candidate.x, candidate.y - 1, candidate.z)
                        };
                        !is_solid_at(chunks, support_pos)
                    }
                    mdminecraft_world::redstone_blocks::LEVER
                    | mdminecraft_world::redstone_blocks::STONE_BUTTON
                    | mdminecraft_world::redstone_blocks::OAK_BUTTON => {
                        let support_pos = if mdminecraft_world::is_wall_mounted(voxel.state) {
                            let facing = mdminecraft_world::wall_mounted_facing(voxel.state);
                            let (dx, dz) = facing.offset();
                            IVec3::new(candidate.x - dx, candidate.y, candidate.z - dz)
                        } else if mdminecraft_world::is_ceiling_mounted(voxel.state) {
                            IVec3::new(candidate.x, candidate.y + 1, candidate.z)
                        } else {
                            IVec3::new(candidate.x, candidate.y - 1, candidate.z)
                        };
                        !is_solid_at(chunks, support_pos)
                    }
                    mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE
                    | mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE
                    | mdminecraft_world::redstone_blocks::REDSTONE_WIRE => {
                        let support_pos = IVec3::new(candidate.x, candidate.y - 1, candidate.z);
                        !is_solid_at(chunks, support_pos)
                    }
                    _ => false,
                };

                if !unsupported {
                    continue;
                }

                let chunk_pos = ChunkPos::new(
                    candidate.x.div_euclid(CHUNK_SIZE_X as i32),
                    candidate.z.div_euclid(CHUNK_SIZE_Z as i32),
                );
                let Some(chunk) = chunks.get_mut(&chunk_pos) else {
                    continue;
                };
                let local_x = candidate.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
                let local_z = candidate.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
                let local_y = candidate.y as usize;
                if local_y >= CHUNK_SIZE_Y {
                    continue;
                }

                chunk.set_voxel(local_x, local_y, local_z, Voxel::default());
                removed.push((candidate, voxel.id));
                queue.push_back(candidate);
            }
        }

        removed
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
        let camera_pos = self.renderer.camera().position;
        self.audio
            .set_listener_position([camera_pos.x, camera_pos.y, camera_pos.z]);

        let mut close_inventory_requested = false;
        let mut close_crafting_requested = false;
        let mut close_furnace_requested = false;
        let mut close_enchanting_requested = false;
        let mut close_brewing_requested = false;
        let mut close_chest_requested = false;
        let mut enchanting_result: Option<EnchantingResult> = None;
        let mut spill_items: Vec<ItemStack> = Vec::new();
        let mut pause_action = PauseMenuAction::None;
        let initial_fov_degrees = self.renderer.camera().fov.to_degrees();
        let mut fov_degrees = initial_fov_degrees;
        let initial_render_distance = self.render_distance;
        let mut render_distance = initial_render_distance;
        let mut input_bindings_changed = false;
        let initial_controls = Arc::clone(&self.controls);

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
            let chest_open = self.chest_open;
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
                        render_hotbar(ctx, &self.hotbar, &self.registry);
                        render_xp_bar(ctx, &self.player_xp);
                        render_health_bar(ctx, &self.player_health);
                        render_hunger_bar(ctx, &self.player_health);
                        render_armor_bar(ctx, &self.player_armor);
                        render_tool_durability(ctx, &self.hotbar);

                        // Show inventory if open
                        if inventory_open {
                            let (close, items) = render_inventory(
                                ctx,
                                &mut self.hotbar,
                                &mut self.main_inventory,
                                &mut self.player_armor,
                                &mut self.personal_crafting_grid,
                                &mut self.ui_cursor_stack,
                                &mut self.ui_drag_state,
                            );
                            close_inventory_requested = close;
                            spill_items.extend(items);
                        }

                        // Show crafting if open
                        if crafting_open {
                            let (close, items) = render_crafting(
                                ctx,
                                &mut self.crafting_grid,
                                &mut self.hotbar,
                                &mut self.main_inventory,
                                &mut self.ui_cursor_stack,
                                &mut self.ui_drag_state,
                            );
                            close_crafting_requested = close;
                            spill_items.extend(items);
                        }

                        // Show furnace if open
                        if furnace_open {
                            if let Some(pos) = self.open_furnace_pos {
                                if let Some(furnace) = self.furnaces.get_mut(&pos) {
                                    close_furnace_requested = render_furnace(
                                        ctx,
                                        furnace,
                                        &mut self.hotbar,
                                        &mut self.main_inventory,
                                        &mut self.ui_cursor_stack,
                                        &mut self.ui_drag_state,
                                    );
                                }
                            }
                        }

                        // Show enchanting table if open
                        if enchanting_open {
                            if let Some(pos) = self.open_enchanting_pos {
                                if let Some(table) = self.enchanting_tables.get_mut(&pos) {
                                    let result = render_enchanting_table(
                                        ctx,
                                        table,
                                        &self.player_xp,
                                        &mut self.hotbar,
                                        &mut self.main_inventory,
                                        &mut self.ui_cursor_stack,
                                        &mut self.ui_drag_state,
                                    );
                                    close_enchanting_requested = result.close_requested;
                                    if result.enchantment_applied.is_some() {
                                        enchanting_result = Some(result);
                                    }
                                }
                            }
                        }

                        // Show brewing stand if open
                        if self.brewing_open {
                            if let Some(pos) = self.open_brewing_pos {
                                if let Some(stand) = self.brewing_stands.get_mut(&pos) {
                                    close_brewing_requested = render_brewing_stand(
                                        ctx,
                                        stand,
                                        &mut self.hotbar,
                                        &mut self.main_inventory,
                                        &mut self.ui_cursor_stack,
                                        &mut self.ui_drag_state,
                                    );
                                }
                            }
                        }

                        // Show chest if open
                        if chest_open {
                            if let Some(pos) = self.open_chest_pos {
                                if let Some(chest) = self.chests.get_mut(&pos) {
                                    close_chest_requested = render_chest(
                                        ctx,
                                        chest,
                                        &mut self.hotbar,
                                        &mut self.main_inventory,
                                        &mut self.ui_cursor_stack,
                                        &mut self.ui_drag_state,
                                    );
                                }
                            }
                        }

                        // Show pause menu if open (singleplayer pause).
                        if self.pause_menu_open && !is_dead {
                            pause_action = render_pause_menu(
                                ctx,
                                &mut self.pause_menu_view,
                                &mut self.controls,
                                &mut self.pause_controls_dirty,
                                &mut fov_degrees,
                                &mut render_distance,
                                &mut input_bindings_changed,
                            );
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

        // Handle overflow spills from UI actions (e.g., shift-crafting).
        for stack in spill_items {
            tracing::warn!(
                item = ?stack.item_type,
                count = stack.count,
                "Inventory full; spilling items"
            );
            self.spill_stack_to_world(stack);
        }

        // Handle furnace close
        if close_furnace_requested {
            self.close_furnace();
        }

        // Handle enchanting table close
        if close_enchanting_requested {
            self.close_enchanting_table();
        }

        // Handle brewing stand close
        if close_brewing_requested {
            self.close_brewing_stand();
        }

        // Handle chest close
        if close_chest_requested {
            self.close_chest();
        }

        match pause_action {
            PauseMenuAction::None => {}
            PauseMenuAction::Resume => {
                self.close_pause_menu();
            }
            PauseMenuAction::ReturnToMenu => {
                self.pending_action = Some(GameAction::ReturnToMenu);
            }
            PauseMenuAction::Quit => {
                self.pending_action = Some(GameAction::Quit);
            }
        }

        if (fov_degrees - initial_fov_degrees).abs() > f32::EPSILON {
            self.renderer.camera_mut().fov = fov_degrees.to_radians();
        }
        if render_distance != initial_render_distance {
            self.render_distance = render_distance;
        }
        if input_bindings_changed {
            self.input_processor = InputProcessor::new(&self.controls);
        }
        if !Arc::ptr_eq(&initial_controls, &self.controls) {
            self.audio
                .update_settings(Self::audio_settings_from_controls(&self.controls));
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

        self.audio.update();
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
        let block_properties = &self.block_properties;
        let get_ground_height = |x: f64, z: f64| -> f64 {
            Self::column_ground_height(chunks, block_properties, x as f32, z as f32) as f64
        };

        // Update item physics
        self.item_manager.update(get_ground_height);

        // Merge nearby items
        self.item_manager.merge_nearby_items();

        // Check for item pickup
        let picked_up = self.item_manager.pickup_items(player_x, player_y, player_z);
        let mut played_pickup_sound = false;

        // Add picked up items to player storage (hotbar → main inventory)
        for (drop_type, count) in picked_up {
            if let Some(core_item_type) = Self::convert_dropped_item_type(drop_type) {
                let stack = ItemStack::new(core_item_type, count);
                if let Some(remainder) = self.try_add_stack_to_storage(stack) {
                    let inserted = count.saturating_sub(remainder.count);
                    if inserted > 0 && !played_pickup_sound {
                        played_pickup_sound = true;
                        self.audio.play_sfx(SoundId::ItemPickup);
                    }
                    tracing::warn!(
                        item = ?remainder.item_type,
                        count = remainder.count,
                        "Inventory full; re-spawning picked up items"
                    );
                    // `pickup_items` already removed the drop from the world; re-spawn any remainder
                    // just outside the pickup radius to avoid immediate re-pickup loops.
                    self.item_manager.spawn_item(
                        player_x + 2.0,
                        player_y + 0.5,
                        player_z,
                        drop_type,
                        remainder.count,
                    );
                } else {
                    if !played_pickup_sound {
                        played_pickup_sound = true;
                        self.audio.play_sfx(SoundId::ItemPickup);
                    }
                    tracing::info!("Picked up {:?} x{}", drop_type, count);
                }
            }
        }
    }

    /// Update mob AI and movement
    fn update_mobs(&mut self, _dt: f32) {
        // Use frame count as tick for deterministic behavior
        // TODO: Use proper SimTick for multiplayer sync
        let tick = self.sim_tick.0;

        // Get player position for hostile mob targeting
        let player_pos = self.renderer.camera().position;
        let player_x = player_pos.x as f64;
        let player_y = player_pos.y as f64;
        let player_z = player_pos.z as f64;

        // Check if it's night time (hostile mobs spawn at night)
        // Time: 0.0-0.25 Night→Dawn, 0.75-1.0 Dusk→Night
        let time = self.sim_time.time_of_day() as f32;
        let is_night = !(0.25..=0.75).contains(&time);

        // Update each mob and track damage to player
        let mut total_damage = 0.0f32;
        let mut exploded_creeper = false;
        let mut explosion_positions: Vec<(f64, f64, f64, f32)> = Vec::new();
        for mob in &mut self.mobs {
            // Update fire damage (from Fire Aspect enchantment)
            mob.update_fire();

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
                    MobType::Villager => 0, // Villagers don't drop XP
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

                        // Vanilla-ish: zombies can drop carrots/potatoes (deterministic).
                        let pos_x = mob.x.floor() as i32;
                        let pos_z = mob.z.floor() as i32;
                        let roll = (tick as u32)
                            .wrapping_add((pos_x as u32).wrapping_mul(31))
                            .wrapping_add((pos_z as u32).wrapping_mul(131))
                            % 100;
                        if roll < 2 {
                            loot_drops.push((
                                mob.x,
                                mob.y + 0.5,
                                mob.z,
                                DroppedItemType::Carrot,
                                1,
                            ));
                        } else if roll < 4 {
                            loot_drops.push((
                                mob.x,
                                mob.y + 0.5,
                                mob.z,
                                DroppedItemType::Potato,
                                1,
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

                        // Vanilla-ish: spiders also drop 0-1 spider eyes (deterministic).
                        let pos_x = mob.x.floor() as i32;
                        let pos_z = mob.z.floor() as i32;
                        let roll = (tick as u32)
                            .wrapping_add((pos_x as u32).wrapping_mul(37))
                            .wrapping_add((pos_z as u32).wrapping_mul(101))
                            % 3;
                        if roll == 0 {
                            loot_drops.push((
                                mob.x,
                                mob.y + 0.5,
                                mob.z,
                                DroppedItemType::SpiderEye,
                                1,
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
                    MobType::Villager => {
                        // Villagers don't drop items when killed
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
        for (spawn_index, (x, y, z, xp_value)) in xp_orb_spawns.into_iter().enumerate() {
            let pos = glam::Vec3::new(x as f32, y as f32, z as f32);
            let seed = self.world_seed
                ^ self.sim_tick.0.wrapping_mul(0xA24B_AED4_963E_E407)
                ^ (spawn_index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (xp_value as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                ^ 0x5850_5F4F_5242_u64;
            self.xp_orbs.push(XPOrb::new(pos, xp_value, seed));
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
                    // Hit a block
                    if projectile.projectile_type.is_splash_potion() {
                        // Splash potion breaks and marks for AoE effect
                        projectile.hit();
                    } else {
                        // Arrow sticks
                        projectile.stick();
                    }
                }
            }
        }

        // Check for mob collisions (arrows deal damage, splash potions trigger on hit)
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
                    if projectile.projectile_type.is_splash_potion() {
                        // Splash potion breaks on mob hit (AoE handled below)
                        projectile.hit();
                        tracing::debug!("Splash potion hit {:?}", mob.mob_type);
                    } else {
                        // Arrow deals damage
                        let damage = projectile.damage();
                        mob.damage(damage);
                        projectile.hit();

                        // Apply knockback from arrow direction
                        let knock_dir_x = projectile.vel_x;
                        let knock_dir_z = projectile.vel_z;
                        let knock_len =
                            (knock_dir_x * knock_dir_x + knock_dir_z * knock_dir_z).sqrt();
                        if knock_len > 0.001 {
                            mob.apply_knockback(
                                knock_dir_x / knock_len,
                                knock_dir_z / knock_len,
                                0.3,
                            );
                        }

                        tracing::debug!("Arrow hit {:?} for {:.1} damage", mob.mob_type, damage);
                    }
                    break; // Only hit one mob per projectile
                }
            }
        }

        // Handle splash potion AoE effects for projectiles that just broke
        // Collect splash potion impact data before applying (to avoid borrow conflicts)
        let mut splash_impacts: Vec<(f64, f64, f64, u16)> = Vec::new();
        for projectile in &self.projectiles.projectiles {
            if projectile.dead && projectile.hit_entity {
                if let Some(potion_id) = projectile.projectile_type.potion_id() {
                    splash_impacts.push((projectile.x, projectile.y, projectile.z, potion_id));
                }
            }
        }

        // Apply splash potion AoE effects
        for (splash_x, splash_y, splash_z, potion_id) in splash_impacts {
            let effect_radius = 4.0; // 4 block radius for splash effect

            // Apply effect to player if in range
            let player_pos = self.renderer.camera().position;
            let player_dist = ((player_pos.x as f64 - splash_x).powi(2)
                + (player_pos.y as f64 - splash_y).powi(2)
                + (player_pos.z as f64 - splash_z).powi(2))
            .sqrt();

            if player_dist < effect_radius {
                // Apply potion effect to player (reduced by distance)
                let effectiveness = 1.0 - (player_dist / effect_radius);
                self.apply_splash_potion_to_player(potion_id, effectiveness);
            }

            // Apply effect to mobs in range
            for mob in &mut self.mobs {
                if mob.dead {
                    continue;
                }

                let mob_dist = ((mob.x - splash_x).powi(2)
                    + (mob.y - splash_y).powi(2)
                    + (mob.z - splash_z).powi(2))
                .sqrt();

                if mob_dist < effect_radius {
                    let effectiveness = 1.0 - (mob_dist / effect_radius);
                    // Apply splash potion effect to mob (inlined to avoid borrow issues)
                    match potion_id {
                        id if id == potion_ids::HEALING => {
                            // Healing heals living mobs - add health directly
                            let heal_amount = (4.0 * effectiveness) as f32;
                            mob.health = (mob.health + heal_amount).min(mob.mob_type.max_health());
                            tracing::debug!(
                                "Splash healing on {:?}: +{:.1} HP",
                                mob.mob_type,
                                heal_amount
                            );
                        }
                        id if id == potion_ids::HARMING => {
                            // Harming damages living mobs
                            let damage = (6.0 * effectiveness) as f32;
                            mob.damage(damage);
                            tracing::debug!(
                                "Splash harming on {:?}: -{:.1} HP",
                                mob.mob_type,
                                damage
                            );
                        }
                        id if id == potion_ids::POISON => {
                            // Poison does damage over time - simplified to instant damage
                            let damage = (2.0 * effectiveness) as f32;
                            mob.damage(damage);
                            tracing::debug!(
                                "Splash poison on {:?}: -{:.1} HP",
                                mob.mob_type,
                                damage
                            );
                        }
                        _ => {
                            // Other effects don't affect mobs in this implementation
                        }
                    }
                }
            }

            tracing::info!(
                "Splash potion (ID: {}) exploded at ({:.1}, {:.1}, {:.1})",
                potion_id,
                splash_x,
                splash_y,
                splash_z
            );
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
        let mut affected_chunks = std::collections::BTreeSet::new();
        let mut removed_blocks: Vec<(IVec3, BlockId)> = Vec::new();

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
                            let removed_id = voxel.id;
                            chunk.set_voxel(local_x, local_y, local_z, Voxel::default());
                            affected_chunks.insert(chunk_pos);
                            removed_blocks
                                .push((IVec3::new(block_x, block_y, block_z), removed_id));
                        }
                    }
                }
            }
        }

        for (pos, removed_id) in &removed_blocks {
            self.on_block_entity_removed(*pos, *removed_id);
            self.fluid_sim
                .on_fluid_removed(FluidPos::new(pos.x, pos.y, pos.z), &self.chunks);
            self.schedule_redstone_updates_around(*pos);
        }

        let removed_support = Self::remove_unsupported_blocks(
            &mut self.chunks,
            &self.block_properties,
            removed_blocks.iter().map(|(pos, _)| *pos),
        );
        for (pos, removed_block_id) in removed_support {
            self.on_block_entity_removed(pos, removed_block_id);
            affected_chunks.insert(ChunkPos::new(
                pos.x.div_euclid(CHUNK_SIZE_X as i32),
                pos.z.div_euclid(CHUNK_SIZE_Z as i32),
            ));
            self.fluid_sim
                .on_fluid_removed(FluidPos::new(pos.x, pos.y, pos.z), &self.chunks);
            self.schedule_redstone_updates_around(pos);
        }

        // Update lighting and meshes for affected chunks
        let mut mesh_refresh = std::collections::BTreeSet::new();
        for chunk_pos in affected_chunks {
            self.recompute_chunk_lighting(chunk_pos);
            mesh_refresh.insert(chunk_pos);
            for neighbor in Self::neighbor_chunk_positions(chunk_pos) {
                mesh_refresh.insert(neighbor);
            }

            let affected = mdminecraft_world::recompute_block_light_local(
                &mut self.chunks,
                &self.registry,
                chunk_pos,
            );
            mesh_refresh.extend(affected);
        }
        for chunk_pos in mesh_refresh {
            let _ = self.upload_chunk_mesh(chunk_pos);
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

            // Get enchantment levels from selected item
            let (sharpness_level, knockback_level, fire_aspect_level) =
                if let Some(item) = self.hotbar.selected_item() {
                    (
                        item.enchantment_level(EnchantmentType::Sharpness),
                        item.enchantment_level(EnchantmentType::Knockback),
                        item.enchantment_level(EnchantmentType::FireAspect),
                    )
                } else {
                    (0, 0, 0)
                };

            // Apply Sharpness enchantment bonus
            // Sharpness I: +1 damage, each additional level: +0.5 damage
            // (Minecraft Java: 0.5 + 0.5 * level extra damage)
            if sharpness_level > 0 {
                let bonus = 0.5 + 0.5 * sharpness_level as f32;
                damage += bonus;
                tracing::debug!("Sharpness {} adds {:.1} damage", sharpness_level, bonus);
            }

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

            // Calculate knockback strength with Knockback enchantment bonus
            // Base knockback: 0.5, each level adds 0.4
            let knockback_strength = 0.5 + 0.4 * knockback_level as f64;

            // Apply damage and knockback
            let mob = &mut self.mobs[idx];
            let _died = mob.damage(damage);
            mob.apply_knockback(dx, dz, knockback_strength);

            // Apply Fire Aspect: set target on fire
            // Fire Aspect I: 4 seconds (80 ticks), Fire Aspect II: 8 seconds (160 ticks)
            if fire_aspect_level > 0 {
                let fire_ticks = 80 * fire_aspect_level as u32;
                mob.set_on_fire(fire_ticks);
                tracing::debug!(
                    "Fire Aspect {} sets mob on fire for {} ticks",
                    fire_aspect_level,
                    fire_ticks
                );
            }

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

            // Sword sweep attack: if using a sword, nearby mobs take 1 + enchantment bonus damage
            // Sweep attacks hit mobs within 1 block of the primary target horizontally
            let is_sword = matches!(tool, Some((ToolType::Sword, _)));
            if is_sword {
                let target_x = mob.x;
                let target_z = mob.z;
                let sweep_damage = 1.0
                    + if sharpness_level > 0 {
                        0.5 + 0.5 * sharpness_level as f32
                    } else {
                        0.0
                    };
                let sweep_range = 1.0; // 1 block radius for sweep

                // Collect indices of mobs to sweep (excluding the primary target)
                let sweep_targets: Vec<usize> = self
                    .mobs
                    .iter()
                    .enumerate()
                    .filter_map(|(i, m)| {
                        if i == idx {
                            return None; // Skip primary target
                        }
                        let dx = m.x - target_x;
                        let dz = m.z - target_z;
                        let dist_sq = dx * dx + dz * dz;
                        if dist_sq <= sweep_range * sweep_range {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect();

                // Apply sweep damage to nearby mobs
                let sweep_count = sweep_targets.len();
                for sweep_idx in sweep_targets {
                    let sweep_mob = &mut self.mobs[sweep_idx];
                    sweep_mob.damage(sweep_damage);
                    // Small knockback for sweep targets
                    let sdx = sweep_mob.x - origin.x as f64;
                    let sdz = sweep_mob.z - origin.z as f64;
                    sweep_mob.apply_knockback(sdx, sdz, 0.3);

                    // Fire Aspect also applies to sweep targets
                    if fire_aspect_level > 0 {
                        let fire_ticks = 80 * fire_aspect_level as u32;
                        sweep_mob.set_on_fire(fire_ticks);
                    }
                }

                if sweep_count > 0 {
                    tracing::info!(
                        "SWEEP ATTACK! Hit {} additional mobs for {:.1} damage each",
                        sweep_count,
                        sweep_damage
                    );
                }
            }

            // Use tool durability if we have a tool
            // (damage_durability handles Unbreaking enchantment internally)
            if let Some(item) = self.hotbar.selected_item_mut() {
                if matches!(item.item_type, ItemType::Tool(_, _)) {
                    item.damage_durability(1);
                    if item.is_broken() {
                        // Tool broke
                        self.hotbar.slots[self.hotbar.selected] = None;
                        tracing::info!("Tool broke!");
                    }
                }
            }

            return true;
        }

        false
    }

    fn try_add_stack_to_storage(&mut self, stack: ItemStack) -> Option<ItemStack> {
        let remainder = self.hotbar.add_stack(stack)?;
        self.main_inventory.add_stack(remainder)
    }

    fn return_stack_to_storage_or_spill(&mut self, stack: ItemStack) {
        if let Some(remainder) = self.try_add_stack_to_storage(stack) {
            self.spill_stack_to_world(remainder);
        }
    }

    fn spill_stack_to_world(&mut self, stack: ItemStack) {
        let Some(dropped_type) = Self::convert_core_item_type_to_dropped(stack.item_type) else {
            tracing::warn!(
                item = ?stack.item_type,
                count = stack.count,
                "No dropped-item mapping; keeping in UI cursor"
            );
            let _ = try_add_stack_to_cursor(&mut self.ui_cursor_stack, stack);
            return;
        };

        let camera_pos = self.renderer.camera().position;
        let x = camera_pos.x as f64;
        let y = (camera_pos.y - self.player_physics.eye_height) as f64 + 0.5;
        let z = camera_pos.z as f64;

        self.item_manager
            .spawn_item(x, y, z, dropped_type, stack.count);
    }

    fn drop_selected_hotbar_item(&mut self, drop_stack: bool) {
        if self.player_state != PlayerState::Alive {
            return;
        }

        let Some(item) = self.hotbar.selected_item().cloned() else {
            return;
        };

        let Some(dropped_type) = Self::convert_core_item_type_to_dropped(item.item_type) else {
            tracing::warn!(
                item = ?item.item_type,
                "No dropped-item mapping; cannot drop from hotbar"
            );
            return;
        };

        let count_to_drop = if drop_stack || item.max_stack_size() == 1 {
            item.count
        } else {
            1.min(item.count)
        };
        if count_to_drop == 0 {
            return;
        }

        // Remove from hotbar.
        if drop_stack || item.count <= 1 || item.max_stack_size() == 1 {
            self.hotbar.slots[self.hotbar.selected] = None;
        } else if let Some(slot) = self.hotbar.selected_item_mut() {
            slot.count = slot.count.saturating_sub(count_to_drop);
            if slot.count == 0 {
                self.hotbar.slots[self.hotbar.selected] = None;
            }
        }

        let camera = self.renderer.camera();
        let (forward, _) = Self::flat_directions(camera);

        let base_x = camera.position.x as f64;
        let base_y = (camera.position.y - self.player_physics.eye_height) as f64 + 0.5;
        let base_z = camera.position.z as f64;

        // Spawn just outside pickup radius, in front of the player (vanilla-ish).
        let mut x = base_x + forward.x as f64 * 2.0;
        let y = base_y;
        let mut z = base_z + forward.z as f64 * 2.0;
        if forward.length_squared() <= f32::EPSILON {
            x = base_x + 2.0;
            z = base_z;
        }

        // Split into multiple dropped stacks if needed.
        let max = dropped_type.max_stack_size().max(1);
        let mut remaining = count_to_drop;
        while remaining > 0 {
            let batch = remaining.min(max);
            remaining -= batch;
            self.item_manager.spawn_item(x, y, z, dropped_type, batch);
        }
    }

    /// Toggle inventory UI open/closed
    fn toggle_inventory(&mut self) {
        self.inventory_open = !self.inventory_open;
        self.crafting_open = false; // Close crafting when toggling inventory
        self.ui_drag_state.reset();
        if self.inventory_open {
            // Release cursor when inventory is open
            let _ = self.input.enter_ui_overlay(&self.window);
            self.audio.play_sfx(SoundId::InventoryOpen);
            tracing::info!("Inventory opened");
        } else {
            if let Some(stack) = self.ui_cursor_stack.take() {
                self.return_stack_to_storage_or_spill(stack);
            }

            // Capture cursor when inventory is closed
            let _ = self.input.enter_gameplay(&self.window);
            self.audio.play_sfx(SoundId::InventoryClose);
            tracing::info!("Inventory closed");
        }
    }

    /// Open crafting table UI
    fn open_crafting(&mut self) {
        self.crafting_open = true;
        self.inventory_open = false; // Close inventory when opening crafting
        self.ui_drag_state.reset();
        // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        self.audio.play_sfx(SoundId::InventoryOpen);
        tracing::info!("Crafting table opened");
    }

    /// Close crafting UI
    fn close_crafting(&mut self) {
        self.crafting_open = false;
        self.ui_drag_state.reset();
        let mut returned: Vec<ItemStack> = Vec::new();
        if let Some(stack) = self.ui_cursor_stack.take() {
            returned.push(stack);
        }

        // Return any items still in the crafting grid.
        for row in &mut self.crafting_grid {
            for slot in row.iter_mut() {
                if let Some(stack) = slot.take() {
                    returned.push(stack);
                }
            }
        }

        for stack in returned {
            self.return_stack_to_storage_or_spill(stack);
        }
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        self.audio.play_sfx(SoundId::InventoryClose);
        tracing::info!("Crafting closed");
    }

    /// Open furnace UI at the given position
    fn open_furnace(&mut self, block_pos: IVec3) {
        let key = Self::overworld_block_entity_key(block_pos);
        self.furnace_open = true;
        self.open_furnace_pos = Some(key);
        self.inventory_open = false;
        self.crafting_open = false;
        self.ui_drag_state.reset();
        // Create furnace state if it doesn't exist
        self.furnaces.entry(key).or_default();
        // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        self.audio.play_sfx(SoundId::InventoryOpen);
        tracing::info!("Furnace opened at {:?}", block_pos);
    }

    /// Close furnace UI
    fn close_furnace(&mut self) {
        self.furnace_open = false;
        self.open_furnace_pos = None;
        self.ui_drag_state.reset();
        if let Some(stack) = self.ui_cursor_stack.take() {
            self.return_stack_to_storage_or_spill(stack);
        }
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        self.audio.play_sfx(SoundId::InventoryClose);
        tracing::info!("Furnace closed");
    }

    /// Open enchanting table UI at the given position
    fn open_enchanting_table(&mut self, block_pos: IVec3) {
        let key = Self::overworld_block_entity_key(block_pos);
        self.enchanting_open = true;
        self.open_enchanting_pos = Some(key);
        self.inventory_open = false;
        self.crafting_open = false;
        self.furnace_open = false;
        self.ui_drag_state.reset();
        // Count nearby bookshelves first (before borrowing enchanting_tables)
        let bookshelf_count = self.count_nearby_bookshelves(block_pos);
        // Create enchanting table state if it doesn't exist and update bookshelf count
        let table = self.enchanting_tables.entry(key).or_default();
        table.set_bookshelf_count(bookshelf_count);
        // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        self.audio.play_sfx(SoundId::InventoryOpen);
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
        self.ui_drag_state.reset();
        if let Some(stack) = self.ui_cursor_stack.take() {
            self.return_stack_to_storage_or_spill(stack);
        }
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        self.audio.play_sfx(SoundId::InventoryClose);
        tracing::info!("Enchanting table closed");
    }

    /// Open brewing stand UI at the given position
    fn open_brewing_stand(&mut self, block_pos: IVec3) {
        let key = Self::overworld_block_entity_key(block_pos);
        self.brewing_open = true;
        self.open_brewing_pos = Some(key);
        self.inventory_open = false;
        self.crafting_open = false;
        self.furnace_open = false;
        self.enchanting_open = false;
        self.ui_drag_state.reset();
        // Create brewing stand state if it doesn't exist
        self.brewing_stands.entry(key).or_default();
        // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        self.audio.play_sfx(SoundId::InventoryOpen);
        tracing::info!("Brewing stand opened at {:?}", block_pos);
    }

    /// Close brewing stand UI
    fn close_brewing_stand(&mut self) {
        self.brewing_open = false;
        self.open_brewing_pos = None;
        self.ui_drag_state.reset();
        if let Some(stack) = self.ui_cursor_stack.take() {
            self.return_stack_to_storage_or_spill(stack);
        }
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        self.audio.play_sfx(SoundId::InventoryClose);
        tracing::info!("Brewing stand closed");
    }

    /// Open chest UI at the given position
    fn open_chest(&mut self, block_pos: IVec3) {
        let key = Self::overworld_block_entity_key(block_pos);
        self.chest_open = true;
        self.open_chest_pos = Some(key);
        self.inventory_open = false;
        self.crafting_open = false;
        self.furnace_open = false;
        self.enchanting_open = false;
        self.brewing_open = false;
        self.ui_drag_state.reset();
        // Create chest state if it doesn't exist
        self.chests.entry(key).or_default();
        // Release cursor for UI interaction
        let _ = self.input.enter_ui_overlay(&self.window);
        self.audio.play_sfx(SoundId::InventoryOpen);
        tracing::info!("Chest opened at {:?}", block_pos);
    }

    /// Close chest UI
    fn close_chest(&mut self) {
        self.chest_open = false;
        self.open_chest_pos = None;
        self.ui_drag_state.reset();
        if let Some(stack) = self.ui_cursor_stack.take() {
            self.return_stack_to_storage_or_spill(stack);
        }
        // Capture cursor for gameplay
        let _ = self.input.enter_gameplay(&self.window);
        self.audio.play_sfx(SoundId::InventoryClose);
        tracing::info!("Chest closed");
    }

    fn on_block_entity_removed(&mut self, block_pos: IVec3, removed_block_id: BlockId) {
        let key = Self::overworld_block_entity_key(block_pos);

        let drop_pos = (
            block_pos.x as f64 + 0.5,
            block_pos.y as f64 + 0.5,
            block_pos.z as f64 + 0.5,
        );

        match removed_block_id {
            interactive_blocks::CHEST => {
                if self.chest_open && self.open_chest_pos == Some(key) {
                    self.close_chest();
                }

                let Some(chest) = self.chests.remove(&key) else {
                    return;
                };

                for (slot_idx, stack) in chest.slots.into_iter().enumerate() {
                    let Some(stack) = stack else {
                        continue;
                    };

                    let Some(drop_type) = Self::convert_core_item_type_to_dropped(stack.item_type)
                    else {
                        tracing::warn!(
                            slot = slot_idx,
                            item = ?stack.item_type,
                            "Chest contained an undroppable item type"
                        );
                        continue;
                    };

                    self.item_manager.spawn_item(
                        drop_pos.0,
                        drop_pos.1,
                        drop_pos.2,
                        drop_type,
                        stack.count,
                    );
                }
            }
            BLOCK_FURNACE | BLOCK_FURNACE_LIT => {
                if self.furnace_open && self.open_furnace_pos == Some(key) {
                    self.close_furnace();
                }

                let Some(furnace) = self.furnaces.remove(&key) else {
                    return;
                };

                for (drop_type, count) in [furnace.input, furnace.fuel, furnace.output]
                    .into_iter()
                    .flatten()
                {
                    self.item_manager
                        .spawn_item(drop_pos.0, drop_pos.1, drop_pos.2, drop_type, count);
                }
            }
            BLOCK_BREWING_STAND => {
                if self.brewing_open && self.open_brewing_pos == Some(key) {
                    self.close_brewing_stand();
                }

                let Some(stand) = self.brewing_stands.remove(&key) else {
                    return;
                };

                let BrewingStandState {
                    bottles,
                    bottle_is_splash,
                    ingredient,
                    fuel,
                    ..
                } = stand;

                if fuel > 0 {
                    self.item_manager.spawn_item(
                        drop_pos.0,
                        drop_pos.1,
                        drop_pos.2,
                        DroppedItemType::BlazePowder,
                        fuel,
                    );
                }

                if let Some((ingredient_id, count)) = ingredient {
                    if let Some(core_item) = brew_ingredient_id_to_core_item_type(ingredient_id) {
                        if let Some(drop_type) = Self::convert_core_item_type_to_dropped(core_item)
                        {
                            self.item_manager
                                .spawn_item(drop_pos.0, drop_pos.1, drop_pos.2, drop_type, count);
                        }
                    }
                }

                for (idx, bottle) in bottles.into_iter().enumerate() {
                    let Some(bottle) = bottle else {
                        continue;
                    };
                    let core_stack = bottle_to_core_item_stack(bottle, bottle_is_splash[idx]);
                    if let Some(drop_type) =
                        Self::convert_core_item_type_to_dropped(core_stack.item_type)
                    {
                        self.item_manager
                            .spawn_item(drop_pos.0, drop_pos.1, drop_pos.2, drop_type, 1);
                    }
                }
            }
            BLOCK_ENCHANTING_TABLE => {
                if self.enchanting_open && self.open_enchanting_pos == Some(key) {
                    self.close_enchanting_table();
                }

                let Some(table) = self.enchanting_tables.remove(&key) else {
                    return;
                };

                if table.lapis_count > 0 {
                    self.item_manager.spawn_item(
                        drop_pos.0,
                        drop_pos.1,
                        drop_pos.2,
                        DroppedItemType::LapisLazuli,
                        table.lapis_count,
                    );
                }
            }
            _ => {}
        }
    }

    fn open_pause_menu(&mut self) {
        self.pause_menu_open = true;
        self.pause_menu_view = PauseMenuView::Main;
        self.ui_drag_state.reset();
        let _ = self.input.enter_menu(&self.window);
        tracing::info!("Pause menu opened");
    }

    fn close_pause_menu(&mut self) {
        self.pause_menu_open = false;
        self.pause_menu_view = PauseMenuView::Main;
        self.ui_drag_state.reset();
        let _ = self.input.enter_gameplay(&self.window);
        tracing::info!("Pause menu closed");
    }

    fn handle_escape_pressed(&mut self) {
        if self.player_state != PlayerState::Alive {
            return;
        }

        if self.chest_open {
            self.close_chest();
            return;
        }
        if self.brewing_open {
            self.close_brewing_stand();
            return;
        }
        if self.enchanting_open {
            self.close_enchanting_table();
            return;
        }
        if self.furnace_open {
            self.close_furnace();
            return;
        }
        if self.crafting_open {
            self.close_crafting();
            return;
        }
        if self.inventory_open {
            self.toggle_inventory();
            return;
        }

        if self.pause_menu_open {
            if self.pause_menu_view == PauseMenuView::Options {
                self.pause_menu_view = PauseMenuView::Main;
            } else {
                self.close_pause_menu();
            }
            return;
        }

        self.open_pause_menu();
    }

    /// Drink a potion and apply its status effect
    /// Returns true if the potion was successfully drunk
    fn drink_potion(&mut self, potion_id: u16) -> bool {
        // Convert potion ID to PotionType
        let potion_type = match potion_id {
            potion_ids::AWKWARD => PotionType::Awkward,
            potion_ids::NIGHT_VISION => PotionType::NightVision,
            potion_ids::INVISIBILITY => PotionType::Invisibility,
            potion_ids::LEAPING => PotionType::Leaping,
            potion_ids::FIRE_RESISTANCE => PotionType::FireResistance,
            potion_ids::SWIFTNESS => PotionType::Swiftness,
            potion_ids::SLOWNESS => PotionType::Slowness,
            potion_ids::WATER_BREATHING => PotionType::WaterBreathing,
            potion_ids::HEALING => PotionType::Healing,
            potion_ids::HARMING => PotionType::Harming,
            potion_ids::POISON => PotionType::Poison,
            potion_ids::REGENERATION => PotionType::Regeneration,
            potion_ids::STRENGTH => PotionType::Strength,
            potion_ids::WEAKNESS => PotionType::Weakness,
            _ => {
                tracing::warn!("Unknown potion ID: {}", potion_id);
                return false;
            }
        };

        // Get the status effect from the potion type
        if let Some(effect_type) = potion_type.effect() {
            let duration = potion_type.base_duration_ticks();
            let amplifier = 0; // Level I

            // Apply the effect
            let effect = StatusEffect::new(effect_type, amplifier, duration);
            self.status_effects.add(effect);
            tracing::info!(
                "Drank {:?} potion - applied {:?} for {} ticks",
                potion_type,
                effect_type,
                duration
            );
            true
        } else {
            // Awkward, Mundane, Thick potions have no effect
            tracing::info!("Drank {:?} potion (no effect)", potion_type);
            true
        }
    }

    /// Throw a splash potion
    fn throw_splash_potion(&mut self, potion_id: u16) {
        use mdminecraft_world::Projectile;

        // Get player position and look direction from camera
        let camera = self.renderer.camera();
        let player_pos = camera.position;
        let yaw = camera.yaw;
        let pitch = camera.pitch;

        // Create the splash potion projectile
        let projectile = Projectile::throw_splash_potion(
            player_pos.x as f64,
            player_pos.y as f64,
            player_pos.z as f64,
            yaw,
            pitch,
            potion_id,
        );

        // Add to projectile manager
        self.projectiles.spawn(projectile);

        tracing::info!("Threw splash potion (ID: {})", potion_id);
    }

    /// Apply splash potion effect to the player
    fn apply_splash_potion_to_player(&mut self, potion_id: u16, effectiveness: f64) {
        // Convert potion ID to PotionType
        let potion_type = match potion_id {
            potion_ids::AWKWARD => PotionType::Awkward,
            potion_ids::NIGHT_VISION => PotionType::NightVision,
            potion_ids::INVISIBILITY => PotionType::Invisibility,
            potion_ids::LEAPING => PotionType::Leaping,
            potion_ids::FIRE_RESISTANCE => PotionType::FireResistance,
            potion_ids::SWIFTNESS => PotionType::Swiftness,
            potion_ids::SLOWNESS => PotionType::Slowness,
            potion_ids::WATER_BREATHING => PotionType::WaterBreathing,
            potion_ids::HEALING => PotionType::Healing,
            potion_ids::HARMING => PotionType::Harming,
            potion_ids::POISON => PotionType::Poison,
            potion_ids::REGENERATION => PotionType::Regeneration,
            potion_ids::STRENGTH => PotionType::Strength,
            potion_ids::WEAKNESS => PotionType::Weakness,
            _ => return,
        };

        // Instant effects (Healing/Harming) apply immediately
        match potion_type {
            PotionType::Healing => {
                let heal_amount = (4.0 * effectiveness) as f32;
                self.player_health.heal(heal_amount);
                tracing::info!("Splash healing: +{:.1} HP", heal_amount);
            }
            PotionType::Harming => {
                let damage = (6.0 * effectiveness) as f32;
                self.player_health.damage(damage);
                tracing::info!("Splash harming: -{:.1} HP", damage);
            }
            _ => {
                // Duration effects - apply with reduced duration based on distance
                if let Some(effect_type) = potion_type.effect() {
                    let base_duration = potion_type.base_duration_ticks();
                    let duration = (base_duration as f64 * effectiveness) as u32;
                    if duration > 0 {
                        let effect = StatusEffect::new(effect_type, 0, duration);
                        self.status_effects.add(effect);
                        tracing::info!(
                            "Splash {:?} applied to player for {} ticks",
                            effect_type,
                            duration
                        );
                    }
                }
            }
        }
    }

    /// Count bookshelves within 2 blocks of the enchanting table (vanilla mechanics)
    fn count_nearby_bookshelves(&self, table_pos: IVec3) -> u32 {
        // Vanilla: bookshelves must be 2 blocks away, 1 block higher, with air in between
        // Simplified: check 5x5x2 area centered on table, 1 block up
        let bookshelf_id: BlockId = BLOCK_BOOKSHELF;

        let mut count = 0u32;
        for dy in 0..2 {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    // Skip center 3x3 area (too close to table)
                    if dx.abs() <= 1 && dz.abs() <= 1 {
                        continue;
                    }

                    let check_pos =
                        IVec3::new(table_pos.x + dx, table_pos.y + dy, table_pos.z + dz);

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

        for (key, furnace) in &mut self.furnaces {
            if key.dimension != DimensionId::Overworld {
                continue;
            }
            let was_lit = furnace.is_lit;
            furnace.update(dt);
            if was_lit != furnace.is_lit {
                lit_changes.push((IVec3::new(key.x, key.y, key.z), furnace.is_lit));
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

    /// Update all brewing stands in the world
    fn update_brewing_stands(&mut self, dt: f32) {
        for stand in self.brewing_stands.values_mut() {
            stand.update(dt);
        }
    }

    /// Update player status effects (called every frame)
    fn update_status_effects(&mut self, _dt: f32) {
        // Tick all effects and remove expired ones
        // Note: StatusEffects.tick() handles one tick per call, independent of dt
        // In a real implementation, we'd track tick timing
        let expired = self.status_effects.tick();
        for effect_type in expired {
            tracing::info!("Status effect {:?} expired", effect_type);
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
            DroppedItemType::CookedPork | DroppedItemType::CookedBeef => {
                Some(ItemType::Food(FoodType::CookedMeat))
            }
            DroppedItemType::Apple => Some(ItemType::Food(FoodType::Apple)),
            DroppedItemType::Bow => Some(ItemType::Item(1)),
            DroppedItemType::Arrow => Some(ItemType::Item(2)),
            DroppedItemType::Stick => Some(ItemType::Item(3)),
            DroppedItemType::String => Some(ItemType::Item(4)),
            DroppedItemType::Flint => Some(ItemType::Item(5)),
            DroppedItemType::Feather => Some(ItemType::Item(6)),
            DroppedItemType::IronIngot => Some(ItemType::Item(7)),
            DroppedItemType::Coal => Some(ItemType::Item(8)),
            DroppedItemType::GoldIngot => Some(ItemType::Item(9)),
            DroppedItemType::Diamond => Some(ItemType::Item(14)),
            DroppedItemType::LapisLazuli => Some(ItemType::Item(15)),
            DroppedItemType::Leather => Some(ItemType::Item(102)),
            DroppedItemType::Wool => Some(ItemType::Item(103)),
            DroppedItemType::Egg => Some(ItemType::Item(104)),
            DroppedItemType::Sapling => Some(ItemType::Item(105)),
            DroppedItemType::GlassBottle => Some(ItemType::Item(CORE_ITEM_GLASS_BOTTLE)),
            DroppedItemType::WaterBottle => Some(ItemType::Item(CORE_ITEM_WATER_BOTTLE)),
            DroppedItemType::NetherWart => Some(ItemType::Item(CORE_ITEM_NETHER_WART)),
            DroppedItemType::BlazePowder => Some(ItemType::Item(CORE_ITEM_BLAZE_POWDER)),
            DroppedItemType::Gunpowder => Some(ItemType::Item(CORE_ITEM_GUNPOWDER)),
            DroppedItemType::SpiderEye => Some(ItemType::Item(CORE_ITEM_SPIDER_EYE)),
            DroppedItemType::FermentedSpiderEye => {
                Some(ItemType::Item(CORE_ITEM_FERMENTED_SPIDER_EYE))
            }
            DroppedItemType::MagmaCream => Some(ItemType::Item(CORE_ITEM_MAGMA_CREAM)),
            DroppedItemType::Sugar => Some(ItemType::Item(CORE_ITEM_SUGAR)),
            DroppedItemType::Paper => Some(ItemType::Item(CORE_ITEM_PAPER)),
            DroppedItemType::Book => Some(ItemType::Item(CORE_ITEM_BOOK)),
            DroppedItemType::WheatSeeds => Some(ItemType::Item(CORE_ITEM_WHEAT_SEEDS)),
            DroppedItemType::Wheat => Some(ItemType::Item(CORE_ITEM_WHEAT)),
            DroppedItemType::Bread => Some(ItemType::Food(FoodType::Bread)),
            DroppedItemType::Carrot => Some(ItemType::Food(FoodType::Carrot)),
            DroppedItemType::Potato => Some(ItemType::Food(FoodType::Potato)),
            DroppedItemType::BakedPotato => Some(ItemType::Food(FoodType::BakedPotato)),
            DroppedItemType::GoldenCarrot => Some(ItemType::Food(FoodType::GoldenCarrot)),
            DroppedItemType::PotionAwkward => Some(ItemType::Potion(potion_ids::AWKWARD)),
            DroppedItemType::PotionNightVision => Some(ItemType::Potion(potion_ids::NIGHT_VISION)),
            DroppedItemType::PotionInvisibility => Some(ItemType::Potion(potion_ids::INVISIBILITY)),
            DroppedItemType::PotionLeaping => Some(ItemType::Potion(potion_ids::LEAPING)),
            DroppedItemType::PotionFireResistance => {
                Some(ItemType::Potion(potion_ids::FIRE_RESISTANCE))
            }
            DroppedItemType::PotionSwiftness => Some(ItemType::Potion(potion_ids::SWIFTNESS)),
            DroppedItemType::PotionSlowness => Some(ItemType::Potion(potion_ids::SLOWNESS)),
            DroppedItemType::PotionWaterBreathing => {
                Some(ItemType::Potion(potion_ids::WATER_BREATHING))
            }
            DroppedItemType::PotionHealing => Some(ItemType::Potion(potion_ids::HEALING)),
            DroppedItemType::PotionHarming => Some(ItemType::Potion(potion_ids::HARMING)),
            DroppedItemType::PotionPoison => Some(ItemType::Potion(potion_ids::POISON)),
            DroppedItemType::PotionRegeneration => Some(ItemType::Potion(potion_ids::REGENERATION)),
            DroppedItemType::PotionStrength => Some(ItemType::Potion(potion_ids::STRENGTH)),
            DroppedItemType::PotionWeakness => Some(ItemType::Potion(potion_ids::WEAKNESS)),
            DroppedItemType::SplashPotionAwkward => {
                Some(ItemType::SplashPotion(potion_ids::AWKWARD))
            }
            DroppedItemType::SplashPotionNightVision => {
                Some(ItemType::SplashPotion(potion_ids::NIGHT_VISION))
            }
            DroppedItemType::SplashPotionInvisibility => {
                Some(ItemType::SplashPotion(potion_ids::INVISIBILITY))
            }
            DroppedItemType::SplashPotionLeaping => {
                Some(ItemType::SplashPotion(potion_ids::LEAPING))
            }
            DroppedItemType::SplashPotionFireResistance => {
                Some(ItemType::SplashPotion(potion_ids::FIRE_RESISTANCE))
            }
            DroppedItemType::SplashPotionSwiftness => {
                Some(ItemType::SplashPotion(potion_ids::SWIFTNESS))
            }
            DroppedItemType::SplashPotionSlowness => {
                Some(ItemType::SplashPotion(potion_ids::SLOWNESS))
            }
            DroppedItemType::SplashPotionWaterBreathing => {
                Some(ItemType::SplashPotion(potion_ids::WATER_BREATHING))
            }
            DroppedItemType::SplashPotionHealing => {
                Some(ItemType::SplashPotion(potion_ids::HEALING))
            }
            DroppedItemType::SplashPotionHarming => {
                Some(ItemType::SplashPotion(potion_ids::HARMING))
            }
            DroppedItemType::SplashPotionPoison => Some(ItemType::SplashPotion(potion_ids::POISON)),
            DroppedItemType::SplashPotionRegeneration => {
                Some(ItemType::SplashPotion(potion_ids::REGENERATION))
            }
            DroppedItemType::SplashPotionStrength => {
                Some(ItemType::SplashPotion(potion_ids::STRENGTH))
            }
            DroppedItemType::SplashPotionWeakness => {
                Some(ItemType::SplashPotion(potion_ids::WEAKNESS))
            }
            DroppedItemType::WoodenPickaxe => {
                Some(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood))
            }
            DroppedItemType::StonePickaxe => {
                Some(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Stone))
            }
            DroppedItemType::IronPickaxe => {
                Some(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron))
            }
            DroppedItemType::DiamondPickaxe => {
                Some(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Diamond))
            }
            DroppedItemType::GoldPickaxe => {
                Some(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Gold))
            }
            DroppedItemType::WoodenAxe => Some(ItemType::Tool(ToolType::Axe, ToolMaterial::Wood)),
            DroppedItemType::StoneAxe => Some(ItemType::Tool(ToolType::Axe, ToolMaterial::Stone)),
            DroppedItemType::IronAxe => Some(ItemType::Tool(ToolType::Axe, ToolMaterial::Iron)),
            DroppedItemType::DiamondAxe => {
                Some(ItemType::Tool(ToolType::Axe, ToolMaterial::Diamond))
            }
            DroppedItemType::GoldAxe => Some(ItemType::Tool(ToolType::Axe, ToolMaterial::Gold)),
            DroppedItemType::WoodenShovel => {
                Some(ItemType::Tool(ToolType::Shovel, ToolMaterial::Wood))
            }
            DroppedItemType::StoneShovel => {
                Some(ItemType::Tool(ToolType::Shovel, ToolMaterial::Stone))
            }
            DroppedItemType::IronShovel => {
                Some(ItemType::Tool(ToolType::Shovel, ToolMaterial::Iron))
            }
            DroppedItemType::DiamondShovel => {
                Some(ItemType::Tool(ToolType::Shovel, ToolMaterial::Diamond))
            }
            DroppedItemType::GoldShovel => {
                Some(ItemType::Tool(ToolType::Shovel, ToolMaterial::Gold))
            }
            DroppedItemType::WoodenSword => {
                Some(ItemType::Tool(ToolType::Sword, ToolMaterial::Wood))
            }
            DroppedItemType::StoneSword => {
                Some(ItemType::Tool(ToolType::Sword, ToolMaterial::Stone))
            }
            DroppedItemType::IronSword => Some(ItemType::Tool(ToolType::Sword, ToolMaterial::Iron)),
            DroppedItemType::DiamondSword => {
                Some(ItemType::Tool(ToolType::Sword, ToolMaterial::Diamond))
            }
            DroppedItemType::GoldSword => Some(ItemType::Tool(ToolType::Sword, ToolMaterial::Gold)),
            DroppedItemType::WoodenHoe => Some(ItemType::Tool(ToolType::Hoe, ToolMaterial::Wood)),
            DroppedItemType::StoneHoe => Some(ItemType::Tool(ToolType::Hoe, ToolMaterial::Stone)),
            DroppedItemType::IronHoe => Some(ItemType::Tool(ToolType::Hoe, ToolMaterial::Iron)),
            DroppedItemType::DiamondHoe => {
                Some(ItemType::Tool(ToolType::Hoe, ToolMaterial::Diamond))
            }
            DroppedItemType::GoldHoe => Some(ItemType::Tool(ToolType::Hoe, ToolMaterial::Gold)),
            _ => None,
        }
    }

    fn convert_core_item_type_to_dropped(item_type: ItemType) -> Option<DroppedItemType> {
        use mdminecraft_core::item::FoodType;

        if let Some(armor) = item_type_to_armor_dropped(item_type) {
            return Some(armor);
        }

        match item_type {
            ItemType::Block(block_id) => DroppedItemType::from_placeable_block(block_id),
            ItemType::Food(food) => match food {
                FoodType::Apple => Some(DroppedItemType::Apple),
                FoodType::Bread => Some(DroppedItemType::Bread),
                FoodType::RawMeat => Some(DroppedItemType::RawPork),
                FoodType::CookedMeat => Some(DroppedItemType::CookedPork),
                FoodType::Carrot => Some(DroppedItemType::Carrot),
                FoodType::Potato => Some(DroppedItemType::Potato),
                FoodType::BakedPotato => Some(DroppedItemType::BakedPotato),
                FoodType::GoldenCarrot => Some(DroppedItemType::GoldenCarrot),
            },
            ItemType::Item(id) => match id {
                1 => Some(DroppedItemType::Bow),
                2 => Some(DroppedItemType::Arrow),
                3 => Some(DroppedItemType::Stick),
                4 => Some(DroppedItemType::String),
                5 => Some(DroppedItemType::Flint),
                6 => Some(DroppedItemType::Feather),
                7 => Some(DroppedItemType::IronIngot),
                8 => Some(DroppedItemType::Coal),
                9 => Some(DroppedItemType::GoldIngot),
                14 => Some(DroppedItemType::Diamond),
                15 => Some(DroppedItemType::LapisLazuli),
                102 => Some(DroppedItemType::Leather),
                103 => Some(DroppedItemType::Wool),
                104 => Some(DroppedItemType::Egg),
                105 => Some(DroppedItemType::Sapling),
                CORE_ITEM_GLASS_BOTTLE => Some(DroppedItemType::GlassBottle),
                CORE_ITEM_WATER_BOTTLE => Some(DroppedItemType::WaterBottle),
                CORE_ITEM_NETHER_WART => Some(DroppedItemType::NetherWart),
                CORE_ITEM_BLAZE_POWDER => Some(DroppedItemType::BlazePowder),
                CORE_ITEM_GUNPOWDER => Some(DroppedItemType::Gunpowder),
                CORE_ITEM_SPIDER_EYE => Some(DroppedItemType::SpiderEye),
                CORE_ITEM_FERMENTED_SPIDER_EYE => Some(DroppedItemType::FermentedSpiderEye),
                CORE_ITEM_SUGAR => Some(DroppedItemType::Sugar),
                CORE_ITEM_MAGMA_CREAM => Some(DroppedItemType::MagmaCream),
                CORE_ITEM_PAPER => Some(DroppedItemType::Paper),
                CORE_ITEM_BOOK => Some(DroppedItemType::Book),
                CORE_ITEM_WHEAT_SEEDS => Some(DroppedItemType::WheatSeeds),
                CORE_ITEM_WHEAT => Some(DroppedItemType::Wheat),
                _ => None,
            },
            ItemType::Tool(tool, material) => Some(match (tool, material) {
                (ToolType::Pickaxe, ToolMaterial::Wood) => DroppedItemType::WoodenPickaxe,
                (ToolType::Pickaxe, ToolMaterial::Stone) => DroppedItemType::StonePickaxe,
                (ToolType::Pickaxe, ToolMaterial::Iron) => DroppedItemType::IronPickaxe,
                (ToolType::Pickaxe, ToolMaterial::Diamond) => DroppedItemType::DiamondPickaxe,
                (ToolType::Pickaxe, ToolMaterial::Gold) => DroppedItemType::GoldPickaxe,
                (ToolType::Axe, ToolMaterial::Wood) => DroppedItemType::WoodenAxe,
                (ToolType::Axe, ToolMaterial::Stone) => DroppedItemType::StoneAxe,
                (ToolType::Axe, ToolMaterial::Iron) => DroppedItemType::IronAxe,
                (ToolType::Axe, ToolMaterial::Diamond) => DroppedItemType::DiamondAxe,
                (ToolType::Axe, ToolMaterial::Gold) => DroppedItemType::GoldAxe,
                (ToolType::Shovel, ToolMaterial::Wood) => DroppedItemType::WoodenShovel,
                (ToolType::Shovel, ToolMaterial::Stone) => DroppedItemType::StoneShovel,
                (ToolType::Shovel, ToolMaterial::Iron) => DroppedItemType::IronShovel,
                (ToolType::Shovel, ToolMaterial::Diamond) => DroppedItemType::DiamondShovel,
                (ToolType::Shovel, ToolMaterial::Gold) => DroppedItemType::GoldShovel,
                (ToolType::Sword, ToolMaterial::Wood) => DroppedItemType::WoodenSword,
                (ToolType::Sword, ToolMaterial::Stone) => DroppedItemType::StoneSword,
                (ToolType::Sword, ToolMaterial::Iron) => DroppedItemType::IronSword,
                (ToolType::Sword, ToolMaterial::Diamond) => DroppedItemType::DiamondSword,
                (ToolType::Sword, ToolMaterial::Gold) => DroppedItemType::GoldSword,
                (ToolType::Hoe, ToolMaterial::Wood) => DroppedItemType::WoodenHoe,
                (ToolType::Hoe, ToolMaterial::Stone) => DroppedItemType::StoneHoe,
                (ToolType::Hoe, ToolMaterial::Iron) => DroppedItemType::IronHoe,
                (ToolType::Hoe, ToolMaterial::Diamond) => DroppedItemType::DiamondHoe,
                (ToolType::Hoe, ToolMaterial::Gold) => DroppedItemType::GoldHoe,
            }),
            ItemType::Potion(id) => match id {
                potion_ids::AWKWARD => Some(DroppedItemType::PotionAwkward),
                potion_ids::NIGHT_VISION => Some(DroppedItemType::PotionNightVision),
                potion_ids::INVISIBILITY => Some(DroppedItemType::PotionInvisibility),
                potion_ids::LEAPING => Some(DroppedItemType::PotionLeaping),
                potion_ids::FIRE_RESISTANCE => Some(DroppedItemType::PotionFireResistance),
                potion_ids::SWIFTNESS => Some(DroppedItemType::PotionSwiftness),
                potion_ids::SLOWNESS => Some(DroppedItemType::PotionSlowness),
                potion_ids::WATER_BREATHING => Some(DroppedItemType::PotionWaterBreathing),
                potion_ids::HEALING => Some(DroppedItemType::PotionHealing),
                potion_ids::HARMING => Some(DroppedItemType::PotionHarming),
                potion_ids::POISON => Some(DroppedItemType::PotionPoison),
                potion_ids::REGENERATION => Some(DroppedItemType::PotionRegeneration),
                potion_ids::STRENGTH => Some(DroppedItemType::PotionStrength),
                potion_ids::WEAKNESS => Some(DroppedItemType::PotionWeakness),
                _ => None,
            },
            ItemType::SplashPotion(id) => match id {
                potion_ids::AWKWARD => Some(DroppedItemType::SplashPotionAwkward),
                potion_ids::NIGHT_VISION => Some(DroppedItemType::SplashPotionNightVision),
                potion_ids::INVISIBILITY => Some(DroppedItemType::SplashPotionInvisibility),
                potion_ids::LEAPING => Some(DroppedItemType::SplashPotionLeaping),
                potion_ids::FIRE_RESISTANCE => Some(DroppedItemType::SplashPotionFireResistance),
                potion_ids::SWIFTNESS => Some(DroppedItemType::SplashPotionSwiftness),
                potion_ids::SLOWNESS => Some(DroppedItemType::SplashPotionSlowness),
                potion_ids::WATER_BREATHING => Some(DroppedItemType::SplashPotionWaterBreathing),
                potion_ids::HEALING => Some(DroppedItemType::SplashPotionHealing),
                potion_ids::HARMING => Some(DroppedItemType::SplashPotionHarming),
                potion_ids::POISON => Some(DroppedItemType::SplashPotionPoison),
                potion_ids::REGENERATION => Some(DroppedItemType::SplashPotionRegeneration),
                potion_ids::STRENGTH => Some(DroppedItemType::SplashPotionStrength),
                potion_ids::WEAKNESS => Some(DroppedItemType::SplashPotionWeakness),
                _ => None,
            },
        }
    }
}

fn render_hotbar(ctx: &egui::Context, hotbar: &Hotbar, registry: &BlockRegistry) {
    egui::Area::new(egui::Id::new("hotbar"))
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -20.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for i in 0..9 {
                    let is_selected = i == hotbar.selected;
                    let item_stack = hotbar.slots[i].as_ref();
                    let item_name = hotbar.item_name(item_stack, registry);

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

                    let inner = frame.show(ui, |ui| {
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

                            // Show count.
                            if let Some(stack) = item_stack {
                                match stack.item_type {
                                    ItemType::Tool(_, _) => {}
                                    ItemType::Block(_)
                                    | ItemType::Item(_)
                                    | ItemType::Food(_)
                                    | ItemType::Potion(_)
                                    | ItemType::SplashPotion(_) => {
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

                    if let Some(stack) = item_stack {
                        if let (Some(current), Some(max)) =
                            (stack.durability, stack.max_durability())
                        {
                            paint_durability_bar(ui, inner.response.rect, current, max);
                        }

                        let mut tooltip = item_name.clone();
                        tooltip.push_str(&format!("\nCount: {}", stack.count));
                        if let (Some(current), Some(max)) =
                            (stack.durability, stack.max_durability())
                        {
                            tooltip.push_str(&format!("\nDurability: {}/{}", current, max));
                        }
                        let enchants = stack.get_enchantments();
                        if !enchants.is_empty() {
                            tooltip.push_str("\nEnchantments:");
                            for enchant in enchants {
                                tooltip.push_str(&format!(
                                    "\n- {:?} {}",
                                    enchant.enchantment_type, enchant.level
                                ));
                            }
                        }
                        let _ = inner.response.on_hover_text(tooltip);
                    } else {
                        let _ = inner.response.on_hover_text("Empty");
                    }
                }
            });
        });
}

fn paint_durability_bar(ui: &egui::Ui, rect: egui::Rect, current: u32, max: u32) {
    if max == 0 {
        return;
    }

    let fraction = (current as f32 / max as f32).clamp(0.0, 1.0);
    let padding = 4.0;
    let height = 4.0;

    let max_width = (rect.width() - 2.0 * padding).max(0.0);
    let width = max_width * fraction;

    let bg_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + padding, rect.max.y - padding - height),
        egui::vec2(max_width, height),
    );
    let fg_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + padding, rect.max.y - padding - height),
        egui::vec2(width, height),
    );

    let color = if fraction < 0.2 {
        egui::Color32::from_rgb(220, 60, 60)
    } else if fraction < 0.5 {
        egui::Color32::from_rgb(230, 200, 70)
    } else {
        egui::Color32::from_rgb(80, 200, 80)
    };

    ui.painter().rect_filled(
        bg_rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120),
    );
    ui.painter().rect_filled(fg_rect, 0.0, color);
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

fn render_pause_menu(
    ctx: &egui::Context,
    view: &mut PauseMenuView,
    controls: &mut Arc<ControlsConfig>,
    controls_dirty: &mut bool,
    fov_degrees: &mut f32,
    render_distance: &mut i32,
    bindings_changed: &mut bool,
) -> PauseMenuAction {
    let mut action = PauseMenuAction::None;

    // Semi-transparent dark overlay
    egui::Area::new(egui::Id::new("pause_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    egui::Window::new(match view {
        PauseMenuView::Main => "Game Menu",
        PauseMenuView::Options => "Options",
    })
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .show(ctx, |ui| {
        ui.set_min_width(420.0);

        match view {
            PauseMenuView::Main => {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Paused").size(22.0).strong());
                    ui.add_space(10.0);

                    let button = |text: &str| {
                        egui::Button::new(egui::RichText::new(text).size(16.0))
                            .min_size(egui::vec2(260.0, 38.0))
                    };

                    if ui.add(button("Resume Game")).clicked() {
                        action = PauseMenuAction::Resume;
                    }
                    if ui.add(button("Options...")).clicked() {
                        *view = PauseMenuView::Options;
                    }
                    if ui.add(button("Save & Quit to Title")).clicked() {
                        action = PauseMenuAction::ReturnToMenu;
                    }
                    if ui.add(button("Quit Game")).clicked() {
                        action = PauseMenuAction::Quit;
                    }

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Press Esc to resume")
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                });
            }
            PauseMenuView::Options => {
                ui.label(
                    egui::RichText::new("Controls")
                        .size(18.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);

                let mut next_controls = (**controls).clone();
                let mut changed_controls = false;

                let mut sensitivity = next_controls.mouse_sensitivity;
                if ui
                    .add(
                        egui::Slider::new(&mut sensitivity, 0.001..=0.02)
                            .text("Mouse Sensitivity")
                            .show_value(true),
                    )
                    .changed()
                {
                    next_controls.mouse_sensitivity = sensitivity;
                    changed_controls = true;
                }

                let mut invert_y = next_controls.invert_y;
                if ui.checkbox(&mut invert_y, "Invert Y").changed() {
                    next_controls.invert_y = invert_y;
                    changed_controls = true;
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(10.0);

                ui.label(
                    egui::RichText::new("Keybinds")
                        .size(18.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);

                let key_options: &[(&str, &str)] = &[
                    ("KeyW", "W"),
                    ("KeyA", "A"),
                    ("KeyS", "S"),
                    ("KeyD", "D"),
                    ("KeyQ", "Q"),
                    ("KeyE", "E"),
                    ("KeyR", "R"),
                    ("KeyF", "F"),
                    ("KeyC", "C"),
                    ("KeyV", "V"),
                    ("Space", "Space"),
                    ("ShiftLeft", "Left Shift"),
                    ("ControlLeft", "Left Ctrl"),
                    ("Tab", "Tab"),
                    ("F3", "F3"),
                    ("F4", "F4"),
                    ("Digit1", "1"),
                    ("Digit2", "2"),
                    ("Digit3", "3"),
                    ("Digit4", "4"),
                    ("Digit5", "5"),
                    ("Digit6", "6"),
                    ("Digit7", "7"),
                    ("Digit8", "8"),
                    ("Digit9", "9"),
                ];

                let token_label = |token: &str| -> String {
                    key_options
                        .iter()
                        .find_map(|(key, label)| (*key == token).then_some(*label))
                        .map(|label| label.to_string())
                        .unwrap_or_else(|| token.to_string())
                };

                let rows: &[(&str, &str, &str)] = &[
                    ("Forward", "MoveForward", "KeyW"),
                    ("Back", "MoveBackward", "KeyS"),
                    ("Left", "MoveLeft", "KeyA"),
                    ("Right", "MoveRight", "KeyD"),
                    ("Jump", "Jump", "Space"),
                    ("Sprint", "Sprint", "ControlLeft"),
                    ("Crouch", "Crouch", "ShiftLeft"),
                    ("Drop Item", "DropItem", "KeyQ"),
                    ("Toggle Cursor", "ToggleCursor", "Tab"),
                    ("Toggle Fly", "ToggleFly", "F4"),
                ];

                for (label, action_name, default_token) in rows {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(*label)
                                .size(13.0)
                                .color(egui::Color32::LIGHT_GRAY),
                        );
                        ui.add_space(10.0);

                        let current_override = next_controls
                            .bindings
                            .base
                            .get(*action_name)
                            .and_then(|list| list.first())
                            .cloned();
                        let mut selection: Option<String> = current_override.clone();

                        let selected_text = if let Some(token) = current_override.as_deref() {
                            token_label(token)
                        } else {
                            format!("Default ({})", token_label(default_token))
                        };

                        egui::ComboBox::from_id_source(("bind", action_name))
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut selection,
                                    None,
                                    format!("Default ({})", token_label(default_token)),
                                );
                                for (token, display) in key_options {
                                    ui.selectable_value(
                                        &mut selection,
                                        Some((*token).to_string()),
                                        (*display).to_string(),
                                    );
                                }
                            });

                        if selection != current_override {
                            match selection {
                                None => {
                                    next_controls.bindings.base.remove(*action_name);
                                }
                                Some(token) => {
                                    next_controls
                                        .bindings
                                        .base
                                        .insert((*action_name).to_string(), vec![token]);
                                }
                            }
                            changed_controls = true;
                            *bindings_changed = true;
                        }
                    });
                }

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Keybinds are limited to a fixed set for determinism.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(10.0);

                ui.label(
                    egui::RichText::new("Video")
                        .size(18.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);

                if ui
                    .add(
                        egui::Slider::new(fov_degrees, 60.0..=110.0)
                            .text("Field of View")
                            .suffix("°")
                            .show_value(true),
                    )
                    .changed()
                {
                    next_controls.fov_degrees = *fov_degrees;
                    changed_controls = true;
                }

                if ui
                    .add(
                        egui::Slider::new(render_distance, 2..=16)
                            .text("Render Distance")
                            .suffix(" chunks")
                            .show_value(true),
                    )
                    .changed()
                {
                    next_controls.render_distance = *render_distance;
                    changed_controls = true;
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(10.0);

                ui.label(
                    egui::RichText::new("Audio")
                        .size(18.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);

                let mut audio_muted = next_controls.audio_muted;
                if ui.checkbox(&mut audio_muted, "Mute").changed() {
                    next_controls.audio_muted = audio_muted;
                    changed_controls = true;
                }

                let mut master_volume = next_controls.master_volume;
                if ui
                    .add(
                        egui::Slider::new(&mut master_volume, 0.0..=1.0)
                            .text("Master Volume")
                            .show_value(true),
                    )
                    .changed()
                {
                    next_controls.master_volume = master_volume;
                    changed_controls = true;
                }

                let mut music_volume = next_controls.music_volume;
                if ui
                    .add(
                        egui::Slider::new(&mut music_volume, 0.0..=1.0)
                            .text("Music Volume")
                            .show_value(true),
                    )
                    .changed()
                {
                    next_controls.music_volume = music_volume;
                    changed_controls = true;
                }

                let mut sfx_volume = next_controls.sfx_volume;
                if ui
                    .add(
                        egui::Slider::new(&mut sfx_volume, 0.0..=1.0)
                            .text("SFX Volume")
                            .show_value(true),
                    )
                    .changed()
                {
                    next_controls.sfx_volume = sfx_volume;
                    changed_controls = true;
                }

                let mut ambient_volume = next_controls.ambient_volume;
                if ui
                    .add(
                        egui::Slider::new(&mut ambient_volume, 0.0..=1.0)
                            .text("Ambient Volume")
                            .show_value(true),
                    )
                    .changed()
                {
                    next_controls.ambient_volume = ambient_volume;
                    changed_controls = true;
                }

                if changed_controls {
                    *controls = Arc::new(next_controls);
                    *controls_dirty = true;
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(*controls_dirty, egui::Button::new("Save Settings"))
                        .clicked()
                    {
                        if let Err(err) = controls.as_ref().save() {
                            tracing::warn!(?err, "Failed to save settings");
                        } else {
                            *controls_dirty = false;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            *view = PauseMenuView::Main;
                        }
                    });
                });
            }
        }
    });

    action
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

/// Render the inventory UI.
/// Returns `(close_clicked, spill_items)`.
fn render_inventory(
    ctx: &egui::Context,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    player_armor: &mut PlayerArmor,
    personal_crafting_grid: &mut [[Option<ItemStack>; 2]; 2],
    ui_cursor_stack: &mut Option<ItemStack>,
    ui_drag: &mut UiDragState,
) -> (bool, Vec<ItemStack>) {
    let mut close_clicked = false;
    let mut spill_items = Vec::new();
    ui_drag.begin_frame();

    if ui_cursor_stack.is_none() {
        ui_drag.reset();
    } else if let Some(button) = ui_drag.active_button {
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let secondary_down = ctx.input(|i| i.pointer.secondary_down());
        match button {
            UiDragButton::Primary if !primary_down => {
                let visited = std::mem::take(&mut ui_drag.visited);
                let mut dummy_crafting_grid: [[Option<ItemStack>; 3]; 3] = Default::default();
                apply_primary_drag_distribution(
                    ui_cursor_stack,
                    &visited,
                    hotbar,
                    main_inventory,
                    personal_crafting_grid,
                    &mut dummy_crafting_grid,
                );
                ui_drag.finish_drag();
            }
            UiDragButton::Secondary if !secondary_down => {
                ui_drag.finish_drag();
            }
            _ => {}
        }
    }

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
            ui.set_min_width(600.0);

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
                        let slot_idx = row * 9 + col;
                        render_core_slot_interactive_shift_moves_to_hotbar(
                            ui,
                            UiCoreSlotId::MainInventory(slot_idx),
                            &mut main_inventory.slots[slot_idx],
                            ui_cursor_stack,
                            hotbar,
                            ui_drag,
                            UiSlotVisual::new(36.0, false),
                        );
                    }
                });
            }

            ui.add_space(8.0);
            ui.separator();

            ui.label("Armor");
            ui.horizontal(|ui| {
                render_armor_slot_interactive(
                    ui,
                    ArmorSlot::Helmet,
                    player_armor,
                    ui_cursor_stack,
                    36.0,
                    ui_drag,
                );
                render_armor_slot_interactive(
                    ui,
                    ArmorSlot::Chestplate,
                    player_armor,
                    ui_cursor_stack,
                    36.0,
                    ui_drag,
                );
                render_armor_slot_interactive(
                    ui,
                    ArmorSlot::Leggings,
                    player_armor,
                    ui_cursor_stack,
                    36.0,
                    ui_drag,
                );
                render_armor_slot_interactive(
                    ui,
                    ArmorSlot::Boots,
                    player_armor,
                    ui_cursor_stack,
                    36.0,
                    ui_drag,
                );
            });

            ui.add_space(10.0);
            ui.separator();

            // Personal crafting (2x2).
            ui.label("Crafting (2x2)");
            ui.horizontal(|ui| {
                ui.label("Cursor:");
                render_crafting_slot(ui, ui_cursor_stack.as_ref());
                ui.label(
                    egui::RichText::new("Left click: pick/place. Right click: split/place one.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
            });

            let recipe_match = {
                let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
                #[allow(clippy::needless_range_loop)]
                for r in 0..2 {
                    #[allow(clippy::needless_range_loop)]
                    for c in 0..2 {
                        grid[r][c] = personal_crafting_grid[r][c].clone();
                    }
                }
                match_crafting_recipe(&grid, CraftingGridSize::TwoByTwo)
            };

            ui.horizontal(|ui| {
                let cursor_empty = ui_cursor_stack.is_none();
                let grid_empty = crafting_grid_is_empty(personal_crafting_grid);

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Recipes").strong());
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Fill requires empty cursor + grid.")
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );

                    let recipes = get_crafting_recipes();
                    let max_dim = CraftingGridSize::TwoByTwo.dimension();
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for recipe in recipes
                                .iter()
                                .filter(|recipe| recipe.min_grid_size.dimension() <= max_dim)
                            {
                                let crafts = crafting_max_crafts_in_storage(
                                    hotbar,
                                    main_inventory,
                                    &recipe.inputs,
                                );
                                let craftable = crafts > 0;
                                ui.horizontal(|ui| {
                                    let label =
                                        format!("{:?} x{}", recipe.output, recipe.output_count);
                                    ui.label(egui::RichText::new(label).color(if craftable {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::DARK_GRAY
                                    }));

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let enabled = craftable && cursor_empty && grid_empty;
                                            if ui
                                                .add_enabled(enabled, egui::Button::new("Fill"))
                                                .clicked()
                                            {
                                                let _ = try_autofill_crafting_grid(
                                                    personal_crafting_grid,
                                                    hotbar,
                                                    main_inventory,
                                                    recipe,
                                                );
                                            }
                                        },
                                    );
                                });
                            }
                        });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    // 2x2 grid
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Grid");
                            if ui
                                .add_enabled(cursor_empty, egui::Button::new("Clear"))
                                .clicked()
                            {
                                clear_crafting_grid_to_storage(
                                    personal_crafting_grid,
                                    hotbar,
                                    main_inventory,
                                    &mut spill_items,
                                );
                            }
                        });
                        #[allow(clippy::needless_range_loop)]
                        for r in 0..2 {
                            ui.horizontal(|ui| {
                                #[allow(clippy::needless_range_loop)]
                                for c in 0..2 {
                                    let slot_idx = r * 2 + c;
                                    render_core_slot_interactive_shift_moves_to_storage(
                                        ui,
                                        UiCoreSlotId::PersonalCrafting(slot_idx),
                                        &mut personal_crafting_grid[r][c],
                                        ui_cursor_stack,
                                        (&mut *hotbar, &mut *main_inventory),
                                        ui_drag,
                                        UiSlotVisual::new(40.0, false),
                                    );
                                }
                            });
                        }
                    });

                    ui.add_space(20.0);

                    // Output
                    ui.vertical(|ui| {
                        ui.label("Output");
                        if let Some(recipe) = recipe_match.as_ref() {
                            let result_stack = ItemStack::new(recipe.output, recipe.output_count);
                            let response = render_crafting_output_slot_interactive(
                                ui,
                                Some(&result_stack),
                                "Click to craft\nShift-click: craft all to inventory",
                            );
                            let clicked = response.clicked_by(egui::PointerButton::Primary)
                                || response.clicked_by(egui::PointerButton::Secondary);
                            if clicked {
                                let shift = ui.input(|i| i.modifiers.shift);
                                if shift {
                                    let crafts =
                                        crafting_max_crafts_2x2(personal_crafting_grid, recipe);
                                    let mut crafted = 0_u32;
                                    for _ in 0..crafts {
                                        if consume_crafting_inputs_2x2(
                                            personal_crafting_grid,
                                            recipe,
                                        ) {
                                            crafted += 1;
                                        } else {
                                            break;
                                        }
                                    }

                                    if crafted > 0 {
                                        let total = recipe.output_count.saturating_mul(crafted);
                                        let output_stack = ItemStack::new(recipe.output, total);
                                        if let Some(remainder) = add_stack_to_storage(
                                            hotbar,
                                            main_inventory,
                                            output_stack,
                                        ) {
                                            spill_items.push(remainder);
                                        }
                                    }
                                } else if cursor_can_accept_full_stack(
                                    ui_cursor_stack,
                                    &result_stack,
                                ) && consume_crafting_inputs_2x2(
                                    personal_crafting_grid,
                                    recipe,
                                ) {
                                    cursor_add_full_stack(ui_cursor_stack, result_stack);
                                }
                            }
                        } else {
                            render_crafting_output_slot_interactive(ui, None, "No recipe");
                            ui.label(
                                egui::RichText::new("No recipe")
                                    .size(10.0)
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    });
                });
            });

            ui.add_space(10.0);
            ui.separator();

            // Hotbar (9 slots)
            ui.label("Hotbar");
            ui.horizontal(|ui| {
                for i in 0..9 {
                    let is_selected = i == hotbar.selected;
                    render_core_slot_interactive_shift_moves_to_main_inventory(
                        ui,
                        UiCoreSlotId::Hotbar(i),
                        &mut hotbar.slots[i],
                        ui_cursor_stack,
                        main_inventory,
                        ui_drag,
                        UiSlotVisual::new(36.0, is_selected),
                    );
                }
            });

            ui.add_space(5.0);
            ui.label(
                egui::RichText::new("Press E to close")
                    .size(12.0)
                    .color(egui::Color32::GRAY),
            );
        });

    (close_clicked, spill_items)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiSlotClick {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiCoreSlotId {
    Hotbar(usize),
    MainInventory(usize),
    Chest(usize),
    PersonalCrafting(usize),
    CraftingGrid(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiDragButton {
    Primary,
    Secondary,
}

#[derive(Debug, Default)]
struct UiDragState {
    active_button: Option<UiDragButton>,
    visited: Vec<UiCoreSlotId>,
    suppress_clicks: bool,
}

#[derive(Debug, Clone, Copy)]
struct UiSlotVisual {
    size: f32,
    is_selected: bool,
}

impl UiSlotVisual {
    fn new(size: f32, is_selected: bool) -> Self {
        Self { size, is_selected }
    }
}

impl UiDragState {
    fn reset(&mut self) {
        self.active_button = None;
        self.visited.clear();
        self.suppress_clicks = false;
    }

    fn begin_frame(&mut self) {
        self.suppress_clicks = false;
    }

    fn finish_drag(&mut self) {
        self.active_button = None;
        self.visited.clear();
        self.suppress_clicks = true;
    }

    fn is_active(&self) -> bool {
        self.active_button.is_some()
    }

    fn start(&mut self, button: UiDragButton, slot_id: UiCoreSlotId) {
        self.active_button = Some(button);
        self.visited.clear();
        self.visited.push(slot_id);
    }

    fn push_slot(&mut self, slot_id: UiCoreSlotId) -> bool {
        if self.visited.contains(&slot_id) {
            return false;
        }

        self.visited.push(slot_id);
        true
    }
}

fn stacks_match_for_merge(a: &ItemStack, b: &ItemStack) -> bool {
    a.item_type == b.item_type && a.durability == b.durability && a.enchantments == b.enchantments
}

const INVENTORY_STACK_METADATA_VERSION_V1: u8 = 1;
const INVENTORY_STACK_METADATA_FLAG_DURABILITY: u8 = 1 << 0;
const INVENTORY_STACK_METADATA_FLAG_ENCHANTMENTS: u8 = 1 << 1;

fn enchantment_type_to_id(enchantment_type: EnchantmentType) -> u8 {
    match enchantment_type {
        EnchantmentType::Efficiency => 1,
        EnchantmentType::SilkTouch => 2,
        EnchantmentType::Fortune => 3,
        EnchantmentType::Sharpness => 4,
        EnchantmentType::Knockback => 5,
        EnchantmentType::FireAspect => 6,
        EnchantmentType::Protection => 7,
        EnchantmentType::FireProtection => 8,
        EnchantmentType::BlastProtection => 9,
        EnchantmentType::ProjectileProtection => 10,
        EnchantmentType::Unbreaking => 11,
        EnchantmentType::Mending => 12,
    }
}

fn enchantment_type_from_id(id: u8) -> Option<EnchantmentType> {
    match id {
        1 => Some(EnchantmentType::Efficiency),
        2 => Some(EnchantmentType::SilkTouch),
        3 => Some(EnchantmentType::Fortune),
        4 => Some(EnchantmentType::Sharpness),
        5 => Some(EnchantmentType::Knockback),
        6 => Some(EnchantmentType::FireAspect),
        7 => Some(EnchantmentType::Protection),
        8 => Some(EnchantmentType::FireProtection),
        9 => Some(EnchantmentType::BlastProtection),
        10 => Some(EnchantmentType::ProjectileProtection),
        11 => Some(EnchantmentType::Unbreaking),
        12 => Some(EnchantmentType::Mending),
        _ => None,
    }
}

fn encode_inventory_stack_metadata(stack: &ItemStack) -> Option<Vec<u8>> {
    let durability = stack.durability.and_then(|current| {
        let max = stack.max_durability()?;
        if current < max {
            Some(current)
        } else {
            None
        }
    });
    let enchantments = stack
        .enchantments
        .as_ref()
        .filter(|enchants| !enchants.is_empty());

    if durability.is_none() && enchantments.is_none() {
        return None;
    }

    let mut bytes = Vec::with_capacity(2 + 4 + 1 + 2 * enchantments.map_or(0, |v| v.len()));
    bytes.push(INVENTORY_STACK_METADATA_VERSION_V1);

    let mut flags = 0u8;
    if durability.is_some() {
        flags |= INVENTORY_STACK_METADATA_FLAG_DURABILITY;
    }
    if enchantments.is_some() {
        flags |= INVENTORY_STACK_METADATA_FLAG_ENCHANTMENTS;
    }
    bytes.push(flags);

    if let Some(durability) = durability {
        bytes.extend_from_slice(&durability.to_le_bytes());
    }

    if let Some(enchantments) = enchantments {
        let count = u8::try_from(enchantments.len()).unwrap_or(u8::MAX);
        bytes.push(count);
        for enchantment in enchantments.iter().take(count as usize) {
            bytes.push(enchantment_type_to_id(enchantment.enchantment_type));
            bytes.push(enchantment.level);
        }
    }

    Some(bytes)
}

fn decode_inventory_stack_metadata(
    metadata: &[u8],
) -> Option<(Option<u32>, Option<Vec<Enchantment>>)> {
    if metadata.len() < 2 {
        return None;
    }

    let version = metadata[0];
    if version != INVENTORY_STACK_METADATA_VERSION_V1 {
        return None;
    }

    let flags = metadata[1];
    let mut offset = 2usize;

    let durability = if flags & INVENTORY_STACK_METADATA_FLAG_DURABILITY != 0 {
        let end = offset.checked_add(4)?;
        if end > metadata.len() {
            return None;
        }
        let durability = u32::from_le_bytes(metadata[offset..end].try_into().ok()?);
        offset = end;
        Some(durability)
    } else {
        None
    };

    let enchantments = if flags & INVENTORY_STACK_METADATA_FLAG_ENCHANTMENTS != 0 {
        let count = *metadata.get(offset)? as usize;
        offset = offset.checked_add(1)?;
        let end = offset.checked_add(count.checked_mul(2)?)?;
        if end > metadata.len() {
            return None;
        }

        let mut enchants = Vec::with_capacity(count);
        for _ in 0..count {
            let type_id = metadata.get(offset).copied()?;
            let level = metadata.get(offset + 1).copied()?;
            offset += 2;

            let enchantment_type = enchantment_type_from_id(type_id)?;
            enchants.push(Enchantment::new(enchantment_type, level));
        }

        Some(enchants)
    } else {
        None
    };

    Some((durability, enchantments))
}

fn apply_inventory_stack_metadata(stack: &mut ItemStack, metadata: &[u8]) {
    let Some((durability, enchantments)) = decode_inventory_stack_metadata(metadata) else {
        return;
    };

    if let Some(durability) = durability {
        if let Some(max) = stack.max_durability() {
            stack.durability = Some(durability.min(max));
        }
    }

    if let Some(enchantments) = enchantments {
        if !enchantments.is_empty() {
            stack.enchantments = Some(enchantments);
        }
    }
}

fn apply_slot_click(
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    click: UiSlotClick,
) {
    match click {
        UiSlotClick::Primary => apply_slot_primary_click(slot, cursor),
        UiSlotClick::Secondary => apply_slot_secondary_click(slot, cursor),
    }
}

fn apply_slot_primary_click(slot: &mut Option<ItemStack>, cursor: &mut Option<ItemStack>) {
    if cursor.is_none() {
        *cursor = slot.take();
        return;
    }

    if slot.is_none() {
        *slot = cursor.take();
        return;
    }

    let Some(slot_stack) = slot.as_mut() else {
        return;
    };
    let Some(cursor_stack) = cursor.as_mut() else {
        return;
    };

    if stacks_match_for_merge(slot_stack, cursor_stack) {
        let max = slot_stack.max_stack_size();
        if slot_stack.count < max {
            let space = max - slot_stack.count;
            let to_move = space.min(cursor_stack.count);
            slot_stack.count += to_move;
            cursor_stack.count -= to_move;
            if cursor_stack.count == 0 {
                *cursor = None;
            }
            return;
        }
    }

    std::mem::swap(slot, cursor);
}

fn apply_slot_secondary_click(slot: &mut Option<ItemStack>, cursor: &mut Option<ItemStack>) {
    if cursor.is_none() {
        let Some(slot_stack) = slot.as_mut() else {
            return;
        };

        let take = slot_stack.count.div_ceil(2);
        let mut taken = slot_stack.clone();
        taken.count = take;
        slot_stack.count -= take;
        if slot_stack.count == 0 {
            *slot = None;
        }
        *cursor = Some(taken);
        return;
    }

    let Some(cursor_stack) = cursor.as_mut() else {
        return;
    };

    if slot.is_none() {
        let mut placed = cursor_stack.clone();
        placed.count = 1.min(cursor_stack.count);
        cursor_stack.count -= placed.count;
        if cursor_stack.count == 0 {
            *cursor = None;
        }
        *slot = Some(placed);
        return;
    }

    let Some(slot_stack) = slot.as_mut() else {
        return;
    };

    if !stacks_match_for_merge(slot_stack, cursor_stack) {
        return;
    }

    let max = slot_stack.max_stack_size();
    if slot_stack.count >= max {
        return;
    }

    slot_stack.count += 1;
    cursor_stack.count -= 1;
    if cursor_stack.count == 0 {
        *cursor = None;
    }
}

fn try_drag_place_one_from_cursor(
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
) -> bool {
    let Some(cursor_stack) = cursor.as_mut() else {
        return false;
    };
    if cursor_stack.count == 0 {
        *cursor = None;
        return false;
    }

    if slot.is_none() {
        let mut placed = cursor_stack.clone();
        placed.count = 1;
        cursor_stack.count -= 1;
        if cursor_stack.count == 0 {
            *cursor = None;
        }
        *slot = Some(placed);
        return true;
    }

    let Some(slot_stack) = slot.as_mut() else {
        return false;
    };
    if !stacks_match_for_merge(slot_stack, cursor_stack) {
        return false;
    }

    let max = slot_stack.max_stack_size();
    if slot_stack.count >= max {
        return false;
    }

    slot_stack.count += 1;
    cursor_stack.count -= 1;
    if cursor_stack.count == 0 {
        *cursor = None;
    }
    true
}

fn apply_primary_drag_distribution(
    cursor: &mut Option<ItemStack>,
    visited: &[UiCoreSlotId],
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    personal_crafting_grid: &mut [[Option<ItemStack>; 2]; 2],
    crafting_grid: &mut [[Option<ItemStack>; 3]; 3],
) {
    if visited.is_empty() {
        return;
    }

    while cursor.as_ref().is_some_and(|stack| stack.count > 0) {
        let mut progress = false;

        for slot_id in visited {
            let moved = match *slot_id {
                UiCoreSlotId::Hotbar(i) => {
                    if i < hotbar.slots.len() {
                        try_drag_place_one_from_cursor(&mut hotbar.slots[i], cursor)
                    } else {
                        false
                    }
                }
                UiCoreSlotId::MainInventory(i) => {
                    if i < main_inventory.slots.len() {
                        try_drag_place_one_from_cursor(&mut main_inventory.slots[i], cursor)
                    } else {
                        false
                    }
                }
                UiCoreSlotId::Chest(_) => false,
                UiCoreSlotId::PersonalCrafting(i) => {
                    let row = i / 2;
                    let col = i % 2;
                    if row < 2 && col < 2 {
                        try_drag_place_one_from_cursor(
                            &mut personal_crafting_grid[row][col],
                            cursor,
                        )
                    } else {
                        false
                    }
                }
                UiCoreSlotId::CraftingGrid(i) => {
                    let row = i / 3;
                    let col = i % 3;
                    if row < 3 && col < 3 {
                        try_drag_place_one_from_cursor(&mut crafting_grid[row][col], cursor)
                    } else {
                        false
                    }
                }
            };

            if moved {
                progress = true;
                if cursor.is_none() {
                    break;
                }
            }
        }

        if !progress {
            break;
        }
    }
}

fn apply_primary_drag_distribution_with_chest(
    cursor: &mut Option<ItemStack>,
    visited: &[UiCoreSlotId],
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    chest: &mut ChestState,
) {
    if visited.is_empty() {
        return;
    }

    while cursor.as_ref().is_some_and(|stack| stack.count > 0) {
        let mut progress = false;

        for slot_id in visited {
            let moved = match *slot_id {
                UiCoreSlotId::Hotbar(i) => {
                    if i < hotbar.slots.len() {
                        try_drag_place_one_from_cursor(&mut hotbar.slots[i], cursor)
                    } else {
                        false
                    }
                }
                UiCoreSlotId::MainInventory(i) => {
                    if i < main_inventory.slots.len() {
                        try_drag_place_one_from_cursor(&mut main_inventory.slots[i], cursor)
                    } else {
                        false
                    }
                }
                UiCoreSlotId::Chest(i) => {
                    if i < chest.slots.len() {
                        try_drag_place_one_from_cursor(&mut chest.slots[i], cursor)
                    } else {
                        false
                    }
                }
                UiCoreSlotId::PersonalCrafting(_) | UiCoreSlotId::CraftingGrid(_) => false,
            };

            if moved {
                progress = true;
                if cursor.is_none() {
                    break;
                }
            }
        }

        if !progress {
            break;
        }
    }
}

fn try_add_stack_to_cursor(
    cursor: &mut Option<ItemStack>,
    mut stack: ItemStack,
) -> Option<ItemStack> {
    if stack.count == 0 {
        return None;
    }

    let Some(cursor_stack) = cursor.as_mut() else {
        let max = stack.max_stack_size();
        if stack.count <= max {
            *cursor = Some(stack);
            return None;
        }

        let mut placed = stack.clone();
        placed.count = max;
        stack.count -= max;
        *cursor = Some(placed);
        return Some(stack);
    };

    if !stacks_match_for_merge(cursor_stack, &stack) {
        return Some(stack);
    }

    let max = cursor_stack.max_stack_size();
    if cursor_stack.count >= max {
        return Some(stack);
    }

    let space = max - cursor_stack.count;
    let to_add = space.min(stack.count);
    cursor_stack.count += to_add;
    stack.count -= to_add;

    if stack.count == 0 {
        None
    } else {
        Some(stack)
    }
}

fn render_core_slot_visual(
    ui: &mut egui::Ui,
    slot: &Option<ItemStack>,
    size: f32,
    is_selected: bool,
) -> egui::Response {
    let mut response = ui.allocate_response(egui::vec2(size, size), egui::Sense::click_and_drag());
    let rect = response.rect;

    let (fill, stroke) = if is_selected {
        (
            egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200),
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        )
    } else {
        (
            egui::Color32::from_rgba_unmultiplied(40, 40, 40, 180),
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        )
    };

    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().rect_stroke(rect, 0.0, stroke);

    ui.allocate_ui_at_rect(rect.shrink(4.0), |ui| {
        if let Some(stack) = slot.as_ref() {
            ui.vertical_centered(|ui| {
                let name = match stack.item_type {
                    mdminecraft_core::ItemType::Tool(tool, _) => format!("{:?}", tool),
                    mdminecraft_core::ItemType::Block(id) => format!("B{}", id),
                    mdminecraft_core::ItemType::Food(food) => format!("{:?}", food),
                    mdminecraft_core::ItemType::Potion(id) => format!("P{}", id),
                    mdminecraft_core::ItemType::SplashPotion(id) => format!("SP{}", id),
                    mdminecraft_core::ItemType::Item(id) => format!("I{}", id),
                };
                ui.label(
                    egui::RichText::new(&name[..name.len().min(4)])
                        .size(9.0)
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

    response = if let Some(stack) = slot.as_ref() {
        let mut tooltip = format!("{:?}", stack.item_type);
        tooltip.push_str(&format!("\nCount: {}", stack.count));

        if let (Some(current), Some(max)) = (stack.durability, stack.max_durability()) {
            paint_durability_bar(ui, rect, current, max);
            tooltip.push_str(&format!("\nDurability: {}/{}", current, max));
        }

        let enchants = stack.get_enchantments();
        if !enchants.is_empty() {
            tooltip.push_str("\nEnchantments:");
            for enchant in enchants {
                tooltip.push_str(&format!(
                    "\n- {:?} {}",
                    enchant.enchantment_type, enchant.level
                ));
            }
        }
        response.on_hover_text(tooltip)
    } else {
        response.on_hover_text("Empty")
    };

    response
}

fn armor_slot_short_label(slot: ArmorSlot) -> &'static str {
    match slot {
        ArmorSlot::Helmet => "H",
        ArmorSlot::Chestplate => "C",
        ArmorSlot::Leggings => "L",
        ArmorSlot::Boots => "B",
    }
}

fn render_armor_slot_interactive(
    ui: &mut egui::Ui,
    armor_slot: ArmorSlot,
    player_armor: &mut PlayerArmor,
    cursor: &mut Option<ItemStack>,
    size: f32,
    drag: &UiDragState,
) {
    let stack = player_armor
        .get(armor_slot)
        .and_then(armor_piece_to_core_stack);
    let mut response = render_core_slot_visual(ui, &stack, size, false);

    if stack.is_none() {
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            armor_slot_short_label(armor_slot),
            egui::FontId::proportional(12.0),
            egui::Color32::GRAY,
        );
    }

    if let Some(piece) = player_armor.get(armor_slot) {
        paint_durability_bar(ui, response.rect, piece.durability, piece.max_durability);

        let mut tooltip = format!("{:?}", piece.item_type);
        tooltip.push_str(&format!("\nSlot: {:?}", piece.slot));
        tooltip.push_str(&format!(
            "\nDurability: {}/{}",
            piece.durability, piece.max_durability
        ));

        if !piece.enchantments.is_empty() {
            tooltip.push_str("\nEnchantments:");
            for enchant in &piece.enchantments {
                tooltip.push_str(&format!(
                    "\n- {:?} {}",
                    enchant.enchantment_type, enchant.level
                ));
            }
        }

        response = response.on_hover_text(tooltip);
    } else {
        response = response.on_hover_text(format!("Empty {:?} slot", armor_slot));
    }

    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(_click) = click else {
        return;
    };

    if cursor.is_none() {
        let Some(piece) = player_armor.unequip(armor_slot) else {
            return;
        };

        if let Some(stack) = armor_piece_to_core_stack(&piece) {
            *cursor = Some(stack);
        } else {
            tracing::warn!(
                slot = ?armor_slot,
                item = ?piece.item_type,
                "Unequipped armor could not be represented as a core stack"
            );
        }
        return;
    }

    let Some(cursor_stack) = cursor.take() else {
        return;
    };

    let Some(new_piece) = armor_piece_from_core_stack(&cursor_stack) else {
        *cursor = Some(cursor_stack);
        return;
    };

    if new_piece.slot != armor_slot {
        *cursor = Some(cursor_stack);
        return;
    }

    let old_piece = player_armor.equip(new_piece);
    *cursor = old_piece.and_then(|piece| armor_piece_to_core_stack(&piece));
}

fn ui_drag_handle_slot(
    ui: &egui::Ui,
    response: &egui::Response,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    drag: &mut UiDragState,
) -> bool {
    let cursor_present = cursor.is_some();

    if cursor_present && !drag.is_active() {
        if response.drag_started_by(egui::PointerButton::Primary) {
            drag.start(UiDragButton::Primary, slot_id);
            return true;
        }

        if response.drag_started_by(egui::PointerButton::Secondary) {
            drag.start(UiDragButton::Secondary, slot_id);
            let _ = try_drag_place_one_from_cursor(slot, cursor);
            return true;
        }
    }

    let Some(active) = drag.active_button else {
        return false;
    };

    let is_button_down = ui.input(|i| match active {
        UiDragButton::Primary => i.pointer.primary_down(),
        UiDragButton::Secondary => i.pointer.secondary_down(),
    });
    if !is_button_down {
        return false;
    }

    if response.hovered() && drag.push_slot(slot_id) && active == UiDragButton::Secondary {
        let _ = try_drag_place_one_from_cursor(slot, cursor);
    }

    true
}

fn render_core_slot_interactive_shift_moves_to_hotbar(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    hotbar: &mut Hotbar,
    drag: &mut UiDragState,
    visual: UiSlotVisual,
) {
    let response = render_core_slot_visual(ui, slot, visual.size, visual.is_selected);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(stack) = slot.take() {
            *slot = hotbar.add_stack(stack);
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_storage(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    (hotbar, main_inventory): (&mut Hotbar, &mut MainInventory),
    drag: &mut UiDragState,
    visual: UiSlotVisual,
) {
    let response = render_core_slot_visual(ui, slot, visual.size, visual.is_selected);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(stack) = slot.take() {
            *slot = add_stack_to_storage(hotbar, main_inventory, stack);
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_main_inventory(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    main_inventory: &mut MainInventory,
    drag: &mut UiDragState,
    visual: UiSlotVisual,
) {
    let response = render_core_slot_visual(ui, slot, visual.size, visual.is_selected);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(stack) = slot.take() {
            *slot = main_inventory.add_stack(stack);
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn add_stack_to_storage(
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    stack: ItemStack,
) -> Option<ItemStack> {
    let remainder = hotbar.add_stack(stack)?;
    main_inventory.add_stack(remainder)
}

fn render_player_storage(
    ui: &mut egui::Ui,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    cursor: &mut Option<ItemStack>,
    drag: &mut UiDragState,
) {
    ui.label(
        egui::RichText::new("Inventory")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    for row in 0..3 {
        ui.horizontal(|ui| {
            for col in 0..9 {
                let slot_idx = row * 9 + col;
                render_core_slot_interactive_shift_moves_to_hotbar(
                    ui,
                    UiCoreSlotId::MainInventory(slot_idx),
                    &mut main_inventory.slots[slot_idx],
                    cursor,
                    hotbar,
                    drag,
                    UiSlotVisual::new(36.0, false),
                );
            }
        });
    }

    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Hotbar")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    ui.horizontal(|ui| {
        for i in 0..9 {
            let is_selected = i == hotbar.selected;
            render_core_slot_interactive_shift_moves_to_main_inventory(
                ui,
                UiCoreSlotId::Hotbar(i),
                &mut hotbar.slots[i],
                cursor,
                main_inventory,
                drag,
                UiSlotVisual::new(36.0, is_selected),
            );
        }
    });
}

fn furnace_try_insert(
    slot: &mut Option<(DroppedItemType, u32)>,
    item_type: DroppedItemType,
    count: u32,
) -> u32 {
    if count == 0 {
        return 0;
    }

    let max = item_type.max_stack_size().max(1);

    match slot {
        None => {
            let moved = count.min(max);
            *slot = Some((item_type, moved));
            moved
        }
        Some((existing_type, existing_count)) => {
            if *existing_type != item_type {
                return 0;
            }

            let space = max.saturating_sub(*existing_count);
            let moved = space.min(count);
            *existing_count += moved;
            moved
        }
    }
}

fn try_shift_move_core_stack_into_furnace(
    stack: &mut ItemStack,
    furnace: &mut FurnaceState,
) -> bool {
    let Some(dropped_type) = GameWorld::convert_core_item_type_to_dropped(stack.item_type) else {
        return false;
    };

    if furnace_slot_accepts_item(FurnaceSlotKind::Input, dropped_type) {
        let moved = furnace_try_insert(&mut furnace.input, dropped_type, stack.count);
        if moved > 0 {
            stack.count = stack.count.saturating_sub(moved);
            return true;
        }
    }

    if furnace_slot_accepts_item(FurnaceSlotKind::Fuel, dropped_type) {
        let moved = furnace_try_insert(&mut furnace.fuel, dropped_type, stack.count);
        if moved > 0 {
            stack.count = stack.count.saturating_sub(moved);
            return true;
        }
    }

    false
}

fn try_shift_move_core_stack_into_brewing_stand(
    stack: &mut ItemStack,
    stand: &mut BrewingStandState,
) -> bool {
    if stack.count == 0 {
        return false;
    }

    // Blaze powder prefers the fuel slot, but can also be a brewing ingredient.
    if stack.item_type == ItemType::Item(CORE_ITEM_BLAZE_POWDER) {
        let remainder = stand.add_fuel(stack.count);
        let moved = stack.count.saturating_sub(remainder);
        if moved > 0 {
            stack.count = remainder;
            return true;
        }
    }

    if let Some((potion_type, is_splash)) = core_item_stack_to_bottle(stack) {
        let mut remaining = stack.count;
        let mut moved_any = false;
        let (bottles, bottle_is_splash) = (&mut stand.bottles, &mut stand.bottle_is_splash);
        for (idx, slot) in bottles.iter_mut().enumerate() {
            if remaining == 0 {
                break;
            }
            if slot.is_none() {
                *slot = Some(potion_type);
                bottle_is_splash[idx] = is_splash && potion_type != PotionType::Water;
                remaining = remaining.saturating_sub(1);
                moved_any = true;
            }
        }

        if moved_any {
            stack.count = remaining;
            return true;
        }

        return false;
    }

    let Some(ingredient_id) = core_item_type_to_brew_ingredient_id(stack.item_type) else {
        return false;
    };

    let remainder = stand.add_ingredient(ingredient_id, stack.count);
    let moved = stack.count.saturating_sub(remainder);
    stack.count = remainder;
    moved > 0
}

fn try_shift_move_core_stack_into_enchanting_table(
    stack: &mut ItemStack,
    table: &mut EnchantingTableState,
) -> bool {
    if stack.count == 0 {
        return false;
    }

    // Lapis lazuli only.
    if stack.item_type != ItemType::Item(15) {
        return false;
    }

    let remainder = table.add_lapis(stack.count);
    let moved = stack.count.saturating_sub(remainder);
    stack.count = remainder;
    moved > 0
}

fn try_shift_move_core_stack_into_chest(stack: &mut ItemStack, chest: &mut ChestState) -> bool {
    if stack.count == 0 {
        return false;
    }

    let before = stack.count;

    // Merge into existing stacks first.
    for existing in chest.slots.iter_mut().flatten() {
        if stack.count == 0 {
            break;
        }
        if !stacks_match_for_merge(existing, stack) {
            continue;
        }

        let max = existing.max_stack_size();
        if existing.count >= max {
            continue;
        }

        let space = max - existing.count;
        let to_add = space.min(stack.count);
        existing.count += to_add;
        stack.count -= to_add;
    }

    // Then fill empty slots, splitting if needed.
    for slot in &mut chest.slots {
        if stack.count == 0 {
            break;
        }
        if slot.is_some() {
            continue;
        }

        let max = stack.max_stack_size();
        if stack.count <= max {
            *slot = Some(stack.clone());
            stack.count = 0;
            break;
        }

        let mut placed = stack.clone();
        placed.count = max;
        *slot = Some(placed);
        stack.count -= max;
    }

    stack.count != before
}

fn render_core_slot_interactive_shift_moves_to_furnace_or_hotbar(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    (furnace, hotbar): (&mut FurnaceState, &mut Hotbar),
    drag: &mut UiDragState,
    size: f32,
) {
    let response = render_core_slot_visual(ui, slot, size, false);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(mut stack) = slot.take() {
            let moved_any = try_shift_move_core_stack_into_furnace(&mut stack, furnace);
            if moved_any {
                if stack.count > 0 {
                    *slot = Some(stack);
                }
            } else {
                *slot = hotbar.add_stack(stack);
            }
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_brewing_or_hotbar(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    (stand, hotbar): (&mut BrewingStandState, &mut Hotbar),
    drag: &mut UiDragState,
    size: f32,
) {
    let response = render_core_slot_visual(ui, slot, size, false);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(mut stack) = slot.take() {
            let moved_any = try_shift_move_core_stack_into_brewing_stand(&mut stack, stand);
            if moved_any {
                if stack.count > 0 {
                    *slot = Some(stack);
                }
            } else {
                *slot = hotbar.add_stack(stack);
            }
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_enchanting_or_hotbar(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    (table, hotbar): (&mut EnchantingTableState, &mut Hotbar),
    drag: &mut UiDragState,
    size: f32,
) {
    let response = render_core_slot_visual(ui, slot, size, false);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(mut stack) = slot.take() {
            let moved_any = try_shift_move_core_stack_into_enchanting_table(&mut stack, table);
            if moved_any {
                if stack.count > 0 {
                    *slot = Some(stack);
                }
            } else {
                *slot = hotbar.add_stack(stack);
            }
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_furnace_or_main_inventory(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    (furnace, main_inventory): (&mut FurnaceState, &mut MainInventory),
    drag: &mut UiDragState,
    visual: UiSlotVisual,
) {
    let response = render_core_slot_visual(ui, slot, visual.size, visual.is_selected);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(mut stack) = slot.take() {
            let moved_any = try_shift_move_core_stack_into_furnace(&mut stack, furnace);
            if moved_any {
                if stack.count > 0 {
                    *slot = Some(stack);
                }
            } else {
                *slot = main_inventory.add_stack(stack);
            }
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_brewing_or_main_inventory(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    (stand, main_inventory): (&mut BrewingStandState, &mut MainInventory),
    drag: &mut UiDragState,
    visual: UiSlotVisual,
) {
    let response = render_core_slot_visual(ui, slot, visual.size, visual.is_selected);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(mut stack) = slot.take() {
            let moved_any = try_shift_move_core_stack_into_brewing_stand(&mut stack, stand);
            if moved_any {
                if stack.count > 0 {
                    *slot = Some(stack);
                }
            } else {
                *slot = main_inventory.add_stack(stack);
            }
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_enchanting_or_main_inventory(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    (table, main_inventory): (&mut EnchantingTableState, &mut MainInventory),
    drag: &mut UiDragState,
    visual: UiSlotVisual,
) {
    let response = render_core_slot_visual(ui, slot, visual.size, visual.is_selected);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        if let Some(mut stack) = slot.take() {
            let moved_any = try_shift_move_core_stack_into_enchanting_table(&mut stack, table);
            if moved_any {
                if stack.count > 0 {
                    *slot = Some(stack);
                }
            } else {
                *slot = main_inventory.add_stack(stack);
            }
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_core_slot_interactive_shift_moves_to_chest(
    ui: &mut egui::Ui,
    slot_id: UiCoreSlotId,
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    chest: &mut ChestState,
    drag: &mut UiDragState,
    visual: UiSlotVisual,
) {
    let response = render_core_slot_visual(ui, slot, visual.size, visual.is_selected);
    if ui_drag_handle_slot(ui, &response, slot_id, slot, cursor, drag) {
        return;
    }
    if drag.suppress_clicks {
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    let Some(click) = click else {
        return;
    };

    let shift = ui.input(|i| i.modifiers.shift);
    if shift {
        let Some(mut stack) = slot.take() else {
            return;
        };

        let moved_any = try_shift_move_core_stack_into_chest(&mut stack, chest);
        if !moved_any || stack.count > 0 {
            *slot = Some(stack);
        }
        return;
    }

    apply_slot_click(slot, cursor, click);
}

fn render_player_storage_for_chest(
    ui: &mut egui::Ui,
    chest: &mut ChestState,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    cursor: &mut Option<ItemStack>,
    drag: &mut UiDragState,
) {
    ui.label(
        egui::RichText::new("Inventory")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    for row in 0..3 {
        ui.horizontal(|ui| {
            for col in 0..9 {
                let slot_idx = row * 9 + col;
                render_core_slot_interactive_shift_moves_to_chest(
                    ui,
                    UiCoreSlotId::MainInventory(slot_idx),
                    &mut main_inventory.slots[slot_idx],
                    cursor,
                    &mut *chest,
                    drag,
                    UiSlotVisual::new(36.0, false),
                );
            }
        });
    }

    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Hotbar")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    ui.horizontal(|ui| {
        for i in 0..9 {
            let is_selected = i == hotbar.selected;
            render_core_slot_interactive_shift_moves_to_chest(
                ui,
                UiCoreSlotId::Hotbar(i),
                &mut hotbar.slots[i],
                cursor,
                &mut *chest,
                drag,
                UiSlotVisual::new(36.0, is_selected),
            );
        }
    });
}

fn render_player_storage_for_furnace(
    ui: &mut egui::Ui,
    furnace: &mut FurnaceState,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    cursor: &mut Option<ItemStack>,
    drag: &mut UiDragState,
) {
    ui.label(
        egui::RichText::new("Inventory")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    for row in 0..3 {
        ui.horizontal(|ui| {
            for col in 0..9 {
                let slot_idx = row * 9 + col;
                render_core_slot_interactive_shift_moves_to_furnace_or_hotbar(
                    ui,
                    UiCoreSlotId::MainInventory(slot_idx),
                    &mut main_inventory.slots[slot_idx],
                    cursor,
                    (&mut *furnace, &mut *hotbar),
                    drag,
                    36.0,
                );
            }
        });
    }

    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Hotbar")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    ui.horizontal(|ui| {
        for i in 0..9 {
            let is_selected = i == hotbar.selected;
            render_core_slot_interactive_shift_moves_to_furnace_or_main_inventory(
                ui,
                UiCoreSlotId::Hotbar(i),
                &mut hotbar.slots[i],
                cursor,
                (&mut *furnace, &mut *main_inventory),
                drag,
                UiSlotVisual::new(36.0, is_selected),
            );
        }
    });
}

fn render_player_storage_for_brewing_stand(
    ui: &mut egui::Ui,
    stand: &mut BrewingStandState,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    cursor: &mut Option<ItemStack>,
    drag: &mut UiDragState,
) {
    ui.label(
        egui::RichText::new("Inventory")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    for row in 0..3 {
        ui.horizontal(|ui| {
            for col in 0..9 {
                let slot_idx = row * 9 + col;
                render_core_slot_interactive_shift_moves_to_brewing_or_hotbar(
                    ui,
                    UiCoreSlotId::MainInventory(slot_idx),
                    &mut main_inventory.slots[slot_idx],
                    cursor,
                    (&mut *stand, &mut *hotbar),
                    drag,
                    36.0,
                );
            }
        });
    }

    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Hotbar")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    ui.horizontal(|ui| {
        for i in 0..9 {
            let is_selected = i == hotbar.selected;
            render_core_slot_interactive_shift_moves_to_brewing_or_main_inventory(
                ui,
                UiCoreSlotId::Hotbar(i),
                &mut hotbar.slots[i],
                cursor,
                (&mut *stand, &mut *main_inventory),
                drag,
                UiSlotVisual::new(36.0, is_selected),
            );
        }
    });
}

fn render_player_storage_for_enchanting_table(
    ui: &mut egui::Ui,
    table: &mut EnchantingTableState,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    cursor: &mut Option<ItemStack>,
    drag: &mut UiDragState,
) {
    ui.label(
        egui::RichText::new("Inventory")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    for row in 0..3 {
        ui.horizontal(|ui| {
            for col in 0..9 {
                let slot_idx = row * 9 + col;
                render_core_slot_interactive_shift_moves_to_enchanting_or_hotbar(
                    ui,
                    UiCoreSlotId::MainInventory(slot_idx),
                    &mut main_inventory.slots[slot_idx],
                    cursor,
                    (&mut *table, &mut *hotbar),
                    drag,
                    36.0,
                );
            }
        });
    }

    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Hotbar")
            .size(12.0)
            .color(egui::Color32::GRAY),
    );
    ui.horizontal(|ui| {
        for i in 0..9 {
            let is_selected = i == hotbar.selected;
            render_core_slot_interactive_shift_moves_to_enchanting_or_main_inventory(
                ui,
                UiCoreSlotId::Hotbar(i),
                &mut hotbar.slots[i],
                cursor,
                (&mut *table, &mut *main_inventory),
                drag,
                UiSlotVisual::new(36.0, is_selected),
            );
        }
    });
}

#[derive(Debug, Clone)]
struct CraftingPattern {
    width: usize,
    height: usize,
    cells: [[Option<ItemType>; 3]; 3],
}

impl CraftingPattern {
    fn mirrored_horizontal(&self) -> Self {
        if self.width == 0 || self.height == 0 {
            return self.clone();
        }

        let mut cells: [[Option<ItemType>; 3]; 3] = [[None; 3]; 3];
        for (r, row) in cells.iter_mut().enumerate().take(self.height) {
            let source_row = &self.cells[r];
            for (c, source) in source_row.iter().copied().enumerate().take(self.width) {
                row[self.width - 1 - c] = source;
            }
        }

        Self {
            width: self.width,
            height: self.height,
            cells,
        }
    }
}

#[derive(Debug, Clone)]
struct CraftingRecipe {
    inputs: Vec<(ItemType, u32)>,
    output: ItemType,
    output_count: u32,
    pattern: Option<CraftingPattern>,
    /// If true, shaped recipes also match a horizontally mirrored pattern.
    ///
    /// Used for recipes like stairs/bows where vanilla accepts both orientations.
    allow_horizontal_mirror: bool,
    min_grid_size: CraftingGridSize,
    /// If true, allow extra counts of the required item types (no extra item types).
    ///
    /// This is used sparingly to avoid ambiguous matches with subset/superset recipes.
    allow_extra_counts_of_required_types: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CraftingGridSize {
    TwoByTwo,
    ThreeByThree,
}

impl CraftingGridSize {
    fn dimension(self) -> usize {
        match self {
            CraftingGridSize::TwoByTwo => 2,
            CraftingGridSize::ThreeByThree => 3,
        }
    }
}

/// Get available crafting recipes as (inputs, output, output_count)
/// Inputs are a list of (ItemType, count) required
fn get_crafting_recipes() -> Vec<CraftingRecipe> {
    let mut recipes = vec![
        // Furnace: 8 cobblestone → furnace
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_COBBLESTONE), 8)],
            output: ItemType::Block(BLOCK_FURNACE),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        None,
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Chest: 8 planks → chest
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 8)],
            output: ItemType::Block(interactive_blocks::CHEST),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Planks: 1 log → 4 planks
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_LOG), 1)],
            output: ItemType::Block(BLOCK_OAK_PLANKS),
            output_count: 4,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: true,
        },
        // Crafting Table: 4 planks → crafting table
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 4)],
            output: ItemType::Block(BLOCK_CRAFTING_TABLE),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 2,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                    ],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Sticks: 2 planks → 4 sticks
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 2)],
            output: ItemType::Item(3),
            output_count: 4,
            pattern: Some(CraftingPattern {
                width: 1,
                height: 2,
                cells: [
                    [Some(ItemType::Block(BLOCK_OAK_PLANKS)), None, None],
                    [Some(ItemType::Block(BLOCK_OAK_PLANKS)), None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        }, // Item(3) = Stick
        // Torches: 1 coal + 1 stick → 4 torches
        CraftingRecipe {
            inputs: vec![(ItemType::Item(8), 1), (ItemType::Item(3), 1)], // Item(8) = Coal
            output: ItemType::Block(interactive_blocks::TORCH),
            output_count: 4,
            pattern: Some(CraftingPattern {
                width: 1,
                height: 2,
                cells: [
                    [Some(ItemType::Item(8)), None, None],
                    [Some(ItemType::Item(3)), None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Bread: 3 wheat → bread
        CraftingRecipe {
            inputs: vec![(ItemType::Item(CORE_ITEM_WHEAT), 3)],
            output: ItemType::Food(mdminecraft_core::item::FoodType::Bread),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 1,
                cells: [
                    [
                        Some(ItemType::Item(CORE_ITEM_WHEAT)),
                        Some(ItemType::Item(CORE_ITEM_WHEAT)),
                        Some(ItemType::Item(CORE_ITEM_WHEAT)),
                    ],
                    [None, None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Sugar: 1 sugar cane → sugar
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_SUGAR_CANE), 1)],
            output: ItemType::Item(CORE_ITEM_SUGAR),
            output_count: 1,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: true,
        },
        // Paper: 3 sugar cane → 3 paper
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_SUGAR_CANE), 3)],
            output: ItemType::Item(CORE_ITEM_PAPER),
            output_count: 3,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 1,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_SUGAR_CANE)),
                        Some(ItemType::Block(BLOCK_SUGAR_CANE)),
                        Some(ItemType::Block(BLOCK_SUGAR_CANE)),
                    ],
                    [None, None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Book: 3 paper + 1 leather → 1 book
        CraftingRecipe {
            inputs: vec![
                (ItemType::Item(CORE_ITEM_PAPER), 3),
                (ItemType::Item(102), 1), // Item(102) = Leather
            ],
            output: ItemType::Item(CORE_ITEM_BOOK),
            output_count: 1,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: true,
        },
        // Fermented Spider Eye (vanilla-ish): 1 brown mushroom + 1 sugar + 1 spider eye → 1 fermented spider eye
        CraftingRecipe {
            inputs: vec![
                (ItemType::Block(BLOCK_BROWN_MUSHROOM), 1),
                (ItemType::Item(CORE_ITEM_SUGAR), 1),
                (ItemType::Item(CORE_ITEM_SPIDER_EYE), 1),
            ],
            output: ItemType::Item(CORE_ITEM_FERMENTED_SPIDER_EYE),
            output_count: 1,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: true,
        },
        // Golden Carrot (vanilla-ish): 1 carrot + 1 gold ingot → golden carrot
        CraftingRecipe {
            inputs: vec![
                (ItemType::Food(mdminecraft_core::item::FoodType::Carrot), 1),
                (ItemType::Item(9), 1), // Item(9) = Gold ingot
            ],
            output: ItemType::Food(mdminecraft_core::item::FoodType::GoldenCarrot),
            output_count: 1,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Glass Bottles: 3 glass → 3 bottles
        CraftingRecipe {
            inputs: vec![(ItemType::Block(interactive_blocks::GLASS), 3)],
            output: ItemType::Item(CORE_ITEM_GLASS_BOTTLE),
            output_count: 3,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                        None,
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                    ],
                    [None, Some(ItemType::Block(interactive_blocks::GLASS)), None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Glass Panes: 6 glass → 16 glass panes
        CraftingRecipe {
            inputs: vec![(ItemType::Block(interactive_blocks::GLASS), 6)],
            output: ItemType::Block(interactive_blocks::GLASS_PANE),
            output_count: 16,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                    ],
                    [
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                        Some(ItemType::Block(interactive_blocks::GLASS)),
                    ],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Nether Wart Block: 1 block → 9 nether wart items
        CraftingRecipe {
            inputs: vec![(
                ItemType::Block(mdminecraft_world::BLOCK_NETHER_WART_BLOCK),
                1,
            )],
            output: ItemType::Item(CORE_ITEM_NETHER_WART),
            output_count: 9,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Bed: 3 wool + 3 planks → 1 bed
        CraftingRecipe {
            inputs: vec![
                (ItemType::Item(103), 3),
                (ItemType::Block(BLOCK_OAK_PLANKS), 3),
            ], // Item(103) = Wool
            output: ItemType::Block(interactive_blocks::BED_FOOT),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Item(103)),
                        Some(ItemType::Item(103)),
                        Some(ItemType::Item(103)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Brewing Stand: 1 blaze powder + 3 cobblestone → 1 brewing stand
        CraftingRecipe {
            inputs: vec![
                (ItemType::Item(CORE_ITEM_BLAZE_POWDER), 1),
                (ItemType::Block(BLOCK_COBBLESTONE), 3),
            ],
            output: ItemType::Block(BLOCK_BREWING_STAND),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [None, Some(ItemType::Item(CORE_ITEM_BLAZE_POWDER)), None],
                    [
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                    ],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Enchanting Table (vanilla-ish): 4 obsidian + 2 diamonds + 1 lapis → table
        CraftingRecipe {
            inputs: vec![
                (ItemType::Block(BLOCK_OBSIDIAN), 4),
                (ItemType::Item(14), 2), // Item(14) = Diamond
                (ItemType::Item(15), 1), // Item(15) = Lapis Lazuli
            ],
            output: ItemType::Block(BLOCK_ENCHANTING_TABLE),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [None, Some(ItemType::Item(15)), None],
                    [
                        Some(ItemType::Item(14)),
                        Some(ItemType::Block(BLOCK_OBSIDIAN)),
                        Some(ItemType::Item(14)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OBSIDIAN)),
                        Some(ItemType::Block(BLOCK_OBSIDIAN)),
                        Some(ItemType::Block(BLOCK_OBSIDIAN)),
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Bookshelf (vanilla-ish): 6 planks + 3 books → bookshelf
        CraftingRecipe {
            inputs: vec![
                (ItemType::Block(BLOCK_OAK_PLANKS), 6),
                (ItemType::Item(CORE_ITEM_BOOK), 3),
            ],
            output: ItemType::Block(BLOCK_BOOKSHELF),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [
                        Some(ItemType::Item(CORE_ITEM_BOOK)),
                        Some(ItemType::Item(CORE_ITEM_BOOK)),
                        Some(ItemType::Item(CORE_ITEM_BOOK)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Ladder: 7 sticks → 3 ladders
        CraftingRecipe {
            inputs: vec![(ItemType::Item(3), 7)],
            output: ItemType::Block(interactive_blocks::LADDER),
            output_count: 3,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [Some(ItemType::Item(3)), None, Some(ItemType::Item(3))],
                    [
                        Some(ItemType::Item(3)),
                        Some(ItemType::Item(3)),
                        Some(ItemType::Item(3)),
                    ],
                    [Some(ItemType::Item(3)), None, Some(ItemType::Item(3))],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Oak Door: 6 planks → 3 doors
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 6)],
            output: ItemType::Block(interactive_blocks::OAK_DOOR_LOWER),
            output_count: 3,
            pattern: Some(CraftingPattern {
                width: 2,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Iron Door: 6 iron ingots → 3 doors
        CraftingRecipe {
            inputs: vec![(ItemType::Item(7), 6)], // Item(7) = Iron Ingot
            output: ItemType::Block(interactive_blocks::IRON_DOOR_LOWER),
            output_count: 3,
            pattern: Some(CraftingPattern {
                width: 2,
                height: 3,
                cells: [
                    [Some(ItemType::Item(7)), Some(ItemType::Item(7)), None],
                    [Some(ItemType::Item(7)), Some(ItemType::Item(7)), None],
                    [Some(ItemType::Item(7)), Some(ItemType::Item(7)), None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Trapdoor: 6 planks → 2 trapdoors
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 6)],
            output: ItemType::Block(interactive_blocks::TRAPDOOR),
            output_count: 2,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Oak Fence: 4 planks + 2 sticks → 3 fences
        CraftingRecipe {
            inputs: vec![
                (ItemType::Block(BLOCK_OAK_PLANKS), 4),
                (ItemType::Item(3), 2),
            ],
            output: ItemType::Block(interactive_blocks::OAK_FENCE),
            output_count: 3,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Item(3)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Item(3)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Oak Fence Gate: 2 planks + 4 sticks → 1 fence gate
        CraftingRecipe {
            inputs: vec![
                (ItemType::Block(BLOCK_OAK_PLANKS), 2),
                (ItemType::Item(3), 4),
            ],
            output: ItemType::Block(interactive_blocks::OAK_FENCE_GATE),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Item(3)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Item(3)),
                    ],
                    [
                        Some(ItemType::Item(3)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Item(3)),
                    ],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Oak Pressure Plate: 2 planks → 1 plate
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 2)],
            output: ItemType::Block(mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 2,
                height: 1,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                    ],
                    [None, None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Oak Button: 1 plank → 1 button
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 1)],
            output: ItemType::Block(mdminecraft_world::redstone_blocks::OAK_BUTTON),
            output_count: 1,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Stone Button: 1 stone → 1 button
        CraftingRecipe {
            inputs: vec![(ItemType::Block(mdminecraft_world::BLOCK_STONE), 1)],
            output: ItemType::Block(mdminecraft_world::redstone_blocks::STONE_BUTTON),
            output_count: 1,
            pattern: None,
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Lever: 1 stick + 1 cobblestone → 1 lever
        CraftingRecipe {
            inputs: vec![
                (ItemType::Item(3), 1),
                (ItemType::Block(BLOCK_COBBLESTONE), 1),
            ],
            output: ItemType::Block(mdminecraft_world::redstone_blocks::LEVER),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 1,
                height: 2,
                cells: [
                    [Some(ItemType::Item(3)), None, None],
                    [Some(ItemType::Block(BLOCK_COBBLESTONE)), None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Stone Pressure Plate: 2 stone → 1 plate
        CraftingRecipe {
            inputs: vec![(ItemType::Block(mdminecraft_world::BLOCK_STONE), 2)],
            output: ItemType::Block(mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 2,
                height: 1,
                cells: [
                    [
                        Some(ItemType::Block(mdminecraft_world::BLOCK_STONE)),
                        Some(ItemType::Block(mdminecraft_world::BLOCK_STONE)),
                        None,
                    ],
                    [None, None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::TwoByTwo,
            allow_extra_counts_of_required_types: false,
        },
        // Stone Slab: 3 cobblestone → 6 slabs
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_COBBLESTONE), 3)],
            output: ItemType::Block(interactive_blocks::STONE_SLAB),
            output_count: 6,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 1,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                    ],
                    [None, None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Oak Slab: 3 planks → 6 slabs
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 3)],
            output: ItemType::Block(interactive_blocks::OAK_SLAB),
            output_count: 6,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 1,
                cells: [
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                    [None, None, None],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Stone Stairs: 6 cobblestone → 4 stairs
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_COBBLESTONE), 6)],
            output: ItemType::Block(interactive_blocks::STONE_STAIRS),
            output_count: 4,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [Some(ItemType::Block(BLOCK_COBBLESTONE)), None, None],
                    [
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        None,
                    ],
                    [
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                        Some(ItemType::Block(BLOCK_COBBLESTONE)),
                    ],
                ],
            }),
            allow_horizontal_mirror: true,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Oak Stairs: 6 planks → 4 stairs
        CraftingRecipe {
            inputs: vec![(ItemType::Block(BLOCK_OAK_PLANKS), 6)],
            output: ItemType::Block(interactive_blocks::OAK_STAIRS),
            output_count: 4,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [Some(ItemType::Block(BLOCK_OAK_PLANKS)), None, None],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        None,
                    ],
                    [
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                        Some(ItemType::Block(BLOCK_OAK_PLANKS)),
                    ],
                ],
            }),
            allow_horizontal_mirror: true,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        },
        // Bow: 3 sticks + 3 string
        CraftingRecipe {
            inputs: vec![(ItemType::Item(3), 3), (ItemType::Item(4), 3)],
            output: ItemType::Item(1),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [Some(ItemType::Item(3)), Some(ItemType::Item(4)), None],
                    [Some(ItemType::Item(3)), None, Some(ItemType::Item(4))],
                    [Some(ItemType::Item(3)), Some(ItemType::Item(4)), None],
                ],
            }),
            allow_horizontal_mirror: true,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(4) = String
        // Arrow: 1 flint + 1 stick + 1 feather
        CraftingRecipe {
            inputs: vec![
                (ItemType::Item(5), 1),
                (ItemType::Item(3), 1),
                (ItemType::Item(6), 1),
            ],
            output: ItemType::Item(2),
            output_count: 4,
            pattern: Some(CraftingPattern {
                width: 1,
                height: 3,
                cells: [
                    [Some(ItemType::Item(5)), None, None],
                    [Some(ItemType::Item(3)), None, None],
                    [Some(ItemType::Item(6)), None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(5) = Flint, Item(6) = Feather
        // Leather armor (Item(102) = Leather)
        // Leather Helmet: 5 leather
        CraftingRecipe {
            inputs: vec![(ItemType::Item(102), 5)],
            output: ItemType::Item(20),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                    ],
                    [Some(ItemType::Item(102)), None, Some(ItemType::Item(102))],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(20) = LeatherHelmet
        // Leather Chestplate: 8 leather
        CraftingRecipe {
            inputs: vec![(ItemType::Item(102), 8)],
            output: ItemType::Item(21),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                    ],
                    [Some(ItemType::Item(102)), None, Some(ItemType::Item(102))],
                    [
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(21) = LeatherChestplate
        // Leather Leggings: 7 leather
        CraftingRecipe {
            inputs: vec![(ItemType::Item(102), 7)],
            output: ItemType::Item(22),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                        Some(ItemType::Item(102)),
                    ],
                    [Some(ItemType::Item(102)), None, Some(ItemType::Item(102))],
                    [Some(ItemType::Item(102)), None, Some(ItemType::Item(102))],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(22) = LeatherLeggings
        // Leather Boots: 4 leather
        CraftingRecipe {
            inputs: vec![(ItemType::Item(102), 4)],
            output: ItemType::Item(23),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [Some(ItemType::Item(102)), None, Some(ItemType::Item(102))],
                    [Some(ItemType::Item(102)), None, Some(ItemType::Item(102))],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(23) = LeatherBoots
        // Iron armor (Item(7) = IronIngot)
        // Iron Helmet: 5 iron ingots
        CraftingRecipe {
            inputs: vec![(ItemType::Item(7), 5)],
            output: ItemType::Item(10),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                    ],
                    [Some(ItemType::Item(7)), None, Some(ItemType::Item(7))],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(10) = IronHelmet
        // Iron Chestplate: 8 iron ingots
        CraftingRecipe {
            inputs: vec![(ItemType::Item(7), 8)],
            output: ItemType::Item(11),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                    ],
                    [Some(ItemType::Item(7)), None, Some(ItemType::Item(7))],
                    [
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(11) = IronChestplate
        // Iron Leggings: 7 iron ingots
        CraftingRecipe {
            inputs: vec![(ItemType::Item(7), 7)],
            output: ItemType::Item(12),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                        Some(ItemType::Item(7)),
                    ],
                    [Some(ItemType::Item(7)), None, Some(ItemType::Item(7))],
                    [Some(ItemType::Item(7)), None, Some(ItemType::Item(7))],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(12) = IronLeggings
        // Iron Boots: 4 iron ingots
        CraftingRecipe {
            inputs: vec![(ItemType::Item(7), 4)],
            output: ItemType::Item(13),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [Some(ItemType::Item(7)), None, Some(ItemType::Item(7))],
                    [Some(ItemType::Item(7)), None, Some(ItemType::Item(7))],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(13) = IronBoots
        // Diamond armor (Item(14) = Diamond)
        // Diamond Helmet: 5 diamonds
        CraftingRecipe {
            inputs: vec![(ItemType::Item(14), 5)],
            output: ItemType::Item(30),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                    ],
                    [Some(ItemType::Item(14)), None, Some(ItemType::Item(14))],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(30) = DiamondHelmet
        // Diamond Chestplate: 8 diamonds
        CraftingRecipe {
            inputs: vec![(ItemType::Item(14), 8)],
            output: ItemType::Item(31),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                    ],
                    [Some(ItemType::Item(14)), None, Some(ItemType::Item(14))],
                    [
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                    ],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(31) = DiamondChestplate
        // Diamond Leggings: 7 diamonds
        CraftingRecipe {
            inputs: vec![(ItemType::Item(14), 7)],
            output: ItemType::Item(32),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                        Some(ItemType::Item(14)),
                    ],
                    [Some(ItemType::Item(14)), None, Some(ItemType::Item(14))],
                    [Some(ItemType::Item(14)), None, Some(ItemType::Item(14))],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(32) = DiamondLeggings
        // Diamond Boots: 4 diamonds
        CraftingRecipe {
            inputs: vec![(ItemType::Item(14), 4)],
            output: ItemType::Item(33),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 2,
                cells: [
                    [Some(ItemType::Item(14)), None, Some(ItemType::Item(14))],
                    [Some(ItemType::Item(14)), None, Some(ItemType::Item(14))],
                    [None, None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        }, // Item(33) = DiamondBoots
    ];

    recipes.extend(get_tool_crafting_recipes());
    recipes
}

fn get_tool_crafting_recipes() -> Vec<CraftingRecipe> {
    let stick = ItemType::Item(3);
    let tool_materials: [(ToolMaterial, ItemType); 5] = [
        (ToolMaterial::Wood, ItemType::Block(BLOCK_OAK_PLANKS)),
        (ToolMaterial::Stone, ItemType::Block(BLOCK_COBBLESTONE)),
        (ToolMaterial::Iron, ItemType::Item(7)),
        (ToolMaterial::Diamond, ItemType::Item(14)),
        (ToolMaterial::Gold, ItemType::Item(9)),
    ];

    let mut recipes = Vec::new();
    for (tool_material, material_item) in tool_materials {
        // Pickaxe: 3 materials + 2 sticks.
        recipes.push(CraftingRecipe {
            inputs: vec![(material_item, 3), (stick, 2)],
            output: ItemType::Tool(ToolType::Pickaxe, tool_material),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 3,
                height: 3,
                cells: [
                    [
                        Some(material_item),
                        Some(material_item),
                        Some(material_item),
                    ],
                    [None, Some(stick), None],
                    [None, Some(stick), None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        });

        // Axe: 3 materials + 2 sticks (mirrorable).
        recipes.push(CraftingRecipe {
            inputs: vec![(material_item, 3), (stick, 2)],
            output: ItemType::Tool(ToolType::Axe, tool_material),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 2,
                height: 3,
                cells: [
                    [Some(material_item), Some(material_item), None],
                    [Some(material_item), Some(stick), None],
                    [None, Some(stick), None],
                ],
            }),
            allow_horizontal_mirror: true,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        });

        // Shovel: 1 material + 2 sticks.
        recipes.push(CraftingRecipe {
            inputs: vec![(material_item, 1), (stick, 2)],
            output: ItemType::Tool(ToolType::Shovel, tool_material),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 1,
                height: 3,
                cells: [
                    [Some(material_item), None, None],
                    [Some(stick), None, None],
                    [Some(stick), None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        });

        // Sword: 2 materials + 1 stick.
        recipes.push(CraftingRecipe {
            inputs: vec![(material_item, 2), (stick, 1)],
            output: ItemType::Tool(ToolType::Sword, tool_material),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 1,
                height: 3,
                cells: [
                    [Some(material_item), None, None],
                    [Some(material_item), None, None],
                    [Some(stick), None, None],
                ],
            }),
            allow_horizontal_mirror: false,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        });

        // Hoe: 2 materials + 2 sticks (mirrorable).
        recipes.push(CraftingRecipe {
            inputs: vec![(material_item, 2), (stick, 2)],
            output: ItemType::Tool(ToolType::Hoe, tool_material),
            output_count: 1,
            pattern: Some(CraftingPattern {
                width: 2,
                height: 3,
                cells: [
                    [Some(material_item), Some(material_item), None],
                    [None, Some(stick), None],
                    [None, Some(stick), None],
                ],
            }),
            allow_horizontal_mirror: true,
            min_grid_size: CraftingGridSize::ThreeByThree,
            allow_extra_counts_of_required_types: false,
        });
    }

    recipes
}

fn match_crafting_recipe(
    crafting_grid: &[[Option<ItemStack>; 3]; 3],
    grid_size: CraftingGridSize,
) -> Option<CraftingRecipe> {
    let grid_dim = grid_size.dimension();
    // Treat any items outside the active grid size as an automatic mismatch.
    for (r, row) in crafting_grid.iter().enumerate() {
        for (c, slot) in row.iter().enumerate() {
            if (r >= grid_dim || c >= grid_dim) && slot.is_some() {
                return None;
            }
        }
    }

    // Gather items from grid
    let mut grid_items: std::collections::HashMap<ItemType, u32> = std::collections::HashMap::new();
    for row in crafting_grid {
        for stack in row.iter().flatten() {
            *grid_items.entry(stack.item_type).or_insert(0) += stack.count;
        }
    }

    let pattern_matches = |pattern: &CraftingPattern| -> bool {
        if pattern.width > grid_dim || pattern.height > grid_dim {
            return false;
        }

        for row_offset in 0..=grid_dim.saturating_sub(pattern.height) {
            for col_offset in 0..=grid_dim.saturating_sub(pattern.width) {
                let mut ok = true;
                for (r, row) in crafting_grid.iter().enumerate().take(grid_dim) {
                    for (c, slot) in row.iter().enumerate().take(grid_dim) {
                        let expected = if (row_offset..row_offset + pattern.height).contains(&r)
                            && (col_offset..col_offset + pattern.width).contains(&c)
                        {
                            pattern.cells[r - row_offset][c - col_offset]
                        } else {
                            None
                        };

                        let actual = slot.as_ref().map(|stack| stack.item_type);
                        if expected != actual {
                            ok = false;
                            break;
                        }
                    }
                    if !ok {
                        break;
                    }
                }

                if ok {
                    return true;
                }
            }
        }

        false
    };

    // Prefer shaped recipes when they match the grid exactly (vanilla behavior).
    let recipes = get_crafting_recipes();
    for recipe in recipes.iter() {
        if recipe.min_grid_size.dimension() > grid_size.dimension() {
            continue;
        }

        let Some(pattern) = recipe.pattern.as_ref() else {
            continue;
        };
        if pattern_matches(pattern) {
            return Some(recipe.clone());
        }
        if recipe.allow_horizontal_mirror {
            let mirrored = pattern.mirrored_horizontal();
            if pattern_matches(&mirrored) {
                return Some(recipe.clone());
            }
        }
    }

    // Check shapeless recipes.
    for recipe in recipes {
        if recipe.min_grid_size.dimension() > grid_size.dimension() {
            continue;
        }

        if recipe.pattern.is_some() {
            continue;
        }

        let mut matches = true;
        let mut required: std::collections::HashMap<ItemType, u32> =
            std::collections::HashMap::new();
        for (item_type, needed) in &recipe.inputs {
            *required.entry(*item_type).or_insert(0) += needed;
        }

        // Check required items are present (optionally allowing extra counts).
        for (item_type, needed) in &required {
            match grid_items.get(item_type) {
                Some(have) if *have >= *needed => {
                    if !recipe.allow_extra_counts_of_required_types && *have != *needed {
                        matches = false;
                        break;
                    }
                }
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
            return Some(recipe);
        }
    }

    None
}

fn storage_count_item_type(
    hotbar: &Hotbar,
    main_inventory: &MainInventory,
    item_type: ItemType,
) -> u32 {
    let mut total: u32 = 0;
    for stack in main_inventory
        .slots
        .iter()
        .chain(hotbar.slots.iter())
        .flatten()
    {
        if stack.item_type == item_type {
            total = total.saturating_add(stack.count);
        }
    }
    total
}

fn crafting_max_crafts_in_storage(
    hotbar: &Hotbar,
    main_inventory: &MainInventory,
    inputs: &[(ItemType, u32)],
) -> u32 {
    let mut required: std::collections::HashMap<ItemType, u32> = std::collections::HashMap::new();
    for (item_type, count) in inputs.iter().copied() {
        if count == 0 {
            continue;
        }
        *required.entry(item_type).or_insert(0) += count;
    }

    let mut crafts = u32::MAX;
    for (item_type, needed) in required {
        if needed == 0 {
            continue;
        }
        let have = storage_count_item_type(hotbar, main_inventory, item_type);
        crafts = crafts.min(have / needed);
    }

    if crafts == u32::MAX {
        0
    } else {
        crafts
    }
}

fn take_items_from_storage(
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    item_type: ItemType,
    mut remaining: u32,
) -> bool {
    if remaining == 0 {
        return true;
    }

    for slot in main_inventory
        .slots
        .iter_mut()
        .chain(hotbar.slots.iter_mut())
    {
        if remaining == 0 {
            break;
        }

        let Some(stack) = slot.as_mut() else {
            continue;
        };
        if stack.item_type != item_type || stack.count == 0 {
            continue;
        }

        let take = remaining.min(stack.count);
        stack.count -= take;
        remaining -= take;
        if stack.count == 0 {
            *slot = None;
        }
    }

    remaining == 0
}

fn crafting_grid_is_empty<const N: usize>(grid: &[[Option<ItemStack>; N]; N]) -> bool {
    grid.iter().all(|row| row.iter().all(Option::is_none))
}

fn try_autofill_crafting_grid<const N: usize>(
    grid: &mut [[Option<ItemStack>; N]; N],
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    recipe: &CraftingRecipe,
) -> bool {
    if !crafting_grid_is_empty(grid) {
        return false;
    }

    if let Some(pattern) = recipe.pattern.as_ref() {
        if pattern.width > N || pattern.height > N {
            return false;
        }

        let mut required: std::collections::HashMap<ItemType, u32> =
            std::collections::HashMap::new();
        for r in 0..pattern.height {
            for c in 0..pattern.width {
                let Some(item_type) = pattern.cells[r][c] else {
                    continue;
                };
                *required.entry(item_type).or_insert(0) += 1;
            }
        }

        for (item_type, needed) in required.iter() {
            let have = storage_count_item_type(hotbar, main_inventory, *item_type);
            if have < *needed {
                return false;
            }
        }

        for (item_type, needed) in required.iter() {
            let ok = take_items_from_storage(hotbar, main_inventory, *item_type, *needed);
            debug_assert!(ok, "pre-checked storage but take_items_from_storage failed");
            if !ok {
                return false;
            }
        }

        for (r, row) in grid.iter_mut().enumerate().take(pattern.height) {
            for (c, slot) in row.iter_mut().enumerate().take(pattern.width) {
                let Some(item_type) = pattern.cells[r][c] else {
                    continue;
                };
                *slot = Some(ItemStack::new(item_type, 1));
            }
        }

        return true;
    }

    let inputs = &recipe.inputs;
    let mut required: std::collections::HashMap<ItemType, u32> = std::collections::HashMap::new();
    for (item_type, count) in inputs.iter().copied() {
        if count == 0 {
            continue;
        }
        *required.entry(item_type).or_insert(0) += count;
    }

    let mut needed_slots = 0_usize;
    for (item_type, count) in required.iter() {
        let max = ItemStack::new(*item_type, 1).max_stack_size().max(1);
        needed_slots += count.div_ceil(max) as usize;
    }
    if needed_slots > N * N {
        return false;
    }

    for (item_type, needed) in required.iter() {
        let have = storage_count_item_type(hotbar, main_inventory, *item_type);
        if have < *needed {
            return false;
        }
    }

    for (item_type, needed) in required.iter() {
        if !take_items_from_storage(hotbar, main_inventory, *item_type, *needed) {
            return false;
        }
    }

    for (item_type, count) in inputs.iter().copied() {
        let max = ItemStack::new(item_type, 1).max_stack_size().max(1);
        let mut remaining = count;
        while remaining > 0 {
            let placed = remaining.min(max);
            remaining -= placed;
            let placed_stack = ItemStack::new(item_type, placed);

            let mut target = None;
            #[allow(clippy::needless_range_loop)]
            for row in 0..N {
                #[allow(clippy::needless_range_loop)]
                for col in 0..N {
                    if grid[row][col].is_none() {
                        target = Some((row, col));
                        break;
                    }
                }
                if target.is_some() {
                    break;
                }
            }

            let Some((row, col)) = target else {
                return false;
            };

            grid[row][col] = Some(placed_stack);
        }
    }

    true
}

fn clear_crafting_grid_to_storage<const N: usize>(
    grid: &mut [[Option<ItemStack>; N]; N],
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    spill_items: &mut Vec<ItemStack>,
) {
    for row in grid.iter_mut() {
        for slot in row.iter_mut() {
            let Some(stack) = slot.take() else {
                continue;
            };

            if let Some(remainder) = add_stack_to_storage(hotbar, main_inventory, stack) {
                spill_items.push(remainder);
            }
        }
    }
}

fn consume_crafting_inputs_3x3(
    crafting_grid: &mut [[Option<ItemStack>; 3]; 3],
    recipe: &CraftingRecipe,
) -> bool {
    if let Some(pattern) = recipe.pattern.as_ref() {
        let required_cells: usize = (0..pattern.height)
            .flat_map(|r| (0..pattern.width).map(move |c| pattern.cells[r][c]))
            .flatten()
            .count();
        if required_cells == 0 {
            return false;
        }

        let non_empty_cells = crafting_grid
            .iter()
            .flatten()
            .filter(|slot| slot.is_some())
            .count();
        if non_empty_cells != required_cells {
            return false;
        }

        for row in crafting_grid.iter_mut() {
            for slot in row.iter_mut() {
                let Some(stack) = slot.as_mut() else {
                    continue;
                };
                stack.count = stack.count.saturating_sub(1);
                if stack.count == 0 {
                    *slot = None;
                }
            }
        }

        return true;
    }

    for (item_type, mut remaining) in recipe.inputs.iter().copied() {
        if remaining == 0 {
            continue;
        }

        for row in crafting_grid.iter_mut() {
            for slot in row.iter_mut() {
                if remaining == 0 {
                    break;
                }

                let Some(stack) = slot.as_mut() else {
                    continue;
                };
                if stack.item_type != item_type || stack.count == 0 {
                    continue;
                }

                let take = remaining.min(stack.count);
                stack.count -= take;
                remaining -= take;

                if stack.count == 0 {
                    *slot = None;
                }
            }
        }

        if remaining != 0 {
            return false;
        }
    }

    true
}

fn consume_crafting_inputs_2x2(
    crafting_grid: &mut [[Option<ItemStack>; 2]; 2],
    recipe: &CraftingRecipe,
) -> bool {
    if let Some(pattern) = recipe.pattern.as_ref() {
        let required_cells: usize = (0..pattern.height)
            .flat_map(|r| (0..pattern.width).map(move |c| pattern.cells[r][c]))
            .flatten()
            .count();
        if required_cells == 0 {
            return false;
        }

        let non_empty_cells = crafting_grid
            .iter()
            .flatten()
            .filter(|slot| slot.is_some())
            .count();
        if non_empty_cells != required_cells {
            return false;
        }

        for row in crafting_grid.iter_mut() {
            for slot in row.iter_mut() {
                let Some(stack) = slot.as_mut() else {
                    continue;
                };
                stack.count = stack.count.saturating_sub(1);
                if stack.count == 0 {
                    *slot = None;
                }
            }
        }

        return true;
    }

    for (item_type, mut remaining) in recipe.inputs.iter().copied() {
        if remaining == 0 {
            continue;
        }

        for row in crafting_grid.iter_mut() {
            for slot in row.iter_mut() {
                if remaining == 0 {
                    break;
                }

                let Some(stack) = slot.as_mut() else {
                    continue;
                };
                if stack.item_type != item_type || stack.count == 0 {
                    continue;
                }

                let take = remaining.min(stack.count);
                stack.count -= take;
                remaining -= take;

                if stack.count == 0 {
                    *slot = None;
                }
            }
        }

        if remaining != 0 {
            return false;
        }
    }

    true
}

fn cursor_can_accept_full_stack(cursor: &Option<ItemStack>, stack: &ItemStack) -> bool {
    if stack.count == 0 {
        return false;
    }

    match cursor {
        None => stack.count <= stack.max_stack_size(),
        Some(cursor_stack) => {
            stacks_match_for_merge(cursor_stack, stack)
                && cursor_stack.count + stack.count <= cursor_stack.max_stack_size()
        }
    }
}

fn cursor_add_full_stack(cursor: &mut Option<ItemStack>, stack: ItemStack) {
    if stack.count == 0 {
        return;
    }

    match cursor {
        None => {
            *cursor = Some(stack);
        }
        Some(cursor_stack) => {
            debug_assert!(stacks_match_for_merge(cursor_stack, &stack));
            cursor_stack.count += stack.count;
        }
    }
}

fn crafting_max_crafts_3x3(
    crafting_grid: &[[Option<ItemStack>; 3]; 3],
    recipe: &CraftingRecipe,
) -> u32 {
    if let Some(pattern) = recipe.pattern.as_ref() {
        let required_cells: usize = (0..pattern.height)
            .flat_map(|r| (0..pattern.width).map(move |c| pattern.cells[r][c]))
            .flatten()
            .count();
        if required_cells == 0 {
            return 0;
        }

        let mut min_count = u32::MAX;
        let mut non_empty_cells = 0_usize;
        for stack in crafting_grid.iter().flatten().flatten() {
            non_empty_cells += 1;
            min_count = min_count.min(stack.count);
        }
        if non_empty_cells != required_cells {
            return 0;
        }

        if min_count == u32::MAX {
            0
        } else {
            min_count
        }
    } else {
        let mut crafts = u32::MAX;
        for (item_type, needed) in recipe.inputs.iter().copied() {
            if needed == 0 {
                continue;
            }

            let have: u32 = crafting_grid
                .iter()
                .flatten()
                .flatten()
                .filter(|stack| stack.item_type == item_type)
                .map(|stack| stack.count)
                .sum();
            crafts = crafts.min(have / needed);
        }

        if crafts == u32::MAX {
            0
        } else {
            crafts
        }
    }
}

fn crafting_max_crafts_2x2(
    crafting_grid: &[[Option<ItemStack>; 2]; 2],
    recipe: &CraftingRecipe,
) -> u32 {
    if let Some(pattern) = recipe.pattern.as_ref() {
        let required_cells: usize = (0..pattern.height)
            .flat_map(|r| (0..pattern.width).map(move |c| pattern.cells[r][c]))
            .flatten()
            .count();
        if required_cells == 0 {
            return 0;
        }

        let mut min_count = u32::MAX;
        let mut non_empty_cells = 0_usize;
        for stack in crafting_grid.iter().flatten().flatten() {
            non_empty_cells += 1;
            min_count = min_count.min(stack.count);
        }
        if non_empty_cells != required_cells {
            return 0;
        }

        if min_count == u32::MAX {
            0
        } else {
            min_count
        }
    } else {
        let mut crafts = u32::MAX;
        for (item_type, needed) in recipe.inputs.iter().copied() {
            if needed == 0 {
                continue;
            }

            let have: u32 = crafting_grid
                .iter()
                .flatten()
                .flatten()
                .filter(|stack| stack.item_type == item_type)
                .map(|stack| stack.count)
                .sum();
            crafts = crafts.min(have / needed);
        }

        if crafts == u32::MAX {
            0
        } else {
            crafts
        }
    }
}

/// Check if the crafting grid matches a recipe.
///
/// This only reports the output; see [`match_crafting_recipe`] for the full match.
#[cfg(test)]
fn check_crafting_recipe(crafting_grid: &[[Option<ItemStack>; 3]; 3]) -> Option<(ItemType, u32)> {
    match_crafting_recipe(crafting_grid, CraftingGridSize::ThreeByThree)
        .map(|recipe| (recipe.output, recipe.output_count))
}

/// Render the crafting table UI
/// Returns (close_clicked, spill_items)
fn render_crafting(
    ctx: &egui::Context,
    crafting_grid: &mut [[Option<ItemStack>; 3]; 3],
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    ui_cursor_stack: &mut Option<ItemStack>,
    ui_drag: &mut UiDragState,
) -> (bool, Vec<ItemStack>) {
    let mut close_clicked = false;
    let mut spill_items = Vec::new();
    ui_drag.begin_frame();

    if ui_cursor_stack.is_none() {
        ui_drag.reset();
    } else if let Some(button) = ui_drag.active_button {
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let secondary_down = ctx.input(|i| i.pointer.secondary_down());
        match button {
            UiDragButton::Primary if !primary_down => {
                let visited = std::mem::take(&mut ui_drag.visited);
                let mut dummy_personal_grid: [[Option<ItemStack>; 2]; 2] = Default::default();
                apply_primary_drag_distribution(
                    ui_cursor_stack,
                    &visited,
                    hotbar,
                    main_inventory,
                    &mut dummy_personal_grid,
                    crafting_grid,
                );
                ui_drag.finish_drag();
            }
            UiDragButton::Secondary if !secondary_down => {
                ui_drag.finish_drag();
            }
            _ => {}
        }
    }

    // Check for matching recipe
    let recipe_match = match_crafting_recipe(crafting_grid, CraftingGridSize::ThreeByThree);

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
            ui.set_min_width(640.0);

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

            ui.horizontal(|ui| {
                ui.label("Cursor:");
                render_crafting_slot(ui, ui_cursor_stack.as_ref());
                ui.label(
                    egui::RichText::new("Left click: pick/place. Right click: split/place one.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
            });

            ui.separator();

            ui.horizontal(|ui| {
                let cursor_empty = ui_cursor_stack.is_none();
                let grid_empty = crafting_grid_is_empty(crafting_grid);

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Recipe Book").strong());
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Fill requires empty cursor + grid.")
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );

                    ui.add_space(6.0);

                    let recipes = get_crafting_recipes();
                    egui::ScrollArea::vertical()
                        .max_height(210.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for recipe in recipes.iter() {
                                let crafts = crafting_max_crafts_in_storage(
                                    hotbar,
                                    main_inventory,
                                    &recipe.inputs,
                                );
                                let craftable = crafts > 0;
                                ui.horizontal(|ui| {
                                    let label =
                                        format!("{:?} x{}", recipe.output, recipe.output_count);
                                    ui.label(egui::RichText::new(label).color(if craftable {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::DARK_GRAY
                                    }));

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let enabled = craftable && cursor_empty && grid_empty;
                                            if ui
                                                .add_enabled(enabled, egui::Button::new("Fill"))
                                                .clicked()
                                            {
                                                let _ = try_autofill_crafting_grid(
                                                    crafting_grid,
                                                    hotbar,
                                                    main_inventory,
                                                    recipe,
                                                );
                                            }
                                        },
                                    );
                                });
                                ui.add_space(2.0);
                            }
                        });
                });

                ui.add_space(18.0);
                ui.separator();
                ui.add_space(18.0);

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // 3x3 Crafting grid with clickable slots
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Crafting Grid");
                                if ui
                                    .add_enabled(cursor_empty, egui::Button::new("Clear"))
                                    .clicked()
                                {
                                    clear_crafting_grid_to_storage(
                                        crafting_grid,
                                        hotbar,
                                        main_inventory,
                                        &mut spill_items,
                                    );
                                }
                            });
                            #[allow(clippy::needless_range_loop)]
                            for row_idx in 0..3 {
                                ui.horizontal(|ui| {
                                    #[allow(clippy::needless_range_loop)]
                                    for col_idx in 0..3 {
                                        render_core_slot_interactive_shift_moves_to_storage(
                                            ui,
                                            UiCoreSlotId::CraftingGrid(row_idx * 3 + col_idx),
                                            &mut crafting_grid[row_idx][col_idx],
                                            ui_cursor_stack,
                                            (&mut *hotbar, &mut *main_inventory),
                                            ui_drag,
                                            UiSlotVisual::new(40.0, false),
                                        );
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
                            if let Some(recipe) = recipe_match.as_ref() {
                                let result_stack =
                                    ItemStack::new(recipe.output, recipe.output_count);
                                let response = render_crafting_output_slot_interactive(
                                    ui,
                                    Some(&result_stack),
                                    "Click to craft\nShift-click: craft all to inventory",
                                );
                                let clicked = response.clicked_by(egui::PointerButton::Primary)
                                    || response.clicked_by(egui::PointerButton::Secondary);
                                if clicked {
                                    let shift = ui.input(|i| i.modifiers.shift);
                                    if shift {
                                        let crafts = crafting_max_crafts_3x3(crafting_grid, recipe);
                                        let mut crafted = 0_u32;
                                        for _ in 0..crafts {
                                            if consume_crafting_inputs_3x3(crafting_grid, recipe) {
                                                crafted += 1;
                                            } else {
                                                break;
                                            }
                                        }

                                        if crafted > 0 {
                                            let total = recipe.output_count.saturating_mul(crafted);
                                            let output_stack = ItemStack::new(recipe.output, total);
                                            if let Some(remainder) = add_stack_to_storage(
                                                hotbar,
                                                main_inventory,
                                                output_stack,
                                            ) {
                                                spill_items.push(remainder);
                                            }
                                        }
                                    } else if cursor_can_accept_full_stack(
                                        ui_cursor_stack,
                                        &result_stack,
                                    ) && consume_crafting_inputs_3x3(
                                        crafting_grid,
                                        recipe,
                                    ) {
                                        cursor_add_full_stack(ui_cursor_stack, result_stack);
                                    }
                                }
                            } else {
                                render_crafting_output_slot_interactive(ui, None, "No recipe");
                                ui.label(
                                    egui::RichText::new("No recipe")
                                        .size(10.0)
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                    });
                });
            });

            ui.add_space(10.0);
            ui.separator();
            render_player_storage(ui, hotbar, main_inventory, ui_cursor_stack, ui_drag);
        });

    (close_clicked, spill_items)
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
                    mdminecraft_core::ItemType::Potion(id) => format!("P{}", id),
                    mdminecraft_core::ItemType::SplashPotion(id) => format!("SP{}", id),
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

fn render_crafting_output_slot_interactive(
    ui: &mut egui::Ui,
    item: Option<&ItemStack>,
    hover_text: &str,
) -> egui::Response {
    let mut response = ui.allocate_response(egui::vec2(40.0, 40.0), egui::Sense::click());
    let rect = response.rect;

    let fill = egui::Color32::from_rgba_unmultiplied(60, 60, 60, 200);
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter()
        .rect_stroke(rect, 0.0, egui::Stroke::new(1.0, egui::Color32::GRAY));

    ui.allocate_ui_at_rect(rect.shrink(4.0), |ui| {
        if let Some(stack) = item {
            ui.vertical_centered(|ui| {
                let name = match stack.item_type {
                    mdminecraft_core::ItemType::Tool(tool, _) => format!("{:?}", tool),
                    mdminecraft_core::ItemType::Block(id) => format!("B{}", id),
                    mdminecraft_core::ItemType::Food(food) => format!("{:?}", food),
                    mdminecraft_core::ItemType::Potion(id) => format!("P{}", id),
                    mdminecraft_core::ItemType::SplashPotion(id) => format!("SP{}", id),
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

    if let Some(stack) = item {
        let mut tooltip = format!("{:?}\nCount: {}", stack.item_type, stack.count);
        if let (Some(current), Some(max)) = (stack.durability, stack.max_durability()) {
            paint_durability_bar(ui, rect, current, max);
            tooltip.push_str(&format!("\nDurability: {}/{}", current, max));
        }
        let enchants = stack.get_enchantments();
        if !enchants.is_empty() {
            tooltip.push_str("\nEnchantments:");
            for enchant in enchants {
                tooltip.push_str(&format!(
                    "\n- {:?} {}",
                    enchant.enchantment_type, enchant.level
                ));
            }
        }
        tooltip.push_str(&format!("\n\n{}", hover_text));
        response = response.on_hover_text(tooltip);
    } else {
        response = response.on_hover_text(hover_text);
    }

    response
}

/// Render the chest UI
/// Returns true if the close button was clicked
fn render_chest(
    ctx: &egui::Context,
    chest: &mut ChestState,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    ui_cursor_stack: &mut Option<ItemStack>,
    ui_drag: &mut UiDragState,
) -> bool {
    let mut close_clicked = false;
    ui_drag.begin_frame();

    if ui_cursor_stack.is_none() {
        ui_drag.reset();
    } else if let Some(button) = ui_drag.active_button {
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let secondary_down = ctx.input(|i| i.pointer.secondary_down());
        match button {
            UiDragButton::Primary if !primary_down => {
                let visited = std::mem::take(&mut ui_drag.visited);
                apply_primary_drag_distribution_with_chest(
                    ui_cursor_stack,
                    &visited,
                    hotbar,
                    main_inventory,
                    chest,
                );
                ui_drag.finish_drag();
            }
            UiDragButton::Secondary if !secondary_down => {
                ui_drag.finish_drag();
            }
            _ => {}
        }
    }

    // Semi-transparent dark overlay
    egui::Area::new(egui::Id::new("chest_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    // Chest window
    egui::Window::new("Chest")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(420.0);

            // Close button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Chest").size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        close_clicked = true;
                    }
                });
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Cursor:");
                render_crafting_slot(ui, ui_cursor_stack.as_ref());
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Shift: quick-move. Drag: distribute.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
            });

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Chest")
                    .size(12.0)
                    .color(egui::Color32::GRAY),
            );
            for row in 0..3 {
                ui.horizontal(|ui| {
                    for col in 0..9 {
                        let slot_idx = row * 9 + col;
                        render_core_slot_interactive_shift_moves_to_storage(
                            ui,
                            UiCoreSlotId::Chest(slot_idx),
                            &mut chest.slots[slot_idx],
                            ui_cursor_stack,
                            (hotbar, main_inventory),
                            ui_drag,
                            UiSlotVisual::new(36.0, false),
                        );
                    }
                });
            }

            ui.add_space(10.0);
            ui.separator();
            render_player_storage_for_chest(
                ui,
                chest,
                hotbar,
                main_inventory,
                ui_cursor_stack,
                ui_drag,
            );

            ui.add_space(5.0);
            ui.label(
                egui::RichText::new("Escape or X to close")
                    .size(11.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        });

    close_clicked
}

/// Render the furnace UI
/// Returns true if the close button was clicked
fn render_furnace(
    ctx: &egui::Context,
    furnace: &mut FurnaceState,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    ui_cursor_stack: &mut Option<ItemStack>,
    ui_drag: &mut UiDragState,
) -> bool {
    let mut close_clicked = false;
    ui_drag.begin_frame();

    if ui_cursor_stack.is_none() {
        ui_drag.reset();
    } else if let Some(button) = ui_drag.active_button {
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let secondary_down = ctx.input(|i| i.pointer.secondary_down());
        match button {
            UiDragButton::Primary if !primary_down => {
                let visited = std::mem::take(&mut ui_drag.visited);
                let mut dummy_personal_grid: [[Option<ItemStack>; 2]; 2] = Default::default();
                let mut dummy_crafting_grid: [[Option<ItemStack>; 3]; 3] = Default::default();
                apply_primary_drag_distribution(
                    ui_cursor_stack,
                    &visited,
                    hotbar,
                    main_inventory,
                    &mut dummy_personal_grid,
                    &mut dummy_crafting_grid,
                );
                ui_drag.finish_drag();
            }
            UiDragButton::Secondary if !secondary_down => {
                ui_drag.finish_drag();
            }
            _ => {}
        }
    }

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
            ui.set_min_width(360.0);

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
                ui.label("Cursor:");
                render_crafting_slot(ui, ui_cursor_stack.as_ref());
            });
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                // Input and Fuel slots column
                ui.vertical(|ui| {
                    ui.label("Input (smeltable)");
                    render_furnace_slot_interactive(
                        ui,
                        &mut furnace.input,
                        ui_cursor_stack,
                        hotbar,
                        main_inventory,
                        FurnaceSlotKind::Input,
                    );

                    ui.add_space(10.0);

                    ui.label("Fuel");
                    render_furnace_slot_interactive(
                        ui,
                        &mut furnace.fuel,
                        ui_cursor_stack,
                        hotbar,
                        main_inventory,
                        FurnaceSlotKind::Fuel,
                    );
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
                    render_furnace_slot_interactive(
                        ui,
                        &mut furnace.output,
                        ui_cursor_stack,
                        hotbar,
                        main_inventory,
                        FurnaceSlotKind::Output,
                    );
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
            ui.separator();
            render_player_storage_for_furnace(
                ui,
                furnace,
                hotbar,
                main_inventory,
                ui_cursor_stack,
                ui_drag,
            );

            ui.label(
                egui::RichText::new("Escape or X to close")
                    .size(11.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        });

    close_clicked
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FurnaceSlotKind {
    Input,
    Fuel,
    Output,
}

fn furnace_slot_accepts_item(kind: FurnaceSlotKind, item: DroppedItemType) -> bool {
    match kind {
        FurnaceSlotKind::Input => mdminecraft_world::get_smelt_output(item).is_some(),
        FurnaceSlotKind::Fuel => mdminecraft_world::is_fuel(item),
        FurnaceSlotKind::Output => false,
    }
}

fn apply_furnace_slot_click(
    slot: &mut Option<(DroppedItemType, u32)>,
    cursor: &mut Option<ItemStack>,
    kind: FurnaceSlotKind,
    click: UiSlotClick,
) {
    let Some((slot_drop_type, slot_count)) = slot.take() else {
        if kind == FurnaceSlotKind::Output {
            return;
        }

        let Some(mut cursor_stack) = cursor.take() else {
            return;
        };

        let Some(cursor_drop_type) =
            GameWorld::convert_core_item_type_to_dropped(cursor_stack.item_type)
        else {
            *cursor = Some(cursor_stack);
            return;
        };

        if !furnace_slot_accepts_item(kind, cursor_drop_type) {
            *cursor = Some(cursor_stack);
            return;
        }

        let max = cursor_drop_type.max_stack_size();
        let to_place = match click {
            UiSlotClick::Primary => cursor_stack.count.min(max),
            UiSlotClick::Secondary => 1.min(cursor_stack.count).min(max),
        };
        if to_place == 0 {
            *cursor = Some(cursor_stack);
            return;
        }

        *slot = Some((cursor_drop_type, to_place));
        cursor_stack.count -= to_place;
        if cursor_stack.count > 0 {
            *cursor = Some(cursor_stack);
        }
        return;
    };

    // Slot contains an item.
    let Some(core_item_type) = GameWorld::convert_dropped_item_type(slot_drop_type) else {
        *slot = Some((slot_drop_type, slot_count));
        return;
    };

    let Some(mut cursor_stack) = cursor.take() else {
        let take = match click {
            UiSlotClick::Primary => slot_count,
            UiSlotClick::Secondary => slot_count.div_ceil(2),
        };
        if take == 0 {
            *slot = Some((slot_drop_type, slot_count));
            return;
        }

        *cursor = Some(ItemStack::new(core_item_type, take));
        let remaining = slot_count - take;
        if remaining > 0 {
            *slot = Some((slot_drop_type, remaining));
        }
        return;
    };

    if kind == FurnaceSlotKind::Output {
        if cursor_stack.item_type != core_item_type {
            *cursor = Some(cursor_stack);
            *slot = Some((slot_drop_type, slot_count));
            return;
        }

        let max = cursor_stack.max_stack_size();
        if cursor_stack.count >= max {
            *cursor = Some(cursor_stack);
            *slot = Some((slot_drop_type, slot_count));
            return;
        }

        let space = max - cursor_stack.count;
        let to_take = match click {
            UiSlotClick::Primary => space.min(slot_count),
            UiSlotClick::Secondary => 1.min(space).min(slot_count),
        };
        if to_take == 0 {
            *cursor = Some(cursor_stack);
            *slot = Some((slot_drop_type, slot_count));
            return;
        }

        cursor_stack.count += to_take;
        *cursor = Some(cursor_stack);

        let remaining = slot_count - to_take;
        if remaining > 0 {
            *slot = Some((slot_drop_type, remaining));
        }
        return;
    }

    let Some(cursor_drop_type) =
        GameWorld::convert_core_item_type_to_dropped(cursor_stack.item_type)
    else {
        *cursor = Some(cursor_stack);
        *slot = Some((slot_drop_type, slot_count));
        return;
    };

    if !furnace_slot_accepts_item(kind, cursor_drop_type) {
        *cursor = Some(cursor_stack);
        *slot = Some((slot_drop_type, slot_count));
        return;
    }

    if cursor_drop_type == slot_drop_type {
        let max = slot_drop_type.max_stack_size();
        if slot_count >= max {
            *cursor = Some(cursor_stack);
            *slot = Some((slot_drop_type, slot_count));
            return;
        }

        let space = max - slot_count;
        let to_move = match click {
            UiSlotClick::Primary => space.min(cursor_stack.count),
            UiSlotClick::Secondary => 1.min(space).min(cursor_stack.count),
        };
        if to_move == 0 {
            *cursor = Some(cursor_stack);
            *slot = Some((slot_drop_type, slot_count));
            return;
        }

        *slot = Some((slot_drop_type, slot_count + to_move));
        cursor_stack.count -= to_move;
        if cursor_stack.count > 0 {
            *cursor = Some(cursor_stack);
        }
        return;
    }

    // Different item types: left-click swaps if the cursor stack fits entirely.
    if click == UiSlotClick::Secondary {
        *cursor = Some(cursor_stack);
        *slot = Some((slot_drop_type, slot_count));
        return;
    }

    if cursor_stack.count > cursor_drop_type.max_stack_size() {
        *cursor = Some(cursor_stack);
        *slot = Some((slot_drop_type, slot_count));
        return;
    }

    *slot = Some((cursor_drop_type, cursor_stack.count));
    *cursor = Some(ItemStack::new(core_item_type, slot_count));
}

fn render_furnace_slot_interactive(
    ui: &mut egui::Ui,
    slot: &mut Option<(DroppedItemType, u32)>,
    cursor: &mut Option<ItemStack>,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    kind: FurnaceSlotKind,
) {
    let mut response = ui.allocate_response(egui::vec2(48.0, 48.0), egui::Sense::click());
    let rect = response.rect;
    let fill = if slot.is_some() {
        egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200)
    } else {
        egui::Color32::from_rgba_unmultiplied(40, 40, 40, 180)
    };
    ui.painter()
        .rect(rect, 2.0, fill, egui::Stroke::new(1.0, egui::Color32::GRAY));

    if let Some((item_type, count)) = slot.as_ref() {
        let name = format!("{:?}", item_type);
        let display_name = if name.len() > 6 { &name[..6] } else { &name };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            display_name,
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );
        if *count > 1 {
            ui.painter().text(
                rect.right_bottom() - egui::vec2(4.0, 4.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{}", count),
                egui::FontId::proportional(9.0),
                egui::Color32::YELLOW,
            );
        }

        let mut tooltip = format!("{:?}\nCount: {}", item_type, count);
        if kind == FurnaceSlotKind::Input {
            tooltip.push_str("\nSmeltable");
        } else if kind == FurnaceSlotKind::Fuel {
            tooltip.push_str("\nFuel");
        } else {
            tooltip.push_str("\nOutput");
        }
        response = response.on_hover_text(tooltip);
    } else {
        response = response.on_hover_text(match kind {
            FurnaceSlotKind::Input => "Input (smeltable)",
            FurnaceSlotKind::Fuel => "Fuel",
            FurnaceSlotKind::Output => "Output",
        });
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };

    if let Some(click) = click {
        let shift = ui.input(|i| i.modifiers.shift);
        if shift {
            let Some((item_type, count)) = slot.take() else {
                return;
            };

            let Some(core_item_type) = GameWorld::convert_dropped_item_type(item_type) else {
                *slot = Some((item_type, count));
                return;
            };

            let stack = ItemStack::new(core_item_type, count);
            if let Some(remainder) = add_stack_to_storage(hotbar, main_inventory, stack) {
                *slot = Some((item_type, remainder.count));
            }
            return;
        }

        apply_furnace_slot_click(slot, cursor, kind, click);
    }
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
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    ui_cursor_stack: &mut Option<ItemStack>,
    ui_drag: &mut UiDragState,
) -> EnchantingResult {
    use mdminecraft_world::LAPIS_COSTS;

    let mut result = EnchantingResult {
        close_requested: false,
        enchantment_applied: None,
        xp_to_consume: 0,
    };
    ui_drag.begin_frame();

    if ui_cursor_stack.is_none() {
        ui_drag.reset();
    } else if let Some(button) = ui_drag.active_button {
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let secondary_down = ctx.input(|i| i.pointer.secondary_down());
        match button {
            UiDragButton::Primary if !primary_down => {
                let visited = std::mem::take(&mut ui_drag.visited);
                let mut dummy_personal_grid: [[Option<ItemStack>; 2]; 2] = Default::default();
                let mut dummy_crafting_grid: [[Option<ItemStack>; 3]; 3] = Default::default();
                apply_primary_drag_distribution(
                    ui_cursor_stack,
                    &visited,
                    hotbar,
                    main_inventory,
                    &mut dummy_personal_grid,
                    &mut dummy_crafting_grid,
                );
                ui_drag.finish_drag();
            }
            UiDragButton::Secondary if !secondary_down => {
                ui_drag.finish_drag();
            }
            _ => {}
        }
    }

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

            let selected_item = hotbar.selected_item();
            let selected_item_enchantable = selected_item
                .map(|item| item.is_enchantable())
                .unwrap_or(false);
            let selected_item_id = selected_item
                .and_then(core_item_to_enchanting_id)
                .filter(|_| selected_item_enchantable);

            // Keep the table's internal "preview item" in sync with the selected hotbar tool.
            let current_item_id = table.item.map(|(id, _)| id);
            if selected_item_id != current_item_id {
                let _ = table.take_item();
                if let Some(id) = selected_item_id {
                    let _ = table.add_item(id, 1);
                }
            }

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
                ui.label("Lapis:");
                render_enchanting_lapis_slot(ui, &mut table.lapis_count, ui_cursor_stack);
                ui.add_space(20.0);
                ui.label(format!("Your Level: {}", player_xp.level));
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Cursor:");
                render_crafting_slot(ui, ui_cursor_stack.as_ref());
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
            ui.label(
                egui::RichText::new("Enchantment Options:")
                    .size(14.0)
                    .strong(),
            );
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
                    let bookshelf_count = table.bookshelf_count;
                    table.set_bookshelf_count(bookshelf_count);
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

            render_player_storage_for_enchanting_table(
                ui,
                table,
                hotbar,
                main_inventory,
                ui_cursor_stack,
                ui_drag,
            );

            ui.label(
                egui::RichText::new("Escape or X to close")
                    .size(11.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        });

    result
}

fn core_item_to_enchanting_id(stack: &ItemStack) -> Option<u16> {
    match stack.item_type {
        ItemType::Tool(tool, material) => {
            let tool_index: u16 = match tool {
                ToolType::Pickaxe => 0,
                ToolType::Axe => 1,
                ToolType::Shovel => 2,
                ToolType::Hoe => 3,
                ToolType::Sword => 4,
            };
            let material_index = material as u16;
            Some(
                mdminecraft_world::TOOL_ID_START
                    .saturating_add(tool_index.saturating_mul(5))
                    .saturating_add(material_index),
            )
        }
        ItemType::Item(1) => Some(mdminecraft_world::BOW_ID),
        _ => None,
    }
}

fn render_enchanting_lapis_slot(
    ui: &mut egui::Ui,
    lapis_count: &mut u32,
    cursor: &mut Option<ItemStack>,
) {
    let mut response = ui.allocate_response(egui::vec2(48.0, 48.0), egui::Sense::click());
    let rect = response.rect;
    let fill = if *lapis_count > 0 {
        egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200)
    } else {
        egui::Color32::from_rgba_unmultiplied(40, 40, 40, 180)
    };
    ui.painter()
        .rect(rect, 2.0, fill, egui::Stroke::new(1.0, egui::Color32::GRAY));

    if *lapis_count > 0 {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Lapis",
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );
        if *lapis_count > 1 {
            ui.painter().text(
                rect.right_bottom() - egui::vec2(4.0, 4.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{}", lapis_count),
                egui::FontId::proportional(9.0),
                egui::Color32::YELLOW,
            );
        }
        response = response.on_hover_text(format!("Lapis Lazuli\nCount: {}", lapis_count));
    } else {
        response = response.on_hover_text("Lapis Lazuli");
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(UiSlotClick::Primary)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(UiSlotClick::Secondary)
    } else {
        None
    };
    let Some(click) = click else {
        return;
    };

    let Some(mut cursor_stack) = cursor.take() else {
        if *lapis_count == 0 {
            return;
        }
        let take = match click {
            UiSlotClick::Primary => *lapis_count,
            UiSlotClick::Secondary => lapis_count.div_ceil(2),
        };
        if take == 0 {
            return;
        }
        *lapis_count -= take;
        *cursor = Some(ItemStack::new(ItemType::Item(15), take));
        return;
    };

    if cursor_stack.item_type != ItemType::Item(15) {
        *cursor = Some(cursor_stack);
        return;
    }

    let to_add = match click {
        UiSlotClick::Primary => cursor_stack.count,
        UiSlotClick::Secondary => 1.min(cursor_stack.count),
    };
    if to_add == 0 {
        *cursor = Some(cursor_stack);
        return;
    }

    let max = 64_u32;
    let space = max.saturating_sub(*lapis_count);
    let added = to_add.min(space);
    if added == 0 {
        *cursor = Some(cursor_stack);
        return;
    }

    *lapis_count += added;
    cursor_stack.count -= added;
    if cursor_stack.count > 0 {
        *cursor = Some(cursor_stack);
    }
}

/// Render the brewing stand UI
/// Returns true if close was requested
fn render_brewing_stand(
    ctx: &egui::Context,
    stand: &mut BrewingStandState,
    hotbar: &mut Hotbar,
    main_inventory: &mut MainInventory,
    ui_cursor_stack: &mut Option<ItemStack>,
    ui_drag: &mut UiDragState,
) -> bool {
    let mut close_clicked = false;
    ui_drag.begin_frame();

    if ui_cursor_stack.is_none() {
        ui_drag.reset();
    } else if let Some(button) = ui_drag.active_button {
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let secondary_down = ctx.input(|i| i.pointer.secondary_down());
        match button {
            UiDragButton::Primary if !primary_down => {
                let visited = std::mem::take(&mut ui_drag.visited);
                let mut dummy_personal_grid: [[Option<ItemStack>; 2]; 2] = Default::default();
                let mut dummy_crafting_grid: [[Option<ItemStack>; 3]; 3] = Default::default();
                apply_primary_drag_distribution(
                    ui_cursor_stack,
                    &visited,
                    hotbar,
                    main_inventory,
                    &mut dummy_personal_grid,
                    &mut dummy_crafting_grid,
                );
                ui_drag.finish_drag();
            }
            UiDragButton::Secondary if !secondary_down => {
                ui_drag.finish_drag();
            }
            _ => {}
        }
    }

    // Semi-transparent dark overlay
    egui::Area::new(egui::Id::new("brewing_overlay"))
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    // Brewing stand window
    egui::Window::new("Brewing Stand")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(420.0);

            // Close button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Brewing Stand").size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        close_clicked = true;
                    }
                });
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Cursor:");
                render_crafting_slot(ui, ui_cursor_stack.as_ref());
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Left: pick/place/swap, Right: split/place one.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
            });

            ui.add_space(10.0);

            // Main brewing layout
            ui.horizontal(|ui| {
                // Left side: Fuel + Ingredient
                ui.vertical(|ui| {
                    ui.label("Fuel (Blaze Powder)");
                    let mut fuel_stack = if stand.fuel > 0 {
                        Some(ItemStack::new(
                            ItemType::Item(CORE_ITEM_BLAZE_POWDER),
                            stand.fuel,
                        ))
                    } else {
                        None
                    };
                    let response = render_core_slot_visual(ui, &fuel_stack, 48.0, false)
                        .on_hover_text("Fuel: Blaze Powder");
                    let click = if response.clicked_by(egui::PointerButton::Primary) {
                        Some(UiSlotClick::Primary)
                    } else if response.clicked_by(egui::PointerButton::Secondary) {
                        Some(UiSlotClick::Secondary)
                    } else {
                        None
                    };
                    if let Some(click) = click {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift {
                            if let Some(stack) = fuel_stack.take() {
                                if let Some(remainder) =
                                    add_stack_to_storage(hotbar, main_inventory, stack)
                                {
                                    fuel_stack = Some(remainder);
                                }
                            }
                        } else if ui_cursor_stack.as_ref().is_some_and(|stack| {
                            stack.item_type != ItemType::Item(CORE_ITEM_BLAZE_POWDER)
                        }) {
                            // Fuel slot only accepts blaze powder.
                        } else {
                            apply_slot_click(&mut fuel_stack, ui_cursor_stack, click);
                        }
                    }
                    stand.fuel = fuel_stack.as_ref().map(|s| s.count).unwrap_or(0);

                    ui.add_space(10.0);

                    ui.label("Ingredient");
                    let mut ingredient_stack =
                        stand.ingredient.and_then(|(ingredient_id, count)| {
                            brew_ingredient_id_to_core_item_type(ingredient_id)
                                .map(|item_type| ItemStack::new(item_type, count))
                        });
                    let ingredient_hover = stand
                        .ingredient
                        .map(|(id, count)| format!("{} x{}", brew_ingredient_label(id), count))
                        .unwrap_or_else(|| "Empty".to_string());
                    let response = render_core_slot_visual(ui, &ingredient_stack, 48.0, false)
                        .on_hover_text(ingredient_hover);
                    let click = if response.clicked_by(egui::PointerButton::Primary) {
                        Some(UiSlotClick::Primary)
                    } else if response.clicked_by(egui::PointerButton::Secondary) {
                        Some(UiSlotClick::Secondary)
                    } else {
                        None
                    };
                    if let Some(click) = click {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift {
                            if let Some(stack) = ingredient_stack.take() {
                                if let Some(remainder) =
                                    add_stack_to_storage(hotbar, main_inventory, stack)
                                {
                                    ingredient_stack = Some(remainder);
                                }
                            }
                        } else if ui_cursor_stack.as_ref().is_some_and(|stack| {
                            core_item_type_to_brew_ingredient_id(stack.item_type).is_none()
                        }) {
                            // Ingredient slot only accepts valid brewing ingredients.
                        } else {
                            apply_slot_click(&mut ingredient_stack, ui_cursor_stack, click);
                        }
                    }
                    stand.ingredient = ingredient_stack.and_then(|stack| {
                        core_item_type_to_brew_ingredient_id(stack.item_type)
                            .map(|ingredient_id| (ingredient_id, stack.count))
                    });
                });

                ui.add_space(20.0);

                // Middle: Progress arrow
                ui.vertical(|ui| {
                    ui.add_space(15.0);

                    // Progress bar
                    let progress_bar = egui::ProgressBar::new(stand.brew_progress)
                        .desired_width(60.0)
                        .text(format!("{:.0}%", stand.brew_progress * 100.0));
                    ui.add(progress_bar);

                    ui.add_space(5.0);

                    // Brewing icon
                    let brew_color = if stand.is_brewing {
                        egui::Color32::from_rgb(100, 200, 255)
                    } else {
                        egui::Color32::DARK_GRAY
                    };
                    ui.label(egui::RichText::new("⚗").size(24.0).color(brew_color));
                });

                ui.add_space(20.0);

                // Right side: Bottle slots (3 bottles in a row)
                ui.vertical(|ui| {
                    ui.label("Bottles");
                    ui.horizontal(|ui| {
                        let (bottles, bottle_is_splash) =
                            (&mut stand.bottles, &mut stand.bottle_is_splash);
                        for (i, bottle) in bottles.iter_mut().enumerate() {
                            let before = *bottle;
                            let before_splash = bottle_is_splash[i];
                            let mut bottle_stack = (*bottle)
                                .map(|potion| bottle_to_core_item_stack(potion, before_splash));
                            let response = render_core_slot_visual(ui, &bottle_stack, 52.0, false);
                            let click = if response.clicked_by(egui::PointerButton::Primary) {
                                Some(UiSlotClick::Primary)
                            } else if response.clicked_by(egui::PointerButton::Secondary) {
                                Some(UiSlotClick::Secondary)
                            } else {
                                None
                            };
                            if let Some(click) = click {
                                let shift = ui.input(|i| i.modifiers.shift);
                                if shift {
                                    if let Some(stack) = bottle_stack.take() {
                                        if let Some(remainder) =
                                            add_stack_to_storage(hotbar, main_inventory, stack)
                                        {
                                            bottle_stack = Some(remainder);
                                        }
                                    }
                                } else {
                                    apply_brewing_bottle_slot_click(
                                        &mut bottle_stack,
                                        ui_cursor_stack,
                                        click,
                                    );
                                }
                            }

                            let mapped = bottle_stack.as_ref().and_then(core_item_stack_to_bottle);
                            match mapped {
                                Some((potion, is_splash)) => {
                                    *bottle = Some(potion);
                                    bottle_is_splash[i] = is_splash && potion != PotionType::Water;
                                }
                                None if bottle_stack.is_none() => {
                                    *bottle = None;
                                    bottle_is_splash[i] = false;
                                }
                                _ => {
                                    *bottle = before;
                                    bottle_is_splash[i] = before_splash;
                                }
                            }
                            if i < 2 {
                                ui.add_space(4.0);
                            }
                        }
                    });
                });
            });

            ui.add_space(10.0);
            ui.separator();

            // Status text
            let status = if stand.is_brewing {
                "Brewing in progress..."
            } else if stand.fuel == 0 {
                "Need blaze powder for fuel"
            } else if !stand.bottles.iter().any(|b| b.is_some()) {
                "Add potions to brew"
            } else if stand.ingredient.is_none() {
                "Add ingredient to brew"
            } else {
                "Ready to brew"
            };
            ui.label(
                egui::RichText::new(status)
                    .size(12.0)
                    .color(if stand.is_brewing {
                        egui::Color32::from_rgb(100, 200, 255)
                    } else {
                        egui::Color32::GRAY
                    }),
            );

            ui.add_space(10.0);
            ui.separator();
            render_player_storage_for_brewing_stand(
                ui,
                stand,
                hotbar,
                main_inventory,
                ui_cursor_stack,
                ui_drag,
            );

            if std::env::var("MDM_DEBUG_BREWING_QUICK_ADD").as_deref() == Ok("1") {
                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("Debug: quick add")
                        .size(12.0)
                        .color(egui::Color32::DARK_GRAY),
                );
                ui.horizontal(|ui| {
                    if ui.button("+ Water Bottle").clicked() {
                        for bottle in &mut stand.bottles {
                            if bottle.is_none() {
                                *bottle = Some(PotionType::Water);
                                break;
                            }
                        }
                    }
                    if ui.button("+ Nether Wart").clicked() {
                        stand.add_ingredient(item_ids::NETHER_WART, 1);
                    }
                    if ui.button("+ Fuel").clicked() {
                        stand.add_fuel(1);
                    }
                });
            }

            ui.label(
                egui::RichText::new("Escape or X to close")
                    .size(11.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        });

    close_clicked
}

fn apply_brewing_bottle_slot_click(
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    _click: UiSlotClick,
) {
    let is_bottle = |stack: &ItemStack| -> bool { core_item_stack_to_bottle(stack).is_some() };

    if cursor.is_none() {
        *cursor = slot.take();
        return;
    }

    let Some(cursor_stack) = cursor.as_mut() else {
        return;
    };

    if !is_bottle(cursor_stack) {
        return;
    }

    if slot.is_none() {
        // Place one water bottle out of a stack; potions already have max stack size 1.
        if cursor_stack.item_type == ItemType::Item(CORE_ITEM_WATER_BOTTLE)
            && cursor_stack.count > 1
        {
            let mut placed = cursor_stack.clone();
            placed.count = 1;
            cursor_stack.count -= 1;
            if cursor_stack.count == 0 {
                *cursor = None;
            }
            *slot = Some(placed);
            return;
        }

        *slot = cursor.take();
        return;
    }

    // Merge extra water bottles into the cursor stack when possible.
    if let Some(slot_stack) = slot.as_ref() {
        if slot_stack.item_type == ItemType::Item(CORE_ITEM_WATER_BOTTLE)
            && cursor_stack.item_type == ItemType::Item(CORE_ITEM_WATER_BOTTLE)
        {
            let max = cursor_stack.max_stack_size();
            if cursor_stack.count < max {
                cursor_stack.count += 1;
                *slot = None;
            }
            return;
        }
    }

    // Only allow swapping if the cursor holds a single bottle.
    if cursor_stack.count != 1 {
        return;
    }

    std::mem::swap(slot, cursor);
}

fn armor_dropped_to_core_item_id(item_type: DroppedItemType) -> Option<u16> {
    match item_type {
        DroppedItemType::LeatherHelmet => Some(20),
        DroppedItemType::LeatherChestplate => Some(21),
        DroppedItemType::LeatherLeggings => Some(22),
        DroppedItemType::LeatherBoots => Some(23),
        DroppedItemType::IronHelmet => Some(10),
        DroppedItemType::IronChestplate => Some(11),
        DroppedItemType::IronLeggings => Some(12),
        DroppedItemType::IronBoots => Some(13),
        DroppedItemType::DiamondHelmet => Some(30),
        DroppedItemType::DiamondChestplate => Some(31),
        DroppedItemType::DiamondLeggings => Some(32),
        DroppedItemType::DiamondBoots => Some(33),
        _ => None,
    }
}

fn armor_piece_to_core_stack(piece: &ArmorPiece) -> Option<ItemStack> {
    let core_id = armor_dropped_to_core_item_id(piece.item_type)?;
    let mut stack = ItemStack::new(ItemType::Item(core_id), 1);
    stack.durability = Some(piece.durability);
    if !piece.enchantments.is_empty() {
        stack.enchantments = Some(piece.enchantments.clone());
    }
    Some(stack)
}

fn armor_piece_from_core_stack(stack: &ItemStack) -> Option<ArmorPiece> {
    if stack.count != 1 {
        return None;
    }

    let dropped_type = item_type_to_armor_dropped(stack.item_type)?;
    let enchantments = stack.enchantments.clone().unwrap_or_default();
    let mut piece = ArmorPiece::from_item_with_enchantments(dropped_type, enchantments)?;

    if let Some(durability) = stack.durability {
        piece.durability = durability.min(piece.max_durability);
    }

    Some(piece)
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
    use super::{
        add_stack_to_storage, apply_brewing_bottle_slot_click, apply_furnace_slot_click,
        apply_primary_drag_distribution, apply_slot_click, armor_piece_from_core_stack,
        armor_piece_to_core_stack, check_crafting_recipe, consume_crafting_inputs_3x3,
        core_item_to_enchanting_id, crafting_max_crafts_2x2, crafting_max_crafts_3x3,
        cursor_can_accept_full_stack, frames_to_complete, furnace_try_insert, get_crafting_recipes,
        interactive_blocks, item_ids, match_crafting_recipe, potion_ids, try_add_stack_to_cursor,
        try_autofill_crafting_grid, try_shift_move_core_stack_into_brewing_stand,
        try_shift_move_core_stack_into_chest, try_shift_move_core_stack_into_enchanting_table,
        try_shift_move_core_stack_into_furnace, ArmorPiece, ArmorSlot, BlockPropertiesRegistry,
        BrewingStandState, ChestState, Chunk, ChunkPos, CraftingGridSize, DroppedItemType,
        EnchantingTableState, Enchantment, EnchantmentType, FurnaceSlotKind, FurnaceState,
        GameWorld, Hotbar, ItemStack, ItemType, MainInventory, PlayerHealth, PlayerPhysics,
        ToolMaterial, ToolType, UiCoreSlotId, UiSlotClick, Voxel, AABB, BLOCK_BOOKSHELF,
        BLOCK_BREWING_STAND, BLOCK_BROWN_MUSHROOM, BLOCK_COBBLESTONE, BLOCK_CRAFTING_TABLE,
        BLOCK_ENCHANTING_TABLE, BLOCK_FURNACE, BLOCK_OAK_LOG, BLOCK_OAK_PLANKS, BLOCK_OBSIDIAN,
        BLOCK_SUGAR_CANE, CORE_ITEM_BLAZE_POWDER, CORE_ITEM_BOOK, CORE_ITEM_FERMENTED_SPIDER_EYE,
        CORE_ITEM_GLASS_BOTTLE, CORE_ITEM_GUNPOWDER, CORE_ITEM_MAGMA_CREAM, CORE_ITEM_NETHER_WART,
        CORE_ITEM_PAPER, CORE_ITEM_SPIDER_EYE, CORE_ITEM_SUGAR, CORE_ITEM_WATER_BOTTLE,
        CORE_ITEM_WHEAT, CORE_ITEM_WHEAT_SEEDS,
    };

    #[test]
    fn collisions_use_block_solidity_not_opacity() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_GLASS,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_WATER,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            2,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::TORCH,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let block_properties = BlockPropertiesRegistry::new();

        let glass_aabb = AABB {
            min: glam::Vec3::new(0.0, 64.0, 0.0),
            max: glam::Vec3::new(1.0, 65.0, 1.0),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &glass_aabb
        ));

        let water_aabb = AABB {
            min: glam::Vec3::new(1.0, 64.0, 0.0),
            max: glam::Vec3::new(2.0, 65.0, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &water_aabb
        ));

        let torch_aabb = AABB {
            min: glam::Vec3::new(2.0, 64.0, 0.0),
            max: glam::Vec3::new(3.0, 65.0, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &torch_aabb
        ));
    }

    #[test]
    fn torch_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            65,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::TORCH,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(0, 65, 0),
                mdminecraft_world::interactive_blocks::TORCH
            )),
            "Expected torch to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(0, 65, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Torch should be cleared when its support is removed"
        );
    }

    #[test]
    fn wall_torch_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::TORCH,
                state: mdminecraft_world::torch_wall_state(mdminecraft_world::Facing::East),
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(1, 64, 0),
                mdminecraft_world::interactive_blocks::TORCH
            )),
            "Expected wall torch to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(1, 64, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Wall torch should be cleared when its support is removed"
        );
    }

    #[test]
    fn wall_button_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::redstone_blocks::STONE_BUTTON,
                state: mdminecraft_world::wall_mount_state(mdminecraft_world::Facing::East),
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(1, 64, 0),
                mdminecraft_world::redstone_blocks::STONE_BUTTON
            )),
            "Expected wall button to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(1, 64, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Wall button should be cleared when its support is removed"
        );
    }

    #[test]
    fn ceiling_lever_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            63,
            0,
            Voxel {
                id: mdminecraft_world::redstone_blocks::LEVER,
                state: mdminecraft_world::ceiling_mount_state(),
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(0, 63, 0),
                mdminecraft_world::redstone_blocks::LEVER
            )),
            "Expected ceiling lever to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(0, 63, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Ceiling lever should be cleared when its support is removed"
        );
    }

    #[test]
    fn ladder_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::LADDER,
                state: mdminecraft_world::Facing::West.to_state(),
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(1, 64, 0),
                mdminecraft_world::interactive_blocks::LADDER
            )),
            "Expected ladder to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(1, 64, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Ladder should be cleared when its support is removed"
        );
    }

    #[test]
    fn redstone_wire_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            65,
            0,
            Voxel {
                id: mdminecraft_world::redstone_blocks::REDSTONE_WIRE,
                state: 0,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(0, 65, 0),
                mdminecraft_world::redstone_blocks::REDSTONE_WIRE
            )),
            "Expected redstone wire to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(0, 65, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Redstone wire should be cleared when its support is removed"
        );
    }

    #[test]
    fn sugar_cane_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_DIRT,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            65,
            0,
            Voxel {
                id: BLOCK_SUGAR_CANE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            66,
            0,
            Voxel {
                id: BLOCK_SUGAR_CANE,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );

        assert!(
            removed.contains(&(glam::IVec3::new(0, 65, 0), BLOCK_SUGAR_CANE)),
            "Expected sugar cane base to be removed, got: {:?}",
            removed
        );
        assert!(
            removed.contains(&(glam::IVec3::new(0, 66, 0), BLOCK_SUGAR_CANE)),
            "Expected sugar cane above to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(chunk.voxel(0, 65, 0).id, mdminecraft_world::BLOCK_AIR);
        assert_eq!(chunk.voxel(0, 66, 0).id, mdminecraft_world::BLOCK_AIR);
    }

    #[test]
    fn door_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );

        let door_state = mdminecraft_world::Facing::North.to_state();
        chunk.set_voxel(
            0,
            65,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
                state: door_state,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            66,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER,
                state: door_state,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(0, 64, 0, Voxel::default());
        }

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(0, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(0, 65, 0),
                mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER
            )),
            "Expected door lower to be removed, got: {:?}",
            removed
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(0, 66, 0),
                mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER
            )),
            "Expected door upper to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(0, 65, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Door lower should be cleared when its support is removed"
        );
        assert_eq!(
            chunk.voxel(0, 66, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Door upper should be cleared when its support is removed"
        );
    }

    #[test]
    fn bed_breaks_when_support_removed() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );

        let bed_state = mdminecraft_world::Facing::East.to_state();
        chunk.set_voxel(
            0,
            65,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::BED_FOOT,
                state: bed_state,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            1,
            65,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::BED_HEAD,
                state: bed_state,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        // Remove the support under the head.
        chunks
            .get_mut(&ChunkPos::new(0, 0))
            .expect("chunk exists")
            .set_voxel(1, 64, 0, Voxel::default());

        let removed = GameWorld::remove_unsupported_blocks(
            &mut chunks,
            &block_properties,
            [glam::IVec3::new(1, 64, 0)],
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(0, 65, 0),
                mdminecraft_world::interactive_blocks::BED_FOOT
            )),
            "Expected bed foot to be removed, got: {:?}",
            removed
        );
        assert!(
            removed.contains(&(
                glam::IVec3::new(1, 65, 0),
                mdminecraft_world::interactive_blocks::BED_HEAD
            )),
            "Expected bed head to be removed, got: {:?}",
            removed
        );

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            chunk.voxel(0, 65, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Bed foot should be cleared when its support is removed"
        );
        assert_eq!(
            chunk.voxel(1, 65, 0).id,
            mdminecraft_world::BLOCK_AIR,
            "Bed head should be cleared when its support is removed"
        );
    }

    #[test]
    fn bed_pair_removal_works_across_chunks() {
        let bed_state = mdminecraft_world::Facing::East.to_state();

        // Case 1: removing from the foot clears the head in the neighboring +X chunk.
        {
            let mut left = Chunk::new(ChunkPos::new(0, 0));
            let mut right = Chunk::new(ChunkPos::new(1, 0));

            left.set_voxel(
                15,
                65,
                0,
                Voxel {
                    id: mdminecraft_world::interactive_blocks::BED_FOOT,
                    state: bed_state,
                    ..Default::default()
                },
            );
            right.set_voxel(
                0,
                65,
                0,
                Voxel {
                    id: mdminecraft_world::interactive_blocks::BED_HEAD,
                    state: bed_state,
                    ..Default::default()
                },
            );

            let mut chunks = std::collections::HashMap::new();
            chunks.insert(ChunkPos::new(0, 0), left);
            chunks.insert(ChunkPos::new(1, 0), right);

            let removed = GameWorld::try_remove_other_bed_half(
                &mut chunks,
                glam::IVec3::new(15, 65, 0),
                mdminecraft_world::interactive_blocks::BED_FOOT,
                bed_state,
            );
            assert_eq!(removed, Some(glam::IVec3::new(16, 65, 0)));

            let right = chunks.get(&ChunkPos::new(1, 0)).unwrap();
            assert_eq!(right.voxel(0, 65, 0).id, mdminecraft_world::BLOCK_AIR);
        }

        // Case 2: removing from the head clears the foot in the neighboring -X chunk.
        {
            let mut left = Chunk::new(ChunkPos::new(0, 0));
            let mut right = Chunk::new(ChunkPos::new(1, 0));

            left.set_voxel(
                15,
                65,
                0,
                Voxel {
                    id: mdminecraft_world::interactive_blocks::BED_FOOT,
                    state: bed_state,
                    ..Default::default()
                },
            );
            right.set_voxel(
                0,
                65,
                0,
                Voxel {
                    id: mdminecraft_world::interactive_blocks::BED_HEAD,
                    state: bed_state,
                    ..Default::default()
                },
            );

            let mut chunks = std::collections::HashMap::new();
            chunks.insert(ChunkPos::new(0, 0), left);
            chunks.insert(ChunkPos::new(1, 0), right);

            let removed = GameWorld::try_remove_other_bed_half(
                &mut chunks,
                glam::IVec3::new(16, 65, 0),
                mdminecraft_world::interactive_blocks::BED_HEAD,
                bed_state,
            );
            assert_eq!(removed, Some(glam::IVec3::new(15, 65, 0)));

            let left = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert_eq!(left.voxel(15, 65, 0).id, mdminecraft_world::BLOCK_AIR);
        }
    }

    #[test]
    fn collision_shapes_respect_slabs_and_trapdoors() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::STONE_SLAB,
                state: 0,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::STONE_SLAB,
                state: 0x04, // top slab
                ..Default::default()
            },
        );
        chunk.set_voxel(
            2,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::TRAPDOOR,
                state: 0,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            3,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::TRAPDOOR,
                state: mdminecraft_world::set_trapdoor_open(0, true),
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        // Bottom slab occupies y..y+0.5.
        let above_bottom_slab = AABB {
            min: glam::Vec3::new(0.0, 64.6, 0.0),
            max: glam::Vec3::new(1.0, 64.9, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &above_bottom_slab
        ));

        // Top slab occupies y+0.5..y+1.0.
        let below_top_slab = AABB {
            min: glam::Vec3::new(1.0, 64.1, 0.0),
            max: glam::Vec3::new(2.0, 64.4, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &below_top_slab
        ));
        let inside_top_slab = AABB {
            min: glam::Vec3::new(1.0, 64.6, 0.0),
            max: glam::Vec3::new(2.0, 64.9, 1.0),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &inside_top_slab
        ));

        // Closed trapdoor occupies only the bottom plate.
        let above_trapdoor = AABB {
            min: glam::Vec3::new(2.0, 64.3, 0.0),
            max: glam::Vec3::new(3.0, 64.5, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &above_trapdoor
        ));
        let inside_trapdoor_plate = AABB {
            min: glam::Vec3::new(2.0, 64.05, 0.0),
            max: glam::Vec3::new(3.0, 64.1, 1.0),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &inside_trapdoor_plate
        ));

        // Open trapdoor becomes a thin vertical plane (like a door).
        let near_north_edge = AABB {
            min: glam::Vec3::new(3.0, 64.0, 0.0),
            max: glam::Vec3::new(4.0, 65.0, 0.1),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &near_north_edge
        ));

        let near_south_edge = AABB {
            min: glam::Vec3::new(3.0, 64.0, 0.9),
            max: glam::Vec3::new(4.0, 65.0, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &near_south_edge
        ));
    }

    #[test]
    fn door_collision_shape_is_thin_and_rotates_when_open() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        let base = mdminecraft_world::Facing::North.to_state();
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
                state: base,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        // Closed north-facing door occupies a thin slice at the north edge (low Z).
        let near_north_edge = AABB {
            min: glam::Vec3::new(0.0, 64.0, 0.0),
            max: glam::Vec3::new(1.0, 65.0, 0.1),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &near_north_edge
        ));
        let near_south_edge = AABB {
            min: glam::Vec3::new(0.0, 64.0, 0.9),
            max: glam::Vec3::new(1.0, 65.0, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &near_south_edge
        ));

        // Open door swings "left" (simplified), so it becomes a thin slice at the west edge (low X).
        chunks
            .get_mut(&ChunkPos::new(0, 0))
            .expect("chunk exists")
            .set_voxel(
                0,
                64,
                0,
                Voxel {
                    id: mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
                    state: mdminecraft_world::set_door_open(base, true),
                    ..Default::default()
                },
            );

        let near_west_edge = AABB {
            min: glam::Vec3::new(0.0, 64.0, 0.0),
            max: glam::Vec3::new(0.1, 65.0, 1.0),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &near_west_edge
        ));
        let near_east_edge = AABB {
            min: glam::Vec3::new(0.9, 64.0, 0.0),
            max: glam::Vec3::new(1.0, 65.0, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &near_east_edge
        ));
    }

    #[test]
    fn fence_collision_bounds_expand_when_connected() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_FENCE,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        let corner_inside_block = AABB {
            min: glam::Vec3::new(0.0, 64.0, 0.0),
            max: glam::Vec3::new(0.2, 65.0, 0.2),
        };
        assert!(
            !GameWorld::aabb_collides_with_world(&chunks, &block_properties, &corner_inside_block),
            "Isolated fence post should not fill the block corner"
        );

        let center_of_post = AABB {
            min: glam::Vec3::new(0.45, 64.0, 0.45),
            max: glam::Vec3::new(0.55, 65.0, 0.55),
        };
        assert!(
            GameWorld::aabb_collides_with_world(&chunks, &block_properties, &center_of_post),
            "Fence post should collide in the center"
        );

        // Add a solid neighbor to the east so the fence expands in +X.
        chunks
            .get_mut(&ChunkPos::new(0, 0))
            .expect("chunk exists")
            .set_voxel(
                1,
                64,
                0,
                Voxel {
                    id: mdminecraft_world::BLOCK_STONE,
                    ..Default::default()
                },
            );

        let east_edge_slice = AABB {
            min: glam::Vec3::new(0.9, 64.0, 0.45),
            max: glam::Vec3::new(1.0, 65.0, 0.55),
        };
        assert!(
            GameWorld::aabb_collides_with_world(&chunks, &block_properties, &east_edge_slice),
            "Fence should expand its collision bounds toward a connected neighbor"
        );
    }

    #[test]
    fn fence_collision_does_not_fill_corners_when_connected() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_FENCE,
                ..Default::default()
            },
        );

        // Connect fence to east and south.
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            64,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        let east_arm_slice = AABB {
            min: glam::Vec3::new(0.9, 64.0, 0.45),
            max: glam::Vec3::new(1.0, 65.0, 0.55),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &east_arm_slice
        ));

        let south_arm_slice = AABB {
            min: glam::Vec3::new(0.45, 64.0, 0.9),
            max: glam::Vec3::new(0.55, 65.0, 1.0),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &south_arm_slice
        ));

        // Connected fence should still leave the far corner empty (no union bbox).
        let far_corner = AABB {
            min: glam::Vec3::new(0.9, 64.0, 0.9),
            max: glam::Vec3::new(1.0, 65.0, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &far_corner
        ));
    }

    #[test]
    fn fence_gate_closed_collides_and_open_does_not() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        let base = mdminecraft_world::Facing::North.to_state();
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_FENCE_GATE,
                state: base,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        // Closed gate: thin centered plane, should collide in the middle.
        let center_probe = AABB {
            min: glam::Vec3::new(0.45, 64.0, 0.45),
            max: glam::Vec3::new(0.55, 65.0, 0.55),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &center_probe
        ));

        // Open gate: swings "left" (simplified), so center should be passable while the hinge side
        // still collides.
        chunks
            .get_mut(&ChunkPos::new(0, 0))
            .expect("chunk exists")
            .set_voxel(
                0,
                64,
                0,
                Voxel {
                    id: mdminecraft_world::interactive_blocks::OAK_FENCE_GATE,
                    state: mdminecraft_world::set_fence_gate_open(base, true),
                    ..Default::default()
                },
            );
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &center_probe
        ));

        let near_hinge_corner = AABB {
            min: glam::Vec3::new(0.0, 64.0, 0.0),
            max: glam::Vec3::new(0.1, 65.0, 0.1),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &near_hinge_corner
        ));
    }

    #[test]
    fn glass_pane_collision_is_thin() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::GLASS_PANE,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        let corner_inside_block = AABB {
            min: glam::Vec3::new(0.0, 64.0, 0.0),
            max: glam::Vec3::new(0.2, 65.0, 0.2),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &corner_inside_block
        ));

        let center_slice = AABB {
            min: glam::Vec3::new(0.45, 64.0, 0.45),
            max: glam::Vec3::new(0.55, 65.0, 0.55),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &center_slice
        ));
    }

    #[test]
    fn glass_pane_collision_connects_without_filling_corners() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::GLASS_PANE,
                ..Default::default()
            },
        );

        // Connect to east and south.
        chunk.set_voxel(
            1,
            64,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            0,
            64,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        let east_arm_slice = AABB {
            min: glam::Vec3::new(0.9, 64.0, 0.45),
            max: glam::Vec3::new(1.0, 65.0, 0.55),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &east_arm_slice
        ));

        let south_arm_slice = AABB {
            min: glam::Vec3::new(0.45, 64.0, 0.9),
            max: glam::Vec3::new(0.55, 65.0, 1.0),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &south_arm_slice
        ));

        // Even with two arms, the far corner stays empty (multi-AABB, not union bbox).
        let far_corner = AABB {
            min: glam::Vec3::new(0.9, 64.0, 0.9),
            max: glam::Vec3::new(1.0, 65.0, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &far_corner
        ));
    }

    #[test]
    fn walking_steps_up_onto_bottom_slab() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Solid floor at y=0.
        for x in 0..3 {
            chunk.set_voxel(
                x,
                0,
                0,
                Voxel {
                    id: mdminecraft_world::BLOCK_STONE,
                    ..Default::default()
                },
            );
        }

        // A bottom slab one block ahead (requires a 0.5-block step).
        chunk.set_voxel(
            1,
            1,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::STONE_SLAB,
                state: 0,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        let start_feet_y = 1.0;
        let current_aabb = AABB::from_center_size(
            glam::Vec3::new(0.5, start_feet_y + 0.9, 0.5),
            glam::Vec3::new(0.6, 1.8, 0.6),
        );

        let move_right = glam::Vec3::new(0.6, 0.0, 0.0);

        let (no_step_offset, _) = GameWorld::move_with_collision(
            &chunks,
            &block_properties,
            &current_aabb,
            move_right,
            0.0,
        );
        assert!(
            no_step_offset.x.abs() < 1e-6,
            "Without step-up, the slab should block horizontal movement"
        );

        let (step_offset, _) = GameWorld::move_with_collision(
            &chunks,
            &block_properties,
            &current_aabb,
            move_right,
            PlayerPhysics::STEP_HEIGHT,
        );
        assert!(
            step_offset.x > 0.5,
            "Step-up should allow moving onto the slab"
        );
        assert!(
            (step_offset.y - 0.5).abs() < 1e-6,
            "Stepping onto a bottom slab should raise feet by 0.5 (got {})",
            step_offset.y
        );
    }

    #[test]
    fn walking_steps_up_onto_stairs() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Solid floor at y=0.
        for x in 0..3 {
            chunk.set_voxel(
                x,
                0,
                0,
                Voxel {
                    id: mdminecraft_world::BLOCK_STONE,
                    ..Default::default()
                },
            );
        }

        // Stairs one block ahead. Collision is simplified to a 0.5-block step.
        chunk.set_voxel(
            1,
            1,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_STAIRS,
                state: mdminecraft_world::Facing::East.to_state(),
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        let start_feet_y = 1.0;
        let current_aabb = AABB::from_center_size(
            glam::Vec3::new(0.5, start_feet_y + 0.9, 0.5),
            glam::Vec3::new(0.6, 1.8, 0.6),
        );

        let move_right = glam::Vec3::new(0.6, 0.0, 0.0);

        let (step_offset, _) = GameWorld::move_with_collision(
            &chunks,
            &block_properties,
            &current_aabb,
            move_right,
            PlayerPhysics::STEP_HEIGHT,
        );
        assert!(
            step_offset.x > 0.5,
            "Step-up should allow moving onto the stairs"
        );
        assert!(
            (step_offset.y - 0.5).abs() < 1e-6,
            "Stepping onto stairs should raise feet by 0.5 (got {})",
            step_offset.y
        );
    }

    #[test]
    fn top_stairs_collision_is_inverted() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            64,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::OAK_STAIRS,
                state: mdminecraft_world::Facing::East.to_state() | 0x04,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        // Upper half is a full block for upside-down stairs.
        let upper_inside = AABB {
            min: glam::Vec3::new(0.1, 64.6, 0.1),
            max: glam::Vec3::new(0.9, 64.9, 0.9),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &upper_inside
        ));

        // Lower half only occupies the facing half-footprint (east side).
        let lower_west_clear = AABB {
            min: glam::Vec3::new(0.0, 64.1, 0.0),
            max: glam::Vec3::new(0.4, 64.4, 1.0),
        };
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &lower_west_clear
        ));

        let lower_east_solid = AABB {
            min: glam::Vec3::new(0.6, 64.1, 0.0),
            max: glam::Vec3::new(0.9, 64.4, 1.0),
        };
        assert!(GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &lower_east_solid
        ));
    }

    #[test]
    fn ladder_is_detected_without_blocking_movement() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            0,
            1,
            0,
            Voxel {
                id: mdminecraft_world::interactive_blocks::LADDER,
                state: 0,
                ..Default::default()
            },
        );

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), chunk);
        let block_properties = BlockPropertiesRegistry::new();

        let player_aabb = AABB::from_center_size(
            glam::Vec3::new(0.5, 1.0 + 0.9, 0.5),
            glam::Vec3::new(0.6, 1.8, 0.6),
        );

        assert!(GameWorld::aabb_touches_ladder(&chunks, &player_aabb));
        assert!(!GameWorld::aabb_collides_with_world(
            &chunks,
            &block_properties,
            &player_aabb
        ));
    }

    #[test]
    fn ladder_placement_state_is_opposite_face_normal() {
        let state = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::LADDER,
            0.0,
            glam::IVec3::new(-1, 0, 0),
            0.0,
        )
        .expect("ladder state should be set when placed on a wall");
        assert_eq!(state, mdminecraft_world::Facing::East.to_state());

        let state = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::LADDER,
            0.0,
            glam::IVec3::new(0, 0, 1),
            0.0,
        )
        .expect("ladder state should be set when placed on a wall");
        assert_eq!(state, mdminecraft_world::Facing::North.to_state());

        assert!(GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::LADDER,
            0.0,
            glam::IVec3::new(0, 1, 0),
            0.0,
        )
        .is_none());
    }

    #[test]
    fn torch_placement_state_allows_wall_mounts() {
        let floor = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::TORCH,
            0.0,
            glam::IVec3::new(0, 1, 0),
            0.0,
        )
        .expect("torch placement should produce a state on top faces");
        assert!(!mdminecraft_world::is_torch_wall(floor));

        let wall = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::TORCH,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.0,
        )
        .expect("torch placement should produce a state on side faces");
        assert!(mdminecraft_world::is_torch_wall(wall));
        assert_eq!(
            mdminecraft_world::torch_facing(wall),
            mdminecraft_world::Facing::East
        );

        assert!(GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::TORCH,
            0.0,
            glam::IVec3::new(0, -1, 0),
            0.0,
        )
        .is_none());
    }

    #[test]
    fn button_placement_state_allows_wall_and_ceiling_mounts() {
        let floor = GameWorld::placement_state_for_block(
            mdminecraft_world::redstone_blocks::STONE_BUTTON,
            0.0,
            glam::IVec3::new(0, 1, 0),
            0.0,
        )
        .expect("button placement should produce a state on top faces");
        assert!(!mdminecraft_world::is_wall_mounted(floor));
        assert!(!mdminecraft_world::is_ceiling_mounted(floor));

        let wall = GameWorld::placement_state_for_block(
            mdminecraft_world::redstone_blocks::STONE_BUTTON,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.0,
        )
        .expect("button placement should produce a state on side faces");
        assert!(mdminecraft_world::is_wall_mounted(wall));
        assert_eq!(
            mdminecraft_world::wall_mounted_facing(wall),
            mdminecraft_world::Facing::East
        );
        assert!(!mdminecraft_world::is_ceiling_mounted(wall));

        let ceiling = GameWorld::placement_state_for_block(
            mdminecraft_world::redstone_blocks::STONE_BUTTON,
            0.0,
            glam::IVec3::new(0, -1, 0),
            0.0,
        )
        .expect("button placement should produce a state on bottom faces");
        assert!(!mdminecraft_world::is_wall_mounted(ceiling));
        assert!(mdminecraft_world::is_ceiling_mounted(ceiling));
    }

    #[test]
    fn slab_placement_state_uses_hit_height_for_top_bottom() {
        let bottom = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::STONE_SLAB,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.25,
        )
        .expect("slab placement should always produce a state");
        assert_eq!(bottom, mdminecraft_world::SlabPosition::Bottom.to_state(0));

        let top = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::STONE_SLAB,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.75,
        )
        .expect("slab placement should always produce a state");
        assert_eq!(top, mdminecraft_world::SlabPosition::Top.to_state(0));

        // Placing against the bottom face forces a top slab.
        let forced_top = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::STONE_SLAB,
            0.0,
            glam::IVec3::new(0, -1, 0),
            0.25,
        )
        .expect("slab placement should always produce a state");
        assert_eq!(forced_top, mdminecraft_world::SlabPosition::Top.to_state(0));
    }

    #[test]
    fn trapdoor_placement_state_sets_top_bit_from_hit_height() {
        let bottom = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::TRAPDOOR,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.25,
        )
        .expect("trapdoor placement should always produce a state");
        assert!(!mdminecraft_world::is_trapdoor_open(bottom));
        assert!(!mdminecraft_world::is_trapdoor_top(bottom));

        let top = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::TRAPDOOR,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.75,
        )
        .expect("trapdoor placement should always produce a state");
        assert!(!mdminecraft_world::is_trapdoor_open(top));
        assert!(mdminecraft_world::is_trapdoor_top(top));

        // Placing against the bottom face forces a top trapdoor.
        let forced_top = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::TRAPDOOR,
            0.0,
            glam::IVec3::new(0, -1, 0),
            0.25,
        )
        .expect("trapdoor placement should always produce a state");
        assert!(mdminecraft_world::is_trapdoor_top(forced_top));
    }

    #[test]
    fn stairs_placement_state_sets_top_bit_from_hit_height() {
        let bottom = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::OAK_STAIRS,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.25,
        )
        .expect("stairs placement should always produce a state");
        assert_eq!(bottom & 0x04, 0);

        let top = GameWorld::placement_state_for_block(
            mdminecraft_world::interactive_blocks::OAK_STAIRS,
            0.0,
            glam::IVec3::new(1, 0, 0),
            0.75,
        )
        .expect("stairs placement should always produce a state");
        assert_ne!(top & 0x04, 0);
    }

    #[test]
    fn door_placement_places_upper_and_lower() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        let state = mdminecraft_world::Facing::West.to_state();
        assert!(GameWorld::try_place_door(
            &mut chunk,
            1,
            64,
            1,
            mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
            state,
        ));

        let lower = chunk.voxel(1, 64, 1);
        assert_eq!(
            lower.id,
            mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER
        );
        assert_eq!(lower.state, state);

        let upper = chunk.voxel(1, 65, 1);
        assert_eq!(
            upper.id,
            mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER
        );
        assert_eq!(upper.state, state);
    }

    #[test]
    fn door_placement_fails_when_upper_is_occupied() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.set_voxel(
            1,
            65,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_STONE,
                ..Default::default()
            },
        );

        assert!(!GameWorld::try_place_door(
            &mut chunk,
            1,
            64,
            1,
            mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
            mdminecraft_world::Facing::North.to_state(),
        ));
        assert_eq!(chunk.voxel(1, 64, 1).id, mdminecraft_world::BLOCK_AIR);
    }

    #[test]
    fn door_break_removes_the_other_half() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        let state = mdminecraft_world::Facing::North.to_state();
        assert!(GameWorld::try_place_door(
            &mut chunk,
            1,
            64,
            1,
            mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
            state,
        ));

        // Break the lower half.
        chunk.set_voxel(1, 64, 1, Voxel::default());
        assert_eq!(
            chunk.voxel(1, 65, 1).id,
            mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER
        );

        let removed = GameWorld::try_remove_other_door_half(
            &mut chunk,
            1,
            64,
            1,
            mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
        );
        assert_eq!(removed, Some(65));
        assert_eq!(chunk.voxel(1, 65, 1).id, mdminecraft_world::BLOCK_AIR);
    }

    #[test]
    fn stage3_first_night_scenario_survives_save_load() {
        let mut hotbar = Hotbar {
            slots: std::array::from_fn(|_| None),
            selected: 0,
        };
        let mut main_inventory = MainInventory::new();
        let recipes = get_crafting_recipes();
        let planks_recipe = recipes
            .iter()
            .find(|recipe| recipe.output == ItemType::Block(BLOCK_OAK_PLANKS))
            .expect("planks recipe exists");
        let crafting_table_recipe = recipes
            .iter()
            .find(|recipe| recipe.output == ItemType::Block(BLOCK_CRAFTING_TABLE))
            .expect("crafting table recipe exists");
        let sticks_recipe = recipes
            .iter()
            .find(|recipe| recipe.output == ItemType::Item(3))
            .expect("sticks recipe exists");
        let torches_recipe = recipes
            .iter()
            .find(|recipe| recipe.output == ItemType::Block(interactive_blocks::TORCH))
            .expect("torches recipe exists");

        // "Gather": give the player a couple logs + some coal.
        assert!(add_stack_to_storage(
            &mut hotbar,
            &mut main_inventory,
            ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 2),
        )
        .is_none());
        assert!(add_stack_to_storage(
            &mut hotbar,
            &mut main_inventory,
            ItemStack::new(ItemType::Item(8), 1), // coal
        )
        .is_none());

        // Craft: 2 logs -> 8 planks.
        for _ in 0..2 {
            let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
            assert!(try_autofill_crafting_grid(
                &mut grid,
                &mut hotbar,
                &mut main_inventory,
                planks_recipe,
            ));
            let recipe = match_crafting_recipe(&grid, CraftingGridSize::TwoByTwo)
                .expect("planks recipe matches");
            assert_eq!(recipe.output, ItemType::Block(BLOCK_OAK_PLANKS));
            assert!(consume_crafting_inputs_3x3(&mut grid, &recipe));
            assert!(add_stack_to_storage(
                &mut hotbar,
                &mut main_inventory,
                ItemStack::new(recipe.output, recipe.output_count),
            )
            .is_none());
        }

        // Craft: 4 planks -> crafting table.
        {
            let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
            assert!(try_autofill_crafting_grid(
                &mut grid,
                &mut hotbar,
                &mut main_inventory,
                crafting_table_recipe,
            ));
            let recipe = match_crafting_recipe(&grid, CraftingGridSize::TwoByTwo)
                .expect("table recipe matches");
            assert_eq!(recipe.output, ItemType::Block(BLOCK_CRAFTING_TABLE));
            assert!(consume_crafting_inputs_3x3(&mut grid, &recipe));
            assert!(add_stack_to_storage(
                &mut hotbar,
                &mut main_inventory,
                ItemStack::new(recipe.output, recipe.output_count),
            )
            .is_none());
        }

        // Craft: 2 planks -> 4 sticks.
        {
            let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
            assert!(try_autofill_crafting_grid(
                &mut grid,
                &mut hotbar,
                &mut main_inventory,
                sticks_recipe,
            ));
            let recipe = match_crafting_recipe(&grid, CraftingGridSize::TwoByTwo)
                .expect("sticks recipe matches");
            assert_eq!(recipe.output, ItemType::Item(3));
            assert!(consume_crafting_inputs_3x3(&mut grid, &recipe));
            assert!(add_stack_to_storage(
                &mut hotbar,
                &mut main_inventory,
                ItemStack::new(recipe.output, recipe.output_count),
            )
            .is_none());
        }

        // Craft: 1 coal + 1 stick -> 4 torches.
        {
            let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
            assert!(try_autofill_crafting_grid(
                &mut grid,
                &mut hotbar,
                &mut main_inventory,
                torches_recipe,
            ));
            let recipe = match_crafting_recipe(&grid, CraftingGridSize::TwoByTwo)
                .expect("torch recipe matches");
            assert_eq!(recipe.output, ItemType::Block(interactive_blocks::TORCH));
            assert!(consume_crafting_inputs_3x3(&mut grid, &recipe));
            assert!(add_stack_to_storage(
                &mut hotbar,
                &mut main_inventory,
                ItemStack::new(recipe.output, recipe.output_count),
            )
            .is_none());
        }

        // Survival tick: moving drains hunger.
        let mut health = PlayerHealth::new();
        health.set_active(true);
        for _ in 0..610 {
            health.update(0.05);
        }
        assert!(health.hunger < 20.0);

        // Sleep: bed sets spawn point.
        let mut bed = mdminecraft_world::BedSystem::new();
        assert_eq!(
            bed.try_sleep((10, 64, 10), true, false),
            mdminecraft_world::SleepResult::Success
        );
        let spawn = bed.spawn_point().expect("spawn point set");

        // Save world state (including player) and reload it.
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mdminecraft_stage3_first_night_{timestamp}"));
        let store = mdminecraft_world::RegionStore::new(&dir).expect("region store");

        let player_save = mdminecraft_world::PlayerSave {
            transform: mdminecraft_world::PlayerTransform {
                dimension: mdminecraft_core::DimensionId::DEFAULT,
                x: 0.0,
                y: 65.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
            },
            spawn_point: mdminecraft_world::WorldPoint {
                dimension: mdminecraft_core::DimensionId::DEFAULT,
                x: spawn.0 as f64,
                y: spawn.1 as f64,
                z: spawn.2 as f64,
            },
            hotbar: hotbar.slots.clone(),
            hotbar_selected: hotbar.selected,
            inventory: GameWorld::persisted_inventory_from_main_inventory(&main_inventory),
            health: health.current,
            hunger: health.hunger,
            xp_level: 0,
            xp_current: 0,
            xp_next_level_xp: 0,
            armor: mdminecraft_world::PlayerArmor::default(),
            status_effects: mdminecraft_world::StatusEffects::new(),
        };

        let state = mdminecraft_world::WorldState {
            tick: mdminecraft_core::SimTick::ZERO,
            sim_time: mdminecraft_world::SimTime::default(),
            weather: mdminecraft_world::WeatherToggle::default(),
            weather_next_change_tick: mdminecraft_core::SimTick::ZERO,
            player: Some(player_save.clone()),
            entities: mdminecraft_world::WorldEntitiesState::default(),
            block_entities: mdminecraft_world::BlockEntitiesState::default(),
        };
        store.save_world_state(&state).expect("save world state");
        let loaded = store.load_world_state().expect("load world state");
        let loaded_player = loaded.player.expect("player loaded");

        // Ensure core survival + inventory state survives save/load.
        assert_eq!(loaded_player.spawn_point, player_save.spawn_point);
        assert_eq!(loaded_player.hotbar, player_save.hotbar);

        let loaded_main =
            GameWorld::main_inventory_from_persisted_inventory(loaded_player.inventory.clone());
        assert_eq!(loaded_main.slots, main_inventory.slots);

        assert!((loaded_player.health - health.current).abs() < 1e-6);
        assert!((loaded_player.hunger - health.hunger).abs() < 1e-6);
    }

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

    #[test]
    fn drowning_triggers_after_air_depletes() {
        let mut health = PlayerHealth::new();

        for _ in 0..300 {
            assert!(!health.tick_air(true, false));
        }
        for _ in 0..19 {
            assert!(!health.tick_air(true, false));
        }
        assert!(health.tick_air(true, false));

        // Leaving water regenerates air and clears drowning timer.
        assert!(!health.tick_air(false, false));
        assert!(health.air_ticks > 0);

        // Water breathing keeps air full.
        assert!(!health.tick_air(true, true));
        assert_eq!(health.air_ticks, 300);
    }

    #[test]
    fn burning_triggers_periodic_damage_and_can_be_extinguished() {
        let mut health = PlayerHealth::new();
        health.ignite(40);

        let mut events = 0;
        for _ in 0..40 {
            if health.tick_burning(false, false) {
                events += 1;
            }
        }
        assert_eq!(events, 2);

        // Fire resistance suppresses damage ticks.
        health.ignite(40);
        for _ in 0..40 {
            assert!(!health.tick_burning(false, true));
        }

        // Water extinguishes.
        health.ignite(40);
        assert!(!health.tick_burning(true, false));
        assert_eq!(health.burning_ticks, 0);
    }

    #[test]
    fn crafting_log_to_planks() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 1));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(BLOCK_OAK_PLANKS), 4))
        );
    }

    #[test]
    fn ui_slot_primary_click_picks_up_places_merges_and_swaps() {
        let mut slot = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 10));
        let mut cursor = None;

        apply_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert!(slot.is_none());
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 10))
        );

        apply_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert_eq!(
            slot,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 10))
        );
        assert!(cursor.is_none());

        cursor = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 5));
        apply_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert_eq!(
            slot,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 15))
        );
        assert!(cursor.is_none());

        cursor = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 1));
        apply_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert_eq!(
            slot,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 1))
        );
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 15))
        );
    }

    #[test]
    fn ui_slot_secondary_click_splits_picks_and_places_one() {
        let mut slot = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 9));
        let mut cursor = None;

        apply_slot_click(&mut slot, &mut cursor, UiSlotClick::Secondary);
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 5))
        );
        assert_eq!(
            slot,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 4))
        );

        let mut empty_slot = None;
        apply_slot_click(&mut empty_slot, &mut cursor, UiSlotClick::Secondary);
        assert_eq!(
            empty_slot,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1))
        );
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 4))
        );

        apply_slot_click(&mut empty_slot, &mut cursor, UiSlotClick::Secondary);
        assert_eq!(
            empty_slot,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 2))
        );
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 3))
        );

        // Different stack: right click does nothing.
        let mut different_slot = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 1));
        apply_slot_click(&mut different_slot, &mut cursor, UiSlotClick::Secondary);
        assert_eq!(
            different_slot,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 1))
        );
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 3))
        );
    }

    #[test]
    fn furnace_ui_output_slot_takes_and_merges_into_cursor() {
        let mut output = Some((DroppedItemType::IronIngot, 10));
        let mut cursor = None;

        apply_furnace_slot_click(
            &mut output,
            &mut cursor,
            FurnaceSlotKind::Output,
            UiSlotClick::Secondary,
        );
        assert_eq!(output, Some((DroppedItemType::IronIngot, 5)));
        assert_eq!(cursor, Some(ItemStack::new(ItemType::Item(7), 5)));

        cursor = Some(ItemStack::new(ItemType::Item(7), 60));
        apply_furnace_slot_click(
            &mut output,
            &mut cursor,
            FurnaceSlotKind::Output,
            UiSlotClick::Primary,
        );
        assert_eq!(cursor, Some(ItemStack::new(ItemType::Item(7), 64)));
        assert_eq!(output, Some((DroppedItemType::IronIngot, 1)));
    }

    #[test]
    fn furnace_ui_input_slot_rejects_non_smeltable_and_allows_swap() {
        // Reject planks (not smeltable).
        let mut input_slot = None;
        let mut cursor = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 4));
        apply_furnace_slot_click(
            &mut input_slot,
            &mut cursor,
            FurnaceSlotKind::Input,
            UiSlotClick::Primary,
        );
        assert!(input_slot.is_none());
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 4))
        );

        // Accept cobblestone (smeltable into stone).
        cursor = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 8));
        apply_furnace_slot_click(
            &mut input_slot,
            &mut cursor,
            FurnaceSlotKind::Input,
            UiSlotClick::Primary,
        );
        assert_eq!(input_slot, Some((DroppedItemType::Cobblestone, 8)));
        assert!(cursor.is_none());

        // Swap to oak log (smeltable into coal/charcoal).
        cursor = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 3));
        apply_furnace_slot_click(
            &mut input_slot,
            &mut cursor,
            FurnaceSlotKind::Input,
            UiSlotClick::Primary,
        );
        assert_eq!(input_slot, Some((DroppedItemType::OakLog, 3)));
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 8))
        );
    }

    #[test]
    fn furnace_try_insert_merges_up_to_stack_limit() {
        let mut slot = None;

        let moved = furnace_try_insert(&mut slot, DroppedItemType::Coal, 10);
        assert_eq!(moved, 10);
        assert_eq!(slot, Some((DroppedItemType::Coal, 10)));

        let moved = furnace_try_insert(&mut slot, DroppedItemType::Coal, 60);
        assert_eq!(moved, 54);
        assert_eq!(slot, Some((DroppedItemType::Coal, 64)));
    }

    #[test]
    fn shift_move_core_stack_into_furnace_prefers_input_over_fuel() {
        let mut furnace = FurnaceState::default();
        let mut stack = ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 5);

        assert!(try_shift_move_core_stack_into_furnace(
            &mut stack,
            &mut furnace
        ));
        assert_eq!(stack.count, 0);
        assert_eq!(furnace.input, Some((DroppedItemType::OakLog, 5)));
        assert!(furnace.fuel.is_none());
    }

    #[test]
    fn shift_move_core_stack_into_furnace_falls_back_to_fuel_when_input_blocked() {
        let mut furnace = FurnaceState {
            input: Some((DroppedItemType::Cobblestone, 1)),
            ..Default::default()
        };

        let mut stack = ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 3);
        assert!(try_shift_move_core_stack_into_furnace(
            &mut stack,
            &mut furnace
        ));
        assert_eq!(stack.count, 0);
        assert_eq!(furnace.fuel, Some((DroppedItemType::OakLog, 3)));
    }

    #[test]
    fn shift_move_core_stack_into_furnace_inserts_fuel() {
        let mut furnace = FurnaceState::default();
        let mut stack = ItemStack::new(ItemType::Item(8), 10); // coal

        assert!(try_shift_move_core_stack_into_furnace(
            &mut stack,
            &mut furnace
        ));
        assert_eq!(stack.count, 0);
        assert_eq!(furnace.fuel, Some((DroppedItemType::Coal, 10)));
    }

    #[test]
    fn shift_move_core_stack_into_chest_merges_then_fills_empty_slots() {
        let mut chest = ChestState::default();
        chest.slots[0] = Some(ItemStack::new(ItemType::Item(3), 60));

        let mut stack = ItemStack::new(ItemType::Item(3), 10);
        assert!(try_shift_move_core_stack_into_chest(&mut stack, &mut chest));
        assert_eq!(stack.count, 0);
        assert_eq!(chest.slots[0].as_ref().unwrap().count, 64);
        assert_eq!(chest.slots[1].as_ref().unwrap().count, 6);
    }

    #[test]
    fn shift_move_core_stack_into_chest_returns_false_when_full() {
        let mut chest = ChestState::default();
        let full_stack = ItemStack::new(ItemType::Item(3), 64);
        for slot in &mut chest.slots {
            *slot = Some(full_stack.clone());
        }

        let mut stack = ItemStack::new(ItemType::Item(3), 1);
        assert!(!try_shift_move_core_stack_into_chest(
            &mut stack, &mut chest
        ));
        assert_eq!(stack.count, 1);
    }

    #[test]
    fn crafting_planks_to_sticks() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[1][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));

        assert_eq!(check_crafting_recipe(&grid), Some((ItemType::Item(3), 4)));
    }

    #[test]
    fn crafting_planks_to_crafting_table() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[0][1] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[1][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[1][1] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(BLOCK_CRAFTING_TABLE), 1))
        );
    }

    #[test]
    fn crafting_cobblestone_to_furnace() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for (r, row) in grid.iter_mut().enumerate() {
            for (c, slot) in row.iter_mut().enumerate() {
                if r == 1 && c == 1 {
                    continue;
                }
                *slot = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
            }
        }

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(BLOCK_FURNACE), 1))
        );
    }

    #[test]
    fn crafting_planks_to_chest() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for (r, row) in grid.iter_mut().enumerate() {
            for (c, slot) in row.iter_mut().enumerate() {
                if r == 1 && c == 1 {
                    continue;
                }
                *slot = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
            }
        }

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::CHEST), 1))
        );
    }

    #[test]
    fn crafting_planks_vertical_makes_sticks_and_horizontal_makes_pressure_plate() {
        let plank = ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1);

        // Vertical: sticks.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(plank.clone());
        grid[1][0] = Some(plank.clone());
        assert_eq!(check_crafting_recipe(&grid), Some((ItemType::Item(3), 4)));

        // Horizontal: pressure plate.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(plank.clone());
        grid[0][1] = Some(plank);
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((
                ItemType::Block(mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE),
                1
            ))
        );
    }

    #[test]
    fn crafting_buttons_lever_and_stone_pressure_plate() {
        // Oak button.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((
                ItemType::Block(mdminecraft_world::redstone_blocks::OAK_BUTTON),
                1
            ))
        );

        // Stone button.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(
            ItemType::Block(mdminecraft_world::BLOCK_STONE),
            1,
        ));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((
                ItemType::Block(mdminecraft_world::redstone_blocks::STONE_BUTTON),
                1
            ))
        );

        // Lever.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Item(3), 1));
        grid[1][0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((
                ItemType::Block(mdminecraft_world::redstone_blocks::LEVER),
                1
            ))
        );

        // Stone pressure plate.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(
            ItemType::Block(mdminecraft_world::BLOCK_STONE),
            1,
        ));
        grid[0][1] = Some(ItemStack::new(
            ItemType::Block(mdminecraft_world::BLOCK_STONE),
            1,
        ));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((
                ItemType::Block(mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE),
                1
            ))
        );
    }

    #[test]
    fn crafting_planks_to_oak_door() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for row in &mut grid {
            for slot in row.iter_mut().take(2) {
                *slot = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
            }
        }

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::OAK_DOOR_LOWER), 3))
        );
    }

    #[test]
    fn crafting_iron_ingots_to_iron_door() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for row in &mut grid {
            for slot in row.iter_mut().take(2) {
                *slot = Some(ItemStack::new(ItemType::Item(7), 1));
            }
        }

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::IRON_DOOR_LOWER), 3))
        );
    }

    #[test]
    fn crafting_planks_to_trapdoor() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for row in grid.iter_mut().take(2) {
            for slot in row {
                *slot = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
            }
        }

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::TRAPDOOR), 2))
        );
    }

    #[test]
    fn crafting_planks_and_sticks_to_oak_fence_and_gate() {
        let plank = ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1);
        let stick = ItemStack::new(ItemType::Item(3), 1);

        // Fence: P S P / P S P
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for row in grid.iter_mut().take(2) {
            row[0] = Some(plank.clone());
            row[1] = Some(stick.clone());
            row[2] = Some(plank.clone());
        }
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::OAK_FENCE), 3))
        );

        // Gate: S P S / S P S
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for row in grid.iter_mut().take(2) {
            row[0] = Some(stick.clone());
            row[1] = Some(plank.clone());
            row[2] = Some(stick.clone());
        }
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::OAK_FENCE_GATE), 1))
        );
    }

    #[test]
    fn crafting_cobblestone_and_planks_to_slabs_and_stairs() {
        // Stone slabs.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for slot in &mut grid[0] {
            *slot = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        }
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::STONE_SLAB), 6))
        );

        // Oak slabs.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for slot in &mut grid[0] {
            *slot = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        }
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::OAK_SLAB), 6))
        );

        // Stone stairs.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[1][0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[1][1] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[2][0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[2][1] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[2][2] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::STONE_STAIRS), 4))
        );

        // Stone stairs (mirrored).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][2] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[1][1] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[1][2] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[2][0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[2][1] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        grid[2][2] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::STONE_STAIRS), 4))
        );

        // Oak stairs.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[1][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[1][1] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[2][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[2][1] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[2][2] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::OAK_STAIRS), 4))
        );

        // Oak stairs (mirrored).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][2] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[1][1] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[1][2] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[2][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[2][1] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        grid[2][2] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1));
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::OAK_STAIRS), 4))
        );
    }

    #[test]
    fn crafting_recipes_respect_grid_size() {
        // Furnace is a 3x3-only recipe (crafting table required).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for (r, row) in grid.iter_mut().enumerate() {
            for (c, slot) in row.iter_mut().enumerate() {
                if r == 1 && c == 1 {
                    continue;
                }
                *slot = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
            }
        }

        assert!(match_crafting_recipe(&grid, CraftingGridSize::TwoByTwo).is_none());
        let recipe = match_crafting_recipe(&grid, CraftingGridSize::ThreeByThree)
            .expect("furnace recipe should match");
        assert_eq!(recipe.output, ItemType::Block(BLOCK_FURNACE));
    }

    #[test]
    fn autofill_crafting_grid_consumes_items_from_storage() {
        let mut hotbar = Hotbar::new();
        hotbar.slots = Default::default();
        hotbar.selected = 0;
        let recipes = get_crafting_recipes();
        let furnace_recipe = recipes
            .iter()
            .find(|recipe| recipe.output == ItemType::Block(BLOCK_FURNACE))
            .expect("furnace recipe exists");

        let mut main_inventory = MainInventory::new();
        main_inventory.slots[0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 8));

        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        assert!(try_autofill_crafting_grid(
            &mut grid,
            &mut hotbar,
            &mut main_inventory,
            furnace_recipe
        ));
        assert!(main_inventory.slots[0].is_none());
        for (r, row) in grid.iter().enumerate() {
            for (c, slot) in row.iter().enumerate() {
                if r == 1 && c == 1 {
                    assert!(slot.is_none());
                    continue;
                }

                assert_eq!(
                    *slot,
                    Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1))
                );
            }
        }

        // Not enough items: should be a no-op.
        let mut main_inventory = MainInventory::new();
        main_inventory.slots[0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 7));
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        assert!(!try_autofill_crafting_grid(
            &mut grid,
            &mut hotbar,
            &mut main_inventory,
            furnace_recipe
        ));
        assert_eq!(
            main_inventory.slots[0],
            Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 7))
        );
        assert!(grid.iter().flatten().all(|slot| slot.is_none()));
    }

    #[test]
    fn crafting_coal_and_stick_to_torches() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Item(8), 1));
        grid[1][0] = Some(ItemStack::new(ItemType::Item(3), 1));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::TORCH), 4))
        );
    }

    #[test]
    fn crafting_wheat_to_bread() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WHEAT), 1));
        grid[0][1] = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WHEAT), 1));
        grid[0][2] = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WHEAT), 1));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Food(mdminecraft_core::item::FoodType::Bread), 1))
        );
    }

    #[test]
    fn crafting_sugar_cane_to_sugar() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_SUGAR_CANE), 1));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Item(CORE_ITEM_SUGAR), 1))
        );
    }

    #[test]
    fn crafting_sugar_cane_to_paper() {
        let cane = ItemStack::new(ItemType::Block(BLOCK_SUGAR_CANE), 1);
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(cane.clone());
        grid[0][1] = Some(cane.clone());
        grid[0][2] = Some(cane);

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Item(CORE_ITEM_PAPER), 3))
        );
    }

    #[test]
    fn crafting_paper_and_leather_to_book() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Item(CORE_ITEM_PAPER), 3));
        grid[0][1] = Some(ItemStack::new(ItemType::Item(102), 1)); // Item(102) = Leather

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Item(CORE_ITEM_BOOK), 1))
        );
    }

    #[test]
    fn crafting_mushroom_sugar_spider_eye_to_fermented_spider_eye() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_BROWN_MUSHROOM), 1));
        grid[0][1] = Some(ItemStack::new(ItemType::Item(CORE_ITEM_SUGAR), 1));
        grid[0][2] = Some(ItemStack::new(ItemType::Item(CORE_ITEM_SPIDER_EYE), 1));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Item(CORE_ITEM_FERMENTED_SPIDER_EYE), 1))
        );
    }

    #[test]
    fn crafting_carrot_and_gold_ingot_to_golden_carrot() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(
            ItemType::Food(mdminecraft_core::item::FoodType::Carrot),
            1,
        ));
        grid[0][1] = Some(ItemStack::new(ItemType::Item(9), 1));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((
                ItemType::Food(mdminecraft_core::item::FoodType::GoldenCarrot),
                1
            ))
        );
    }

    #[test]
    fn crafting_planks_and_books_to_bookshelf() {
        let plank = ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1);
        let book = ItemStack::new(ItemType::Item(CORE_ITEM_BOOK), 1);

        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for slot in &mut grid[0] {
            *slot = Some(plank.clone());
        }
        for slot in &mut grid[1] {
            *slot = Some(book.clone());
        }
        for slot in &mut grid[2] {
            *slot = Some(plank.clone());
        }

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(BLOCK_BOOKSHELF), 1))
        );
    }

    #[test]
    fn crafting_bed_is_shaped() {
        let wool = ItemStack::new(ItemType::Item(103), 1);
        let plank = ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1);

        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(wool.clone());
        grid[0][1] = Some(wool.clone());
        grid[0][2] = Some(wool);
        grid[1][0] = Some(plank.clone());
        grid[1][1] = Some(plank.clone());
        grid[1][2] = Some(plank);

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::BED_FOOT), 1))
        );
    }

    #[test]
    fn crafting_glass_to_glass_panes() {
        let glass = ItemStack::new(ItemType::Block(interactive_blocks::GLASS), 1);

        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(glass.clone());
        grid[0][1] = Some(glass.clone());
        grid[0][2] = Some(glass.clone());
        grid[1][0] = Some(glass.clone());
        grid[1][1] = Some(glass.clone());
        grid[1][2] = Some(glass);

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(interactive_blocks::GLASS_PANE), 16))
        );
    }

    #[test]
    fn crafting_glass_to_glass_bottles() {
        let glass = ItemStack::new(ItemType::Block(interactive_blocks::GLASS), 1);

        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(glass.clone());
        grid[0][2] = Some(glass.clone());
        grid[1][1] = Some(glass);

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Item(CORE_ITEM_GLASS_BOTTLE), 3))
        );
    }

    #[test]
    fn crafting_nether_wart_block_to_nether_wart_items() {
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(
            ItemType::Block(mdminecraft_world::BLOCK_NETHER_WART_BLOCK),
            1,
        ));

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Item(CORE_ITEM_NETHER_WART), 9))
        );
    }

    #[test]
    fn crafting_brewing_stand_requires_blaze_powder_and_cobblestone() {
        let blaze_powder = ItemStack::new(ItemType::Item(CORE_ITEM_BLAZE_POWDER), 1);
        let cobble = ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1);

        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][1] = Some(blaze_powder);
        grid[1][0] = Some(cobble.clone());
        grid[1][1] = Some(cobble.clone());
        grid[1][2] = Some(cobble);

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(BLOCK_BREWING_STAND), 1))
        );
    }

    #[test]
    fn crafting_enchanting_table_vanillaish_recipe() {
        let lapis = ItemStack::new(ItemType::Item(15), 1);
        let diamond = ItemStack::new(ItemType::Item(14), 1);
        let obsidian = ItemStack::new(ItemType::Block(BLOCK_OBSIDIAN), 1);

        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][1] = Some(lapis);
        grid[1][0] = Some(diamond.clone());
        grid[1][1] = Some(obsidian.clone());
        grid[1][2] = Some(diamond);
        grid[2][0] = Some(obsidian.clone());
        grid[2][1] = Some(obsidian.clone());
        grid[2][2] = Some(obsidian);

        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Block(BLOCK_ENCHANTING_TABLE), 1))
        );
    }

    #[test]
    fn crafting_bow_and_arrow_are_shaped_and_bow_is_mirrorable() {
        let stick = ItemStack::new(ItemType::Item(3), 1);
        let string = ItemStack::new(ItemType::Item(4), 1);

        // Bow (canonical orientation).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(stick.clone());
        grid[1][0] = Some(stick.clone());
        grid[2][0] = Some(stick.clone());
        grid[0][1] = Some(string.clone());
        grid[2][1] = Some(string.clone());
        grid[1][2] = Some(string.clone());
        assert_eq!(check_crafting_recipe(&grid), Some((ItemType::Item(1), 1)));

        // Bow (mirrored).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][2] = Some(stick.clone());
        grid[1][2] = Some(stick.clone());
        grid[2][2] = Some(stick);
        grid[0][1] = Some(string.clone());
        grid[2][1] = Some(string.clone());
        grid[1][0] = Some(string);
        assert_eq!(check_crafting_recipe(&grid), Some((ItemType::Item(1), 1)));

        // Arrow.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Item(5), 1)); // flint
        grid[1][0] = Some(ItemStack::new(ItemType::Item(3), 1)); // stick
        grid[2][0] = Some(ItemStack::new(ItemType::Item(6), 1)); // feather
        assert_eq!(check_crafting_recipe(&grid), Some((ItemType::Item(2), 4)));
    }

    #[test]
    fn crafting_tools_are_shaped_and_axes_and_hoes_mirror() {
        let plank = ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 1);
        let stick = ItemStack::new(ItemType::Item(3), 1);

        // Wooden pickaxe.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(plank.clone());
        grid[0][1] = Some(plank.clone());
        grid[0][2] = Some(plank.clone());
        grid[1][1] = Some(stick.clone());
        grid[2][1] = Some(stick.clone());
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood), 1))
        );

        // Wooden axe (canonical orientation).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(plank.clone());
        grid[0][1] = Some(plank.clone());
        grid[1][0] = Some(plank.clone());
        grid[1][1] = Some(stick.clone());
        grid[2][1] = Some(stick.clone());
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Tool(ToolType::Axe, ToolMaterial::Wood), 1))
        );

        // Wooden axe (mirrored).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(plank.clone());
        grid[0][1] = Some(plank.clone());
        grid[1][1] = Some(plank.clone());
        grid[1][0] = Some(stick.clone());
        grid[2][0] = Some(stick.clone());
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Tool(ToolType::Axe, ToolMaterial::Wood), 1))
        );

        // Wooden hoe (canonical orientation).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(plank.clone());
        grid[0][1] = Some(plank.clone());
        grid[1][1] = Some(stick.clone());
        grid[2][1] = Some(stick.clone());
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Tool(ToolType::Hoe, ToolMaterial::Wood), 1))
        );

        // Wooden hoe (mirrored).
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(plank.clone());
        grid[0][1] = Some(plank.clone());
        grid[1][0] = Some(stick.clone());
        grid[2][0] = Some(stick);
        assert_eq!(
            check_crafting_recipe(&grid),
            Some((ItemType::Tool(ToolType::Hoe, ToolMaterial::Wood), 1))
        );
    }

    #[test]
    fn crafting_planks_allows_extra_logs_and_consumes_one() {
        // Planks are intentionally allowed to match even with extra logs present,
        // so players can craft multiple times without clearing the grid.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 1));
        grid[0][1] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 1));
        let recipe = match_crafting_recipe(&grid, CraftingGridSize::ThreeByThree)
            .expect("planks recipe should match");
        assert_eq!(
            (recipe.output, recipe.output_count),
            (ItemType::Block(BLOCK_OAK_PLANKS), 4)
        );
        assert!(consume_crafting_inputs_3x3(&mut grid, &recipe));
        let remaining_logs: u32 = grid
            .iter()
            .flatten()
            .flatten()
            .filter(|stack| stack.item_type == ItemType::Block(BLOCK_OAK_LOG))
            .map(|stack| stack.count)
            .sum();
        assert_eq!(remaining_logs, 1);

        // A single stack with multiple logs should still match and only consume one.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 2));
        let recipe = match_crafting_recipe(&grid, CraftingGridSize::ThreeByThree)
            .expect("planks recipe should match");
        assert!(consume_crafting_inputs_3x3(&mut grid, &recipe));
        let remaining_logs: u32 = grid
            .iter()
            .flatten()
            .flatten()
            .filter(|stack| stack.item_type == ItemType::Block(BLOCK_OAK_LOG))
            .map(|stack| stack.count)
            .sum();
        assert_eq!(remaining_logs, 1);

        // 9 cobblestone shouldn't match the 8-cobblestone furnace recipe.
        let mut grid: [[Option<ItemStack>; 3]; 3] = Default::default();
        for row in &mut grid {
            for slot in row {
                *slot = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1));
            }
        }
        assert_eq!(check_crafting_recipe(&grid), None);
    }

    #[test]
    fn try_add_stack_to_cursor_clamps_to_max_stack_size() {
        let mut cursor = None;
        let stack = ItemStack::new(ItemType::Item(3), 200);

        let remainder = try_add_stack_to_cursor(&mut cursor, stack).expect("should overflow");
        let cursor_stack = cursor.expect("cursor should be filled");

        assert_eq!(cursor_stack.count, cursor_stack.max_stack_size());
        assert_eq!(
            remainder.count,
            200_u32.saturating_sub(cursor_stack.max_stack_size())
        );
    }

    #[test]
    fn crafting_max_crafts_computes_min_over_inputs() {
        let recipes = get_crafting_recipes();
        let torches_recipe = recipes
            .iter()
            .find(|recipe| recipe.output == ItemType::Block(interactive_blocks::TORCH))
            .expect("torch recipe exists");
        let planks_recipe = recipes
            .iter()
            .find(|recipe| recipe.output == ItemType::Block(BLOCK_OAK_PLANKS))
            .expect("planks recipe exists");

        let mut grid_2x2: [[Option<ItemStack>; 2]; 2] = Default::default();
        grid_2x2[0][0] = Some(ItemStack::new(ItemType::Item(8), 10)); // coal
        grid_2x2[1][0] = Some(ItemStack::new(ItemType::Item(3), 3)); // stick
        assert_eq!(crafting_max_crafts_2x2(&grid_2x2, torches_recipe), 3);

        let mut grid_3x3: [[Option<ItemStack>; 3]; 3] = Default::default();
        grid_3x3[0][0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_LOG), 5));
        assert_eq!(crafting_max_crafts_3x3(&grid_3x3, planks_recipe), 5);
    }

    #[test]
    fn cursor_can_accept_full_stack_requires_space() {
        let mut cursor = Some(ItemStack::new(ItemType::Item(3), 63));
        let output = ItemStack::new(ItemType::Item(3), 1);
        assert!(cursor_can_accept_full_stack(&cursor, &output));

        // Can't take a full output stack if it would overflow the max stack size.
        let output = ItemStack::new(ItemType::Item(3), 2);
        assert!(!cursor_can_accept_full_stack(&cursor, &output));

        // Incompatible cursor contents prevent taking output.
        let output = ItemStack::new(ItemType::Item(8), 1);
        assert!(!cursor_can_accept_full_stack(&cursor, &output));

        // Empty cursor accepts.
        cursor = None;
        let output = ItemStack::new(ItemType::Item(3), 64);
        assert!(cursor_can_accept_full_stack(&cursor, &output));
    }

    #[test]
    fn primary_drag_distribution_round_robins_until_cursor_empty() {
        let mut hotbar = Hotbar::new();
        hotbar.slots = Default::default();
        hotbar.selected = 0;
        let mut main_inventory = MainInventory::new();
        let mut personal: [[Option<ItemStack>; 2]; 2] = Default::default();
        let mut crafting: [[Option<ItemStack>; 3]; 3] = Default::default();

        let mut cursor = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 5));
        let visited = vec![UiCoreSlotId::Hotbar(0), UiCoreSlotId::Hotbar(1)];

        apply_primary_drag_distribution(
            &mut cursor,
            &visited,
            &mut hotbar,
            &mut main_inventory,
            &mut personal,
            &mut crafting,
        );

        assert!(cursor.is_none());
        assert_eq!(hotbar.slots[0].as_ref().unwrap().count, 3);
        assert_eq!(hotbar.slots[1].as_ref().unwrap().count, 2);
    }

    #[test]
    fn primary_drag_distribution_skips_full_and_incompatible_slots() {
        let mut hotbar = Hotbar::new();
        hotbar.slots = Default::default();
        hotbar.selected = 0;
        hotbar.slots[0] = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 64));
        hotbar.slots[1] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 10));
        let mut main_inventory = MainInventory::new();
        let mut personal: [[Option<ItemStack>; 2]; 2] = Default::default();
        let mut crafting: [[Option<ItemStack>; 3]; 3] = Default::default();

        let mut cursor = Some(ItemStack::new(ItemType::Block(BLOCK_OAK_PLANKS), 3));
        let visited = vec![
            UiCoreSlotId::Hotbar(0),
            UiCoreSlotId::Hotbar(1),
            UiCoreSlotId::Hotbar(2),
        ];

        apply_primary_drag_distribution(
            &mut cursor,
            &visited,
            &mut hotbar,
            &mut main_inventory,
            &mut personal,
            &mut crafting,
        );

        assert!(cursor.is_none());
        assert_eq!(hotbar.slots[0].as_ref().unwrap().count, 64);
        assert_eq!(hotbar.slots[1].as_ref().unwrap().count, 10);
        assert_eq!(hotbar.slots[2].as_ref().unwrap().count, 3);
    }

    #[test]
    fn armor_piece_roundtrips_via_core_stack_preserving_durability_and_enchantments() {
        let mut piece = ArmorPiece::from_item_with_enchantments(
            DroppedItemType::IronHelmet,
            vec![Enchantment::new(EnchantmentType::Protection, 2)],
        )
        .expect("iron helmet should be armor");
        piece.durability = 7;

        let stack = armor_piece_to_core_stack(&piece).expect("should convert to core stack");
        assert_eq!(stack.item_type, ItemType::Item(10));
        assert_eq!(stack.count, 1);
        assert_eq!(stack.durability, Some(7));
        assert_eq!(stack.enchantments.as_ref().unwrap().len(), 1);

        let piece2 = armor_piece_from_core_stack(&stack).expect("should convert back to armor");
        assert_eq!(piece2.item_type, DroppedItemType::IronHelmet);
        assert_eq!(piece2.slot, ArmorSlot::Helmet);
        assert_eq!(piece2.durability, 7);
        assert_eq!(piece2.enchantments.len(), 1);
        assert_eq!(
            piece2.enchantments[0].enchantment_type,
            EnchantmentType::Protection
        );
        assert_eq!(piece2.enchantments[0].level, 2);
    }

    #[test]
    fn lapis_converts_between_dropped_and_core_item_ids() {
        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::LapisLazuli),
            Some(ItemType::Item(15))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(15)),
            Some(DroppedItemType::LapisLazuli)
        );
    }

    #[test]
    fn diamond_converts_between_dropped_and_core_item_ids() {
        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Diamond),
            Some(ItemType::Item(14))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(14)),
            Some(DroppedItemType::Diamond)
        );
    }

    #[test]
    fn tools_convert_between_dropped_and_core_item_types() {
        let tool = ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron);
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(tool),
            Some(DroppedItemType::IronPickaxe)
        );
        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::IronPickaxe),
            Some(tool)
        );

        let tool = ItemType::Tool(ToolType::Shovel, ToolMaterial::Wood);
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(tool),
            Some(DroppedItemType::WoodenShovel)
        );
        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::WoodenShovel),
            Some(tool)
        );
    }

    #[test]
    fn brewing_items_convert_between_dropped_and_core_item_types() {
        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::NetherWart),
            Some(ItemType::Item(CORE_ITEM_NETHER_WART))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_NETHER_WART)),
            Some(DroppedItemType::NetherWart)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::BlazePowder),
            Some(ItemType::Item(CORE_ITEM_BLAZE_POWDER))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_BLAZE_POWDER)),
            Some(DroppedItemType::BlazePowder)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Gunpowder),
            Some(ItemType::Item(CORE_ITEM_GUNPOWDER))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_GUNPOWDER)),
            Some(DroppedItemType::Gunpowder)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::SpiderEye),
            Some(ItemType::Item(CORE_ITEM_SPIDER_EYE))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_SPIDER_EYE)),
            Some(DroppedItemType::SpiderEye)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::FermentedSpiderEye),
            Some(ItemType::Item(CORE_ITEM_FERMENTED_SPIDER_EYE))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(
                CORE_ITEM_FERMENTED_SPIDER_EYE
            )),
            Some(DroppedItemType::FermentedSpiderEye)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::MagmaCream),
            Some(ItemType::Item(CORE_ITEM_MAGMA_CREAM))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_MAGMA_CREAM)),
            Some(DroppedItemType::MagmaCream)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Sugar),
            Some(ItemType::Item(CORE_ITEM_SUGAR))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_SUGAR)),
            Some(DroppedItemType::Sugar)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::GoldenCarrot),
            Some(ItemType::Food(
                mdminecraft_core::item::FoodType::GoldenCarrot
            ))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Food(
                mdminecraft_core::item::FoodType::GoldenCarrot
            )),
            Some(DroppedItemType::GoldenCarrot)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::PotionAwkward),
            Some(ItemType::Potion(potion_ids::AWKWARD))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Potion(potion_ids::AWKWARD)),
            Some(DroppedItemType::PotionAwkward)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::SplashPotionStrength),
            Some(ItemType::SplashPotion(potion_ids::STRENGTH))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::SplashPotion(
                potion_ids::STRENGTH
            )),
            Some(DroppedItemType::SplashPotionStrength)
        );
    }

    #[test]
    fn books_and_paper_convert_between_dropped_and_core_item_types() {
        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Paper),
            Some(ItemType::Item(CORE_ITEM_PAPER))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_PAPER)),
            Some(DroppedItemType::Paper)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Book),
            Some(ItemType::Item(CORE_ITEM_BOOK))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_BOOK)),
            Some(DroppedItemType::Book)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::SugarCane),
            Some(ItemType::Block(BLOCK_SUGAR_CANE))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Block(BLOCK_SUGAR_CANE)),
            Some(DroppedItemType::SugarCane)
        );
    }

    #[test]
    fn farming_items_convert_between_dropped_and_core_item_types() {
        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::WheatSeeds),
            Some(ItemType::Item(CORE_ITEM_WHEAT_SEEDS))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_WHEAT_SEEDS)),
            Some(DroppedItemType::WheatSeeds)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Wheat),
            Some(ItemType::Item(CORE_ITEM_WHEAT))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Item(CORE_ITEM_WHEAT)),
            Some(DroppedItemType::Wheat)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Bread),
            Some(ItemType::Food(mdminecraft_core::item::FoodType::Bread))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Food(
                mdminecraft_core::item::FoodType::Bread
            )),
            Some(DroppedItemType::Bread)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Carrot),
            Some(ItemType::Food(mdminecraft_core::item::FoodType::Carrot))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Food(
                mdminecraft_core::item::FoodType::Carrot
            )),
            Some(DroppedItemType::Carrot)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::Potato),
            Some(ItemType::Food(mdminecraft_core::item::FoodType::Potato))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Food(
                mdminecraft_core::item::FoodType::Potato
            )),
            Some(DroppedItemType::Potato)
        );

        assert_eq!(
            GameWorld::convert_dropped_item_type(DroppedItemType::BakedPotato),
            Some(ItemType::Food(
                mdminecraft_core::item::FoodType::BakedPotato
            ))
        );
        assert_eq!(
            GameWorld::convert_core_item_type_to_dropped(ItemType::Food(
                mdminecraft_core::item::FoodType::BakedPotato
            )),
            Some(DroppedItemType::BakedPotato)
        );
    }

    #[test]
    fn main_inventory_persists_via_world_inventory_roundtrip() {
        let mut main = super::MainInventory::new();
        main.slots[0] = Some(ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 64));

        let mut tool = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron), 1);
        tool.durability = Some(7);
        tool.enchantments = Some(vec![
            mdminecraft_core::Enchantment::new(mdminecraft_core::EnchantmentType::Efficiency, 3),
            mdminecraft_core::Enchantment::new(mdminecraft_core::EnchantmentType::Unbreaking, 2),
        ]);
        main.slots[1] = Some(tool.clone());

        let bread = ItemStack::new(ItemType::Food(mdminecraft_core::item::FoodType::Bread), 3);
        main.slots[2] = Some(bread.clone());

        let persisted = GameWorld::persisted_inventory_from_main_inventory(&main);
        let loaded = GameWorld::main_inventory_from_persisted_inventory(persisted);

        assert_eq!(loaded.slots[0], main.slots[0]);
        assert_eq!(loaded.slots[1], main.slots[1]);
        assert_eq!(loaded.slots[2], main.slots[2]);
    }

    #[test]
    fn brewing_bottle_slot_places_only_one_water_bottle_from_stack() {
        let mut slot = None;
        let mut cursor = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 5));
        apply_brewing_bottle_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert_eq!(
            slot,
            Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 1))
        );
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 4))
        );

        let mut slot = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 1));
        let mut cursor = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 3));
        apply_brewing_bottle_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert!(slot.is_none());
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 4))
        );

        let mut slot = Some(ItemStack::new(ItemType::Potion(potion_ids::AWKWARD), 1));
        let mut cursor = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 1));
        apply_brewing_bottle_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert_eq!(
            slot,
            Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 1))
        );
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Potion(potion_ids::AWKWARD), 1))
        );

        // Cursor stacks with >1 bottles can't be swapped into an occupied bottle slot.
        let mut slot = Some(ItemStack::new(ItemType::Potion(potion_ids::AWKWARD), 1));
        let mut cursor = Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 2));
        apply_brewing_bottle_slot_click(&mut slot, &mut cursor, UiSlotClick::Primary);
        assert_eq!(
            slot,
            Some(ItemStack::new(ItemType::Potion(potion_ids::AWKWARD), 1))
        );
        assert_eq!(
            cursor,
            Some(ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 2))
        );
    }

    #[test]
    fn brewing_shift_move_prefers_fuel_then_falls_back_to_ingredient() {
        let mut stand = BrewingStandState::new();

        let mut powder = ItemStack::new(ItemType::Item(CORE_ITEM_BLAZE_POWDER), 5);
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut powder,
            &mut stand
        ));
        assert_eq!(stand.fuel, 5);
        assert_eq!(powder.count, 0);

        stand.fuel = 64;
        let mut powder = ItemStack::new(ItemType::Item(CORE_ITEM_BLAZE_POWDER), 3);
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut powder,
            &mut stand
        ));
        assert_eq!(stand.fuel, 64);
        assert_eq!(stand.ingredient, Some((item_ids::BLAZE_POWDER, 3)));
        assert_eq!(powder.count, 0);
    }

    #[test]
    fn brewing_shift_move_inserts_gunpowder_as_ingredient() {
        let mut stand = BrewingStandState::new();

        let mut gunpowder = ItemStack::new(ItemType::Item(CORE_ITEM_GUNPOWDER), 4);
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut gunpowder,
            &mut stand
        ));
        assert_eq!(gunpowder.count, 0);
        assert_eq!(stand.ingredient, Some((item_ids::GUNPOWDER, 4)));
    }

    #[test]
    fn brewing_shift_move_inserts_sugar_as_ingredient() {
        let mut stand = BrewingStandState::new();

        let mut sugar = ItemStack::new(ItemType::Item(CORE_ITEM_SUGAR), 7);
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut sugar, &mut stand
        ));
        assert_eq!(sugar.count, 0);
        assert_eq!(stand.ingredient, Some((item_ids::SUGAR, 7)));
    }

    #[test]
    fn brewing_shift_move_inserts_fermented_spider_eye_as_ingredient() {
        let mut stand = BrewingStandState::new();

        let mut eye = ItemStack::new(ItemType::Item(CORE_ITEM_FERMENTED_SPIDER_EYE), 3);
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut eye, &mut stand
        ));
        assert_eq!(eye.count, 0);
        assert_eq!(stand.ingredient, Some((item_ids::FERMENTED_SPIDER_EYE, 3)));
    }

    #[test]
    fn brewing_shift_move_inserts_magma_cream_as_ingredient() {
        let mut stand = BrewingStandState::new();

        let mut cream = ItemStack::new(ItemType::Item(CORE_ITEM_MAGMA_CREAM), 2);
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut cream, &mut stand
        ));
        assert_eq!(cream.count, 0);
        assert_eq!(stand.ingredient, Some((item_ids::MAGMA_CREAM, 2)));
    }

    #[test]
    fn brewing_shift_move_inserts_golden_carrots_as_ingredient() {
        let mut stand = BrewingStandState::new();

        let mut carrots = ItemStack::new(
            ItemType::Food(mdminecraft_core::item::FoodType::GoldenCarrot),
            2,
        );
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut carrots,
            &mut stand
        ));
        assert_eq!(carrots.count, 0);
        assert_eq!(stand.ingredient, Some((item_ids::GOLDEN_CARROT, 2)));
    }

    #[test]
    fn brewing_shift_move_inserts_bottles_into_empty_slots() {
        let mut stand = BrewingStandState::new();

        let mut bottles = ItemStack::new(ItemType::Item(CORE_ITEM_WATER_BOTTLE), 5);
        assert!(try_shift_move_core_stack_into_brewing_stand(
            &mut bottles,
            &mut stand
        ));
        assert_eq!(bottles.count, 2);
        assert_eq!(
            stand.bottles,
            [Some(mdminecraft_world::PotionType::Water); 3]
        );
    }

    #[test]
    fn enchanting_shift_move_inserts_lapis() {
        let mut table = EnchantingTableState::new();

        let mut lapis = ItemStack::new(ItemType::Item(15), 10);
        assert!(try_shift_move_core_stack_into_enchanting_table(
            &mut lapis, &mut table
        ));
        assert_eq!(table.lapis_count, 10);
        assert_eq!(lapis.count, 0);

        table.lapis_count = 60;
        let mut lapis = ItemStack::new(ItemType::Item(15), 10);
        assert!(try_shift_move_core_stack_into_enchanting_table(
            &mut lapis, &mut table
        ));
        assert_eq!(table.lapis_count, 64);
        assert_eq!(lapis.count, 6);

        let mut cobble = ItemStack::new(ItemType::Block(BLOCK_COBBLESTONE), 1);
        assert!(!try_shift_move_core_stack_into_enchanting_table(
            &mut cobble,
            &mut table
        ));
        assert_eq!(cobble.count, 1);
    }

    #[test]
    fn enchanting_table_id_mapping_matches_world_conventions() {
        let pickaxe = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood), 1);
        assert_eq!(
            core_item_to_enchanting_id(&pickaxe),
            Some(mdminecraft_world::TOOL_ID_START)
        );

        let sword = ItemStack::new(ItemType::Tool(ToolType::Sword, ToolMaterial::Gold), 1);
        assert_eq!(
            core_item_to_enchanting_id(&sword),
            Some(mdminecraft_world::TOOL_ID_START + 20 + ToolMaterial::Gold as u16)
        );
    }
}
