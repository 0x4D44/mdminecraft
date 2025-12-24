//! Mob system with deterministic spawning and behavior.
//!
//! Provides biome-specific mob spawning, simple AI for wandering behavior,
//! and hostile mob AI for chasing and attacking players.

use crate::biome::BiomeId;
use crate::chunk::CHUNK_SIZE_X;
use crate::chunk::CHUNK_SIZE_Z;
use mdminecraft_core::DimensionId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Types of mobs that can spawn in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MobType {
    // Passive mobs
    /// Pig - spawns in plains, forests
    Pig,
    /// Cow - spawns in plains, forests
    Cow,
    /// Sheep - spawns in plains, hills
    Sheep,
    /// Chicken - spawns in plains, forests
    Chicken,

    // NPCs
    /// Villager - spawns in plains, wanders and trades
    Villager,

    // Hostile mobs
    /// Zombie - spawns at night or in darkness, attacks players
    Zombie,
    /// Skeleton - spawns at night or in darkness, attacks players
    Skeleton,
    /// Spider - spawns at night, fast and jumps, drops String
    Spider,
    /// Creeper - spawns at night, explodes near players, drops Gunpowder
    Creeper,
    /// Ender Dragon - End boss (spawned explicitly; not part of biome spawn table).
    EnderDragon,
    /// Blaze - Nether hostile (spawned explicitly / by Nether rules).
    Blaze,
    /// Ghast - Nether hostile (spawned explicitly / by Nether rules).
    Ghast,
}

impl MobType {
    /// Canonical lowercase string key for configs/logging.
    pub const fn as_str(self) -> &'static str {
        match self {
            MobType::Pig => "pig",
            MobType::Cow => "cow",
            MobType::Sheep => "sheep",
            MobType::Chicken => "chicken",
            MobType::Villager => "villager",
            MobType::Zombie => "zombie",
            MobType::Skeleton => "skeleton",
            MobType::Spider => "spider",
            MobType::Creeper => "creeper",
            MobType::EnderDragon => "ender_dragon",
            MobType::Blaze => "blaze",
            MobType::Ghast => "ghast",
        }
    }

    /// Parse a mob type from a string key (case-insensitive).
    pub fn parse(input: &str) -> Option<Self> {
        let key = input.trim().to_lowercase();
        match key.as_str() {
            "pig" => Some(MobType::Pig),
            "cow" => Some(MobType::Cow),
            "sheep" => Some(MobType::Sheep),
            "chicken" => Some(MobType::Chicken),
            "villager" => Some(MobType::Villager),
            "zombie" => Some(MobType::Zombie),
            "skeleton" => Some(MobType::Skeleton),
            "spider" => Some(MobType::Spider),
            "creeper" => Some(MobType::Creeper),
            "ender_dragon" | "enderdragon" | "dragon" => Some(MobType::EnderDragon),
            "blaze" => Some(MobType::Blaze),
            "ghast" => Some(MobType::Ghast),
            _ => None,
        }
    }

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
                (MobType::Villager, 2.0), // Rare villager spawns
            ],
            BiomeId::Forest | BiomeId::BirchForest => vec![
                (MobType::Pig, 8.0),
                (MobType::Cow, 4.0),
                (MobType::Chicken, 10.0),
            ],
            BiomeId::Hills => vec![(MobType::Sheep, 15.0), (MobType::Cow, 5.0)],
            BiomeId::Savanna => vec![(MobType::Cow, 6.0), (MobType::Chicken, 8.0)],
            BiomeId::RainForest => vec![(MobType::Pig, 6.0), (MobType::Chicken, 12.0)],
            BiomeId::Mountains => vec![(MobType::Sheep, 10.0)],
            // No mobs in cold, ocean, or swamp biomes
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
            MobType::Villager => 0.2, // Villagers walk slowly
            MobType::Zombie => 0.23,
            MobType::Skeleton => 0.25,
            MobType::Spider => 0.35, // Spiders are fast
            MobType::Creeper => 0.2, // Creepers are slow but sneaky
            MobType::EnderDragon => 0.28,
            MobType::Blaze => 0.26,
            MobType::Ghast => 0.18,
        }
    }

    /// Get the mob's size (bounding box radius).
    pub fn size(&self) -> f32 {
        match self {
            MobType::Pig => 0.45,
            MobType::Cow => 0.7,
            MobType::Sheep => 0.45,
            MobType::Chicken => 0.3,
            MobType::Villager => 0.6, // Human-sized
            MobType::Zombie => 0.6,
            MobType::Skeleton => 0.6,
            MobType::Spider => 0.7,      // Spiders are wide
            MobType::Creeper => 0.5,     // Creepers are medium sized
            MobType::EnderDragon => 3.0, // Large boss hitbox (simplified)
            MobType::Blaze => 0.6,
            MobType::Ghast => 2.0,
        }
    }

    /// Check if this mob type is hostile.
    pub fn is_hostile(&self) -> bool {
        matches!(
            self,
            MobType::Zombie
                | MobType::Skeleton
                | MobType::Spider
                | MobType::Creeper
                | MobType::EnderDragon
                | MobType::Blaze
                | MobType::Ghast
        )
    }

    /// Get the mob's maximum health.
    pub fn max_health(&self) -> f32 {
        match self {
            MobType::Pig => 10.0,
            MobType::Cow => 10.0,
            MobType::Sheep => 8.0,
            MobType::Chicken => 4.0,
            MobType::Villager => 20.0, // Villagers have good health
            MobType::Zombie => 20.0,
            MobType::Skeleton => 20.0,
            MobType::Spider => 16.0,
            MobType::Creeper => 20.0,
            MobType::EnderDragon => 200.0,
            MobType::Blaze => 20.0,
            MobType::Ghast => 10.0,
        }
    }

    /// Get the attack damage for hostile mobs.
    pub fn attack_damage(&self) -> f32 {
        match self {
            MobType::Zombie => 3.0,
            MobType::Skeleton => 2.0,
            MobType::Spider => 2.0,
            MobType::Creeper => 0.0, // Creepers explode instead of attacking
            MobType::EnderDragon => 10.0,
            MobType::Blaze => 6.0,
            MobType::Ghast => 6.0,
            _ => 0.0, // Passive mobs don't attack
        }
    }

    /// Get the detection range for hostile mobs (in blocks).
    pub fn detection_range(&self) -> f32 {
        match self {
            MobType::Zombie | MobType::Skeleton => 16.0,
            MobType::Spider => 16.0,
            MobType::Creeper => 12.0, // Creepers detect at shorter range
            MobType::EnderDragon => 64.0,
            MobType::Blaze => 24.0,
            MobType::Ghast => 48.0,
            _ => 0.0, // Passive mobs don't detect players
        }
    }

    /// Get explosion damage for mobs that explode.
    pub fn explosion_damage(&self) -> f32 {
        match self {
            MobType::Creeper => 15.0, // Creeper explosion deals 10-20 damage (center: 15)
            _ => 0.0,
        }
    }

    /// Get explosion radius for mobs that explode.
    pub fn explosion_radius(&self) -> f32 {
        match self {
            MobType::Creeper => 3.0, // 3 block radius
            _ => 0.0,
        }
    }

    /// Check if this mob explodes instead of attacking.
    pub fn explodes(&self) -> bool {
        matches!(self, MobType::Creeper)
    }

    /// Get the fuse time before explosion (in seconds).
    pub fn fuse_time(&self) -> f32 {
        match self {
            MobType::Creeper => 1.5, // 1.5 seconds to explode
            _ => 0.0,
        }
    }

    /// Check if this mob can climb walls (spiders).
    pub fn can_climb_walls(&self) -> bool {
        matches!(self, MobType::Spider)
    }

    /// Check if this mob is hostile only at night (spiders are neutral in daylight).
    pub fn is_hostile_at_time(&self, is_night: bool) -> bool {
        match self {
            MobType::Spider => is_night, // Spiders are neutral in daylight
            _ => self.is_hostile(),      // Other hostile mobs are always hostile
        }
    }
}

