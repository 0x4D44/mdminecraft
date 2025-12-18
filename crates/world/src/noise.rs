//! Noise generation utilities for terrain generation.
//!
//! Provides deterministic noise functions for heightmap and biome generation.

use noise::{NoiseFn, Perlin, Simplex};

/// Noise layer type for different terrain features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseLayer {
    /// Continental scale - large landmasses vs oceans
    Continental,
    /// Erosion - smoothing and weathering
    Erosion,
    /// Peaks and valleys - fine detail variation
    PeaksValleys,
    /// Temperature for biome assignment
    Temperature,
    /// Humidity for biome assignment
    Humidity,
}

/// Configuration for multi-octave noise generation.
#[derive(Debug, Clone)]
pub struct NoiseConfig {
    /// Number of octaves (layers of detail)
    pub octaves: u32,
    /// Frequency multiplier between octaves
    pub lacunarity: f64,
    /// Amplitude multiplier between octaves (persistence)
    pub persistence: f64,
    /// Base frequency (scale)
    pub frequency: f64,
    /// Seed for deterministic generation
    pub seed: u32,
}

impl Default for NoiseConfig {
    fn default() -> Self {
        Self {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 1.0,
            seed: 0,
        }
    }
}

impl NoiseConfig {
    /// Create config for continental-scale noise (large features).
    pub fn continental(seed: u32) -> Self {
        Self {
            octaves: 3,
            lacunarity: 2.2,
            persistence: 0.6,
            frequency: 0.005, // Very large scale
            seed,
        }
    }

    /// Create config for erosion noise (medium features).
    pub fn erosion(seed: u32) -> Self {
        Self {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.01,
            seed: seed.wrapping_add(1000), // Offset seed
        }
    }

    /// Create config for peaks/valleys noise (fine detail).
    pub fn peaks_valleys(seed: u32) -> Self {
        Self {
            octaves: 5,
            lacunarity: 2.3,
            persistence: 0.4,
            frequency: 0.02,
            seed: seed.wrapping_add(2000), // Offset seed
        }
    }

    /// Create config for temperature noise (biome assignment).
    pub fn temperature(seed: u32) -> Self {
        Self {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.008,
            seed: seed.wrapping_add(3000), // Offset seed
        }
    }

    /// Create config for humidity noise (biome assignment).
    pub fn humidity(seed: u32) -> Self {
        Self {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.008,
            seed: seed.wrapping_add(4000), // Offset seed
        }
    }
}

/// Noise generator using Perlin noise.
pub struct NoiseGenerator {
    perlin: Perlin,
    config: NoiseConfig,
}

impl NoiseGenerator {
    /// Create a new noise generator with the given configuration.
    pub fn new(config: NoiseConfig) -> Self {
        Self {
            perlin: Perlin::new(config.seed),
            config,
        }
    }

    /// Generate noise value at 2D coordinates with multi-octave sampling.
    ///
    /// Returns value in range [-1.0, 1.0].
    pub fn sample_2d(&self, x: f64, y: f64) -> f64 {
        let mut value = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = self.config.frequency;
        let mut max_value = 0.0;

        for _ in 0..self.config.octaves {
            value += self.perlin.get([x * frequency, y * frequency]) * amplitude;
            max_value += amplitude;

            amplitude *= self.config.persistence;
            frequency *= self.config.lacunarity;
        }

        // Normalize to [-1.0, 1.0]
        value / max_value
    }

    /// Generate noise value at 3D coordinates with multi-octave sampling.
    ///
    /// Returns value in range [-1.0, 1.0].
    pub fn sample_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = self.config.frequency;
        let mut max_value = 0.0;

        for _ in 0..self.config.octaves {
            value += self
                .perlin
                .get([x * frequency, y * frequency, z * frequency])
                * amplitude;
            max_value += amplitude;

            amplitude *= self.config.persistence;
            frequency *= self.config.lacunarity;
        }

        // Normalize to [-1.0, 1.0]
        value / max_value
    }

    /// Sample noise and map to a specific range.
    pub fn sample_2d_range(&self, x: f64, y: f64, min: f64, max: f64) -> f64 {
        let noise = self.sample_2d(x, y);
        // Map from [-1, 1] to [min, max]
        (noise + 1.0) * 0.5 * (max - min) + min
    }

    /// Sample noise and map to unsigned byte range [0, 255].
    pub fn sample_2d_u8(&self, x: f64, y: f64) -> u8 {
        self.sample_2d_range(x, y, 0.0, 255.0) as u8
    }
}

/// Simplex noise generator (alternative to Perlin, faster in higher dimensions).
pub struct SimplexGenerator {
    simplex: Simplex,
    config: NoiseConfig,
}

