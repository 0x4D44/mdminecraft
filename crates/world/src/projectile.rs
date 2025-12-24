//! Projectile system for arrows and other ranged attacks.
//!
//! Provides projectile physics, collision detection, and damage calculation.

use mdminecraft_core::DimensionId;
use serde::{Deserialize, Serialize};

/// Types of projectiles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectileType {
    Arrow,
    /// Splash potion - breaks on impact and applies area effect
    /// The u16 is the potion_id (maps to PotionType via potion_ids module)
    SplashPotion(u16),
    /// Ender pearl - breaks on impact and teleports the thrower (handled by game layer).
    EnderPearl,
    /// Dragon fireball - fired by the Ender Dragon (handled by the game layer).
    DragonFireball,
    /// Blaze fireball - fired by Blazes (handled by the game layer).
    BlazeFireball,
    /// Ghast fireball - fired by Ghasts (handled by the game layer).
    GhastFireball,
}

impl ProjectileType {
    /// Get base damage for this projectile type
    pub fn base_damage(&self) -> f32 {
        match self {
            ProjectileType::Arrow => 2.0,
            ProjectileType::SplashPotion(_) => 0.0, // Splash potions don't deal direct damage
            ProjectileType::EnderPearl => 0.0,
            ProjectileType::DragonFireball => 6.0,
            ProjectileType::BlazeFireball => 4.0,
            ProjectileType::GhastFireball => 8.0,
        }
    }

    /// Get gravity strength for this projectile
    pub fn gravity(&self) -> f64 {
        match self {
            ProjectileType::Arrow => 0.05,
            ProjectileType::SplashPotion(_) => 0.06, // Slightly higher gravity for potions
            ProjectileType::EnderPearl => 0.06,
            ProjectileType::DragonFireball => 0.0,
            ProjectileType::BlazeFireball => 0.0,
            ProjectileType::GhastFireball => 0.0,
        }
    }

    /// Get drag coefficient (velocity multiplier per tick)
    pub fn drag(&self) -> f64 {
        match self {
            ProjectileType::Arrow => 0.99,
            ProjectileType::SplashPotion(_) => 0.98, // Slightly more drag
            ProjectileType::EnderPearl => 0.98,
            ProjectileType::DragonFireball => 0.99,
            ProjectileType::BlazeFireball => 0.99,
            ProjectileType::GhastFireball => 0.99,
        }
    }

    /// Get the projectile's hitbox radius
    pub fn hitbox_radius(&self) -> f64 {
        match self {
            ProjectileType::Arrow => 0.3,
            ProjectileType::SplashPotion(_) => 0.25,
            ProjectileType::EnderPearl => 0.25,
            ProjectileType::DragonFireball => 0.6,
            ProjectileType::BlazeFireball => 0.35,
            ProjectileType::GhastFireball => 0.9,
        }
    }

    /// How long the projectile lives (in ticks at 20 TPS)
    pub fn lifetime_ticks(&self) -> u32 {
        match self {
            ProjectileType::Arrow => 1200,          // 60 seconds
            ProjectileType::SplashPotion(_) => 600, // 30 seconds (should break on impact much sooner)
            ProjectileType::EnderPearl => 600,
            ProjectileType::DragonFireball => 200,
            ProjectileType::BlazeFireball => 160,
            ProjectileType::GhastFireball => 240,
        }
    }

    /// Check if this projectile is a splash potion
    pub fn is_splash_potion(&self) -> bool {
        matches!(self, ProjectileType::SplashPotion(_))
    }

    /// Get the potion ID if this is a splash potion
    pub fn potion_id(&self) -> Option<u16> {
        match self {
            ProjectileType::SplashPotion(id) => Some(*id),
            _ => None,
        }
    }

    /// Get the effect radius for area-of-effect projectiles
    pub fn effect_radius(&self) -> f64 {
        match self {
            ProjectileType::Arrow => 0.0,
            ProjectileType::SplashPotion(_) => 4.0, // 4 block radius for splash effect
            ProjectileType::EnderPearl => 0.0,
            ProjectileType::DragonFireball => 2.5,
            ProjectileType::BlazeFireball => 1.5,
            ProjectileType::GhastFireball => 3.0,
        }
    }
}

