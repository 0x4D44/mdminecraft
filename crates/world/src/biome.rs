//! Biome system for terrain generation.
//!
//! Assigns biomes based on temperature and humidity noise values.

use crate::noise::{NoiseConfig, NoiseGenerator};
use serde::{Deserialize, Serialize};

/// Biome identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BiomeId {
    // Cold biomes (low temperature)
    IcePlains,
    IceMountains,
    Tundra,

    // Temperate biomes (medium temperature)
    Plains,
    Forest,
    BirchForest,
    Mountains,
    Hills,

    // Warm biomes (high temperature)
    Desert,
    Savanna,

    // Wet biomes (high humidity)
    Swamp,
    RainForest,

    // Ocean
    Ocean,
    DeepOcean,
}

impl BiomeId {
    /// Get all biome IDs (for iteration).
    pub fn all() -> &'static [BiomeId] {
        &[
            BiomeId::IcePlains,
            BiomeId::IceMountains,
            BiomeId::Tundra,
            BiomeId::Plains,
            BiomeId::Forest,
            BiomeId::BirchForest,
            BiomeId::Mountains,
            BiomeId::Hills,
            BiomeId::Desert,
            BiomeId::Savanna,
            BiomeId::Swamp,
            BiomeId::RainForest,
            BiomeId::Ocean,
            BiomeId::DeepOcean,
        ]
    }
}

/// Biome data with properties for generation.
#[derive(Debug, Clone)]
pub struct BiomeData {
    pub id: BiomeId,
    /// Temperature value [0.0, 1.0] (0=cold, 1=hot)
    pub temperature: f32,
    /// Humidity value [0.0, 1.0] (0=dry, 1=wet)
    pub humidity: f32,
    /// Base height modifier [-1.0, 1.0]
    pub height_modifier: f32,
    /// Height variation multiplier [0.0, 2.0]
    pub height_variation: f32,
    /// Grass color tint (R, G, B)
    pub grass_color: (u8, u8, u8),
    /// Foliage color tint (R, G, B)
    pub foliage_color: (u8, u8, u8),
}

impl BiomeData {
    /// Get biome data for a specific biome ID.
    pub fn get(id: BiomeId) -> Self {
        match id {
            BiomeId::IcePlains => Self {
                id,
                temperature: 0.0,
                humidity: 0.3,
                height_modifier: 0.0,
                height_variation: 0.5,
                grass_color: (200, 220, 255),
                foliage_color: (200, 220, 255),
            },
            BiomeId::IceMountains => Self {
                id,
                temperature: 0.0,
                humidity: 0.5,
                height_modifier: 0.6,
                height_variation: 1.5,
                grass_color: (220, 230, 255),
                foliage_color: (220, 230, 255),
            },
            BiomeId::Tundra => Self {
                id,
                temperature: 0.2,
                humidity: 0.2,
                height_modifier: 0.1,
                height_variation: 0.6,
                grass_color: (180, 200, 220),
                foliage_color: (180, 200, 220),
            },
            BiomeId::Plains => Self {
                id,
                temperature: 0.5,
                humidity: 0.4,
                height_modifier: 0.0,
                height_variation: 0.4,
                grass_color: (140, 200, 80),
                foliage_color: (120, 180, 60),
            },
            BiomeId::Forest => Self {
                id,
                temperature: 0.5,
                humidity: 0.6,
                height_modifier: 0.1,
                height_variation: 0.7,
                grass_color: (120, 180, 70),
                foliage_color: (100, 160, 50),
            },
            BiomeId::BirchForest => Self {
                id,
                temperature: 0.5,
                humidity: 0.5,
                height_modifier: 0.1,
                height_variation: 0.6,
                grass_color: (130, 190, 75),
                foliage_color: (110, 170, 55),
            },
            BiomeId::Mountains => Self {
                id,
                temperature: 0.4,
                humidity: 0.3,
                height_modifier: 0.8,
                height_variation: 1.8,
                grass_color: (160, 190, 100),
                foliage_color: (140, 170, 80),
            },
            BiomeId::Hills => Self {
                id,
                temperature: 0.5,
                humidity: 0.4,
                height_modifier: 0.3,
                height_variation: 1.2,
                grass_color: (150, 190, 90),
                foliage_color: (130, 170, 70),
            },
            BiomeId::Desert => Self {
                id,
                temperature: 0.9,
                humidity: 0.1,
                height_modifier: 0.0,
                height_variation: 0.5,
                grass_color: (230, 200, 120),
                foliage_color: (200, 170, 100),
            },
            BiomeId::Savanna => Self {
                id,
                temperature: 0.8,
                humidity: 0.3,
                height_modifier: 0.1,
                height_variation: 0.6,
                grass_color: (200, 180, 90),
                foliage_color: (180, 160, 70),
            },
            BiomeId::Swamp => Self {
                id,
                temperature: 0.6,
                humidity: 0.9,
                height_modifier: -0.2,
                height_variation: 0.3,
                grass_color: (100, 150, 80),
                foliage_color: (80, 130, 60),
            },
            BiomeId::RainForest => Self {
                id,
                temperature: 0.8,
                humidity: 0.9,
                height_modifier: 0.2,
                height_variation: 0.8,
                grass_color: (100, 180, 70),
                foliage_color: (80, 160, 50),
            },
            BiomeId::Ocean => Self {
                id,
                temperature: 0.5,
                humidity: 1.0,
                height_modifier: -0.5,
                height_variation: 0.2,
                grass_color: (120, 160, 140),
                foliage_color: (100, 140, 120),
            },
            BiomeId::DeepOcean => Self {
                id,
                temperature: 0.5,
                humidity: 1.0,
                height_modifier: -0.8,
                height_variation: 0.3,
                grass_color: (100, 140, 120),
                foliage_color: (80, 120, 100),
            },
        }
    }
}