impl SimplexGenerator {
    /// Create a new simplex noise generator with the given configuration.
    pub fn new(config: NoiseConfig) -> Self {
        Self {
            simplex: Simplex::new(config.seed),
            config,
        }
    }

    /// Generate noise value at 2D coordinates with multi-octave sampling.
    ///
    /// Returns value in range [-1.0, 1.0].
    pub fn sample_2d(&self, x: f64, y: f64) -> f64 {
        let mut value = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = self.config.frequency;
        let mut max_value = 0.0;

        for _ in 0..self.config.octaves {
            value += self.simplex.get([x * frequency, y * frequency]) * amplitude;
            max_value += amplitude;

            amplitude *= self.config.persistence;
            frequency *= self.config.lacunarity;
        }

        // Normalize to [-1.0, 1.0]
        value / max_value
    }

    /// Generate noise value at 3D coordinates with multi-octave sampling.
    ///
    /// Returns value in range [-1.0, 1.0].
    pub fn sample_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = self.config.frequency;
        let mut max_value = 0.0;

        for _ in 0..self.config.octaves {
            value += self
                .simplex
                .get([x * frequency, y * frequency, z * frequency])
                * amplitude;
            max_value += amplitude;

            amplitude *= self.config.persistence;
            frequency *= self.config.lacunarity;
        }

        // Normalize to [-1.0, 1.0]
        value / max_value
    }
}

/// Combines multiple noise layers for terrain generation.
pub struct LayeredNoise {
    continental: NoiseGenerator,
    erosion: NoiseGenerator,
    peaks_valleys: NoiseGenerator,
}

impl LayeredNoise {
    /// Create a new layered noise generator from a world seed.
    pub fn new(world_seed: u64) -> Self {
        let seed = world_seed as u32;

        Self {
            continental: NoiseGenerator::new(NoiseConfig::continental(seed)),
            erosion: NoiseGenerator::new(NoiseConfig::erosion(seed)),
            peaks_valleys: NoiseGenerator::new(NoiseConfig::peaks_valleys(seed)),
        }
    }

    /// Sample all noise layers at the given position.
    ///
    /// Returns (continental, erosion, peaks_valleys) values in range [-1.0, 1.0].
    pub fn sample_layers(&self, x: f64, z: f64) -> (f64, f64, f64) {
        (
            self.continental.sample_2d(x, z),
            self.erosion.sample_2d(x, z),
            self.peaks_valleys.sample_2d(x, z),
        )
    }

