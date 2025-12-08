//! Dropped item system with physics and lifecycle management.
//!
//! Items can be dropped from breaking blocks or defeating mobs.
//! They have physics (gravity, collision), a pickup radius, and despawn after 5 minutes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum lifetime for dropped items (5 minutes = 6000 ticks at 20 TPS).
pub const ITEM_DESPAWN_TICKS: u32 = 6000;

/// Pickup radius in blocks.
pub const PICKUP_RADIUS: f64 = 1.5;

/// Types of items that can be dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    // Block items - terrain
    Stone,
    Cobblestone,
    Dirt,
    Grass,
    Sand,
    Gravel,
    Ice,
    Snow,
    Clay,
    Bedrock,

    // Block items - trees
    OakLog,
    OakLeaves,
    BirchLog,
    BirchLeaves,
    PineLog,
    PineLeaves,

    // Block items - ores
    CoalOre,
    IronOre,
    GoldOre,
    DiamondOre,

    // Mob drops
    RawPork,
    RawBeef,
    Leather,
    Wool,
    Feather,
    Egg,
    Bone,
    RottenFlesh,
    String,
    Gunpowder,

    // Smelted/processed items
    IronIngot,
    GoldIngot,
    CookedPork,
    CookedBeef,
    Coal,

    // Crafted items (for future use)
    Stick,
    Planks,
    OakPlanks,
    BirchPlanks,
    PinePlanks,
    Furnace,

    // Special items
    Sapling,
    Apple,
    Flint,

    // Combat items
    Arrow,
    Bow,

    // Armor - Leather
    LeatherHelmet,
    LeatherChestplate,
    LeatherLeggings,
    LeatherBoots,

    // Armor - Iron
    IronHelmet,
    IronChestplate,
    IronLeggings,
    IronBoots,

    // Armor - Gold
    GoldHelmet,
    GoldChestplate,
    GoldLeggings,
    GoldBoots,

    // Armor - Diamond
    DiamondHelmet,
    DiamondChestplate,
    DiamondLeggings,
    DiamondBoots,

    // Resources
    Diamond,
    LapisLazuli,
}

impl ItemType {
    /// Get the numeric ID for this item type (used in crafting recipes).
    pub fn id(&self) -> u16 {
        *self as u16
    }

    /// Get the maximum stack size for this item type.
    pub fn max_stack_size(&self) -> u32 {
        match self {
            // Most block items stack to 64
            ItemType::Stone
            | ItemType::Cobblestone
            | ItemType::Dirt
            | ItemType::Grass
            | ItemType::Sand
            | ItemType::Gravel
            | ItemType::Ice
            | ItemType::Snow
            | ItemType::Clay
            | ItemType::Bedrock
            | ItemType::OakLog
            | ItemType::OakLeaves
            | ItemType::BirchLog
            | ItemType::BirchLeaves
            | ItemType::PineLog
            | ItemType::PineLeaves
            | ItemType::CoalOre
            | ItemType::IronOre
            | ItemType::GoldOre
            | ItemType::DiamondOre
            | ItemType::Wool
            | ItemType::Feather
            | ItemType::Bone
            | ItemType::RottenFlesh
            | ItemType::String
            | ItemType::Gunpowder
            | ItemType::IronIngot
            | ItemType::GoldIngot
            | ItemType::Coal
            | ItemType::Stick
            | ItemType::Planks
            | ItemType::OakPlanks
            | ItemType::BirchPlanks
            | ItemType::PinePlanks
            | ItemType::Furnace
            | ItemType::Sapling
            | ItemType::Flint
            | ItemType::Arrow
            | ItemType::LapisLazuli => 64,

            // Food and resources stack to 16
            ItemType::RawPork
            | ItemType::RawBeef
            | ItemType::CookedPork
            | ItemType::CookedBeef
            | ItemType::Leather
            | ItemType::Egg
            | ItemType::Apple
            | ItemType::Diamond => 16,

            // Non-stackable items (weapons and armor)
            ItemType::Bow
            | ItemType::LeatherHelmet
            | ItemType::LeatherChestplate
            | ItemType::LeatherLeggings
            | ItemType::LeatherBoots
            | ItemType::IronHelmet
            | ItemType::IronChestplate
            | ItemType::IronLeggings
            | ItemType::IronBoots
            | ItemType::GoldHelmet
            | ItemType::GoldChestplate
            | ItemType::GoldLeggings
            | ItemType::GoldBoots
            | ItemType::DiamondHelmet
            | ItemType::DiamondChestplate
            | ItemType::DiamondLeggings
            | ItemType::DiamondBoots => 1,
        }
    }