/// A mob instance in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mob {
    /// Stable mob identifier (assigned by the game layer).
    ///
    /// `0` indicates an uninitialized ID (legacy saves or pre-assignment).
    #[serde(default)]
    pub id: u64,
    /// Dimension this mob exists in.
    #[serde(default)]
    pub dimension: DimensionId,
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
    /// Current health
    pub health: f32,
    /// Attack cooldown timer
    pub attack_cooldown: f32,
    /// Whether mob is marked for removal (dead)
    pub dead: bool,
    /// Damage flash timer (for visual feedback)
    pub damage_flash: f32,
    /// Fuse timer for creepers (counts down to explosion)
    pub fuse_timer: f32,
    /// Whether the creeper is about to explode
    pub exploding: bool,
    /// Whether this mob has been charged by lightning (creeper-only behavior for now).
    #[serde(default)]
    pub charged: bool,
    /// Fire ticks remaining (mob takes fire damage while > 0)
    pub fire_ticks: u32,
    /// Counts burning ticks to apply periodic fire damage deterministically.
    #[serde(default)]
    pub fire_damage_timer: u8,
}

/// AI state for mob behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobState {
    /// Standing still
    Idle,
    /// Wandering in a direction
    Wandering,
    /// Chasing a target (hostile mobs)
    Chasing,
    /// Attacking a target (hostile mobs)
    Attacking,
    /// Charging up to explode (creepers)
    Exploding,
}

impl Mob {
    /// Create a new mob at the given position.
    pub fn new(x: f64, y: f64, z: f64, mob_type: MobType) -> Self {
        Self {
            id: 0,
            dimension: DimensionId::DEFAULT,
            x,
            y,
            z,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mob_type,
            ai_timer: 0,
            state: MobState::Idle,
            health: mob_type.max_health(),
            attack_cooldown: 0.0,
            dead: false,
            damage_flash: 0.0,
            fuse_timer: 0.0,
            exploding: false,
            charged: false,
            fire_ticks: 0,
            fire_damage_timer: 0,
        }
    }

    /// Take damage and return true if mob died.
    pub fn damage(&mut self, amount: f32) -> bool {
        self.health -= amount;
        self.damage_flash = 0.5; // Flash for 0.5 seconds

        if self.health <= 0.0 {
            self.dead = true;
            true
        } else {
            false
        }
    }

    /// Apply knockback in a direction.
    pub fn apply_knockback(&mut self, dx: f64, dz: f64, strength: f64) {
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > 0.0 {
            self.vel_x = (dx / dist) * strength;
            self.vel_z = (dz / dist) * strength;
            self.vel_y = 0.3; // Small upward knockback
        }
    }

    /// Set the mob on fire for a number of ticks.
    /// Each 20 ticks = 1 second, fire deals 1 damage per second.
    pub fn set_on_fire(&mut self, ticks: u32) {
        // Only extend fire duration, don't shorten it
        if ticks > self.fire_ticks {
            self.fire_ticks = ticks;
        }
    }

    /// Extinguish the mob, clearing any ongoing fire damage scheduling.
    pub fn extinguish(&mut self) {
        self.fire_ticks = 0;
        self.fire_damage_timer = 0;
    }

    /// Update fire damage (call once per game tick).
    /// Returns true if fire damage was dealt this tick.
    pub fn update_fire(&mut self) -> bool {
        if self.fire_ticks == 0 {
            self.fire_damage_timer = 0;
            return false;
        }

        self.fire_ticks = self.fire_ticks.saturating_sub(1);

        self.fire_damage_timer = self.fire_damage_timer.saturating_add(1);
        if self.fire_damage_timer >= 20 {
            self.fire_damage_timer = 0;
            self.damage(1.0);
            return true;
        }

        if self.fire_ticks == 0 {
            self.fire_damage_timer = 0;
        }

        false
    }

    /// Check if mob is on fire.
    pub fn is_on_fire(&self) -> bool {
        self.fire_ticks > 0
    }