    /// Combine noise layers into a single height value.
    ///
    /// Uses weighted combination of layers for realistic terrain.
    pub fn sample_height(&self, x: f64, z: f64) -> f64 {
        let (continental, erosion, peaks_valleys) = self.sample_layers(x, z);

        // Weight layers:
        // - Continental: 0.5 (dominant large-scale features)
        // - Erosion: 0.3 (medium-scale smoothing)
        // - Peaks/Valleys: 0.2 (fine detail)
        continental * 0.5 + erosion * 0.3 + peaks_valleys * 0.2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_determinism() {
        let config = NoiseConfig {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 1.0,
            seed: 12345,
        };

        let gen1 = NoiseGenerator::new(config.clone());
        let gen2 = NoiseGenerator::new(config);

        // Same seed should produce same values
        for x in 0..10 {
            for y in 0..10 {
                let val1 = gen1.sample_2d(x as f64, y as f64);
                let val2 = gen2.sample_2d(x as f64, y as f64);
                assert_eq!(val1, val2, "Noise not deterministic at ({}, {})", x, y);
            }
        }
    }

    #[test]
    fn test_noise_range() {
        let config = NoiseConfig::default();
        let gen = NoiseGenerator::new(config);

        // Sample many points and verify range
        for x in 0..100 {
            for y in 0..100 {
                let val = gen.sample_2d(x as f64 * 0.1, y as f64 * 0.1);
                assert!(
                    (-1.0..=1.0).contains(&val),
                    "Noise value {} out of range at ({}, {})",
                    val,
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn test_different_seeds_produce_different_noise() {
        // Use continental config which has better frequency for testing
        let gen1 = NoiseGenerator::new(NoiseConfig::continental(1));
        let gen2 = NoiseGenerator::new(NoiseConfig::continental(2));

        // Sample multiple points with fractional coordinates to ensure at least one differs
        let mut any_different = false;
        for x in 0..20 {
            for y in 0..20 {
                let val1 = gen1.sample_2d(x as f64 * 0.5, y as f64 * 0.5);
                let val2 = gen2.sample_2d(x as f64 * 0.5, y as f64 * 0.5);
                if (val1 - val2).abs() > 0.001 {
                    any_different = true;
                    break;
                }
            }
            if any_different {
                break;
            }
        }

        assert!(
            any_different,
            "Different seeds should produce different noise"
        );
    }

    #[test]
    fn test_layered_noise_determinism() {
        let layered1 = LayeredNoise::new(12345);
        let layered2 = LayeredNoise::new(12345);

        for x in 0..10 {
            for z in 0..10 {
                let height1 = layered1.sample_height(x as f64, z as f64);
                let height2 = layered2.sample_height(x as f64, z as f64);
                assert_eq!(
                    height1, height2,
                    "Layered noise not deterministic at ({}, {})",
                    x, z
                );
            }
        }
    }

    #[test]
    fn test_simplex_determinism() {
        let config = NoiseConfig {
            seed: 42,
            ..Default::default()
        };

        let gen1 = SimplexGenerator::new(config.clone());
        let gen2 = SimplexGenerator::new(config);

        let val1 = gen1.sample_2d(5.0, 7.0);
        let val2 = gen2.sample_2d(5.0, 7.0);

        assert_eq!(val1, val2, "Simplex noise not deterministic");
    }

    #[test]
    fn test_noise_config_presets() {
        let seed = 123;

        let continental = NoiseConfig::continental(seed);
        assert_eq!(continental.seed, seed);
        assert!(continental.frequency < 0.01); // Very large scale

        let erosion = NoiseConfig::erosion(seed);
        assert_eq!(erosion.seed, seed + 1000); // Offset seed

        let peaks = NoiseConfig::peaks_valleys(seed);
        assert_eq!(peaks.seed, seed + 2000); // Offset seed
    }

    #[test]
    fn test_sample_2d_range() {
        let config = NoiseConfig::default();
        let gen = NoiseGenerator::new(config);

        // Test mapping to custom range
        for x in 0..10 {
            for y in 0..10 {
                let val = gen.sample_2d_range(x as f64 * 0.5, y as f64 * 0.5, 0.0, 100.0);
                assert!(
                    (0.0..=100.0).contains(&val),
                    "Value {} out of range [0, 100]",
                    val
                );
            }
        }

        // Test with negative range
        for x in 0..10 {
            let val = gen.sample_2d_range(x as f64 * 0.5, 0.0, -50.0, 50.0);
            assert!(
                (-50.0..=50.0).contains(&val),
                "Value {} out of range [-50, 50]",
                val
            );
        }
    }

    #[test]
    fn test_sample_2d_u8() {
        let config = NoiseConfig::default();
        let gen = NoiseGenerator::new(config);

        // Test u8 mapping - should always be in [0, 255]
        for x in 0..20 {
            for y in 0..20 {
                let _val = gen.sample_2d_u8(x as f64 * 0.3, y as f64 * 0.3);
            }
        }
    }

    #[test]
    fn test_simplex_3d() {
        let config = NoiseConfig {
            seed: 42,
            octaves: 3,
            ..Default::default()
        };
        let gen = SimplexGenerator::new(config);

        // Test 3D sampling is in valid range
        for x in 0..5 {
            for y in 0..5 {
                for z in 0..5 {
                    let val = gen.sample_3d(x as f64 * 0.5, y as f64 * 0.5, z as f64 * 0.5);
                    assert!((-1.0..=1.0).contains(&val), "3D value {} out of range", val);
                }
            }
        }
    }

    #[test]
    fn test_simplex_3d_determinism() {
        let config = NoiseConfig {
            seed: 12345,
            octaves: 4,
            persistence: 0.5,
            lacunarity: 2.0,
            frequency: 0.1,
        };

        let gen1 = SimplexGenerator::new(config.clone());
        let gen2 = SimplexGenerator::new(config);

        // Same seed should produce same values
        for x in 0..5 {
            for y in 0..5 {
                for z in 0..5 {
                    let val1 = gen1.sample_3d(x as f64, y as f64, z as f64);
                    let val2 = gen2.sample_3d(x as f64, y as f64, z as f64);
                    assert_eq!(
                        val1, val2,
                        "3D noise not deterministic at ({}, {}, {})",
                        x, y, z
                    );
                }
            }
        }
    }

    #[test]
    fn test_layered_noise_components() {
        let layered = LayeredNoise::new(42);

        // Access the individual noise layers
        let continental = layered.continental.sample_2d(10.0, 10.0);
        let erosion = layered.erosion.sample_2d(10.0, 10.0);
        let peaks = layered.peaks_valleys.sample_2d(10.0, 10.0);

        // All should be in valid range
        assert!((-1.0..=1.0).contains(&continental));
        assert!((-1.0..=1.0).contains(&erosion));
        assert!((-1.0..=1.0).contains(&peaks));
    }
}
