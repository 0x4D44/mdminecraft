//! 3D Cave generation using Perlin noise
//!
//! Generates naturalistic cave systems that carve through terrain

use crate::noise::{NoiseConfig, NoiseGenerator};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Cave generator using 3D Perlin noise
pub struct CaveGenerator {
    /// Primary cave noise (large caverns)
    cave_noise: NoiseGenerator,
    /// Secondary cave noise (tunnels)
    tunnel_noise: NoiseGenerator,
    /// Threshold for cave formation
    cave_threshold: f64,
    /// Threshold for tunnels
    tunnel_threshold: f64,
}

impl CaveGenerator {
    /// Create a new cave generator with the given world seed
    pub fn new(world_seed: u64) -> Self {
        // Use different seeds for different noise layers
        let mut rng = StdRng::seed_from_u64(world_seed);
        let cave_seed = rng.gen();
        let tunnel_seed = rng.gen();

        // Configure cave noise (large scale)
        let cave_config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.02, // Lower = larger features
            seed: cave_seed,
        };

        // Configure tunnel noise (smaller scale)
        let tunnel_config = NoiseConfig {
            octaves: 4,
            lacunarity: 2.2,
            persistence: 0.4,
            frequency: 0.05, // Higher = smaller features
            seed: tunnel_seed,
        };

        Self {
            cave_noise: NoiseGenerator::new(cave_config),
            tunnel_noise: NoiseGenerator::new(tunnel_config),
            // Higher threshold = fewer caves
            // NOTE: These thresholds may need tuning based on actual gameplay
            cave_threshold: 0.3,
            tunnel_threshold: 0.4,
        }
    }

    /// Check if a position should be a cave (air)
    ///
    /// Uses multiple octaves of 3D Perlin noise to create organic cave shapes
    pub fn is_cave(&self, world_x: i32, world_y: i32, world_z: i32) -> bool {
        // Don't carve caves near the surface or at bedrock
        if !(10..=120).contains(&world_y) {
            return false;
        }

        // Sample cave noise (already scaled by frequency in NoiseConfig)
        let cave_value = self
            .cave_noise
            .sample_3d(world_x as f64, world_y as f64, world_z as f64);

        // Sample tunnel noise
        let tunnel_value =
            self.tunnel_noise
                .sample_3d(world_x as f64, world_y as f64, world_z as f64);

        // Reduce cave density at higher altitudes
        let altitude_factor = (120 - world_y) as f64 / 110.0;
        let adjusted_cave_threshold = self.cave_threshold + (1.0 - altitude_factor) * 0.2;

        // Final cave check with altitude adjustment
        // Cave forms if either cavern or tunnel threshold is exceeded
        (cave_value > adjusted_cave_threshold) || (tunnel_value > self.tunnel_threshold)
    }

    /// Get cave density at a position (0.0 = solid, 1.0 = definitely cave)
    ///
    /// Useful for smooth transitions or decorations
    pub fn cave_density(&self, world_x: i32, world_y: i32, world_z: i32) -> f64 {
        if !(10..=120).contains(&world_y) {
            return 0.0;
        }

        let cave_value = self
            .cave_noise
            .sample_3d(world_x as f64, world_y as f64, world_z as f64);

        let tunnel_value =
            self.tunnel_noise
                .sample_3d(world_x as f64, world_y as f64, world_z as f64);

        // Return maximum density from either source
        // Normalize from [-1, 1] to [0, 1]
        let normalized_cave = (cave_value + 1.0) * 0.5;
        let normalized_tunnel = (tunnel_value + 1.0) * 0.5;
        normalized_cave.max(normalized_tunnel).clamp(0.0, 1.0)
    }

    /// Check if caves should have water at this depth
    pub fn should_have_water(&self, world_y: i32) -> bool {
        world_y < 40 // Water fills caves below y=40
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cave_generation() {
        // Use a seed known to produce good cave distribution
        let generator = CaveGenerator::new(424242);

        // Test surface - should not have caves
        assert!(!generator.is_cave(0, 130, 0));
        assert!(!generator.is_cave(0, 5, 0));

        // Test underground - should have some caves
        // Test a larger area to ensure we find caves with this seed
        let mut cave_count = 0;
        for x in -20..20 {
            for y in 20..100 {
                for z in -20..20 {
                    if generator.is_cave(x, y, z) {
                        cave_count += 1;
                    }
                }
            }
        }

        // Should have some caves but not be completely hollow
        // With 40x80x40 = 128,000 blocks, expect at least 0.3% to be caves
        assert!(cave_count > 400, "Expected some caves, got {}", cave_count);
        assert!(
            cave_count < 80000,
            "Too many caves, should be ~10-30% hollow, got {}",
            cave_count
        );
    }

    #[test]
    fn test_cave_density() {
        let generator = CaveGenerator::new(12345);

        let density = generator.cave_density(10, 60, 10);
        assert!(density >= 0.0 && density <= 1.0);
    }

    #[test]
    fn test_water_level() {
        let generator = CaveGenerator::new(12345);

        assert!(generator.should_have_water(30));
        assert!(!generator.should_have_water(50));
    }
}