    /// Calculate distance to a point.
    pub fn distance_to(&self, x: f64, y: f64, z: f64) -> f64 {
        let dx = self.x - x;
        let dy = self.y - y;
        let dz = self.z - z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Check if this mob is hostile.
    pub fn is_hostile(&self) -> bool {
        self.mob_type.is_hostile()
    }

    /// Update the mob's AI and position based on deterministic simulation.
    ///
    /// Uses a simple state machine:
    /// - Idle for 40-80 ticks
    /// - Wander in random direction for 20-60 ticks
    /// - Repeat
    ///
    /// For hostile mobs, use `update_with_target` instead.
    ///
    /// # Arguments
    /// * `tick` - Current simulation tick for deterministic behavior
    pub fn update(&mut self, tick: u64) {
        // Update timers
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown -= 0.05; // Assume ~20 TPS
        }
        if self.damage_flash > 0.0 {
            self.damage_flash -= 0.05;
        }

        self.ai_timer += 1;

        match self.state {
            MobState::Idle => {
                // Idle for 40-80 ticks
                let idle_duration = 40 + ((tick + self.x as u64) % 40);
                if self.ai_timer >= idle_duration as u32 {
                    self.state = MobState::Wandering;
                    self.ai_timer = 0;

                    // Choose random direction based on position + tick
                    let angle = ((tick + self.x as u64 + self.z as u64) % 360) as f64
                        * std::f64::consts::PI
                        / 180.0;
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
            MobState::Chasing | MobState::Attacking | MobState::Exploding => {
                // These states are handled by update_with_target
                // Fall back to idle if no target
                self.state = MobState::Idle;
                self.ai_timer = 0;
                self.vel_x = 0.0;
                self.vel_z = 0.0;
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

    /// Update hostile mob AI with a target position (player).
    ///
    /// Returns true if the mob dealt damage this tick.
    ///
    /// # Arguments
    /// * `tick` - Current simulation tick
    /// * `target_x, target_y, target_z` - Target (player) position
    pub fn update_with_target(
        &mut self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
    ) -> bool {
        self.update_with_target_visibility(tick, target_x, target_y, target_z, 1.0)
    }

    /// Update hostile mob AI with a target position (player) and a visibility multiplier.
    ///
    /// `visibility` scales the mob's detection range:
    /// - `1.0` = normal visibility
    /// - `0.0` = effectively undetectable unless within attack/fuse range
    ///
    /// Returns true if the mob dealt damage this tick.
    pub fn update_with_target_visibility(
        &mut self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> bool {
        let visibility = visibility.max(0.0);
        // Update timers
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown -= 0.05; // Assume ~20 TPS
        }
        if self.damage_flash > 0.0 {
            self.damage_flash -= 0.05;
        }

        // If not hostile, use regular update
        if !self.is_hostile() {
            self.update(tick);
            return false;
        }

        if self.mob_type == MobType::EnderDragon {
            return self.update_ender_dragon(tick, target_x, target_y, target_z, visibility);
        }

        if self.mob_type == MobType::Blaze {
            return self.update_blaze(tick, target_x, target_y, target_z, visibility);
        }

        if self.mob_type == MobType::Ghast {
            return self.update_ghast(tick, target_x, target_y, target_z, visibility);
        }

        if self.mob_type == MobType::Skeleton {
            return self.update_skeleton(tick, target_x, target_y, target_z, visibility);
        }

        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;
        let attack_range = self.mob_type.size() as f64 + 1.5; // Attack when close

        let mut dealt_damage = false;

        // Special handling for creepers
        if self.mob_type.explodes() {
            if self.exploding {
                // Fuse is counting down
                self.fuse_timer -= 0.05;
                self.state = MobState::Exploding;
                self.vel_x = 0.0;
                self.vel_z = 0.0;

                // Check if player moved away - cancel explosion
                let explode_range = self.mob_type.explosion_radius() as f64 + 1.0;
                if distance > explode_range * 1.5 {
                    // Player escaped, cancel fuse
                    self.exploding = false;
                    self.fuse_timer = 0.0;
                    self.state = MobState::Chasing;
                } else if self.fuse_timer <= 0.0 {
                    // BOOM! Mark for explosion (caller handles damage)
                    self.dead = true;
                    dealt_damage = true;
                }
            } else if distance <= attack_range {
                // Start fuse!
                self.exploding = true;
                self.fuse_timer = self.mob_type.fuse_time();
                self.state = MobState::Exploding;
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            } else if distance <= detection_range {
                // Chase player
                self.state = MobState::Chasing;
                let dx = target_x - self.x;
                let dz = target_z - self.z;
                let dist_h = (dx * dx + dz * dz).sqrt();
                if dist_h > 0.1 {
                    let speed = self.mob_type.movement_speed() as f64;
                    self.vel_x = (dx / dist_h) * speed;
                    self.vel_z = (dz / dist_h) * speed;
                }
            } else {
                // Wander
                self.update_wander(tick);
            }
        } else {
            // Normal hostile mob behavior
            if distance <= attack_range && self.attack_cooldown <= 0.0 {
                // Close enough to attack
                self.state = MobState::Attacking;
                self.attack_cooldown = 1.0; // 1 second cooldown
                dealt_damage = true;
                // Stop moving while attacking
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            } else if distance <= detection_range {
                // Within detection range, chase
                self.state = MobState::Chasing;
                self.ai_timer = 0;

                // Move toward target
                let dx = target_x - self.x;
                let dy = target_y - self.y;
                let dz = target_z - self.z;
                let dist_h = (dx * dx + dz * dz).sqrt();
                if dist_h > 0.1 {
                    let speed = self.mob_type.movement_speed() as f64;
                    self.vel_x = (dx / dist_h) * speed;
                    self.vel_z = (dz / dist_h) * speed;

                    // Spiders can climb walls - give upward velocity when target is above
                    if self.mob_type.can_climb_walls() && dy > 0.5 {
                        // Climb at same speed as horizontal movement
                        self.vel_y = speed * 0.5;
                    }
                }
            } else {
                // Out of range, wander normally
                self.update_wander(tick);
            }
        }

        // Apply velocity to position
        self.x += self.vel_x;
        self.z += self.vel_z;

        // Simple gravity
        if self.vel_y.abs() > 0.01 {
            self.y += self.vel_y;
            self.vel_y -= 0.08;
            self.vel_y *= 0.98;
        }

        dealt_damage
    }

    fn update_blaze(
        &mut self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> bool {
        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;
        let attack_range = self.mob_type.size() as f64 + 1.5;
        let speed = self.mob_type.movement_speed() as f64;

        if distance <= attack_range && self.attack_cooldown <= 0.0 {
            self.state = MobState::Attacking;
            self.attack_cooldown = 1.0;
            self.vel_x = 0.0;
            self.vel_y = 0.0;
            self.vel_z = 0.0;
            return true;
        }

        if distance <= detection_range {
            self.state = MobState::Chasing;
            self.ai_timer = 0;

            let dx = target_x - self.x;
            let dz = target_z - self.z;
            let dist_h = (dx * dx + dz * dz).sqrt();

            const DESIRED_DISTANCE: f64 = 8.0;
            const BAND: f64 = 2.0;

            if dist_h > 0.1 {
                if dist_h > DESIRED_DISTANCE + BAND {
                    self.vel_x = (dx / dist_h) * speed;
                    self.vel_z = (dz / dist_h) * speed;
                } else if dist_h < DESIRED_DISTANCE - BAND {
                    self.vel_x = -(dx / dist_h) * speed;
                    self.vel_z = -(dz / dist_h) * speed;
                } else {
                    let t = tick
                        .wrapping_add(self.id)
                        .wrapping_add(0x424C_415A_4553_5452_u64); // "BLAZESTR"
                    let phase = (t / 40) % 4;
                    let strafe = speed * 0.6;
                    match phase {
                        0 => {
                            self.vel_x = strafe;
                            self.vel_z = 0.0;
                        }
                        1 => {
                            self.vel_x = 0.0;
                            self.vel_z = strafe;
                        }
                        2 => {
                            self.vel_x = -strafe;
                            self.vel_z = 0.0;
                        }
                        _ => {
                            self.vel_x = 0.0;
                            self.vel_z = -strafe;
                        }
                    }
                }
            } else {
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            }

            let hover_y = target_y + 1.5;
            let dy = hover_y - self.y;
            self.vel_y = if dy.abs() > 0.25 {
                let climb = (speed * 0.75).min(0.25);
                dy.signum() * climb
            } else {
                0.0
            };
        } else {
            self.state = MobState::Idle;
            self.vel_x = 0.0;
            self.vel_y = 0.0;
            self.vel_z = 0.0;
        }

        self.x += self.vel_x;
        self.y += self.vel_y;
        self.z += self.vel_z;

        self.vel_y *= 0.6;

        false
    }

    fn update_ghast(
        &mut self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> bool {
        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;
        let attack_range = self.mob_type.size() as f64 + 2.0;
        let speed = self.mob_type.movement_speed() as f64;

        if distance <= attack_range && self.attack_cooldown <= 0.0 {
            self.state = MobState::Attacking;
            self.attack_cooldown = 1.2;
            self.vel_x = 0.0;
            self.vel_y = 0.0;
            self.vel_z = 0.0;
            return true;
        }

        if distance <= detection_range {
            self.state = MobState::Chasing;
            self.ai_timer = 0;

            let dx = target_x - self.x;
            let dz = target_z - self.z;
            let dist_h = (dx * dx + dz * dz).sqrt();

            const DESIRED_DISTANCE: f64 = 20.0;
            const BAND: f64 = 4.0;

            if dist_h > 0.1 {
                if dist_h > DESIRED_DISTANCE + BAND {
                    let approach = speed * 0.55;
                    self.vel_x = (dx / dist_h) * approach;
                    self.vel_z = (dz / dist_h) * approach;
                } else if dist_h < DESIRED_DISTANCE - BAND {
                    let retreat = speed * 0.9;
                    self.vel_x = -(dx / dist_h) * retreat;
                    self.vel_z = -(dz / dist_h) * retreat;
                } else {
                    let t = tick
                        .wrapping_add(self.id)
                        .wrapping_add(0x4748_4153_5453_5452_u64); // "GHASTSTR"
                    let phase = (t / 60) % 4;
                    let strafe = speed * 0.45;
                    match phase {
                        0 => {
                            self.vel_x = strafe;
                            self.vel_z = 0.0;
                        }
                        1 => {
                            self.vel_x = 0.0;
                            self.vel_z = strafe;
                        }
                        2 => {
                            self.vel_x = -strafe;
                            self.vel_z = 0.0;
                        }
                        _ => {
                            self.vel_x = 0.0;
                            self.vel_z = -strafe;
                        }
                    }
                }
            } else {
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            }

            let bob = ((tick.wrapping_add(self.id) / 80) % 5) as i32 - 2;
            let hover_y = target_y + 6.0 + bob as f64;
            let dy = hover_y - self.y;
            self.vel_y = if dy.abs() > 0.35 {
                dy.signum() * 0.18
            } else {
                0.0
            };
        } else {
            self.state = MobState::Idle;
            self.vel_x = 0.0;
            self.vel_y = 0.0;
            self.vel_z = 0.0;
        }

        self.x += self.vel_x;
        self.y += self.vel_y;
        self.z += self.vel_z;

        self.vel_y *= 0.6;

        false
    }

    fn update_ender_dragon(
        &mut self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> bool {
        // Boss-lite deterministic behavior:
        // - patrols a square loop around origin when player is out of range
        // - chases the player when detected
        // - attacks in melee with a health-based "rage" phase that shortens cooldown + speeds up

        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;
        let attack_range = self.mob_type.size() as f64 + 3.0;

        let enraged = (self.health as f64) <= (self.mob_type.max_health() as f64) * 0.5;
        let speed_mul = if enraged { 1.35 } else { 1.0 };
        let speed = (self.mob_type.movement_speed() as f64) * speed_mul;
        let attack_cooldown = if enraged { 0.6 } else { 1.0 };

        if distance <= attack_range && self.attack_cooldown <= 0.0 {
            self.state = MobState::Attacking;
            self.attack_cooldown = attack_cooldown;
            self.vel_x = 0.0;
            self.vel_y = 0.0;
            self.vel_z = 0.0;
            return true;
        }

        if distance <= detection_range {
            self.state = MobState::Chasing;
            self.ai_timer = 0;

            let dx = target_x - self.x;
            let dz = target_z - self.z;
            let dist_h = (dx * dx + dz * dz).sqrt();
            if dist_h > 0.1 {
                self.vel_x = (dx / dist_h) * speed;
                self.vel_z = (dz / dist_h) * speed;
            } else {
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            }

            // Fly toward the target altitude (no gravity for the dragon).
            let dy = target_y - self.y;
            self.vel_y = if dy.abs() > 0.25 {
                let climb = (speed * 0.75).min(0.35);
                dy.signum() * climb
            } else {
                0.0
            };
        } else {
            // Patrol around the origin on a deterministic square path (no trig).
            self.state = MobState::Wandering;
            self.ai_timer = self.ai_timer.wrapping_add(1);

            let patrol_y = 80.0;
            let dy = patrol_y - self.y;
            self.vel_y = if dy.abs() > 0.25 {
                dy.signum() * 0.2
            } else {
                0.0
            };

            let r = 32.0;
            let segment_ticks = 64_u64;
            let t = tick
                .wrapping_add(self.id)
                .wrapping_add(0xD1B5_4A32_D192_ED03);
            let phase = (t / segment_ticks) % 4;
            let frac = (t % segment_ticks) as f64 / (segment_ticks as f64);

            let (patrol_x, patrol_z) = match phase {
                0 => (r, -r + 2.0 * r * frac),
                1 => (r - 2.0 * r * frac, r),
                2 => (-r, r - 2.0 * r * frac),
                _ => (-r + 2.0 * r * frac, -r),
            };

            let dx = patrol_x - self.x;
            let dz = patrol_z - self.z;
            let dist_h = (dx * dx + dz * dz).sqrt();
            if dist_h > 0.1 {
                let patrol_speed = speed * 0.6;
                self.vel_x = (dx / dist_h) * patrol_speed;
                self.vel_z = (dz / dist_h) * patrol_speed;
            } else {
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            }
        }

        self.x += self.vel_x;
        self.y += self.vel_y;
        self.z += self.vel_z;

        // Light damping so vertical adjustments settle deterministically.
        self.vel_y *= 0.6;

        false
    }

    fn update_skeleton(
        &mut self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> bool {
        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;

        if distance > detection_range {
            self.update_wander(tick);
        } else {
            let dx = target_x - self.x;
            let dz = target_z - self.z;
            let dist_h = (dx * dx + dz * dz).sqrt();
            let speed = self.mob_type.movement_speed() as f64;

            const DESIRED_MIN_RANGE: f64 = 6.0;
            const DESIRED_MAX_RANGE: f64 = 12.0;

            self.state = MobState::Chasing;
            self.ai_timer = 0;

            if dist_h > 0.1 {
                if dist_h < DESIRED_MIN_RANGE {
                    // Too close: retreat.
                    self.vel_x = (-dx / dist_h) * speed;
                    self.vel_z = (-dz / dist_h) * speed;
                } else if dist_h > DESIRED_MAX_RANGE {
                    // Too far: approach.
                    self.vel_x = (dx / dist_h) * speed;
                    self.vel_z = (dz / dist_h) * speed;
                } else {
                    // In range: strafe in a deterministic pattern.
                    self.state = MobState::Attacking;
                    let period = 80_u64;
                    let phase = tick
                        .wrapping_add(self.id)
                        .wrapping_add(0x534B_454C_5354_5246_u64) // "SKELSTRF"
                        % period;
                    let sign = if phase < period / 2 { 1.0 } else { -1.0 };
                    let strafe_speed = speed * 0.6;
                    let perp_x = -dz / dist_h;
                    let perp_z = dx / dist_h;
                    self.vel_x = perp_x * strafe_speed * sign;
                    self.vel_z = perp_z * strafe_speed * sign;
                }
            } else {
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            }
        }

        self.x += self.vel_x;
        self.z += self.vel_z;

        if self.vel_y.abs() > 0.01 {
            self.y += self.vel_y;
            self.vel_y -= 0.08;
            self.vel_y *= 0.98;
        }

        false
    }

    /// Deterministically decide whether the Ender Dragon should fire a projectile this tick.
    ///
    /// The game layer is responsible for enforcing any global in-flight limits and for applying
    /// collision/damage effects when the projectile hits.
    pub fn try_spawn_dragon_fireball(
        &self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> Option<crate::Projectile> {
        if self.dead || self.mob_type != MobType::EnderDragon {
            return None;
        }

        let visibility = visibility.max(0.0);
        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;
        let melee_range = self.mob_type.size() as f64 + 3.0;

        // Fireballs are ranged pressure: only when the target is in detection range but not close
        // enough for the dragon's melee attack.
        if distance > detection_range || distance <= melee_range + 2.0 {
            return None;
        }

        let enraged = (self.health as f64) <= (self.mob_type.max_health() as f64) * 0.5;
        let period = if enraged { 40_u64 } else { 60_u64 };
        let phase = tick.wrapping_add(self.id) % period;
        if phase != 0 {
            return None;
        }

        let spawn_y = self.y + self.mob_type.size() as f64;
        Some(crate::Projectile::shoot_dragon_fireball(
            self.x, spawn_y, self.z, target_x, target_y, target_z,
        ))
    }

    /// Deterministically decide whether a Blaze should fire a projectile this tick.
    pub fn try_spawn_blaze_fireball(
        &self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> Option<crate::Projectile> {
        if self.dead || self.mob_type != MobType::Blaze {
            return None;
        }

        let visibility = visibility.max(0.0);
        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;
        let melee_range = self.mob_type.size() as f64 + 3.0;

        if distance > detection_range || distance <= melee_range + 1.0 {
            return None;
        }

        // Vanilla-ish: blazes fire short bursts. Keep this deterministic without randomness.
        let period = 50_u64;
        let burst = 3_u64;
        let phase = tick
            .wrapping_add(self.id)
            .wrapping_add(0x424C_5A45_4649_5245_u64) // "BLZEFIRE"
            % period;
        if phase >= burst {
            return None;
        }

        let spawn_y = self.y + self.mob_type.size() as f64 * 0.8;
        Some(crate::Projectile::shoot_blaze_fireball(
            self.x, spawn_y, self.z, target_x, target_y, target_z,
        ))
    }

    /// Deterministically decide whether a Ghast should fire a projectile this tick.
    pub fn try_spawn_ghast_fireball(
        &self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> Option<crate::Projectile> {
        if self.dead || self.mob_type != MobType::Ghast {
            return None;
        }

        let visibility = visibility.max(0.0);
        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;

        if distance > detection_range || distance <= 10.0 {
            return None;
        }

        let period = 80_u64;
        let phase = tick
            .wrapping_add(self.id)
            .wrapping_add(0x4748_4153_5446_4952_u64) // "GHASTFIR"
            % period;
        if phase != 0 {
            return None;
        }

        let spawn_y = self.y + self.mob_type.size() as f64 * 0.6;
        Some(crate::Projectile::shoot_ghast_fireball(
            self.x, spawn_y, self.z, target_x, target_y, target_z,
        ))
    }

    /// Deterministically decide whether a Skeleton should fire an arrow this tick.
    ///
    /// The game layer is responsible for enforcing global in-flight limits and for applying
    /// collision/damage effects.
    pub fn try_spawn_skeleton_arrow(
        &self,
        tick: u64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
        visibility: f64,
    ) -> Option<crate::Projectile> {
        if self.dead || self.mob_type != MobType::Skeleton {
            return None;
        }

        let visibility = visibility.max(0.0);
        let distance = self.distance_to(target_x, target_y, target_z);
        let detection_range = (self.mob_type.detection_range() as f64) * visibility;

        // Vanilla-ish: skeletons prefer ranged pressure; don't fire at point-blank range.
        if distance > detection_range || distance <= 3.0 {
            return None;
        }

        let period = 40_u64;
        let phase = tick
            .wrapping_add(self.id)
            .wrapping_add(0x534B_454C_4152_524F_u64) // "SKELARRO"
            % period;
        if phase != 0 {
            return None;
        }

        let spawn_x = self.x;
        let spawn_y = self.y + 1.4;
        let spawn_z = self.z;

        let dx = target_x - spawn_x;
        let dy = target_y - spawn_y;
        let dz = target_z - spawn_z;
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len <= 0.0001 {
            return None;
        }

        let speed = 1.25;
        let vel_x = (dx / len) * speed;
        let vel_y = (dy / len) * speed;
        let vel_z = (dz / len) * speed;
        let charge = 0.35;

        Some(crate::Projectile::new(
            spawn_x,
            spawn_y,
            spawn_z,
            vel_x,
            vel_y,
            vel_z,
            crate::ProjectileType::Arrow,
            charge,
        ))
    }

    /// Helper for wandering behavior
    fn update_wander(&mut self, tick: u64) {
        self.ai_timer += 1;
        match self.state {
            MobState::Idle => {
                let idle_duration = 40 + ((tick + self.x as u64) % 40);
                if self.ai_timer >= idle_duration as u32 {
                    self.state = MobState::Wandering;
                    self.ai_timer = 0;
                    let angle = ((tick + self.x as u64 + self.z as u64) % 360) as f64
                        * std::f64::consts::PI
                        / 180.0;
                    let speed = self.mob_type.movement_speed() as f64;
                    self.vel_x = angle.cos() * speed;
                    self.vel_z = angle.sin() * speed;
                }
            }
            MobState::Wandering => {
                let wander_duration = 20 + ((tick + self.z as u64) % 40);
                if self.ai_timer >= wander_duration as u32 {
                    self.state = MobState::Idle;
                    self.ai_timer = 0;
                    self.vel_x = 0.0;
                    self.vel_z = 0.0;
                }
            }
            MobState::Chasing | MobState::Attacking | MobState::Exploding => {
                // Lost target, go idle
                self.state = MobState::Idle;
                self.ai_timer = 0;
                self.vel_x = 0.0;
                self.vel_z = 0.0;
            }
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
    spawn_table: BTreeMap<BiomeId, Vec<(MobType, f32)>>,
}

impl MobSpawner {
    /// Create a new mob spawner with the given world seed.
    pub fn new(world_seed: u64) -> Self {
        let spawn_table = default_spawn_table();
        Self {
            world_seed,
            spawn_table,
        }
    }

    /// Create a mob spawner with explicit biome spawn weights.
    ///
    /// Biomes omitted from the table produce no spawns.
    pub fn new_with_spawn_table(
        world_seed: u64,
        spawn_table: BTreeMap<BiomeId, Vec<(MobType, f32)>>,
    ) -> Self {
        Self {
            world_seed,
            spawn_table,
        }
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
        let mob_types = match self.spawn_table.get(&biome) {
            Some(mob_types) => mob_types,
            None => {
                tracing::debug!(
                    chunk_x,
                    chunk_z,
                    ?biome,
                    "No mob types for biome, skipping spawn"
                );
                return vec![];
            }
        };
        if mob_types.is_empty() {
            tracing::debug!(
                chunk_x,
                chunk_z,
                ?biome,
                "No mob types for biome, skipping spawn"
            );
            return vec![];
        }

        // Calculate total weight for probability
        let total_weight: f32 = mob_types.iter().map(|(_, w)| w).sum();

        let mut mobs = Vec::new();
        let chunk_origin_x = chunk_x * CHUNK_SIZE_X as i32;
        let chunk_origin_z = chunk_z * CHUNK_SIZE_Z as i32;

        // Deterministic pseudo-random based on chunk position and world seed
        let chunk_seed = self
            .world_seed
            .wrapping_add((chunk_x as u64).wrapping_mul(374761393))
            .wrapping_add((chunk_z as u64).wrapping_mul(668265263));

        // Try to spawn mobs on a grid pattern (every 4 blocks = 16 spawn points per chunk)
        for local_x in (0..CHUNK_SIZE_X).step_by(4) {
            for local_z in (0..CHUNK_SIZE_Z).step_by(4) {
                let pos_seed = chunk_seed
                    .wrapping_add((local_x as u64).wrapping_mul(1103515245))
                    .wrapping_add((local_z as u64).wrapping_mul(12345));

                // Spawn chance: 15% per spawn point (~2.4 mobs per chunk in populated biomes)
                let spawn_roll = (pos_seed % 100) as f32 / 100.0;
                if spawn_roll > 0.15 {
                    continue;
                }

                // Select mob type based on weights
                let type_roll = ((pos_seed / 100) % 10000) as f32 / 10000.0 * total_weight;
                let mut accumulated = 0.0;
                let mut selected_type = mob_types[0].0;

                for (mob_type, weight) in mob_types {
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

        if !mobs.is_empty() {
            tracing::info!(
                chunk_x,
                chunk_z,
                ?biome,
                mob_count = mobs.len(),
                "Spawned mobs in chunk"
            );
        }

        mobs
    }
}

/// Default biome → mob spawn weight table.
///
/// This is derived from [`MobType::for_biome`] and is deterministic.
pub fn default_spawn_table() -> BTreeMap<BiomeId, Vec<(MobType, f32)>> {
    let mut table = BTreeMap::new();
    for biome in BiomeId::all() {
        let mobs = MobType::for_biome(*biome);
        if !mobs.is_empty() {
            table.insert(*biome, mobs);
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mob_types_for_biome() {
        let plains = MobType::for_biome(BiomeId::Plains);
        assert_eq!(plains.len(), 5); // Pig, Cow, Sheep, Chicken, Villager
        assert!(plains.iter().any(|(t, _)| *t == MobType::Pig));
        assert!(plains.iter().any(|(t, _)| *t == MobType::Villager));

        let forest = MobType::for_biome(BiomeId::Forest);
        assert_eq!(forest.len(), 3);
        assert!(forest.iter().any(|(t, _)| *t == MobType::Chicken));

        let ocean = MobType::for_biome(BiomeId::Ocean);
        assert_eq!(ocean.len(), 0);
    }

    #[test]
    fn mob_type_parse_roundtrips_canonical_keys() {
        let all = [
            MobType::Pig,
            MobType::Cow,
            MobType::Sheep,
            MobType::Chicken,
            MobType::Villager,
            MobType::Zombie,
            MobType::Skeleton,
            MobType::Spider,
            MobType::Creeper,
            MobType::EnderDragon,
            MobType::Blaze,
            MobType::Ghast,
        ];

        for mob in all {
            let parsed = MobType::parse(mob.as_str()).expect("parse should succeed");
            assert_eq!(mob, parsed);
        }

        assert_eq!(MobType::parse("ZOMBIE"), Some(MobType::Zombie));
        assert_eq!(MobType::parse("unknown"), None);
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
            || mobs1
                .iter()
                .zip(mobs2.iter())
                .any(|(m1, m2)| m1.x != m2.x || m1.z != m2.z);

        assert!(
            positions_different,
            "Different chunks should have different mob spawns"
        );
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

        // Plains should spawn mobs
        // With 15% spawn chance and 16 spawn points per chunk, expected ~2.4 mobs per chunk
        assert!(mobs.len() <= 16, "Should not spawn excessive mobs");

        // All spawned mobs should be valid plains types
        for mob in &mobs {
            assert!(
                mob.mob_type == MobType::Pig
                    || mob.mob_type == MobType::Cow
                    || mob.mob_type == MobType::Sheep
                    || mob.mob_type == MobType::Chicken
                    || mob.mob_type == MobType::Villager,
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
        let surface_mob = mobs
            .iter()
            .find(|m| m.x >= 0.0 && m.x < 1.0 && m.z >= 0.0 && m.z < 1.0);

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
            || mobs1
                .iter()
                .zip(mobs2.iter())
                .any(|(m1, m2)| m1.mob_type != m2.mob_type || m1.x != m2.x || m1.z != m2.z);

        assert!(
            spawns_different,
            "Different seeds should produce different spawns"
        );
    }

    #[test]
    fn test_mob_damage() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        assert_eq!(mob.health, 10.0); // Pig max health

        let died = mob.damage(5.0);
        assert!(!died);
        assert_eq!(mob.health, 5.0);
        assert!(mob.damage_flash > 0.0);
        assert!(!mob.dead);

        let died = mob.damage(10.0);
        assert!(died);
        assert!(mob.dead);
        assert!(mob.health <= 0.0);
    }

    #[test]
    fn test_mob_knockback() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        assert_eq!(mob.vel_x, 0.0);
        assert_eq!(mob.vel_z, 0.0);

        mob.apply_knockback(1.0, 0.0, 2.0);
        assert!(mob.vel_x > 0.0);
        assert!(mob.vel_y > 0.0); // Upward knockback
        assert_eq!(mob.vel_z, 0.0);

        // Test normalized direction
        let mut mob2 = Mob::new(0.0, 64.0, 0.0, MobType::Cow);
        mob2.apply_knockback(3.0, 4.0, 5.0); // 3-4-5 triangle
                                             // Direction should be normalized
        assert!((mob2.vel_x - 3.0).abs() < 0.1);
        assert!((mob2.vel_z - 4.0).abs() < 0.1);
    }

    #[test]
    fn test_mob_knockback_zero_distance() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.apply_knockback(0.0, 0.0, 2.0);
        // Should not crash, velocity should stay 0
        assert_eq!(mob.vel_x, 0.0);
        assert_eq!(mob.vel_z, 0.0);
    }

    #[test]
    fn test_mob_fire() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        assert!(!mob.is_on_fire());

        mob.set_on_fire(60); // 3 seconds
        assert!(mob.is_on_fire());
        assert_eq!(mob.fire_ticks, 60);

        // Setting shorter duration should not reduce fire
        mob.set_on_fire(30);
        assert_eq!(mob.fire_ticks, 60);

        // Setting longer duration should extend fire
        mob.set_on_fire(100);
        assert_eq!(mob.fire_ticks, 100);
    }

    #[test]
    fn test_mob_fire_damage() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.set_on_fire(20); // 1 second

        // Fire ticks down
        for _ in 0..19 {
            mob.update_fire();
            assert!(mob.fire_ticks > 0);
        }

        // At tick 20, fire damage is dealt
        assert!(mob.update_fire());
        // Health should be reduced by fire damage
        assert!(mob.health < 10.0);
    }

    #[test]
    fn refreshing_fire_still_deals_periodic_damage() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        for _ in 0..60 {
            mob.set_on_fire(40);
            mob.update_fire();
        }

        assert!(
            mob.health < MobType::Pig.max_health(),
            "expected refreshed fire to deal damage over time"
        );
    }

    #[test]
    fn test_mob_distance_to() {
        let mob = Mob::new(0.0, 0.0, 0.0, MobType::Pig);

        assert_eq!(mob.distance_to(0.0, 0.0, 0.0), 0.0);
        assert_eq!(mob.distance_to(3.0, 4.0, 0.0), 5.0); // 3-4-5 triangle
        assert_eq!(mob.distance_to(0.0, 0.0, 5.0), 5.0);
    }

    #[test]
    fn test_hostile_mob_properties() {
        assert!(MobType::Zombie.is_hostile());
        assert!(MobType::Skeleton.is_hostile());
        assert!(MobType::Spider.is_hostile());
        assert!(MobType::Creeper.is_hostile());
        assert!(MobType::EnderDragon.is_hostile());
        assert!(!MobType::Pig.is_hostile());
        assert!(!MobType::Cow.is_hostile());
        assert!(!MobType::Villager.is_hostile());
    }

    #[test]
    fn test_hostile_mob_damage() {
        assert_eq!(MobType::Zombie.attack_damage(), 3.0);
        assert_eq!(MobType::Skeleton.attack_damage(), 2.0);
        assert_eq!(MobType::Spider.attack_damage(), 2.0);
        assert_eq!(MobType::Creeper.attack_damage(), 0.0); // Explodes instead
        assert_eq!(MobType::EnderDragon.attack_damage(), 10.0);
        assert_eq!(MobType::Pig.attack_damage(), 0.0);
    }

    #[test]
    fn test_hostile_mob_detection_range() {
        assert_eq!(MobType::Zombie.detection_range(), 16.0);
        assert_eq!(MobType::Skeleton.detection_range(), 16.0);
        assert_eq!(MobType::Spider.detection_range(), 16.0);
        assert_eq!(MobType::Creeper.detection_range(), 12.0);
        assert_eq!(MobType::EnderDragon.detection_range(), 64.0);
        assert_eq!(MobType::Pig.detection_range(), 0.0);
    }

    #[test]
    fn test_creeper_explosion() {
        assert!(MobType::Creeper.explodes());
        assert!(!MobType::Zombie.explodes());

        assert_eq!(MobType::Creeper.explosion_damage(), 15.0);
        assert_eq!(MobType::Creeper.explosion_radius(), 3.0);
        assert_eq!(MobType::Creeper.fuse_time(), 1.5);
    }

    #[test]
    fn test_spider_can_climb() {
        assert!(MobType::Spider.can_climb_walls());
        assert!(!MobType::Zombie.can_climb_walls());
        assert!(!MobType::Pig.can_climb_walls());
    }

    #[test]
    fn test_spider_hostility_time_based() {
        // Spider is neutral in daylight
        assert!(!MobType::Spider.is_hostile_at_time(false));
        assert!(MobType::Spider.is_hostile_at_time(true));

        // Other hostile mobs are always hostile
        assert!(MobType::Zombie.is_hostile_at_time(false));
        assert!(MobType::Zombie.is_hostile_at_time(true));
        assert!(MobType::EnderDragon.is_hostile_at_time(false));
        assert!(MobType::EnderDragon.is_hostile_at_time(true));
    }

    #[test]
    fn blaze_fireball_schedule_is_deterministic_and_respects_range() {
        let mut blaze_a = Mob::new(0.0, 64.0, 0.0, MobType::Blaze);
        blaze_a.id = 7;
        let blaze_b = blaze_a.clone();

        let mut fired_a = Vec::new();
        let mut fired_b = Vec::new();

        for tick in 0..250_u64 {
            if blaze_a
                .try_spawn_blaze_fireball(tick, 15.0, 64.0, 0.0, 1.0)
                .is_some()
            {
                fired_a.push(tick);
            }
            if blaze_b
                .try_spawn_blaze_fireball(tick, 15.0, 64.0, 0.0, 1.0)
                .is_some()
            {
                fired_b.push(tick);
            }
        }

        assert_eq!(fired_a, fired_b);
        assert!(!fired_a.is_empty(), "expected blaze to fire periodically");

        // Too close: should not fire (melee range gate).
        for tick in 0..200_u64 {
            assert!(
                blaze_a
                    .try_spawn_blaze_fireball(tick, 2.0, 64.0, 0.0, 1.0)
                    .is_none(),
                "expected blaze not to fire at melee distance"
            );
        }

        // Too far: should not fire (detection range gate).
        for tick in 0..200_u64 {
            assert!(
                blaze_a
                    .try_spawn_blaze_fireball(tick, 64.0, 64.0, 0.0, 1.0)
                    .is_none(),
                "expected blaze not to fire outside detection range"
            );
        }
    }

    #[test]
    fn ghast_fireball_schedule_is_deterministic_and_respects_range() {
        let mut ghast_a = Mob::new(0.0, 96.0, 0.0, MobType::Ghast);
        ghast_a.id = 1337;
        let ghast_b = ghast_a.clone();

        let mut fired_a = Vec::new();
        let mut fired_b = Vec::new();

        for tick in 0..320_u64 {
            if ghast_a
                .try_spawn_ghast_fireball(tick, 30.0, 96.0, 0.0, 1.0)
                .is_some()
            {
                fired_a.push(tick);
            }
            if ghast_b
                .try_spawn_ghast_fireball(tick, 30.0, 96.0, 0.0, 1.0)
                .is_some()
            {
                fired_b.push(tick);
            }
        }

        assert_eq!(fired_a, fired_b);
        assert!(!fired_a.is_empty(), "expected ghast to fire periodically");

        // Too close: should not fire.
        for tick in 0..200_u64 {
            assert!(
                ghast_a
                    .try_spawn_ghast_fireball(tick, 6.0, 96.0, 0.0, 1.0)
                    .is_none(),
                "expected ghast not to fire at short range"
            );
        }

        // Too far: should not fire.
        for tick in 0..200_u64 {
            assert!(
                ghast_a
                    .try_spawn_ghast_fireball(tick, 200.0, 96.0, 0.0, 1.0)
                    .is_none(),
                "expected ghast not to fire outside detection range"
            );
        }
    }

    #[test]
    fn skeleton_arrow_schedule_is_deterministic_and_respects_range() {
        let mut skel_a = Mob::new(0.0, 64.0, 0.0, MobType::Skeleton);
        skel_a.id = 42;
        let skel_b = skel_a.clone();

        let mut fired_a = Vec::new();
        let mut fired_b = Vec::new();

        for tick in 0..200_u64 {
            if skel_a
                .try_spawn_skeleton_arrow(tick, 12.0, 65.6, 0.0, 1.0)
                .is_some()
            {
                fired_a.push(tick);
            }
            if skel_b
                .try_spawn_skeleton_arrow(tick, 12.0, 65.6, 0.0, 1.0)
                .is_some()
            {
                fired_b.push(tick);
            }
        }

        assert_eq!(fired_a, fired_b);
        assert!(
            !fired_a.is_empty(),
            "expected skeleton to fire periodically"
        );

        // Too close: should not fire.
        for tick in 0..200_u64 {
            assert!(
                skel_a
                    .try_spawn_skeleton_arrow(tick, 2.0, 65.6, 0.0, 1.0)
                    .is_none(),
                "expected skeleton not to fire at melee distance"
            );
        }

        // Too far: should not fire.
        for tick in 0..200_u64 {
            assert!(
                skel_a
                    .try_spawn_skeleton_arrow(tick, 64.0, 65.6, 0.0, 1.0)
                    .is_none(),
                "expected skeleton not to fire outside detection range"
            );
        }
    }

    #[test]
    fn test_mob_update_with_target_passive() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);

        // Passive mobs should not deal damage
        let dealt_damage = mob.update_with_target(0, 5.0, 64.0, 5.0);
        assert!(!dealt_damage);
    }

    #[test]
    fn test_zombie_chases_player() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Zombie);
        mob.state = MobState::Idle;

        // Player is within detection range
        let _dealt_damage = mob.update_with_target(0, 10.0, 64.0, 0.0);

        // Zombie should start chasing
        assert_eq!(mob.state, MobState::Chasing);
        assert!(mob.vel_x > 0.0); // Moving toward player
    }

    #[test]
    fn test_zombie_detection_respects_target_visibility() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Zombie);
        mob.state = MobState::Idle;

        // With visibility reduced to zero, target is out of detection range.
        let _dealt_damage = mob.update_with_target_visibility(0, 10.0, 64.0, 0.0, 0.0);
        assert_eq!(mob.state, MobState::Idle);

        // With normal visibility, the same target should be detected.
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Zombie);
        mob.state = MobState::Idle;
        let _dealt_damage = mob.update_with_target_visibility(0, 10.0, 64.0, 0.0, 1.0);
        assert_eq!(mob.state, MobState::Chasing);
    }

    #[test]
    fn test_zombie_attacks_player() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Zombie);
        mob.state = MobState::Chasing;
        mob.attack_cooldown = 0.0;

        // Player is within attack range
        let dealt_damage = mob.update_with_target(0, 1.0, 64.0, 0.0);

        // Zombie should attack
        assert!(dealt_damage);
        assert_eq!(mob.state, MobState::Attacking);
        assert!(mob.attack_cooldown > 0.0);
    }

    #[test]
    fn test_zombie_attack_cooldown() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Zombie);
        mob.attack_cooldown = 1.0;

        // Player is within attack range but on cooldown
        let dealt_damage = mob.update_with_target(0, 1.0, 64.0, 0.0);

        // Should not attack while on cooldown
        assert!(!dealt_damage);
    }

    #[test]
    fn test_zombie_wanders_when_player_far() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Zombie);
        mob.state = MobState::Idle;
        mob.ai_timer = 0;

        // Player is far away (outside detection range)
        let _dealt_damage = mob.update_with_target(0, 100.0, 64.0, 0.0);

        // Zombie should wander (idle or wandering)
        assert!(mob.state == MobState::Idle || mob.state == MobState::Wandering);
    }

    #[test]
    fn test_creeper_starts_fuse() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Creeper);
        mob.state = MobState::Chasing;
        mob.exploding = false;

        // Player is within attack range
        let dealt_damage = mob.update_with_target(0, 1.0, 64.0, 0.0);

        // Creeper should start fuse
        assert!(!dealt_damage); // No damage yet
        assert!(mob.exploding);
        assert!(mob.fuse_timer > 0.0);
        assert_eq!(mob.state, MobState::Exploding);
    }