/// Biome lookup table based on temperature and humidity.
///
/// Uses a 2D grid to map (temperature, humidity) to BiomeId.
pub struct BiomeLookup {
    /// Grid resolution for temperature axis
    temp_resolution: usize,
    /// Grid resolution for humidity axis
    humidity_resolution: usize,
    /// Lookup table indexed as [temp_idx][humidity_idx]
    table: Vec<Vec<BiomeId>>,
}

impl BiomeLookup {
    /// Create a new biome lookup table with default resolution.
    pub fn new() -> Self {
        const RESOLUTION: usize = 16;
        let mut table = vec![vec![BiomeId::Plains; RESOLUTION]; RESOLUTION];

        // Fill lookup table based on temperature (rows) and humidity (columns)
        for (temp_idx, row) in table.iter_mut().enumerate() {
            let temp = temp_idx as f32 / (RESOLUTION - 1) as f32;
            for (humidity_idx, cell) in row.iter_mut().enumerate() {
                let humidity = humidity_idx as f32 / (RESOLUTION - 1) as f32;
                *cell = Self::select_biome(temp, humidity);
            }
        }

        Self {
            temp_resolution: RESOLUTION,
            humidity_resolution: RESOLUTION,
            table,
        }
    }

    /// Select biome based on temperature and humidity values [0.0, 1.0].
    fn select_biome(temp: f32, humidity: f32) -> BiomeId {
        // Cold biomes (temp < 0.3)
        if temp < 0.3 {
            if humidity > 0.6 {
                BiomeId::IceMountains
            } else if humidity > 0.3 {
                BiomeId::Tundra
            } else {
                BiomeId::IcePlains
            }
        }
        // Hot biomes (temp > 0.7)
        else if temp > 0.7 {
            if humidity > 0.7 {
                BiomeId::RainForest
            } else if humidity > 0.4 {
                BiomeId::Savanna
            } else {
                BiomeId::Desert
            }
        }
        // Temperate biomes (0.3 <= temp <= 0.7)
        else if humidity > 0.8 {
            BiomeId::Swamp
        } else if humidity > 0.55 {
            BiomeId::Forest
        } else if humidity > 0.45 {
            BiomeId::BirchForest
        } else if humidity > 0.3 {
            BiomeId::Plains
        } else {
            BiomeId::Hills
        }
    }

    /// Look up biome from temperature and humidity values [0.0, 1.0].
    pub fn lookup(&self, temp: f32, humidity: f32) -> BiomeId {
        let temp_clamped = temp.clamp(0.0, 1.0);
        let humidity_clamped = humidity.clamp(0.0, 1.0);

        let temp_idx = (temp_clamped * (self.temp_resolution - 1) as f32) as usize;
        let humidity_idx = (humidity_clamped * (self.humidity_resolution - 1) as f32) as usize;

        self.table[temp_idx][humidity_idx]
    }
}