    /// Get the item that drops when a block is broken.
    ///
    /// Returns Some((item_type, count)) or None if nothing drops.
    ///
    /// Block IDs reference (from blocks.json):
    /// - 0: Air (no drop)
    /// - 1: Stone
    /// - 2: Dirt
    /// - 3: Grass (drops dirt, not grass block)
    /// - 4: Sand
    /// - 5: Gravel
    /// - 6: Water (no drop)
    /// - 7: Ice
    /// - 8: Snow
    /// - 9: Clay
    /// - 10: Bedrock (no drop in survival)
    /// - 11: Oak Log
    /// - 12: Oak Planks
    /// - 13: Crafting Table
    /// - 14: Coal Ore
    /// - 15: Iron Ore
    /// - 16: Gold Ore
    /// - 17: Diamond Ore
    pub fn from_block(block_id: u16) -> Option<(ItemType, u32)> {
        match block_id {
            // Terrain blocks
            1 => Some((ItemType::Cobblestone, 1)), // Stone drops cobblestone (like Minecraft)
            2 => Some((ItemType::Dirt, 1)),
            3 => Some((ItemType::Dirt, 1)), // Grass drops dirt (like Minecraft)
            4 => Some((ItemType::Sand, 1)),
            5 => Some((ItemType::Gravel, 1)),
            7 => Some((ItemType::Ice, 1)),
            8 => Some((ItemType::Snow, 1)),
            9 => Some((ItemType::Clay, 1)),

            // Tree blocks
            11 => Some((ItemType::OakLog, 1)),
            12 => Some((ItemType::OakPlanks, 1)),

            // Ore blocks - coal drops coal, others drop ore blocks
            14 => Some((ItemType::Coal, 1)),
            15 => Some((ItemType::IronOre, 1)),
            16 => Some((ItemType::GoldOre, 1)),
            17 => Some((ItemType::DiamondOre, 1)),

            // Furnace
            18 => Some((ItemType::Furnace, 1)),
            19 => Some((ItemType::Furnace, 1)), // Lit furnace also drops furnace

            // Lapis ore (drops 4-9 lapis, using 6 as average for now)
            98 => Some((ItemType::LapisLazuli, 6)),

            // No drops: Air (0), Water (6), Bedrock (10), Crafting Table (13), Enchanting Table (99)
            _ => None,
        }
    }