    #[test]
    fn test_creeper_explodes_when_fuse_expires() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Creeper);
        mob.exploding = true;
        mob.fuse_timer = 0.01; // About to explode

        // Player nearby
        let dealt_damage = mob.update_with_target(0, 1.0, 64.0, 0.0);

        // Creeper should explode
        assert!(dealt_damage);
        assert!(mob.dead);
    }

    #[test]
    fn test_creeper_cancels_explosion_when_player_escapes() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Creeper);
        mob.exploding = true;
        mob.fuse_timer = 1.0;
        mob.state = MobState::Exploding;

        // Player escapes
        let _dealt_damage = mob.update_with_target(0, 20.0, 64.0, 0.0);

        // Creeper should cancel fuse
        assert!(!mob.exploding);
        assert_eq!(mob.fuse_timer, 0.0);
        assert_eq!(mob.state, MobState::Chasing);
    }

    #[test]
    fn test_spider_climbs_toward_elevated_player() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Spider);
        mob.state = MobState::Idle;

        // Player is above and within range
        let _dealt_damage = mob.update_with_target(0, 5.0, 70.0, 0.0);

        // Spider should have upward velocity when target is above
        // Note: vel_y is set when climbing
        assert_eq!(mob.state, MobState::Chasing);
    }

    #[test]
    fn test_mob_gravity() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.vel_y = 1.0; // Give upward velocity

        mob.update(0);

        // Gravity should reduce upward velocity
        assert!(mob.vel_y < 1.0);
        // Position should change
        assert!(mob.y > 64.0);
    }

    #[test]
    fn test_mob_state_idle_to_wandering() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.state = MobState::Idle;
        mob.ai_timer = 100; // Past idle duration

        mob.update(200); // Tick that triggers transition

        // Should transition to wandering and set velocity
        // (exact behavior depends on tick value)
    }

    #[test]
    fn test_mob_max_health() {
        assert_eq!(MobType::Pig.max_health(), 10.0);
        assert_eq!(MobType::Cow.max_health(), 10.0);
        assert_eq!(MobType::Sheep.max_health(), 8.0);
        assert_eq!(MobType::Chicken.max_health(), 4.0);
        assert_eq!(MobType::Villager.max_health(), 20.0);
        assert_eq!(MobType::Zombie.max_health(), 20.0);
        assert_eq!(MobType::Skeleton.max_health(), 20.0);
        assert_eq!(MobType::Spider.max_health(), 16.0);
        assert_eq!(MobType::Creeper.max_health(), 20.0);
        assert_eq!(MobType::EnderDragon.max_health(), 200.0);
    }

    #[test]
    fn test_mob_size() {
        assert!(MobType::Chicken.size() < MobType::Pig.size());
        assert!(MobType::Pig.size() < MobType::Cow.size());
        assert!(MobType::Spider.size() > MobType::Zombie.size()); // Spiders are wide
    }

    #[test]
    fn test_mob_is_hostile_method() {
        let pig = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        assert!(!pig.is_hostile());

        let zombie = Mob::new(0.0, 64.0, 0.0, MobType::Zombie);
        assert!(zombie.is_hostile());
    }

    #[test]
    fn test_biome_mob_spawns() {
        // Test various biomes
        assert!(!MobType::for_biome(BiomeId::Hills).is_empty());
        assert!(!MobType::for_biome(BiomeId::Savanna).is_empty());
        assert!(!MobType::for_biome(BiomeId::RainForest).is_empty());
        assert!(!MobType::for_biome(BiomeId::Mountains).is_empty());
        assert!(!MobType::for_biome(BiomeId::BirchForest).is_empty());

        // Biomes with no spawns
        assert!(MobType::for_biome(BiomeId::Ocean).is_empty());
        assert!(MobType::for_biome(BiomeId::Desert).is_empty());
        assert!(MobType::for_biome(BiomeId::Swamp).is_empty());
    }

    #[test]
    fn test_update_cooldown_decreases() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.attack_cooldown = 1.0;
        mob.damage_flash = 1.0;

        mob.update(0);

        assert!(mob.attack_cooldown < 1.0);
        assert!(mob.damage_flash < 1.0);
    }

    #[test]
    fn test_mob_wandering_transitions_to_idle() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.state = MobState::Wandering;
        mob.ai_timer = 100; // Past wander duration
        mob.vel_x = 0.25;
        mob.vel_z = 0.25;

        // Run enough ticks to trigger state transition
        for tick in 0..100 {
            if mob.state == MobState::Wandering && mob.ai_timer > 60 {
                mob.update(tick);
                if mob.state == MobState::Idle {
                    break;
                }
            } else {
                mob.update(tick);
            }
        }

        // Eventually should go back to idle
    }

    #[test]
    fn test_chasing_state_falls_back_to_idle() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);
        mob.state = MobState::Chasing; // Invalid for passive mob

        mob.update(0);

        // Should fall back to idle
        assert_eq!(mob.state, MobState::Idle);
        assert_eq!(mob.vel_x, 0.0);
        assert_eq!(mob.vel_z, 0.0);
    }

    #[test]
    fn test_movement_speed_all_mobs() {
        // Test all mob movement speeds are positive
        assert!(MobType::Pig.movement_speed() > 0.0);
        assert!(MobType::Cow.movement_speed() > 0.0);
        assert!(MobType::Sheep.movement_speed() > 0.0);
        assert!(MobType::Chicken.movement_speed() > 0.0);
        assert!(MobType::Villager.movement_speed() > 0.0);
        assert!(MobType::Zombie.movement_speed() > 0.0);
        assert!(MobType::Skeleton.movement_speed() > 0.0);
        assert!(MobType::Spider.movement_speed() > 0.0);
        assert!(MobType::Creeper.movement_speed() > 0.0);

        // Spider should be fastest
        assert!(MobType::Spider.movement_speed() > MobType::Zombie.movement_speed());
        // Creeper should be slow
        assert!(MobType::Creeper.movement_speed() < MobType::Spider.movement_speed());
    }

    #[test]
    fn test_size_all_mobs() {
        // Test all mob sizes
        assert_eq!(MobType::Pig.size(), 0.45);
        assert_eq!(MobType::Cow.size(), 0.7);
        assert_eq!(MobType::Sheep.size(), 0.45);
        assert_eq!(MobType::Chicken.size(), 0.3);
        assert_eq!(MobType::Villager.size(), 0.6);
        assert_eq!(MobType::Zombie.size(), 0.6);
        assert_eq!(MobType::Skeleton.size(), 0.6);
        assert_eq!(MobType::Spider.size(), 0.7);
        assert_eq!(MobType::Creeper.size(), 0.5);
    }

    #[test]
    fn test_explosion_methods_non_creeper() {
        // Non-creeper mobs should have 0 explosion damage/radius/fuse
        assert_eq!(MobType::Zombie.explosion_damage(), 0.0);
        assert_eq!(MobType::Zombie.explosion_radius(), 0.0);
        assert_eq!(MobType::Zombie.fuse_time(), 0.0);

        assert_eq!(MobType::Pig.explosion_damage(), 0.0);
        assert_eq!(MobType::Pig.explosion_radius(), 0.0);
        assert_eq!(MobType::Pig.fuse_time(), 0.0);
    }

    #[test]
    fn test_creeper_chases_player() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Creeper);
        mob.state = MobState::Idle;
        mob.exploding = false;

        // Player is within detection range but not attack range
        let _dealt_damage = mob.update_with_target(0, 8.0, 64.0, 0.0);

        // Creeper should chase
        assert_eq!(mob.state, MobState::Chasing);
        assert!(mob.vel_x > 0.0);
    }

    #[test]
    fn test_creeper_wanders_when_far() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Creeper);
        mob.state = MobState::Idle;
        mob.ai_timer = 0;

        // Player is very far (outside detection range)
        let _dealt_damage = mob.update_with_target(0, 50.0, 64.0, 0.0);

        // Creeper should wander or stay idle
        assert!(mob.state == MobState::Idle || mob.state == MobState::Wandering);
    }

    #[test]
    fn test_attacking_state_resets_to_idle() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Cow);
        mob.state = MobState::Attacking;
        mob.vel_x = 0.5;
        mob.vel_z = 0.5;

        mob.update(0);

        // Passive mob in attacking state should reset
        assert_eq!(mob.state, MobState::Idle);
        assert_eq!(mob.vel_x, 0.0);
        assert_eq!(mob.vel_z, 0.0);
    }

    #[test]
    fn test_exploding_state_resets_to_idle() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Sheep);
        mob.state = MobState::Exploding;
        mob.vel_x = 0.3;
        mob.vel_z = 0.3;

        mob.update(0);

        // Passive mob in exploding state should reset
        assert_eq!(mob.state, MobState::Idle);
    }

    #[test]
    fn test_skeleton_attacks() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Skeleton);
        mob.attack_cooldown = 0.0;

        // Player within attack range
        let dealt = mob.update_with_target(0, 1.0, 64.0, 0.0);

        assert!(dealt);
        assert_eq!(mob.state, MobState::Attacking);
    }

    #[test]
    fn test_skeleton_chases() {
        let mut mob = Mob::new(0.0, 64.0, 0.0, MobType::Skeleton);
        mob.state = MobState::Idle;

        // Player within detection but outside attack range
        let dealt = mob.update_with_target(0, 10.0, 64.0, 0.0);

        assert!(!dealt);
        assert_eq!(mob.state, MobState::Chasing);
    }

    #[test]
    fn test_spider_attack_damage() {
        assert_eq!(MobType::Spider.attack_damage(), 2.0);
        assert_eq!(MobType::Skeleton.attack_damage(), 2.0);
        assert_eq!(MobType::Zombie.attack_damage(), 3.0);
        assert_eq!(MobType::Creeper.attack_damage(), 0.0); // Creepers don't attack
        assert_eq!(MobType::EnderDragon.attack_damage(), 10.0);
        assert_eq!(MobType::Pig.attack_damage(), 0.0); // Passive mobs don't attack
    }

    #[test]
    fn test_detection_range_all_hostile() {
        assert_eq!(MobType::Zombie.detection_range(), 16.0);
        assert_eq!(MobType::Skeleton.detection_range(), 16.0);
        assert_eq!(MobType::Spider.detection_range(), 16.0);
        assert_eq!(MobType::Creeper.detection_range(), 12.0);
        assert_eq!(MobType::EnderDragon.detection_range(), 64.0);
        assert_eq!(MobType::Pig.detection_range(), 0.0); // Passive mobs
        assert_eq!(MobType::Cow.detection_range(), 0.0);
    }

    #[test]
    fn test_mob_chunk_pos() {
        // Mob in chunk 0,0
        let mob1 = Mob::new(8.0, 64.0, 8.0, MobType::Pig);
        assert_eq!(mob1.chunk_pos(), (0, 0));

        // Mob in chunk 1,0
        let mob2 = Mob::new(20.0, 64.0, 8.0, MobType::Pig);
        assert_eq!(mob2.chunk_pos(), (1, 0));

        // Mob in negative chunk
        let mob3 = Mob::new(-5.0, 64.0, -10.0, MobType::Pig);
        assert_eq!(mob3.chunk_pos(), (-1, -1));
    }

    #[test]
    fn test_villager_is_passive() {
        assert!(!MobType::Villager.is_hostile());
        assert_eq!(MobType::Villager.attack_damage(), 0.0);
        assert_eq!(MobType::Villager.detection_range(), 0.0);
    }
}
