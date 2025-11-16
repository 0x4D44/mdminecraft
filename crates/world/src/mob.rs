//! Passive mob system with deterministic spawning and behavior.
//!
//! Provides biome-specific mob spawning and simple AI for wandering behavior.

use crate::biome::BiomeId;
use crate::chunk::CHUNK_SIZE_X;
use crate::chunk::CHUNK_SIZE_Z;
use serde::{Deserialize, Serialize};

/// Types of passive mobs that can spawn in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobType {
    /// Pig - spawns in plains, forests
    Pig,
    /// Cow - spawns in plains, forests
    Cow,
    /// Sheep - spawns in plains, hills
    Sheep,
    /// Chicken - spawns in plains, forests
    Chicken,
}

impl MobType {
    /// Get mob types that can spawn in a given biome.
    ///
    /// Returns a list of mob types with their relative spawn weights.
    /// Higher weight = more common spawns.
    pub fn for_biome(biome: BiomeId) -> Vec<(MobType, f32)> {
        match biome {
            BiomeId::Plains => vec![
                (MobType::Pig, 10.0),
                (MobType::Cow, 8.0),
                (MobType::Sheep, 12.0),
                (MobType::Chicken, 10.0),
            ],
            BiomeId::Forest | BiomeId::BirchForest => vec![
                (MobType::Pig, 8.0),
                (MobType::Cow, 4.0),
                (MobType::Chicken, 10.0),
            ],
            BiomeId::Hills => vec![
                (MobType::Sheep, 15.0),
                (MobType::Cow, 5.0),
            ],
            BiomeId::Savanna => vec![
                (MobType::Cow, 6.0),
                (MobType::Chicken, 8.0),
            ],
            // No mobs in cold, ocean, or extreme biomes
            _ => vec![],
        }
    }

    /// Get the mob's movement speed in blocks per tick.
    pub fn movement_speed(&self) -> f32 {
        match self {
            MobType::Pig => 0.25,
            MobType::Cow => 0.2,
            MobType::Sheep => 0.23,
            MobType::Chicken => 0.4,
        }
    }

    /// Get the mob's size (bounding box radius).
    pub fn size(&self) -> f32 {
        match self {
            MobType::Pig => 0.45,
            MobType::Cow => 0.7,
            MobType::Sheep => 0.45,
            MobType::Chicken => 0.3,
        }
    }
}

/// A passive mob instance in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mob {
    /// World X position (floating point for smooth movement)
    pub x: f64,
    /// World Y position
    pub y: f64,
    /// World Z position
    pub z: f64,
    /// Velocity in X direction
    pub vel_x: f64,
    /// Velocity in Y direction (for jumping/falling)
    pub vel_y: f64,
    /// Velocity in Z direction
    pub vel_z: f64,
    /// Type of mob
    pub mob_type: MobType,
    /// Internal AI state timer
    pub ai_timer: u32,
    /// Current AI state
    pub state: MobState,
}

/// AI state for mob behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobState {
    /// Standing still
    Idle,
    /// Wandering in a direction
    Wandering,
}

impl Mob {
    /// Create a new mob at the given position.
    pub fn new(x: f64, y: f64, z: f64, mob_type: MobType) -> Self {
        Self {
            x,
            y,
            z,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mob_type,
            ai_timer: 0,
            state: MobState::Idle,
        }
    }

    /// Update the mob's AI and position based on deterministic simulation.
    ///
    /// Uses a simple state machine:
    /// - Idle for 40-80 ticks
    /// - Wander in random direction for 20-60 ticks
    /// - Repeat
    ///
    /// # Arguments
    /// * `tick` - Current simulation tick for deterministic behavior
    pub fn update(&mut self, tick: u64) {
        self.ai_timer += 1;

        match self.state {
            MobState::Idle => {
                // Idle for 40-80 ticks
                let idle_duration = 40 + ((tick + self.x as u64) % 40);
                if self.ai_timer >= idle_duration as u32 {
                    self.state = MobState::Wandering;
                    self.ai_timer = 0;

                    // Choose random direction based on position + tick
                    let angle = ((tick + self.x as u64 + self.z as u64) % 360) as f64 * std::f64::consts::PI / 180.0;
                    let speed = self.mob_type.movement_speed() as f64;
                    self.vel_x = angle.cos() * speed;
                    self.vel_z = angle.sin() * speed;
                }
            }
            MobState::Wandering => {
                // Wander for 20-60 ticks
                let wander_duration = 20 + ((tick + self.z as u64) % 40);
                if self.ai_timer >= wander_duration as u32 {
                    self.state = MobState::Idle;
                    self.ai_timer = 0;
                    self.vel_x = 0.0;
                    self.vel_z = 0.0;
                }
            }
        }

        // Apply velocity to position
        self.x += self.vel_x;
        self.z += self.vel_z;

        // Simple gravity
        if self.vel_y.abs() > 0.01 {
            self.y += self.vel_y;
            self.vel_y -= 0.08; // Gravity acceleration
            self.vel_y *= 0.98; // Air resistance
        }
    }