    /// Get the item that drops from leaves with random chance.
    ///
    /// Leaves have a chance to drop saplings (1/16) and oak leaves
    /// have a chance to drop apples (1/200).
    ///
    /// # Arguments
    /// * `block_id` - The block ID of the leaves
    /// * `random_value` - A random value from 0.0 to 1.0
    ///
    /// # Returns
    /// Some((item_type, count)) if a special drop occurs, None otherwise.
    pub fn from_leaves_random(block_id: u16, random_value: f64) -> Option<(ItemType, u32)> {
        match block_id {
            12 => {
                // Oak leaves: 1/200 apple, 1/16 sapling
                if random_value < 0.005 {
                    Some((ItemType::Apple, 1))
                } else if random_value < 0.005 + 0.0625 {
                    Some((ItemType::Sapling, 1))
                } else {
                    None
                }
            }
            14 | 16 => {
                // Birch/Pine leaves: 1/16 sapling
                if random_value < 0.0625 {
                    Some((ItemType::Sapling, 1))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get the block ID that this item places (if applicable).
    ///
    /// # Returns
    /// Some(block_id) if this item can be placed as a block, None otherwise.
    pub fn to_block(&self) -> Option<u16> {
        match self {
            ItemType::Stone => Some(1),
            ItemType::Cobblestone => Some(1), // Cobblestone places as stone (until separate block added)
            ItemType::Dirt => Some(2),
            ItemType::Grass => Some(3),
            ItemType::Sand => Some(4),
            ItemType::Gravel => Some(5),
            ItemType::Ice => Some(7),
            ItemType::Snow => Some(8),
            ItemType::Clay => Some(9),
            ItemType::OakLog => Some(11),
            ItemType::OakPlanks => Some(12),
            ItemType::CoalOre => Some(14),
            ItemType::IronOre => Some(15),
            ItemType::GoldOre => Some(16),
            ItemType::DiamondOre => Some(17),
            ItemType::Furnace => Some(18),
            // Non-placeable items (leaves, mob drops, food, crafted items)
            _ => None,
        }
    }
}

/// A dropped item entity in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroppedItem {
    /// Unique ID for this dropped item.
    pub id: u64,
    /// World X position.
    pub x: f64,
    /// World Y position.
    pub y: f64,
    /// World Z position.
    pub z: f64,
    /// Velocity in X direction.
    pub vel_x: f64,
    /// Velocity in Y direction.
    pub vel_y: f64,
    /// Velocity in Z direction.
    pub vel_z: f64,
    /// Type of item.
    pub item_type: ItemType,
    /// Count/stack size.
    pub count: u32,
    /// Ticks remaining before despawn.
    pub lifetime_ticks: u32,
    /// Whether the item is on the ground (no longer falling).
    pub on_ground: bool,
}

impl DroppedItem {
    /// Create a new dropped item at the given position.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this item
    /// * `x, y, z` - World position
    /// * `item_type` - Type of item
    /// * `count` - Stack size
    ///
    /// Items spawn with small random velocity for visual scatter.
    pub fn new(id: u64, x: f64, y: f64, z: f64, item_type: ItemType, count: u32) -> Self {
        // Simple pseudo-random velocity based on ID
        let vel_x = ((id % 100) as f64 - 50.0) / 200.0; // -0.25 to 0.25
        let vel_z = (((id / 100) % 100) as f64 - 50.0) / 200.0;
        let vel_y = 0.2; // Small upward velocity

        Self {
            id,
            x,
            y,
            z,
            vel_x,
            vel_y,
            vel_z,
            item_type,
            count,
            lifetime_ticks: ITEM_DESPAWN_TICKS,
            on_ground: false,
        }
    }

    /// Update the item's physics and lifetime.
    ///
    /// # Arguments
    /// * `ground_height` - The Y coordinate of the ground at this position
    ///
    /// # Returns
    /// `true` if the item should be removed (despawned), `false` otherwise.
    pub fn update(&mut self, ground_height: f64) -> bool {
        // Decrement lifetime
        if self.lifetime_ticks > 0 {
            self.lifetime_ticks -= 1;
        } else {
            return true; // Despawn
        }

        // Apply physics if not on ground
        if !self.on_ground {
            // Gravity
            self.vel_y -= 0.04; // Gravity acceleration (slightly less than mobs)

            // Air resistance
            self.vel_x *= 0.98;
            self.vel_y *= 0.98;
            self.vel_z *= 0.98;

            // Update position
            self.x += self.vel_x;
            self.y += self.vel_y;
            self.z += self.vel_z;

            // Ground collision (items float slightly above ground)
            let item_ground_level = ground_height + 0.25;
            if self.y <= item_ground_level {
                self.y = item_ground_level;
                self.vel_y = 0.0;
                self.vel_x *= 0.5; // Friction
                self.vel_z *= 0.5;

                // Mark as on ground if velocity is low
                if self.vel_x.abs() < 0.01 && self.vel_z.abs() < 0.01 {
                    self.on_ground = true;
                }
            }
        }

        false // Don't despawn yet
    }

    /// Check if this item can be picked up by a player/mob at the given position.
    ///
    /// # Arguments
    /// * `px, py, pz` - Position of the player/mob
    ///
    /// # Returns
    /// `true` if within pickup radius.
    pub fn can_pickup(&self, px: f64, py: f64, pz: f64) -> bool {
        let dx = self.x - px;
        let dy = self.y - py;
        let dz = self.z - pz;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= PICKUP_RADIUS * PICKUP_RADIUS
    }

    /// Merge another item stack into this one if possible.
    ///
    /// # Arguments
    /// * `other` - Another dropped item to merge
    ///
    /// # Returns
    /// Number of items successfully merged (may be less than other.count if stack limit reached).
    pub fn try_merge(&mut self, other: &DroppedItem) -> u32 {
        if self.item_type != other.item_type {
            return 0; // Can't merge different item types
        }

        let max_stack = self.item_type.max_stack_size();
        let available_space = max_stack.saturating_sub(self.count);
        let merge_amount = available_space.min(other.count);

        self.count += merge_amount;
        merge_amount
    }
}

/// Manages all dropped items in the world.
pub struct ItemManager {
    items: HashMap<u64, DroppedItem>,
    next_id: u64,
}

impl ItemManager {
    /// Create a new empty item manager.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            next_id: 1,
        }
    }

    /// Spawn a new dropped item.
    ///
    /// # Arguments
    /// * `x, y, z` - World position
    /// * `item_type` - Type of item
    /// * `count` - Stack size
    ///
    /// # Returns
    /// The ID of the newly spawned item.
    pub fn spawn_item(&mut self, x: f64, y: f64, z: f64, item_type: ItemType, count: u32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let item = DroppedItem::new(id, x, y, z, item_type, count);
        self.items.insert(id, item);
        id
    }

    /// Update all items (physics and lifetime).
    ///
    /// # Arguments
    /// * `get_ground_height` - Function to get ground height at (x, z) position
    ///
    /// # Returns
    /// Number of items that despawned this tick.
    pub fn update<F>(&mut self, get_ground_height: F) -> usize
    where
        F: Fn(f64, f64) -> f64,
    {
        let mut to_remove = Vec::new();

        for (id, item) in self.items.iter_mut() {
            let ground_height = get_ground_height(item.x, item.z);
            if item.update(ground_height) {
                to_remove.push(*id);
            }
        }

        let despawn_count = to_remove.len();
        for id in to_remove {
            self.items.remove(&id);
        }

        despawn_count
    }

    /// Attempt to pick up items near a given position.
    ///
    /// # Arguments
    /// * `x, y, z` - Position of the player/mob
    ///
    /// # Returns
    /// List of (item_type, count) tuples that were picked up.
    pub fn pickup_items(&mut self, x: f64, y: f64, z: f64) -> Vec<(ItemType, u32)> {
        let mut picked_up = Vec::new();
        let mut to_remove = Vec::new();

        for (id, item) in self.items.iter() {
            if item.can_pickup(x, y, z) {
                picked_up.push((item.item_type, item.count));
                to_remove.push(*id);
            }
        }

        for id in to_remove {
            self.items.remove(&id);
        }

        picked_up
    }

    /// Get the number of active dropped items.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Get a reference to a specific item by ID.
    pub fn get(&self, id: u64) -> Option<&DroppedItem> {
        self.items.get(&id)
    }

    /// Get a mutable reference to a specific item by ID.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut DroppedItem> {
        self.items.get_mut(&id)
    }