/// A projectile instance in the world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    /// Dimension this projectile exists in.
    #[serde(default)]
    pub dimension: DimensionId,
    /// Previous world X position (not persisted; used for per-tick collision sweep).
    #[serde(skip)]
    pub prev_x: f64,
    /// Previous world Y position (not persisted; used for per-tick collision sweep).
    #[serde(skip)]
    pub prev_y: f64,
    /// Previous world Z position (not persisted; used for per-tick collision sweep).
    #[serde(skip)]
    pub prev_z: f64,
    /// World X position
    pub x: f64,
    /// World Y position
    pub y: f64,
    /// World Z position
    pub z: f64,
    /// Velocity X
    pub vel_x: f64,
    /// Velocity Y
    pub vel_y: f64,
    /// Velocity Z
    pub vel_z: f64,
    /// Type of projectile
    pub projectile_type: ProjectileType,
    /// Age in ticks
    pub age: u32,
    /// Whether this projectile is stuck in a block
    pub stuck: bool,
    /// Whether this projectile hit an entity
    pub hit_entity: bool,
    /// Charge level when fired (0.0 to 1.0, affects damage and speed)
    pub charge: f32,
    /// Bow enchantment: Power level applied to this arrow (0 when unenchanted).
    #[serde(default)]
    pub power_level: u8,
    /// Bow enchantment: Punch level applied to this arrow (0 when unenchanted).
    #[serde(default)]
    pub punch_level: u8,
    /// Bow enchantment: Flame applied to this arrow.
    #[serde(default)]
    pub flame: bool,
    /// Whether this projectile should spawn an item pickup (e.g., arrow pickup) on block impact.
    #[serde(default = "default_true")]
    pub can_pick_up: bool,
    /// Whether the projectile should be removed
    pub dead: bool,
}

fn default_true() -> bool {
    true
}