impl Default for BiomeLookup {
    fn default() -> Self {
        Self::new()
    }
}

/// Biome assigner that generates biomes from world coordinates.
pub struct BiomeAssigner {
    temperature_noise: NoiseGenerator,
    humidity_noise: NoiseGenerator,
    lookup: BiomeLookup,
}

impl BiomeAssigner {
    /// Create a new biome assigner from world seed.
    pub fn new(world_seed: u64) -> Self {
        let seed = world_seed as u32;

        Self {
            temperature_noise: NoiseGenerator::new(NoiseConfig::temperature(seed)),
            humidity_noise: NoiseGenerator::new(NoiseConfig::humidity(seed)),
            lookup: BiomeLookup::new(),
        }
    }

    /// Get biome at world coordinates.
    pub fn get_biome(&self, world_x: i32, world_z: i32) -> BiomeId {
        let x = world_x as f64;
        let z = world_z as f64;

        // Sample noise and map from [-1, 1] to [0, 1]
        let temp_raw = self.temperature_noise.sample_2d(x, z);
        let humidity_raw = self.humidity_noise.sample_2d(x, z);

        let temp = (temp_raw + 1.0) * 0.5;
        let humidity = (humidity_raw + 1.0) * 0.5;

        self.lookup.lookup(temp as f32, humidity as f32)
    }

    /// Get biome with blended properties at world coordinates.
    ///
    /// Samples surrounding biomes and blends their properties for smooth transitions.
    pub fn get_blended_biome(&self, world_x: i32, world_z: i32, blend_radius: i32) -> BiomeData {
        if blend_radius == 0 {
            // No blending, just return the biome at this position
            return BiomeData::get(self.get_biome(world_x, world_z));
        }

        // Sample biomes in a grid around the position
        let mut temp_sum = 0.0;
        let mut humidity_sum = 0.0;
        let mut height_mod_sum = 0.0;
        let mut height_var_sum = 0.0;
        let mut grass_r = 0.0;
        let mut grass_g = 0.0;
        let mut grass_b = 0.0;
        let mut foliage_r = 0.0;
        let mut foliage_g = 0.0;
        let mut foliage_b = 0.0;
        let mut total_weight = 0.0;

        for dx in -blend_radius..=blend_radius {
            for dz in -blend_radius..=blend_radius {
                let biome_id = self.get_biome(world_x + dx, world_z + dz);
                let biome_data = BiomeData::get(biome_id);

                // Weight by distance (inverse square)
                let dist_sq = (dx * dx + dz * dz) as f32;
                let weight = if dist_sq == 0.0 {
                    1.0
                } else {
                    1.0 / (1.0 + dist_sq)
                };

                temp_sum += biome_data.temperature * weight;
                humidity_sum += biome_data.humidity * weight;
                height_mod_sum += biome_data.height_modifier * weight;
                height_var_sum += biome_data.height_variation * weight;
                grass_r += biome_data.grass_color.0 as f32 * weight;
                grass_g += biome_data.grass_color.1 as f32 * weight;
                grass_b += biome_data.grass_color.2 as f32 * weight;
                foliage_r += biome_data.foliage_color.0 as f32 * weight;
                foliage_g += biome_data.foliage_color.1 as f32 * weight;
                foliage_b += biome_data.foliage_color.2 as f32 * weight;
                total_weight += weight;
            }
        }

        BiomeData {
            id: self.get_biome(world_x, world_z), // Use center biome ID
            temperature: temp_sum / total_weight,
            humidity: humidity_sum / total_weight,
            height_modifier: height_mod_sum / total_weight,
            height_variation: height_var_sum / total_weight,
            grass_color: (
                (grass_r / total_weight) as u8,
                (grass_g / total_weight) as u8,
                (grass_b / total_weight) as u8,
            ),
            foliage_color: (
                (foliage_r / total_weight) as u8,
                (foliage_g / total_weight) as u8,
                (foliage_b / total_weight) as u8,
            ),
        }
    }

    /// Get the underlying temperature noise generator.
    pub fn temperature_noise(&self) -> &NoiseGenerator {
        &self.temperature_noise
    }