    /// Get all items as a slice.
    pub fn items(&self) -> Vec<&DroppedItem> {
        self.items.values().collect()
    }

    /// Merge nearby items of the same type.
    ///
    /// Items within 1 block of each other will be merged if they're the same type.
    ///
    /// # Returns
    /// Number of items merged (removed).
    pub fn merge_nearby_items(&mut self) -> usize {
        const MERGE_RADIUS: f64 = 1.0;
        let mut merged_count = 0;
        let mut to_remove = Vec::new();

        // Get all item IDs
        let ids: Vec<u64> = self.items.keys().copied().collect();

        for i in 0..ids.len() {
            if to_remove.contains(&ids[i]) {
                continue;
            }

            for j in (i + 1)..ids.len() {
                if to_remove.contains(&ids[j]) {
                    continue;
                }

                let (id_a, id_b) = (ids[i], ids[j]);

                // Check distance
                let (item_a, item_b) = match (self.items.get(&id_a), self.items.get(&id_b)) {
                    (Some(a), Some(b)) => (a.clone(), b.clone()),
                    _ => continue,
                };

                let dx = item_a.x - item_b.x;
                let dy = item_a.y - item_b.y;
                let dz = item_a.z - item_b.z;
                let dist_sq = dx * dx + dy * dy + dz * dz;

                if dist_sq <= MERGE_RADIUS * MERGE_RADIUS {
                    // Try to merge item_b into item_a
                    if let Some(item_a_mut) = self.items.get_mut(&id_a) {
                        let merged = item_a_mut.try_merge(&item_b);
                        if merged == item_b.count {
                            // Fully merged, remove item_b
                            to_remove.push(id_b);
                            merged_count += 1;
                        }
                    }
                }
            }
        }

        for id in to_remove {
            self.items.remove(&id);
        }

        merged_count
    }
}

impl Default for ItemManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_type_max_stack() {
        // Block items stack to 64
        assert_eq!(ItemType::Stone.max_stack_size(), 64);
        assert_eq!(ItemType::OakLog.max_stack_size(), 64);
        assert_eq!(ItemType::Ice.max_stack_size(), 64);
        assert_eq!(ItemType::Feather.max_stack_size(), 64);