impl Projectile {
    /// Create a new projectile
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x: f64,
        y: f64,
        z: f64,
        vel_x: f64,
        vel_y: f64,
        vel_z: f64,
        projectile_type: ProjectileType,
        charge: f32,
    ) -> Self {
        Self {
            dimension: DimensionId::DEFAULT,
            prev_x: x,
            prev_y: y,
            prev_z: z,
            x,
            y,
            z,
            vel_x,
            vel_y,
            vel_z,
            projectile_type,
            age: 0,
            stuck: false,
            hit_entity: false,
            charge: charge.clamp(0.1, 1.0),
            power_level: 0,
            punch_level: 0,
            flame: false,
            can_pick_up: true,
            dead: false,
        }
    }

    /// Create an arrow from player position and look direction
    pub fn shoot_arrow(
        player_x: f64,
        player_y: f64,
        player_z: f64,
        yaw: f32,
        pitch: f32,
        charge: f32,
    ) -> Self {
        // Calculate direction from yaw/pitch
        let pitch_rad = pitch as f64;
        let yaw_rad = yaw as f64;

        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        // Direction vector (looking direction)
        let dir_x = cos_pitch * cos_yaw;
        let dir_y = -sin_pitch;
        let dir_z = cos_pitch * sin_yaw;

        // Base arrow speed, scaled by charge (0.1 to 1.0)
        let speed = 1.5 * charge as f64;

        // Spawn slightly in front of player and at eye level
        let spawn_x = player_x + dir_x * 0.5;
        let spawn_y = player_y + 1.5 + dir_y * 0.5; // Eye level
        let spawn_z = player_z + dir_z * 0.5;

        Self::new(
            spawn_x,
            spawn_y,
            spawn_z,
            dir_x * speed,
            dir_y * speed,
            dir_z * speed,
            ProjectileType::Arrow,
            charge,
        )
    }

    /// Create a thrown splash potion from player position and look direction
    pub fn throw_splash_potion(
        player_x: f64,
        player_y: f64,
        player_z: f64,
        yaw: f32,
        pitch: f32,
        potion_id: u16,
    ) -> Self {
        // Calculate direction from yaw/pitch
        let pitch_rad = pitch as f64;
        let yaw_rad = yaw as f64;

        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        // Direction vector (looking direction)
        let dir_x = cos_pitch * cos_yaw;
        let dir_y = -sin_pitch;
        let dir_z = cos_pitch * sin_yaw;

        // Potion throw speed (fixed, not charged like arrows)
        let speed = 0.8;

        // Spawn slightly in front of player and at eye level
        let spawn_x = player_x + dir_x * 0.5;
        let spawn_y = player_y + 1.5 + dir_y * 0.5; // Eye level
        let spawn_z = player_z + dir_z * 0.5;

        Self::new(
            spawn_x,
            spawn_y,
            spawn_z,
            dir_x * speed,
            dir_y * speed + 0.2, // Add slight upward arc
            dir_z * speed,
            ProjectileType::SplashPotion(potion_id),
            1.0, // Full "charge" for potions (doesn't affect effect)
        )
    }

    /// Create a thrown ender pearl from player position and look direction.
    pub fn throw_ender_pearl(
        player_x: f64,
        player_y: f64,
        player_z: f64,
        yaw: f32,
        pitch: f32,
    ) -> Self {
        // Calculate direction from yaw/pitch
        let pitch_rad = pitch as f64;
        let yaw_rad = yaw as f64;

        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        // Direction vector (looking direction)
        let dir_x = cos_pitch * cos_yaw;
        let dir_y = -sin_pitch;
        let dir_z = cos_pitch * sin_yaw;

        // Similar feel to splash potions: fixed throw speed with a slight upward arc.
        let speed = 0.9;

        // Spawn slightly in front of player and at eye level
        let spawn_x = player_x + dir_x * 0.5;
        let spawn_y = player_y + 1.5 + dir_y * 0.5; // Eye level
        let spawn_z = player_z + dir_z * 0.5;

        Self::new(
            spawn_x,
            spawn_y,
            spawn_z,
            dir_x * speed,
            dir_y * speed + 0.2,
            dir_z * speed,
            ProjectileType::EnderPearl,
            1.0,
        )
    }

    /// Create a dragon fireball from a mob position toward a target point.
    pub fn shoot_dragon_fireball(
        from_x: f64,
        from_y: f64,
        from_z: f64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
    ) -> Self {
        let dx = target_x - from_x;
        let dy = target_y - from_y;
        let dz = target_z - from_z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let (dir_x, dir_y, dir_z) = if dist > 1e-6 {
            (dx / dist, dy / dist, dz / dist)
        } else {
            (1.0, 0.0, 0.0)
        };

        // Fixed speed for deterministic feel parity (no charge mechanic).
        let speed = 0.9;

        Self::new(
            from_x + dir_x * 1.5,
            from_y + dir_y * 1.5,
            from_z + dir_z * 1.5,
            dir_x * speed,
            dir_y * speed,
            dir_z * speed,
            ProjectileType::DragonFireball,
            1.0,
        )
    }

    /// Create a blaze fireball from a mob position toward a target point.
    pub fn shoot_blaze_fireball(
        from_x: f64,
        from_y: f64,
        from_z: f64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
    ) -> Self {
        let dx = target_x - from_x;
        let dy = target_y - from_y;
        let dz = target_z - from_z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let (dir_x, dir_y, dir_z) = if dist > 1e-6 {
            (dx / dist, dy / dist, dz / dist)
        } else {
            (1.0, 0.0, 0.0)
        };

        let speed = 1.0;

        Self::new(
            from_x + dir_x * 1.2,
            from_y + dir_y * 1.2,
            from_z + dir_z * 1.2,
            dir_x * speed,
            dir_y * speed,
            dir_z * speed,
            ProjectileType::BlazeFireball,
            1.0,
        )
    }

    /// Create a ghast fireball from a mob position toward a target point.
    pub fn shoot_ghast_fireball(
        from_x: f64,
        from_y: f64,
        from_z: f64,
        target_x: f64,
        target_y: f64,
        target_z: f64,
    ) -> Self {
        let dx = target_x - from_x;
        let dy = target_y - from_y;
        let dz = target_z - from_z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let (dir_x, dir_y, dir_z) = if dist > 1e-6 {
            (dx / dist, dy / dist, dz / dist)
        } else {
            (1.0, 0.0, 0.0)
        };

        let speed = 0.8;

        Self::new(
            from_x + dir_x * 1.8,
            from_y + dir_y * 1.8,
            from_z + dir_z * 1.8,
            dir_x * speed,
            dir_y * speed,
            dir_z * speed,
            ProjectileType::GhastFireball,
            1.0,
        )
    }

    /// Update projectile physics
    /// Returns true if the projectile should be removed
    pub fn update(&mut self) -> bool {
        if self.dead {
            return true;
        }

        self.age += 1;

        // Check lifetime
        if self.age >= self.projectile_type.lifetime_ticks() {
            self.dead = true;
            return true;
        }

        self.prev_x = self.x;
        self.prev_y = self.y;
        self.prev_z = self.z;

        // If stuck, don't move
        if self.stuck {
            return false;
        }

        // Apply velocity
        self.x += self.vel_x;
        self.y += self.vel_y;
        self.z += self.vel_z;

        // Apply gravity
        self.vel_y -= self.projectile_type.gravity();

        // Apply drag
        let drag = self.projectile_type.drag();
        self.vel_x *= drag;
        self.vel_y *= drag;
        self.vel_z *= drag;

        false
    }

    /// Get the damage this projectile deals
    pub fn damage(&self) -> f32 {
        match self.projectile_type {
            ProjectileType::Arrow => {
                // Damage scales with charge level: 1-10 damage based on draw time
                // charge ranges from 0.1 to 1.0
                // Min damage at 0.1 charge: 1.0
                // Max damage at 1.0 charge: 10.0
                let mut damage = 1.0 + self.charge * 9.0;

                // Vanilla-ish: Power increases arrow damage. Approximated as a flat bonus.
                let power_level = self.power_level.min(5);
                if power_level > 0 {
                    damage += 0.5 + 0.5 * (power_level as f32);
                }

                damage
            }
            _ => self.projectile_type.base_damage(),
        }
    }

    /// Check if this projectile hits a point (simple distance check)
    pub fn hits_point(&self, px: f64, py: f64, pz: f64, radius: f64) -> bool {
        if self.stuck || self.dead {
            return false;
        }

        let dx = self.x - px;
        let dy = self.y - py;
        let dz = self.z - pz;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        let hit_radius = self.projectile_type.hitbox_radius() + radius;
        dist_sq < hit_radius * hit_radius
    }

    /// Mark as stuck in a block
    pub fn stick(&mut self) {
        self.stuck = true;
        self.vel_x = 0.0;
        self.vel_y = 0.0;
        self.vel_z = 0.0;
    }

    /// Mark as having hit an entity
    pub fn hit(&mut self) {
        self.hit_entity = true;
        self.dead = true;
    }

    /// Get velocity magnitude
    pub fn speed(&self) -> f64 {
        (self.vel_x * self.vel_x + self.vel_y * self.vel_y + self.vel_z * self.vel_z).sqrt()
    }
}

