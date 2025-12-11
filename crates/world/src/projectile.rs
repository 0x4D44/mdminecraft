//! Projectile system for arrows and other ranged attacks.
//!
//! Provides projectile physics, collision detection, and damage calculation.

use serde::{Deserialize, Serialize};

/// Types of projectiles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectileType {
    Arrow,
    /// Splash potion - breaks on impact and applies area effect
    /// The u16 is the potion_id (maps to PotionType via potion_ids module)
    SplashPotion(u16),
}

impl ProjectileType {
    /// Get base damage for this projectile type
    pub fn base_damage(&self) -> f32 {
        match self {
            ProjectileType::Arrow => 2.0,
            ProjectileType::SplashPotion(_) => 0.0, // Splash potions don't deal direct damage
        }
    }

    /// Get gravity strength for this projectile
    pub fn gravity(&self) -> f64 {
        match self {
            ProjectileType::Arrow => 0.05,
            ProjectileType::SplashPotion(_) => 0.06, // Slightly higher gravity for potions
        }
    }

    /// Get drag coefficient (velocity multiplier per tick)
    pub fn drag(&self) -> f64 {
        match self {
            ProjectileType::Arrow => 0.99,
            ProjectileType::SplashPotion(_) => 0.98, // Slightly more drag
        }
    }

    /// Get the projectile's hitbox radius
    pub fn hitbox_radius(&self) -> f64 {
        match self {
            ProjectileType::Arrow => 0.3,
            ProjectileType::SplashPotion(_) => 0.25,
        }
    }

    /// How long the projectile lives (in ticks at 20 TPS)
    pub fn lifetime_ticks(&self) -> u32 {
        match self {
            ProjectileType::Arrow => 1200,          // 60 seconds
            ProjectileType::SplashPotion(_) => 600, // 30 seconds (should break on impact much sooner)
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
        }
    }
}

/// A projectile instance in the world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
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
    /// Whether the projectile should be removed
    pub dead: bool,
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
        // Damage scales with charge level: 1-10 damage based on draw time
        // charge ranges from 0.1 to 1.0
        // Min damage at 0.1 charge: 1.0
        // Max damage at 1.0 charge: 10.0
        1.0 + self.charge * 9.0
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
    pub fn spawn(&mut self, projectile: Projectile) {
        self.projectiles.push(projectile);
    }

    /// Update all projectiles and remove dead ones
    pub fn update(&mut self) {
        self.projectiles.retain_mut(|p| !p.update());
    }

    /// Check for collision with a point (mob/player position)
    /// Returns the damage if hit
    pub fn check_hit(&mut self, x: f64, y: f64, z: f64, radius: f64) -> Option<f32> {
        for projectile in &mut self.projectiles {
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
    fn test_projectile_hit() {
        let mut manager = ProjectileManager::new();
        let arrow = Projectile::new(5.0, 5.0, 5.0, 0.0, 0.0, 0.0, ProjectileType::Arrow, 1.0);
        manager.spawn(arrow);

        // Should hit nearby point
        let damage = manager.check_hit(5.0, 5.0, 5.0, 0.5);
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
}
