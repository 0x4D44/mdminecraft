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
    // Block items
    Stone,
    Dirt,
    Grass,
    Sand,
    Gravel,
    Wood,
    Leaves,

    // Mob drops
    RawPork,
    RawBeef,
    Leather,
    Wool,
    Feather,
    Egg,

    // Crafted items (for future use)
    Stick,
    Planks,
}

impl ItemType {
    /// Get the maximum stack size for this item type.
    pub fn max_stack_size(&self) -> u32 {
        match self {
            // Most items stack to 64
            ItemType::Stone
            | ItemType::Dirt
            | ItemType::Grass
            | ItemType::Sand
            | ItemType::Gravel
            | ItemType::Wood
            | ItemType::Leaves
            | ItemType::Wool
            | ItemType::Feather
            | ItemType::Stick
            | ItemType::Planks => 64,

            // Food and resources stack to 16
            ItemType::RawPork | ItemType::RawBeef | ItemType::Leather | ItemType::Egg => 16,
        }
    }

    /// Get the item that drops when a block is broken.
    ///
    /// Returns Some((item_type, count)) or None if nothing drops.
    pub fn from_block(block_id: u16) -> Option<(ItemType, u32)> {
        match block_id {
            1 => Some((ItemType::Stone, 1)),  // Stone block
            2 => Some((ItemType::Dirt, 1)),   // Dirt block
            3 => Some((ItemType::Grass, 1)),  // Grass block -> grass item
            4 => Some((ItemType::Sand, 1)),   // Sand block
            5 => Some((ItemType::Gravel, 1)), // Gravel block
            // TODO: Add more block -> item mappings
            _ => None, // Air, water, etc. don't drop items
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
        assert_eq!(ItemType::Stone.max_stack_size(), 64);
        assert_eq!(ItemType::RawPork.max_stack_size(), 16);
        assert_eq!(ItemType::Feather.max_stack_size(), 64);
    }

    #[test]
    fn test_item_type_from_block() {
        assert_eq!(ItemType::from_block(1), Some((ItemType::Stone, 1)));
        assert_eq!(ItemType::from_block(2), Some((ItemType::Dirt, 1)));
        assert_eq!(ItemType::from_block(0), None); // Air
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