    /// Get the mob's current chunk position.
    pub fn chunk_pos(&self) -> (i32, i32) {
        let chunk_x = (self.x / CHUNK_SIZE_X as f64).floor() as i32;
        let chunk_z = (self.z / CHUNK_SIZE_Z as f64).floor() as i32;
        (chunk_x, chunk_z)
    }
}

/// Generates spawn positions for passive mobs in a chunk.
pub struct MobSpawner {
    world_seed: u64,
}

impl MobSpawner {
    /// Create a new mob spawner with the given world seed.
    pub fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }

    /// Generate mob spawn positions for a chunk at the given coordinates.
    ///
    /// Spawns are deterministic based on world seed and chunk position.
    /// Spawn density is controlled by biome type.
    ///
    /// # Arguments
    /// * `chunk_x` - Chunk X coordinate
    /// * `chunk_z` - Chunk Z coordinate
    /// * `biome` - Primary biome for this chunk
    /// * `surface_heights` - Height of surface blocks for each (x, z) position
    ///
    /// # Returns
    /// List of mobs to spawn in this chunk with their world positions.
    pub fn generate_spawns(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        biome: BiomeId,
        surface_heights: &[[i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z],
    ) -> Vec<Mob> {
        let mob_types = MobType::for_biome(biome);
        if mob_types.is_empty() {
            return vec![];
        }

        // Calculate total weight for probability
        let total_weight: f32 = mob_types.iter().map(|(_, w)| w).sum();

        let mut mobs = Vec::new();
        let chunk_origin_x = chunk_x * CHUNK_SIZE_X as i32;
        let chunk_origin_z = chunk_z * CHUNK_SIZE_Z as i32;

        // Deterministic pseudo-random based on chunk position and world seed
        let chunk_seed = self.world_seed
            .wrapping_add((chunk_x as u64).wrapping_mul(374761393))
            .wrapping_add((chunk_z as u64).wrapping_mul(668265263));

        // Try to spawn mobs on a grid pattern (every 8 blocks)
        for local_x in (0..CHUNK_SIZE_X).step_by(8) {
            for local_z in (0..CHUNK_SIZE_Z).step_by(8) {
                let pos_seed = chunk_seed
                    .wrapping_add((local_x as u64).wrapping_mul(1103515245))
                    .wrapping_add((local_z as u64).wrapping_mul(12345));

                // Spawn chance: 5% per spawn point
                let spawn_roll = (pos_seed % 100) as f32 / 100.0;
                if spawn_roll > 0.05 {
                    continue;
                }

                // Select mob type based on weights
                let type_roll = ((pos_seed / 100) % 10000) as f32 / 10000.0 * total_weight;
                let mut accumulated = 0.0;
                let mut selected_type = mob_types[0].0;

                for (mob_type, weight) in &mob_types {
                    accumulated += weight;
                    if type_roll <= accumulated {
                        selected_type = *mob_type;
                        break;
                    }
                }

                // Calculate world position
                let world_x = (chunk_origin_x + local_x as i32) as f64 + 0.5;
                let world_z = (chunk_origin_z + local_z as i32) as f64 + 0.5;
                let surface_height = surface_heights[local_z][local_x];
                let world_y = surface_height as f64 + 1.0; // Spawn 1 block above surface

                mobs.push(Mob::new(world_x, world_y, world_z, selected_type));
            }
        }

        mobs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mob_types_for_biome() {
        let plains = MobType::for_biome(BiomeId::Plains);
        assert_eq!(plains.len(), 4);
        assert!(plains.iter().any(|(t, _)| *t == MobType::Pig));

        let forest = MobType::for_biome(BiomeId::Forest);
        assert_eq!(forest.len(), 3);
        assert!(forest.iter().any(|(t, _)| *t == MobType::Chicken));

        let ocean = MobType::for_biome(BiomeId::Ocean);
        assert_eq!(ocean.len(), 0);
    }

    #[test]
    fn test_mob_properties() {
        assert_eq!(MobType::Chicken.movement_speed(), 0.4);
        assert_eq!(MobType::Cow.movement_speed(), 0.2);
        assert_eq!(MobType::Pig.size(), 0.45);
        assert_eq!(MobType::Cow.size(), 0.7);
    }

    #[test]
    fn test_mob_creation() {
        let mob = Mob::new(10.5, 64.0, 20.5, MobType::Pig);
        assert_eq!(mob.x, 10.5);
        assert_eq!(mob.y, 64.0);
        assert_eq!(mob.z, 20.5);
        assert_eq!(mob.mob_type, MobType::Pig);
        assert_eq!(mob.state, MobState::Idle);
        assert_eq!(mob.ai_timer, 0);
    }

    #[test]
    fn test_mob_ai_state_transitions() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);

        // Should stay idle for a while
        for tick in 0..50 {
            mob.update(tick);
            if tick < 40 {
                assert_eq!(mob.state, MobState::Idle);
            }
        }

        // Should eventually transition to wandering
        let mut found_wandering = false;
        for tick in 50..150 {
            mob.update(tick);
            if mob.state == MobState::Wandering {
                found_wandering = true;
                break;
            }
        }
        assert!(found_wandering, "Mob should transition to wandering state");
    }

    #[test]
    fn test_mob_movement() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.state = MobState::Wandering;
        mob.vel_x = 0.25;
        mob.vel_z = 0.0;

        let initial_x = mob.x;
        mob.update(0);

        assert!(mob.x > initial_x, "Mob should move when wandering");
    }

    #[test]
    fn test_mob_chunk_position() {
        let mob = Mob::new(17.5, 64.0, -8.3, MobType::Cow);
        let (chunk_x, chunk_z) = mob.chunk_pos();

        // 17.5 / 16 = 1.09... -> chunk 1
        // -8.3 / 16 = -0.51... -> chunk -1
        assert_eq!(chunk_x, 1);
        assert_eq!(chunk_z, -1);
    }

    #[test]
    fn test_mob_spawner_determinism() {
        let spawner = MobSpawner::new(12345);
        let heights = [[64i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        let mobs1 = spawner.generate_spawns(0, 0, BiomeId::Plains, &heights);
        let mobs2 = spawner.generate_spawns(0, 0, BiomeId::Plains, &heights);

        assert_eq!(mobs1.len(), mobs2.len());
        for (m1, m2) in mobs1.iter().zip(mobs2.iter()) {
            assert_eq!(m1.x, m2.x);
            assert_eq!(m1.y, m2.y);
            assert_eq!(m1.z, m2.z);
            assert_eq!(m1.mob_type, m2.mob_type);
        }
    }

    #[test]
    fn test_mob_spawner_different_chunks() {
        let spawner = MobSpawner::new(12345);
        let heights = [[64i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        let mobs1 = spawner.generate_spawns(0, 0, BiomeId::Plains, &heights);
        let mobs2 = spawner.generate_spawns(1, 0, BiomeId::Plains, &heights);

        // Different chunks should potentially have different mob counts or positions
        let positions_different = mobs1.len() != mobs2.len()
            || mobs1.iter().zip(mobs2.iter()).any(|(m1, m2)| {
                m1.x != m2.x || m1.z != m2.z
            });

        assert!(positions_different, "Different chunks should have different mob spawns");
    }

    #[test]
    fn test_mob_spawner_no_spawn_in_ocean() {
        let spawner = MobSpawner::new(12345);
        let heights = [[64i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        let mobs = spawner.generate_spawns(0, 0, BiomeId::Ocean, &heights);
        assert_eq!(mobs.len(), 0, "No mobs should spawn in ocean biome");
    }

    #[test]
    fn test_mob_spawner_plains_spawns() {
        let spawner = MobSpawner::new(12345);
        let heights = [[64i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        let mobs = spawner.generate_spawns(0, 0, BiomeId::Plains, &heights);

        // Plains should spawn mobs (low chance but should get at least some across multiple attempts)
        // With 5% spawn chance and 4 spawn points per chunk, expected ~0.2 mobs per chunk
        // Over many chunks we should see some spawns
        assert!(mobs.len() <= 10, "Should not spawn excessive mobs");

        // All spawned mobs should be valid plains types
        for mob in &mobs {
            assert!(
                mob.mob_type == MobType::Pig
                    || mob.mob_type == MobType::Cow
                    || mob.mob_type == MobType::Sheep
                    || mob.mob_type == MobType::Chicken,
                "Invalid mob type for plains biome"
            );
        }
    }

    #[test]
    fn test_mob_spawns_on_surface() {
        let spawner = MobSpawner::new(12345);
        let mut heights = [[64i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        // Set specific height at spawn point
        heights[0][0] = 100;

        let mobs = spawner.generate_spawns(0, 0, BiomeId::Plains, &heights);

        // Find any mob that spawned at (0, 0)
        let surface_mob = mobs.iter().find(|m| {
            m.x >= 0.0 && m.x < 1.0 && m.z >= 0.0 && m.z < 1.0
        });

        if let Some(mob) = surface_mob {
            assert_eq!(mob.y, 101.0, "Mob should spawn 1 block above surface");
        }
    }

    #[test]
    fn test_mob_different_seeds_different_spawns() {
        let spawner1 = MobSpawner::new(111);
        let spawner2 = MobSpawner::new(222);
        let heights = [[64i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        let mobs1 = spawner1.generate_spawns(5, 5, BiomeId::Plains, &heights);
        let mobs2 = spawner2.generate_spawns(5, 5, BiomeId::Plains, &heights);

        // Different seeds should produce different results
        let spawns_different = mobs1.len() != mobs2.len()
            || mobs1.iter().zip(mobs2.iter()).any(|(m1, m2)| {
                m1.mob_type != m2.mob_type || m1.x != m2.x || m1.z != m2.z
            });

        assert!(spawns_different, "Different seeds should produce different spawns");
    }
}