    /// Get the underlying humidity noise generator.
    pub fn humidity_noise(&self) -> &NoiseGenerator {
        &self.humidity_noise
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biome_lookup_extremes() {
        let lookup = BiomeLookup::new();

        // Cold dry
        let biome = lookup.lookup(0.0, 0.0);
        assert_eq!(biome, BiomeId::IcePlains);

        // Hot dry
        let biome = lookup.lookup(1.0, 0.0);
        assert_eq!(biome, BiomeId::Desert);

        // Hot wet
        let biome = lookup.lookup(1.0, 1.0);
        assert_eq!(biome, BiomeId::RainForest);

        // Cold wet
        let biome = lookup.lookup(0.0, 1.0);
        assert_eq!(biome, BiomeId::IceMountains);
    }

    #[test]
    fn test_biome_lookup_temperate() {
        let lookup = BiomeLookup::new();

        // Temperate medium humidity
        let biome = lookup.lookup(0.5, 0.5);
        assert!(matches!(
            biome,
            BiomeId::Plains | BiomeId::Forest | BiomeId::BirchForest
        ));
    }

    #[test]
    fn test_biome_assigner_determinism() {
        let assigner1 = BiomeAssigner::new(12345);
        let assigner2 = BiomeAssigner::new(12345);

        for x in 0..10 {
            for z in 0..10 {
                let biome1 = assigner1.get_biome(x, z);
                let biome2 = assigner2.get_biome(x, z);
                assert_eq!(biome1, biome2, "Biome assignment not deterministic");
            }
        }
    }

    #[test]
    fn test_different_seeds_produce_different_biomes() {
        let assigner1 = BiomeAssigner::new(111);
        let assigner2 = BiomeAssigner::new(222);

        let mut any_different = false;
        for x in 0..20 {
            for z in 0..20 {
                let biome1 = assigner1.get_biome(x * 10, z * 10);
                let biome2 = assigner2.get_biome(x * 10, z * 10);
                if biome1 != biome2 {
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
            "Different seeds should produce different biomes"
        );
    }

    #[test]
    fn test_biome_data_properties() {
        let plains = BiomeData::get(BiomeId::Plains);
        assert_eq!(plains.id, BiomeId::Plains);
        assert!(plains.temperature > 0.3 && plains.temperature < 0.7);
        assert!(plains.humidity > 0.0 && plains.humidity <= 1.0);

        let desert = BiomeData::get(BiomeId::Desert);
        assert_eq!(desert.id, BiomeId::Desert);
        assert!(desert.temperature > 0.7);
        assert!(desert.humidity < 0.3);
    }

    #[test]
    fn test_blended_biome_smoothing() {
        let assigner = BiomeAssigner::new(42);

        // Get biome without blending
        let biome_no_blend = assigner.get_blended_biome(100, 100, 0);

        // Get biome with blending
        let biome_blended = assigner.get_blended_biome(100, 100, 2);

        // Both should have same center biome ID
        assert_eq!(biome_no_blend.id, biome_blended.id);

        // Blended values should be within reasonable range
        assert!(biome_blended.temperature >= 0.0 && biome_blended.temperature <= 1.0);
        assert!(biome_blended.humidity >= 0.0 && biome_blended.humidity <= 1.0);
    }

    #[test]
    fn test_biome_all_ids() {
        let all_biomes = BiomeId::all();
        assert!(all_biomes.len() > 10);
        assert!(all_biomes.contains(&BiomeId::Plains));
        assert!(all_biomes.contains(&BiomeId::Desert));
        assert!(all_biomes.contains(&BiomeId::Forest));
    }

    #[test]
    fn test_negative_coordinates() {
        let assigner = BiomeAssigner::new(123);

        // Should work with negative coordinates
        let biome = assigner.get_biome(-100, -200);
        assert!(BiomeId::all().contains(&biome));

        // Determinism with negative coordinates
        let biome2 = assigner.get_biome(-100, -200);
        assert_eq!(biome, biome2);
    }

    #[test]
    fn test_biome_lookup_clamping() {
        let lookup = BiomeLookup::new();

        // Should clamp out-of-range values
        let biome1 = lookup.lookup(-0.5, 1.5);
        let biome2 = lookup.lookup(0.0, 1.0);
        assert_eq!(biome1, biome2);
    }
}