        // Food/resources stack to 16
        assert_eq!(ItemType::RawPork.max_stack_size(), 16);
        assert_eq!(ItemType::Apple.max_stack_size(), 16);
    }

    #[test]
    fn test_item_type_from_block() {
        // Terrain blocks - stone drops cobblestone (like Minecraft)
        assert_eq!(ItemType::from_block(1), Some((ItemType::Cobblestone, 1)));
        assert_eq!(ItemType::from_block(2), Some((ItemType::Dirt, 1)));
        assert_eq!(ItemType::from_block(3), Some((ItemType::Dirt, 1))); // Grass drops dirt
        assert_eq!(ItemType::from_block(4), Some((ItemType::Sand, 1)));
        assert_eq!(ItemType::from_block(5), Some((ItemType::Gravel, 1)));
        assert_eq!(ItemType::from_block(7), Some((ItemType::Ice, 1)));
        assert_eq!(ItemType::from_block(8), Some((ItemType::Snow, 1)));
        assert_eq!(ItemType::from_block(9), Some((ItemType::Clay, 1)));

        // Tree/building blocks
        assert_eq!(ItemType::from_block(11), Some((ItemType::OakLog, 1)));
        assert_eq!(ItemType::from_block(12), Some((ItemType::OakPlanks, 1)));

        // Ore blocks - coal ore drops coal directly (like Minecraft)
        assert_eq!(ItemType::from_block(14), Some((ItemType::Coal, 1)));
        assert_eq!(ItemType::from_block(15), Some((ItemType::IronOre, 1)));
        assert_eq!(ItemType::from_block(16), Some((ItemType::GoldOre, 1)));
        assert_eq!(ItemType::from_block(17), Some((ItemType::DiamondOre, 1)));

        // No drops
        assert_eq!(ItemType::from_block(0), None); // Air
        assert_eq!(ItemType::from_block(6), None); // Water
        assert_eq!(ItemType::from_block(10), None); // Bedrock
        assert_eq!(ItemType::from_block(13), None); // Crafting table
    }

    #[test]
    fn test_item_type_to_block() {
        assert_eq!(ItemType::Stone.to_block(), Some(1));
        assert_eq!(ItemType::Dirt.to_block(), Some(2));
        assert_eq!(ItemType::OakLog.to_block(), Some(11));
        assert_eq!(ItemType::OakPlanks.to_block(), Some(12));
        assert_eq!(ItemType::CoalOre.to_block(), Some(14));
        assert_eq!(ItemType::IronOre.to_block(), Some(15));
        assert_eq!(ItemType::GoldOre.to_block(), Some(16));
        assert_eq!(ItemType::DiamondOre.to_block(), Some(17));

        // Non-placeable items
        assert_eq!(ItemType::RawPork.to_block(), None);
        assert_eq!(ItemType::Apple.to_block(), None);
        assert_eq!(ItemType::Stick.to_block(), None);
    }

    #[test]
    fn test_leaves_random_drops() {
        // Note: Block IDs in from_leaves_random still reference the old tree leaf IDs
        // from trees.rs which uses different IDs than blocks.json.
        // This function works with the trees.rs leaf block IDs:
        // 12 = oak leaves (trees.rs), 14 = birch leaves, 16 = pine leaves

        // Oak leaves - apple drop (< 0.005)
        assert_eq!(
            ItemType::from_leaves_random(12, 0.001),
            Some((ItemType::Apple, 1))
        );

        // Oak leaves - sapling drop (0.005 to 0.0675)
        assert_eq!(
            ItemType::from_leaves_random(12, 0.01),
            Some((ItemType::Sapling, 1))
        );

        // Oak leaves - no drop (> 0.0675)
        assert_eq!(ItemType::from_leaves_random(12, 0.1), None);

        // Birch leaves - sapling drop (trees.rs ID 14)
        assert_eq!(
            ItemType::from_leaves_random(14, 0.03),
            Some((ItemType::Sapling, 1))
        );

        // Pine leaves - sapling drop (trees.rs ID 16)
        assert_eq!(
            ItemType::from_leaves_random(16, 0.05),
            Some((ItemType::Sapling, 1))
        );

        // Non-leaf block
        assert_eq!(ItemType::from_leaves_random(1, 0.001), None);
    }

    #[test]
    fn test_dropped_item_creation() {
        let item = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 5);

        assert_eq!(item.id, 1);
        assert_eq!(item.x, 10.0);
        assert_eq!(item.y, 64.0);
        assert_eq!(item.z, 20.0);
        assert_eq!(item.item_type, ItemType::Stone);
        assert_eq!(item.count, 5);
        assert_eq!(item.lifetime_ticks, ITEM_DESPAWN_TICKS);
        assert!(!item.on_ground);
    }

    #[test]
    fn test_dropped_item_physics() {
        let mut item = DroppedItem::new(1, 10.0, 70.0, 20.0, ItemType::Stone, 1);
        let ground_height = 64.0;

        // Simulate falling
        for _ in 0..100 {
            if item.update(ground_height) {
                break;
            }
        }

        // Should have landed on ground
        assert!(item.on_ground);
        assert!((item.y - (ground_height + 0.25)).abs() < 0.1);
    }

    #[test]
    fn test_dropped_item_lifetime() {
        let mut item = DroppedItem::new(1, 10.0, 64.25, 20.0, ItemType::Stone, 1);
        item.on_ground = true;
        item.lifetime_ticks = 2;

        assert!(!item.update(64.0)); // Tick 1
        assert!(!item.update(64.0)); // Tick 2
        assert!(item.update(64.0)); // Tick 3 - should despawn
    }

    #[test]
    fn test_item_pickup_radius() {
        let item = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 1);

        // Within range
        assert!(item.can_pickup(10.0, 64.0, 20.0));
        assert!(item.can_pickup(10.5, 64.0, 20.0));
        assert!(item.can_pickup(10.0, 64.5, 20.0));

        // Out of range
        assert!(!item.can_pickup(12.0, 64.0, 20.0));
        assert!(!item.can_pickup(10.0, 70.0, 20.0));
    }

    #[test]
    fn test_item_merge() {
        let mut item1 = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 10);
        let item2 = DroppedItem::new(2, 10.5, 64.0, 20.0, ItemType::Stone, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 5);
        assert_eq!(item1.count, 15);
    }

    #[test]
    fn test_item_merge_different_types() {
        let mut item1 = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 10);
        let item2 = DroppedItem::new(2, 10.5, 64.0, 20.0, ItemType::Dirt, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 0);
        assert_eq!(item1.count, 10);
    }

    #[test]
    fn test_item_merge_stack_limit() {
        let mut item1 = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 62);
        let item2 = DroppedItem::new(2, 10.5, 64.0, 20.0, ItemType::Stone, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 2); // Can only add 2 more (64 - 62)
        assert_eq!(item1.count, 64);
    }

    #[test]
    fn test_item_manager_spawn() {
        let mut manager = ItemManager::new();

        let id1 = manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        let id2 = manager.spawn_item(15.0, 64.0, 25.0, ItemType::Dirt, 3);

        assert_eq!(manager.count(), 2);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_item_manager_update() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 70.0, 20.0, ItemType::Stone, 1);

        let ground_height = |_x: f64, _z: f64| 64.0;

        // Simulate some ticks
        for _ in 0..50 {
            manager.update(ground_height);
        }

        assert_eq!(manager.count(), 1);

        // Item should be on ground now
        let item = manager.get(1).unwrap();
        assert!(item.on_ground);
    }

    #[test]
    fn test_item_manager_despawn() {
        let mut manager = ItemManager::new();
        let id = manager.spawn_item(10.0, 64.25, 20.0, ItemType::Stone, 1);

        if let Some(item) = manager.items.get_mut(&id) {
            item.on_ground = true;
            item.lifetime_ticks = 1;
        }

        let ground_height = |_x: f64, _z: f64| 64.0;

        manager.update(ground_height);
        assert_eq!(manager.count(), 1);

        let despawned = manager.update(ground_height);
        assert_eq!(despawned, 1);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_item_manager_pickup() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        manager.spawn_item(15.0, 64.0, 25.0, ItemType::Dirt, 3);

        // Pickup near first item
        let picked_up = manager.pickup_items(10.0, 64.0, 20.0);
        assert_eq!(picked_up.len(), 1);
        assert_eq!(picked_up[0], (ItemType::Stone, 5));
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_item_manager_merge() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        manager.spawn_item(10.5, 64.0, 20.0, ItemType::Stone, 3);

        let merged = manager.merge_nearby_items();
        assert_eq!(merged, 1);
        assert_eq!(manager.count(), 1);

        // Get the remaining item (should be the first one spawned)
        let items = manager.items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].count, 8);
        assert_eq!(items[0].item_type, ItemType::Stone);
    }
}