/// Manages projectiles in the world
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectileManager {
    pub projectiles: Vec<Projectile>,
}

impl ProjectileManager {
    /// Create a new projectile manager
    pub fn new() -> Self {
        Self {
            projectiles: Vec::new(),
        }
    }

    /// Spawn a new projectile
    pub fn spawn(&mut self, dimension: DimensionId, mut projectile: Projectile) {
        projectile.dimension = dimension;
        self.projectiles.push(projectile);
    }

    /// Update all projectiles and remove dead ones
    pub fn update(&mut self, dimension: DimensionId) {
        self.projectiles.retain_mut(|projectile| {
            if projectile.dimension != dimension {
                return !projectile.dead;
            }

            !projectile.update()
        });
    }

    /// Check for collision with a point (mob/player position)
    /// Returns the damage if hit
    pub fn check_hit(
        &mut self,
        dimension: DimensionId,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Option<f32> {
        for projectile in &mut self.projectiles {
            if projectile.dimension != dimension {
                continue;
            }
            if projectile.hits_point(x, y, z, radius) {
                let damage = projectile.damage();
                projectile.hit();
                return Some(damage);
            }
        }
        None
    }

    /// Get number of active projectiles
    pub fn count(&self) -> usize {
        self.projectiles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrow_creation() {
        let arrow = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        assert_eq!(arrow.projectile_type, ProjectileType::Arrow);
        assert!(!arrow.stuck);
        assert!(!arrow.dead);
        assert!(arrow.speed() > 0.0);
    }

    #[test]
    fn test_arrow_physics() {
        let mut arrow = Projectile::shoot_arrow(0.0, 100.0, 0.0, 0.0, 0.0, 1.0);
        let initial_y = arrow.y;

        // Update several times
        for _ in 0..10 {
            arrow.update();
        }

        // Arrow should have fallen due to gravity
        assert!(arrow.y < initial_y);
    }

    #[test]
    fn test_arrow_damage() {
        // Full charge: 1 + 1.0 * 9 = 10
        let arrow_full = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        assert_eq!(arrow_full.damage(), 10.0);

        // Half charge: 1 + 0.5 * 9 = 5.5
        let arrow_half = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 0.5);
        assert_eq!(arrow_half.damage(), 5.5);

        // Min charge (clamped to 0.1): 1 + 0.1 * 9 = 1.9
        let arrow_min = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((arrow_min.damage() - 1.9).abs() < 0.01);
    }

    #[test]
    fn test_arrow_damage_power_bonus() {
        let mut arrow = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        arrow.power_level = 3;
        // Base 10.0 + (0.5 + 0.5*3) = 12.0
        assert!((arrow.damage() - 12.0).abs() < 1e-6);
    }

    #[test]
    fn test_projectile_hit() {
        let mut manager = ProjectileManager::new();
        let arrow = Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        manager.spawn(DimensionId::Overworld, arrow);

        // Should hit nearby point
        let damage = manager.check_hit(DimensionId::Overworld, 5.0, 5.0, 5.0, 0.5);
        assert!(damage.is_some());

        // Arrow should be dead after hit
        assert!(manager.projectiles[0].dead);
    }

    #[test]
    fn test_projectile_lifetime() {
        let mut projectile =
            Projectile::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        projectile.age = 1198;

        assert!(!projectile.update()); // age becomes 1199, not dead yet
        assert!(projectile.update()); // age becomes 1200, should be dead now (age >= 1200)
    }

    #[test]
    fn test_projectile_type_properties() {
        // Arrow properties
        assert_eq!(ProjectileType::Arrow.base_damage(), 2.0);
        assert_eq!(ProjectileType::Arrow.gravity(), 0.05);
        assert_eq!(ProjectileType::Arrow.drag(), 0.99);
        assert_eq!(ProjectileType::Arrow.hitbox_radius(), 0.3);
        assert_eq!(ProjectileType::Arrow.lifetime_ticks(), 1200);
        assert!(!ProjectileType::Arrow.is_splash_potion());
        assert_eq!(ProjectileType::Arrow.potion_id(), None);
        assert_eq!(ProjectileType::Arrow.effect_radius(), 0.0);

        // Splash potion properties
        let splash = ProjectileType::SplashPotion(42);
        assert_eq!(splash.base_damage(), 0.0);
        assert_eq!(splash.gravity(), 0.06);
        assert_eq!(splash.drag(), 0.98);
        assert_eq!(splash.hitbox_radius(), 0.25);
        assert_eq!(splash.lifetime_ticks(), 600);
        assert!(splash.is_splash_potion());
        assert_eq!(splash.potion_id(), Some(42));
        assert_eq!(splash.effect_radius(), 4.0);

        // Ender pearl properties
        assert_eq!(ProjectileType::EnderPearl.base_damage(), 0.0);
        assert_eq!(ProjectileType::EnderPearl.gravity(), 0.06);
        assert_eq!(ProjectileType::EnderPearl.drag(), 0.98);
        assert_eq!(ProjectileType::EnderPearl.hitbox_radius(), 0.25);
        assert_eq!(ProjectileType::EnderPearl.lifetime_ticks(), 600);
        assert!(!ProjectileType::EnderPearl.is_splash_potion());
        assert_eq!(ProjectileType::EnderPearl.potion_id(), None);
        assert_eq!(ProjectileType::EnderPearl.effect_radius(), 0.0);

        // Dragon fireball properties
        assert_eq!(ProjectileType::DragonFireball.base_damage(), 6.0);
        assert_eq!(ProjectileType::DragonFireball.gravity(), 0.0);
        assert_eq!(ProjectileType::DragonFireball.drag(), 0.99);
        assert_eq!(ProjectileType::DragonFireball.hitbox_radius(), 0.6);
        assert_eq!(ProjectileType::DragonFireball.lifetime_ticks(), 200);
        assert!(!ProjectileType::DragonFireball.is_splash_potion());
        assert_eq!(ProjectileType::DragonFireball.potion_id(), None);
        assert_eq!(ProjectileType::DragonFireball.effect_radius(), 2.5);
    }

    #[test]
    fn test_arrow_charge_clamping() {
        // Charge should clamp to 0.1-1.0
        let low = Projectile::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 0.0);
        assert_eq!(low.charge, 0.1);

        let high = Projectile::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 2.0);
        assert_eq!(high.charge, 1.0);
    }

    #[test]
    fn test_splash_potion_creation() {
        let potion = Projectile::throw_splash_potion(0.0, 0.0, 0.0, 0.0, 0.0, 123);
        assert!(matches!(
            potion.projectile_type,
            ProjectileType::SplashPotion(123)
        ));
        assert!(potion.speed() > 0.0);
        assert!(!potion.stuck);
        assert!(!potion.dead);
    }

    #[test]
    fn test_ender_pearl_creation() {
        let pearl = Projectile::throw_ender_pearl(0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(pearl.projectile_type, ProjectileType::EnderPearl);
        assert!(pearl.speed() > 0.0);
        assert!(!pearl.stuck);
        assert!(!pearl.dead);
    }

    #[test]
    fn test_dragon_fireball_creation() {
        let fireball = Projectile::shoot_dragon_fireball(0.0, 80.0, 0.0, 10.0, 80.0, 0.0);
        assert_eq!(fireball.projectile_type, ProjectileType::DragonFireball);
        assert!(fireball.speed() > 0.0);
        assert!(!fireball.stuck);
        assert!(!fireball.dead);
    }

    #[test]
    fn test_projectile_stick() {
        let mut arrow = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        assert!(arrow.speed() > 0.0);

        arrow.stick();
        assert!(arrow.stuck);
        assert_eq!(arrow.vel_x, 0.0);
        assert_eq!(arrow.vel_y, 0.0);
        assert_eq!(arrow.vel_z, 0.0);
        assert_eq!(arrow.speed(), 0.0);
    }

    #[test]
    fn test_stuck_projectile_no_movement() {
        let mut arrow = Projectile::shoot_arrow(0.0, 100.0, 0.0, 0.0, 0.0, 1.0);
        let initial_y = arrow.y;
        arrow.stick();

        // Update should not move stuck projectile
        arrow.update();
        assert_eq!(arrow.y, initial_y);
    }

    #[test]
    fn test_projectile_hit_method() {
        let mut arrow = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        assert!(!arrow.hit_entity);
        assert!(!arrow.dead);

        arrow.hit();
        assert!(arrow.hit_entity);
        assert!(arrow.dead);
    }

    #[test]
    fn test_projectile_hits_point_basic() {
        let arrow = Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);

        // Should hit at exact position
        assert!(arrow.hits_point(5.0, 5.0, 5.0, 0.5));

        // Should hit nearby
        assert!(arrow.hits_point(5.2, 5.0, 5.0, 0.5));

        // Should miss far away
        assert!(!arrow.hits_point(10.0, 10.0, 10.0, 0.5));
    }

    #[test]
    fn test_stuck_projectile_no_hit() {
        let mut arrow = Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        arrow.stuck = true;

        // Stuck projectile shouldn't hit anything
        assert!(!arrow.hits_point(5.0, 5.0, 5.0, 0.5));
    }

    #[test]
    fn test_dead_projectile_no_hit() {
        let mut arrow = Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        arrow.dead = true;

        assert!(!arrow.hits_point(5.0, 5.0, 5.0, 0.5));
    }

    #[test]
    fn test_dead_projectile_update() {
        let mut arrow = Projectile::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        arrow.dead = true;

        // Should return true immediately when dead
        assert!(arrow.update());
    }

    #[test]
    fn test_projectile_manager_new() {
        let manager = ProjectileManager::new();
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_projectile_manager_default() {
        let manager = ProjectileManager::default();
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_projectile_manager_spawn() {
        let mut manager = ProjectileManager::new();
        manager.spawn(
            DimensionId::Overworld,
            Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        );
        manager.spawn(
            DimensionId::Overworld,
            Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        );
        assert_eq!(manager.count(), 2);
    }

    #[test]
    fn test_projectile_manager_update_removes_dead() {
        let mut manager = ProjectileManager::new();

        let mut arrow = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        arrow.dead = true;
        manager.spawn(DimensionId::Overworld, arrow);

        manager.update(DimensionId::Overworld);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_projectile_manager_check_hit_miss() {
        let mut manager = ProjectileManager::new();
        let arrow = Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        manager.spawn(DimensionId::Overworld, arrow);

        // Miss
        let damage = manager.check_hit(DimensionId::Overworld, 100.0, 100.0, 100.0, 0.5);
        assert!(damage.is_none());
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_projectile_manager_lifetime_expiry() {
        let mut manager = ProjectileManager::new();
        let mut arrow = Projectile::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        arrow.age = 1199;
        manager.spawn(DimensionId::Overworld, arrow);

        // Should be removed after update (age reaches 1200)
        manager.update(DimensionId::Overworld);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_projectile_manager_update_is_dimension_scoped() {
        let mut manager = ProjectileManager::new();
        manager.spawn(
            DimensionId::Overworld,
            Projectile::new(0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ProjectileType::Arrow, 1.0),
        );
        manager.spawn(
            DimensionId::Nether,
            Projectile::new(0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ProjectileType::Arrow, 1.0),
        );

        manager.update(DimensionId::Overworld);

        let overworld_age = manager
            .projectiles
            .iter()
            .find(|p| p.dimension == DimensionId::Overworld)
            .expect("overworld projectile exists")
            .age;
        let nether_age = manager
            .projectiles
            .iter()
            .find(|p| p.dimension == DimensionId::Nether)
            .expect("nether projectile exists")
            .age;

        assert_eq!(overworld_age, 1);
        assert_eq!(nether_age, 0);
    }

    #[test]
    fn test_projectile_manager_check_hit_is_dimension_scoped() {
        let mut manager = ProjectileManager::new();
        manager.spawn(
            DimensionId::Overworld,
            Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0),
        );
        manager.spawn(
            DimensionId::Nether,
            Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0),
        );

        assert!(manager
            .check_hit(DimensionId::Overworld, 5.0, 5.0, 5.0, 0.5)
            .is_some());
        assert!(manager
            .projectiles
            .iter()
            .any(|p| p.dimension == DimensionId::Overworld && p.dead));
        assert!(manager
            .projectiles
            .iter()
            .any(|p| p.dimension == DimensionId::Nether && !p.dead));

        assert!(manager
            .check_hit(DimensionId::Nether, 5.0, 5.0, 5.0, 0.5)
            .is_some());
        assert!(manager
            .projectiles
            .iter()
            .any(|p| p.dimension == DimensionId::Nether && p.dead));
    }

    #[test]
    fn test_arrow_trajectory_downward() {
        // Positive pitch = looking up = dir_y = -sin(pitch) = negative
        let arrow = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, std::f32::consts::FRAC_PI_4, 1.0);
        // With the game's coordinate system, positive pitch shoots down
        assert!(arrow.vel_y < 0.0);
    }

    #[test]
    fn test_arrow_trajectory_upward() {
        // Negative pitch = looking down = dir_y = -sin(pitch) = positive (since sin(-x) = -sin(x))
        let arrow = Projectile::shoot_arrow(0.0, 0.0, 0.0, 0.0, -std::f32::consts::FRAC_PI_4, 1.0);
        // With the game's coordinate system, negative pitch shoots up
        assert!(arrow.vel_y > 0.0);
    }

    #[test]
    fn test_projectile_speed_calculation() {
        let arrow = Projectile::new(0.0, 0.0, 0.0, 3.0, 4.0, 0.0, ProjectileType::Arrow, 1.0);
        // 3-4-5 triangle
        assert_eq!(arrow.speed(), 5.0);
    }

    #[test]
    fn test_splash_potion_upward_arc() {
        let potion = Projectile::throw_splash_potion(0.0, 0.0, 0.0, 0.0, 0.0, 1);
        // Potion should have slight upward arc added
        assert!(potion.vel_y > 0.0);
    }
}
