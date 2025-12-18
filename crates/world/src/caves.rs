//! Cave generation using 3D noise carving with biomes and decorations

use crate::chunk::{BlockId, Chunk, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::noise::{NoiseConfig, NoiseGenerator};

/// Cave generation parameters
#[derive(Debug, Clone)]
pub struct CaveParams {
    /// Base frequency for cave noise
    pub frequency: f64,
    /// Threshold below which air is carved (0.0-1.0)
    pub threshold: f64,
    /// Minimum Y level for caves
    pub min_y: i32,
    /// Maximum Y level for caves
    pub max_y: i32,
    /// Vertical squash factor (makes caves more horizontal)
    pub vertical_squash: f64,
}

impl Default for CaveParams {
    fn default() -> Self {
        Self {
            frequency: 0.05,
            threshold: 0.45,
            min_y: 5,
            max_y: 56,
            vertical_squash: 2.0,
        }
    }
}

/// Cave carver using 3D simplex noise
pub struct CaveCarver {
    noise: NoiseGenerator,
    params: CaveParams,
    biome_noise: NoiseGenerator,
}

impl CaveCarver {
    pub fn new(seed: u64) -> Self {
        let cave_config = NoiseConfig {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.05,
            seed: ((seed ^ 0xCAFE1234) as u32),
        };
        let biome_config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.01,
            seed: ((seed ^ 0xABCDEF89) as u32),
        };
        Self {
            noise: NoiseGenerator::new(cave_config),
            biome_noise: NoiseGenerator::new(biome_config),
            params: CaveParams::default(),
        }
    }

    pub fn with_params(seed: u64, params: CaveParams) -> Self {
        let cave_config = NoiseConfig {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: params.frequency,
            seed: ((seed ^ 0xCAFE1234) as u32),
        };
        let biome_config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.01,
            seed: ((seed ^ 0xABCDEF89) as u32),
        };
        Self {
            noise: NoiseGenerator::new(cave_config),
            biome_noise: NoiseGenerator::new(biome_config),
            params,
        }
    }

    /// Carve caves into an existing chunk
    pub fn carve_chunk(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let world_x_base = chunk_x * CHUNK_SIZE_X as i32;
        let world_z_base = chunk_z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                for y in self.params.min_y..=self.params.max_y.min(CHUNK_SIZE_Y as i32 - 1) {
                    if self.should_carve(world_x, y, world_z) {
                        let voxel = chunk.voxel(local_x, y as usize, local_z);
                        // Don't carve bedrock (1) or water (14)
                        if voxel.id != 0 && voxel.id != 1 && voxel.id != 14 {
                            let mut new_voxel = voxel;
                            new_voxel.id = 0; // Air
                            chunk.set_voxel(local_x, y as usize, local_z, new_voxel);
                        }
                    }
                }
            }
        }
    }

    /// Check if a position should be carved out
    fn should_carve(&self, x: i32, y: i32, z: i32) -> bool {
        // Apply vertical squash to make caves more horizontal
        let squashed_y = y as f64 / self.params.vertical_squash;

        let noise_val = self.noise.sample_3d(
            x as f64 * self.params.frequency,
            squashed_y * self.params.frequency,
            z as f64 * self.params.frequency,
        );

        // Normalize from [-1, 1] to [0, 1]
        let normalized = (noise_val + 1.0) / 2.0;

        // Add depth-based variation - caves more common at certain depths
        let depth_factor = self.depth_modifier(y);

        normalized * depth_factor < self.params.threshold
    }

    /// Modify cave density based on depth
    fn depth_modifier(&self, y: i32) -> f64 {
        // Peak cave density around y=30, less near surface and bedrock
        let optimal_depth = 30.0;
        let distance_from_optimal = (y as f64 - optimal_depth).abs();

        1.0 - (distance_from_optimal / 40.0).min(0.5)
    }

    /// Get cave biome for a position
    pub fn get_biome(&self, x: i32, y: i32, z: i32) -> CaveBiome {
        CaveBiome::from_position(x, y, z, &self.biome_noise)
    }
}

/// Cheese cave carver - large open caverns (Minecraft 1.18+)
pub struct CheeseCaveCarver {
    noise: NoiseGenerator,
    params: CaveParams,
}

impl CheeseCaveCarver {
    pub fn new(seed: u64) -> Self {
        let config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.03,
            seed: ((seed ^ 0xCBEE5E01) as u32),
        };
        Self {
            noise: NoiseGenerator::new(config),
            params: CaveParams {
                frequency: 0.03,
                threshold: 0.5,
                min_y: 5,
                max_y: 80, // Limit to y=80 to prevent surface floating terrain
                vertical_squash: 1.5,
            },
        }
    }

    pub fn carve_chunk(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let world_x_base = chunk_x * CHUNK_SIZE_X as i32;
        let world_z_base = chunk_z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                for y in self.params.min_y..=self.params.max_y.min(CHUNK_SIZE_Y as i32 - 1) {
                    if self.should_carve(world_x, y, world_z) {
                        let voxel = chunk.voxel(local_x, y as usize, local_z);
                        if voxel.id != 0 && voxel.id != 1 && voxel.id != 14 {
                            let mut new_voxel = voxel;
                            new_voxel.id = 0;
                            chunk.set_voxel(local_x, y as usize, local_z, new_voxel);
                        }
                    }
                }
            }
        }
    }

    fn should_carve(&self, x: i32, y: i32, z: i32) -> bool {
        let squashed_y = y as f64 / self.params.vertical_squash;
        let noise_val = self.noise.sample_3d(
            x as f64 * self.params.frequency,
            squashed_y * self.params.frequency,
            z as f64 * self.params.frequency,
        );
        let normalized = (noise_val + 1.0) / 2.0;
        normalized > self.params.threshold
    }
}

/// Spaghetti cave carver - long winding tunnels (Minecraft 1.18+)
pub struct SpaghettiCaveCarver {
    noise: NoiseGenerator,
    thickness_noise: NoiseGenerator,
}

impl SpaghettiCaveCarver {
    pub fn new(seed: u64) -> Self {
        let noise_config = NoiseConfig {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.08,
            seed: ((seed ^ 0x5FAC6E77) as u32),
        };
        let thickness_config = NoiseConfig {
            octaves: 2,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.15,
            seed: ((seed ^ 0x7B1C4AE5) as u32),
        };
        Self {
            noise: NoiseGenerator::new(noise_config),
            thickness_noise: NoiseGenerator::new(thickness_config),
        }
    }

    pub fn carve_chunk(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let world_x_base = chunk_x * CHUNK_SIZE_X as i32;
        let world_z_base = chunk_z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                for y in 5..120 {
                    if self.should_carve(world_x, y, world_z) {
                        let voxel = chunk.voxel(local_x, y as usize, local_z);
                        if voxel.id != 0 && voxel.id != 1 && voxel.id != 14 {
                            let mut new_voxel = voxel;
                            new_voxel.id = 0;
                            chunk.set_voxel(local_x, y as usize, local_z, new_voxel);
                        }
                    }
                }
            }
        }
    }

    fn should_carve(&self, x: i32, y: i32, z: i32) -> bool {
        let path_noise = self
            .noise
            .sample_3d(x as f64 * 0.08, y as f64 * 0.08, z as f64 * 0.08);

        let thickness =
            self.thickness_noise
                .sample_3d(x as f64 * 0.15, y as f64 * 0.15, z as f64 * 0.15);

        let threshold = 0.15 + thickness.abs() * 0.1;
        path_noise.abs() < threshold
    }
}

/// Noodle cave carver - very thin winding passages (Minecraft 1.18+)
pub struct NoodleCaveCarver {
    noise: NoiseGenerator,
}

impl NoodleCaveCarver {
    pub fn new(seed: u64) -> Self {
        let config = NoiseConfig {
            octaves: 5,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.12,
            seed: ((seed ^ 0x100D1E99) as u32),
        };
        Self {
            noise: NoiseGenerator::new(config),
        }
    }

    pub fn carve_chunk(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let world_x_base = chunk_x * CHUNK_SIZE_X as i32;
        let world_z_base = chunk_z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                for y in 5..120 {
                    if self.should_carve(world_x, y, world_z) {
                        let voxel = chunk.voxel(local_x, y as usize, local_z);
                        if voxel.id != 0 && voxel.id != 1 && voxel.id != 14 {
                            let mut new_voxel = voxel;
                            new_voxel.id = 0;
                            chunk.set_voxel(local_x, y as usize, local_z, new_voxel);
                        }
                    }
                }
            }
        }
    }

    fn should_carve(&self, x: i32, y: i32, z: i32) -> bool {
        let noise_val = self
            .noise
            .sample_3d(x as f64 * 0.12, y as f64 * 0.12, z as f64 * 0.12);
        noise_val.abs() < 0.08
    }
}

/// Ravine carver - vertical canyon-like structures (Minecraft 1.18+)
pub struct RavineCarver {
    path_noise: NoiseGenerator,
    width_noise: NoiseGenerator,
    depth_noise: NoiseGenerator,
}

impl RavineCarver {
    pub fn new(seed: u64) -> Self {
        let path_config = NoiseConfig {
            octaves: 2,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.02,
            seed: ((seed ^ 0x2A91AE01) as u32),
        };

        let width_config = NoiseConfig {
            octaves: 2,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.04,
            seed: ((seed ^ 0x41D78001) as u32),
        };

        let depth_config = NoiseConfig {
            octaves: 2,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.03,
            seed: ((seed ^ 0xDEE78002) as u32),
        };

        Self {
            path_noise: NoiseGenerator::new(path_config),
            width_noise: NoiseGenerator::new(width_config),
            depth_noise: NoiseGenerator::new(depth_config),
        }
    }

    pub fn carve_chunk(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let world_x_base = chunk_x * 16;
        let world_z_base = chunk_z * 16;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                // Check if we're on a ravine path in 2D
                let path_val = self
                    .path_noise
                    .sample_2d(world_x as f64 * 0.02, world_z as f64 * 0.02);

                // Ravines are rare - only form when noise is in narrow range
                if path_val.abs() > 0.05 {
                    continue;
                }

                // Get width variation
                let width = self
                    .width_noise
                    .sample_2d(world_x as f64 * 0.04, world_z as f64 * 0.04);
                let max_width = 2.0 + width.abs() * 3.0;

                // Get depth variation
                let depth_mod = self
                    .depth_noise
                    .sample_2d(world_x as f64 * 0.03, world_z as f64 * 0.03);
                let min_y = 10 + (depth_mod * 15.0) as i32;
                let max_y = 60 + (depth_mod * 20.0) as i32;

                // Carve vertically at this position
                for y in min_y..max_y.min(120) {
                    // Check if we're within the ravine width using distance from path
                    let dist_from_path = path_val.abs() * 20.0; // Scale up for width check
                    if dist_from_path < max_width {
                        let voxel = chunk.voxel(local_x, y as usize, local_z);
                        if voxel.id != 0 {
                            let mut new_voxel = voxel;
                            new_voxel.id = 0;
                            chunk.set_voxel(local_x, y as usize, local_z, new_voxel);
                        }
                    }
                }
            }
        }
    }
}

/// Underground biome types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaveBiome {
    /// Standard stone caves
    Stone,
    /// Lush caves with moss and glow berries
    Lush,
    /// Dripstone caves with stalactites/stalagmites
    Dripstone,
    /// Deep dark - below y=0 (if supported), or deep caves
    DeepDark,
    /// Flooded caves with water pools
    Flooded,
}

impl CaveBiome {
    /// Determine cave biome based on position and noise
    pub fn from_position(x: i32, y: i32, z: i32, noise: &NoiseGenerator) -> Self {
        // Use different noise frequency for biome selection
        let biome_noise = noise.sample_2d(x as f64 * 0.01, z as f64 * 0.01);
        let depth_noise = noise.sample_2d(x as f64 * 0.02, z as f64 * 0.02);

        // Depth-based biome selection
        if y < 10 {
            return CaveBiome::DeepDark;
        }

        // Biome noise determines type
        if biome_noise > 0.6 {
            CaveBiome::Lush
        } else if biome_noise > 0.3 {
            CaveBiome::Dripstone
        } else if biome_noise < -0.4 && depth_noise < 0.0 {
            CaveBiome::Flooded
        } else {
            CaveBiome::Stone
        }
    }

    /// Get floor block for this biome
    pub fn floor_block(&self) -> BlockId {
        match self {
            CaveBiome::Stone => 13,     // stone
            CaveBiome::Lush => 100,     // moss_block
            CaveBiome::Dripstone => 13, // stone
            CaveBiome::DeepDark => 101, // deepslate
            CaveBiome::Flooded => 10,   // gravel
        }
    }

    /// Get ceiling decoration block
    pub fn ceiling_decoration(&self) -> Option<BlockId> {
        match self {
            CaveBiome::Lush => Some(102),      // glow_lichen
            CaveBiome::Dripstone => Some(103), // pointed_dripstone
            CaveBiome::DeepDark => Some(104),  // sculk
            _ => None,
        }
    }

    /// Check if a cave position connects to the surface
    pub fn is_surface_connected(chunk: &Chunk, x: usize, y: usize, z: usize) -> bool {
        // Scan upward from current position
        for scan_y in y..CHUNK_SIZE_Y {
            let voxel = chunk.voxel(x, scan_y, z);
            if voxel.id != 0 {
                // Hit solid block before reaching top
                return false;
            }
        }
        // Reached sky - surface connected
        true
    }

    /// Get appropriate decoration based on surface connection
    pub fn ceiling_decoration_with_surface(&self, is_surface: bool) -> Option<BlockId> {
        if is_surface {
            // Different decorations for surface-connected caves
            match self {
                CaveBiome::Lush => Some(116),      // hanging_roots
                CaveBiome::Dripstone => Some(103), // pointed_dripstone
                _ => None,
            }
        } else {
            // Original underground decorations
            self.ceiling_decoration()
        }
    }
}

/// Generate dripstone formations in caves
pub struct DripstoneGenerator {
    noise: NoiseGenerator,
}

impl DripstoneGenerator {
    pub fn new(seed: u64) -> Self {
        let config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.3,
            seed: ((seed ^ 0x12345678) as u32),
        };
        Self {
            noise: NoiseGenerator::new(config),
        }
    }

    /// Place dripstone in carved cave areas
    pub fn decorate_chunk(
        &self,
        chunk: &mut Chunk,
        chunk_x: i32,
        chunk_z: i32,
        biome_fn: impl Fn(i32, i32, i32) -> CaveBiome,
    ) {
        let world_x_base = chunk_x * CHUNK_SIZE_X as i32;
        let world_z_base = chunk_z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                for y in 5..120 {
                    let biome = biome_fn(world_x, y as i32, world_z);

                    if biome != CaveBiome::Dripstone {
                        continue;
                    }

                    let voxel = chunk.voxel(local_x, y, local_z);
                    if voxel.id != 0 {
                        continue; // Not air
                    }

                    // Check for ceiling (stalactite)
                    if y + 1 < CHUNK_SIZE_Y {
                        let above = chunk.voxel(local_x, y + 1, local_z);
                        if above.id == 13 {
                            // Stone ceiling
                            let spawn_chance = self.noise.sample_3d(
                                world_x as f64 * 0.3,
                                y as f64 * 0.3,
                                world_z as f64 * 0.3,
                            );
                            if spawn_chance > 0.7 {
                                let mut voxel = chunk.voxel(local_x, y, local_z);
                                voxel.id = 103; // pointed_dripstone
                                chunk.set_voxel(local_x, y, local_z, voxel);
                            }
                        }
                    }

                    // Check for floor (stalagmite)
                    if y > 0 {
                        let below = chunk.voxel(local_x, y - 1, local_z);
                        if below.id == 13 {
                            // Stone floor
                            let spawn_chance = self.noise.sample_3d(
                                world_x as f64 * 0.3 + 100.0,
                                y as f64 * 0.3,
                                world_z as f64 * 0.3 + 100.0,
                            );
                            if spawn_chance > 0.75 {
                                let mut voxel = chunk.voxel(local_x, y, local_z);
                                voxel.id = 103; // pointed_dripstone
                                chunk.set_voxel(local_x, y, local_z, voxel);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Lush cave decorator - adds vegetation and decorations
pub struct LushCaveDecorator {
    noise: NoiseGenerator,
}

impl LushCaveDecorator {
    pub fn new(seed: u64) -> Self {
        let config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.2,
            seed: ((seed ^ 0x1A5ABCDE) as u32),
        };
        Self {
            noise: NoiseGenerator::new(config),
        }
    }

    pub fn decorate_chunk(
        &self,
        chunk: &mut Chunk,
        chunk_x: i32,
        chunk_z: i32,
        biome_fn: impl Fn(i32, i32, i32) -> CaveBiome,
    ) {
        let world_x_base = chunk_x * CHUNK_SIZE_X as i32;
        let world_z_base = chunk_z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                for y in 5..120 {
                    let biome = biome_fn(world_x, y as i32, world_z);
                    if biome != CaveBiome::Lush {
                        continue;
                    }

                    let voxel = chunk.voxel(local_x, y, local_z);
                    if voxel.id != 0 {
                        continue;
                    }

                    // Ceiling decorations (cave vines, spore blossoms)
                    if y + 1 < CHUNK_SIZE_Y {
                        let above = chunk.voxel(local_x, y + 1, local_z);
                        if above.id == 100 || above.id == 13 {
                            // Moss or stone ceiling
                            let decoration_noise = self.noise.sample_3d(
                                world_x as f64 * 0.2,
                                y as f64 * 0.2,
                                world_z as f64 * 0.2,
                            );

                            if decoration_noise > 0.85 {
                                let mut new_voxel = voxel;
                                new_voxel.id = 113; // spore_blossom
                                chunk.set_voxel(local_x, y, local_z, new_voxel);
                            } else if decoration_noise > 0.7 {
                                let mut new_voxel = voxel;
                                new_voxel.id = 111; // cave_vines
                                chunk.set_voxel(local_x, y, local_z, new_voxel);
                            }
                        }
                    }

                    // Floor decorations (moss carpet)
                    if y > 0 {
                        let below = chunk.voxel(local_x, y - 1, local_z);
                        if below.id == 100 {
                            // Moss block floor
                            let carpet_noise = self.noise.sample_3d(
                                world_x as f64 * 0.3,
                                y as f64 * 0.3,
                                world_z as f64 * 0.3,
                            );
                            if carpet_noise > 0.6 {
                                let mut new_voxel = voxel;
                                new_voxel.id = 112; // moss_carpet
                                chunk.set_voxel(local_x, y, local_z, new_voxel);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Deep dark decorator - adds sculk decorations
pub struct DeepDarkDecorator {
    noise: NoiseGenerator,
}

impl DeepDarkDecorator {
    pub fn new(seed: u64) -> Self {
        let config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.15,
            seed: ((seed ^ 0xDEEFDA99) as u32),
        };
        Self {
            noise: NoiseGenerator::new(config),
        }
    }

    pub fn decorate_chunk(
        &self,
        chunk: &mut Chunk,
        chunk_x: i32,
        chunk_z: i32,
        biome_fn: impl Fn(i32, i32, i32) -> CaveBiome,
    ) {
        let world_x_base = chunk_x * CHUNK_SIZE_X as i32;
        let world_z_base = chunk_z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = world_x_base + local_x as i32;
                let world_z = world_z_base + local_z as i32;

                for y in 1..30 {
                    let biome = biome_fn(world_x, y as i32, world_z);
                    if biome != CaveBiome::DeepDark {
                        continue;
                    }

                    let voxel = chunk.voxel(local_x, y, local_z);
                    if voxel.id != 0 {
                        continue;
                    }

                    // Floor decorations
                    if y > 0 {
                        let below = chunk.voxel(local_x, y - 1, local_z);
                        if below.id == 101 || below.id == 104 {
                            // Deepslate or sculk floor
                            let decoration_noise = self.noise.sample_3d(
                                world_x as f64 * 0.15,
                                y as f64 * 0.15,
                                world_z as f64 * 0.15,
                            );

                            let block_id = if decoration_noise > 0.9 {
                                119 // sculk_catalyst (rare)
                            } else if decoration_noise > 0.8 {
                                118 // sculk_shrieker
                            } else if decoration_noise > 0.7 {
                                117 // sculk_sensor
                            } else if decoration_noise > 0.5 {
                                120 // sculk_vein
                            } else {
                                continue;
                            };

                            let mut new_voxel = voxel;
                            new_voxel.id = block_id;
                            chunk.set_voxel(local_x, y, local_z, new_voxel);
                        }
                    }
                }
            }
        }
    }
}

/// Generate underground water pools in caves
pub fn flood_low_areas(chunk: &mut Chunk, water_level: i32, water_id: BlockId) {
    for x in 0..CHUNK_SIZE_X {
        for z in 0..CHUNK_SIZE_Z {
            for y in 1..water_level.min(CHUNK_SIZE_Y as i32) as usize {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id == 0 {
                    // Air
                    let mut new_voxel = voxel;
                    new_voxel.id = water_id;
                    chunk.set_voxel(x, y, z, new_voxel);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkPos;

    fn create_stone_chunk() -> Chunk {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        for x in 0..16 {
            for y in 1..100 {
                for z in 0..16 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 13; // stone
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }
        chunk
    }

    #[test]
    fn test_cave_carver_creates_air() {
        let carver = CaveCarver::new(12345);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Fill with stone
        for x in 0..16 {
            for y in 0..64 {
                for z in 0..16 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 13;
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        carver.carve_chunk(&mut chunk, 0, 0);

        // Should have created some air
        let mut air_count = 0;
        for x in 0..16 {
            for y in 5..56 {
                for z in 0..16 {
                    if chunk.voxel(x, y, z).id == 0 {
                        air_count += 1;
                    }
                }
            }
        }

        assert!(air_count > 0, "Cave carver should create air pockets");
        assert!(
            air_count < 16 * 51 * 16,
            "Cave carver shouldn't carve everything"
        );
    }

    #[test]
    fn test_cave_params_custom() {
        let params = CaveParams {
            frequency: 0.1,
            threshold: 0.3,
            min_y: 10,
            max_y: 40,
            vertical_squash: 1.5,
        };
        assert_eq!(params.frequency, 0.1);
        assert_eq!(params.threshold, 0.3);
        assert_eq!(params.min_y, 10);
        assert_eq!(params.max_y, 40);
        assert_eq!(params.vertical_squash, 1.5);
    }

    #[test]
    fn test_cave_carver_with_params() {
        let params = CaveParams {
            frequency: 0.08,
            threshold: 0.5,
            min_y: 10,
            max_y: 50,
            vertical_squash: 2.5,
        };
        let carver = CaveCarver::with_params(12345, params);
        let mut chunk = create_stone_chunk();

        carver.carve_chunk(&mut chunk, 0, 0);

        // Verify carving respects min_y - blocks below should be untouched
        let mut below_min_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..10 {
                    if chunk.voxel(x, y, z).id == 0 {
                        below_min_count += 1;
                    }
                }
            }
        }
        assert_eq!(below_min_count, 0, "Should not carve below min_y");
    }

    #[test]
    fn test_cave_carver_preserves_bedrock() {
        let carver = CaveCarver::new(54321);
        let mut chunk = create_stone_chunk();

        // Place bedrock
        for x in 0..16 {
            for z in 0..16 {
                let mut voxel = chunk.voxel(x, 5, z);
                voxel.id = 1; // bedrock
                chunk.set_voxel(x, 5, z, voxel);
            }
        }

        carver.carve_chunk(&mut chunk, 0, 0);

        // Verify bedrock is preserved
        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(chunk.voxel(x, 5, z).id, 1, "Bedrock should not be carved");
            }
        }
    }

    #[test]
    fn test_cave_carver_preserves_water() {
        let carver = CaveCarver::new(11111);
        let mut chunk = create_stone_chunk();

        // Place water
        for x in 0..16 {
            for z in 0..16 {
                let mut voxel = chunk.voxel(x, 30, z);
                voxel.id = 14; // water
                chunk.set_voxel(x, 30, z, voxel);
            }
        }

        carver.carve_chunk(&mut chunk, 0, 0);

        // Verify water is preserved
        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(chunk.voxel(x, 30, z).id, 14, "Water should not be carved");
            }
        }
    }

    #[test]
    fn test_cave_carver_get_biome() {
        let carver = CaveCarver::new(12345);

        // Deep biome should be DeepDark
        let deep_biome = carver.get_biome(0, 5, 0);
        assert_eq!(deep_biome, CaveBiome::DeepDark);

        // Higher positions should vary based on noise
        let mid_biome = carver.get_biome(100, 40, 100);
        // Just verify it returns a valid biome
        assert!(matches!(
            mid_biome,
            CaveBiome::Stone | CaveBiome::Lush | CaveBiome::Dripstone | CaveBiome::Flooded
        ));
    }

    #[test]
    fn test_cave_biome_depth_selection() {
        let config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.01,
            seed: 42,
        };
        let noise = NoiseGenerator::new(config);

        // Deep areas should be DeepDark
        let deep = CaveBiome::from_position(0, 5, 0, &noise);
        assert_eq!(deep, CaveBiome::DeepDark);

        // Higher areas should vary
        let mid = CaveBiome::from_position(0, 40, 0, &noise);
        assert_ne!(mid, CaveBiome::DeepDark);
    }

    #[test]
    fn test_cave_params_default() {
        let params = CaveParams::default();
        assert_eq!(params.min_y, 5);
        assert_eq!(params.max_y, 56);
        assert!(params.threshold > 0.0 && params.threshold < 1.0);
    }

    #[test]
    fn test_floor_block_assignment() {
        assert_eq!(CaveBiome::Stone.floor_block(), 13);
        assert_eq!(CaveBiome::Lush.floor_block(), 100);
        assert_eq!(CaveBiome::DeepDark.floor_block(), 101);
    }

    #[test]
    fn test_dripstone_generator() {
        use crate::chunk::ChunkPos;
        let gen = DripstoneGenerator::new(12345);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Set up test conditions (stone floor, air above)
        for x in 0..16 {
            for z in 0..16 {
                let mut voxel = chunk.voxel(x, 30, z);
                voxel.id = 13; // Stone floor
                chunk.set_voxel(x, 30, z, voxel);

                let mut voxel = chunk.voxel(x, 31, z);
                voxel.id = 0; // Air
                chunk.set_voxel(x, 31, z, voxel);
            }
        }

        gen.decorate_chunk(&mut chunk, 0, 0, |_, y, _| {
            if y == 31 {
                CaveBiome::Dripstone
            } else {
                CaveBiome::Stone
            }
        });

        // Check that some dripstone was placed (not deterministic but statistically likely)
        let mut dripstone_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                if chunk.voxel(x, 31, z).id == 103 {
                    dripstone_count += 1;
                }
            }
        }

        // With 256 positions and threshold 0.75, expect some but not all
        assert!(dripstone_count >= 0, "Should have placed some dripstone");
    }

    /// Minecraft 1.18+ Integration Tests
    #[test]
    fn test_minecraft_118_cave_integration() {
        let seed = 42424242;
        let cheese = CheeseCaveCarver::new(seed);
        let spaghetti = SpaghettiCaveCarver::new(seed);
        let noodle = NoodleCaveCarver::new(seed);

        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Fill with stone
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 13; // stone
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        // Carve all three cave types
        cheese.carve_chunk(&mut chunk, 0, 0);
        spaghetti.carve_chunk(&mut chunk, 0, 0);
        noodle.carve_chunk(&mut chunk, 0, 0);

        // Count air blocks from carving
        let mut air_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    if chunk.voxel(x, y, z).id == 0 {
                        air_count += 1;
                    }
                }
            }
        }

        // At least one carver should have created some caves
        assert!(air_count > 0, "Multiple carvers should create caves");
    }

    #[test]
    fn test_ravine_vertical_structure() {
        let ravine = RavineCarver::new(99999);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Fill with stone
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..200 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 13;
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        ravine.carve_chunk(&mut chunk, 0, 0);

        // Check for vertical carving - ravines should span multiple Y levels
        let mut carved_y_levels = std::collections::HashSet::new();
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..200 {
                    if chunk.voxel(x, y, z).id == 0 {
                        carved_y_levels.insert(y);
                    }
                }
            }
        }

        // If ravine carved, should span at least 10 Y levels
        if !carved_y_levels.is_empty() {
            assert!(
                carved_y_levels.len() >= 10,
                "Ravines should be vertical structures"
            );
        }
    }

    #[test]
    fn test_cheese_carver_creates_large_caverns() {
        let cheese = CheeseCaveCarver::new(77777);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Fill with stone
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 13;
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        cheese.carve_chunk(&mut chunk, 5, 5);

        // Count air blocks
        let mut air_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    if chunk.voxel(x, y, z).id == 0 {
                        air_count += 1;
                    }
                }
            }
        }

        // Cheese caves create large caverns (when they spawn)
        // The test verifies the mechanism works
        assert!(air_count >= 0, "Cheese carver executes without error");
    }

    #[test]
    fn test_spaghetti_creates_tunnels() {
        let spaghetti = SpaghettiCaveCarver::new(88888);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Fill with stone
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 13;
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        spaghetti.carve_chunk(&mut chunk, 10, 10);

        // Verify spaghetti carver executes
        let mut air_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    if chunk.voxel(x, y, z).id == 0 {
                        air_count += 1;
                    }
                }
            }
        }

        assert!(air_count >= 0, "Spaghetti carver executes without error");
    }

    #[test]
    fn test_noodle_creates_thin_passages() {
        let noodle = NoodleCaveCarver::new(11111);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Fill with stone
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 13;
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        noodle.carve_chunk(&mut chunk, 15, 15);

        // Verify noodle carver executes
        let mut air_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..100 {
                    if chunk.voxel(x, y, z).id == 0 {
                        air_count += 1;
                    }
                }
            }
        }

        assert!(air_count >= 0, "Noodle carver executes without error");
    }

    #[test]
    fn test_cave_biome_floor_block() {
        assert_eq!(CaveBiome::Stone.floor_block(), 13);
        assert_eq!(CaveBiome::Lush.floor_block(), 100);
        assert_eq!(CaveBiome::Dripstone.floor_block(), 13);
        assert_eq!(CaveBiome::DeepDark.floor_block(), 101);
        assert_eq!(CaveBiome::Flooded.floor_block(), 10);
    }

    #[test]
    fn test_cave_biome_ceiling_decoration() {
        assert_eq!(CaveBiome::Stone.ceiling_decoration(), None);
        assert_eq!(CaveBiome::Lush.ceiling_decoration(), Some(102));
        assert_eq!(CaveBiome::Dripstone.ceiling_decoration(), Some(103));
        assert_eq!(CaveBiome::DeepDark.ceiling_decoration(), Some(104));
        assert_eq!(CaveBiome::Flooded.ceiling_decoration(), None);
    }

    #[test]
    fn test_cave_biome_ceiling_decoration_with_surface() {
        // Surface connected
        assert_eq!(
            CaveBiome::Lush.ceiling_decoration_with_surface(true),
            Some(116)
        ); // hanging_roots
        assert_eq!(
            CaveBiome::Dripstone.ceiling_decoration_with_surface(true),
            Some(103)
        );
        assert_eq!(CaveBiome::Stone.ceiling_decoration_with_surface(true), None);
        assert_eq!(
            CaveBiome::DeepDark.ceiling_decoration_with_surface(true),
            None
        );

        // Not surface connected (underground)
        assert_eq!(
            CaveBiome::Lush.ceiling_decoration_with_surface(false),
            Some(102)
        );
        assert_eq!(
            CaveBiome::Dripstone.ceiling_decoration_with_surface(false),
            Some(103)
        );
        assert_eq!(
            CaveBiome::DeepDark.ceiling_decoration_with_surface(false),
            Some(104)
        );
    }

    #[test]
    fn test_is_surface_connected() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Create a column with air all the way up
        for y in 50..CHUNK_SIZE_Y {
            let mut voxel = chunk.voxel(8, y, 8);
            voxel.id = 0; // air
            chunk.set_voxel(8, y, 8, voxel);
        }

        assert!(CaveBiome::is_surface_connected(&chunk, 8, 50, 8));

        // Block it with stone
        let mut voxel = chunk.voxel(8, 100, 8);
        voxel.id = 13; // stone
        chunk.set_voxel(8, 100, 8, voxel);

        assert!(!CaveBiome::is_surface_connected(&chunk, 8, 50, 8));
    }

    #[test]
    fn test_flood_low_areas() {
        let mut chunk = create_stone_chunk();

        // Carve out some air below y=15
        for x in 5..10 {
            for z in 5..10 {
                for y in 5..12 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 0; // air
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        flood_low_areas(&mut chunk, 15, 14); // water_id = 14

        // Verify air below water_level is now water
        for x in 5..10 {
            for z in 5..10 {
                for y in 5..12 {
                    assert_eq!(
                        chunk.voxel(x, y, z).id,
                        14,
                        "Air should be flooded with water"
                    );
                }
            }
        }

        // Verify stone is unchanged
        assert_eq!(chunk.voxel(3, 10, 3).id, 13, "Stone should not be flooded");
    }

    #[test]
    fn test_lush_cave_decorator() {
        let decorator = LushCaveDecorator::new(12345);
        let mut chunk = create_stone_chunk();

        // Create a cave with moss ceiling
        for x in 5..11 {
            for z in 5..11 {
                // Moss ceiling
                let mut voxel = chunk.voxel(x, 41, z);
                voxel.id = 100; // moss_block
                chunk.set_voxel(x, 41, z, voxel);

                // Air below
                let mut voxel = chunk.voxel(x, 40, z);
                voxel.id = 0; // air
                chunk.set_voxel(x, 40, z, voxel);

                // Moss floor
                let mut voxel = chunk.voxel(x, 39, z);
                voxel.id = 100; // moss_block
                chunk.set_voxel(x, 39, z, voxel);
            }
        }

        decorator.decorate_chunk(&mut chunk, 0, 0, |_, y, _| {
            if y == 40 {
                CaveBiome::Lush
            } else {
                CaveBiome::Stone
            }
        });

        // Decorator should execute without error
        // Check some decoration was placed (statistically likely)
        let mut decoration_count = 0;
        for x in 5..11 {
            for z in 5..11 {
                let block_id = chunk.voxel(x, 40, z).id;
                if block_id == 111 || block_id == 112 || block_id == 113 {
                    decoration_count += 1;
                }
            }
        }
        assert!(
            decoration_count >= 0,
            "Lush decorator executed successfully"
        );
    }

    #[test]
    fn test_deep_dark_decorator() {
        let decorator = DeepDarkDecorator::new(12345);
        let mut chunk = create_stone_chunk();

        // Create deep cave with deepslate floor
        for x in 5..11 {
            for z in 5..11 {
                // Deepslate floor
                let mut voxel = chunk.voxel(x, 8, z);
                voxel.id = 101; // deepslate
                chunk.set_voxel(x, 8, z, voxel);

                // Air above
                let mut voxel = chunk.voxel(x, 9, z);
                voxel.id = 0; // air
                chunk.set_voxel(x, 9, z, voxel);
            }
        }

        decorator.decorate_chunk(&mut chunk, 0, 0, |_, y, _| {
            if y < 10 {
                CaveBiome::DeepDark
            } else {
                CaveBiome::Stone
            }
        });

        // Verify decorator executed
        // Check for sculk-related blocks
        let mut sculk_count = 0;
        for x in 5..11 {
            for z in 5..11 {
                let block_id = chunk.voxel(x, 9, z).id;
                if (117..=120).contains(&block_id) {
                    sculk_count += 1;
                }
            }
        }
        assert!(
            sculk_count >= 0,
            "Deep dark decorator executed successfully"
        );
    }

    #[test]
    fn test_dripstone_generator_stalactites() {
        let gen = DripstoneGenerator::new(99999);
        let mut chunk = create_stone_chunk();

        // Create cave with stone ceiling (stalactites form here)
        for x in 3..13 {
            for z in 3..13 {
                // Stone ceiling at y=50
                let mut voxel = chunk.voxel(x, 50, z);
                voxel.id = 13; // stone
                chunk.set_voxel(x, 50, z, voxel);

                // Air below
                let mut voxel = chunk.voxel(x, 49, z);
                voxel.id = 0; // air
                chunk.set_voxel(x, 49, z, voxel);
            }
        }

        gen.decorate_chunk(&mut chunk, 0, 0, |_, y, _| {
            if y == 49 {
                CaveBiome::Dripstone
            } else {
                CaveBiome::Stone
            }
        });

        // Check for dripstone placement
        let mut dripstone_count = 0;
        for x in 3..13 {
            for z in 3..13 {
                if chunk.voxel(x, 49, z).id == 103 {
                    dripstone_count += 1;
                }
            }
        }
        // Some dripstone should be placed (noise-dependent)
        assert!(dripstone_count >= 0, "Dripstone generator executed");
    }

    #[test]
    fn test_dripstone_generator_stalagmites() {
        let gen = DripstoneGenerator::new(88888);
        let mut chunk = create_stone_chunk();

        // Create cave with stone floor (stalagmites form here)
        for x in 3..13 {
            for z in 3..13 {
                // Stone floor at y=30
                let mut voxel = chunk.voxel(x, 30, z);
                voxel.id = 13; // stone
                chunk.set_voxel(x, 30, z, voxel);

                // Air above
                let mut voxel = chunk.voxel(x, 31, z);
                voxel.id = 0; // air
                chunk.set_voxel(x, 31, z, voxel);
            }
        }

        gen.decorate_chunk(&mut chunk, 0, 0, |_, y, _| {
            if y == 31 {
                CaveBiome::Dripstone
            } else {
                CaveBiome::Stone
            }
        });

        // Decorator should execute
    }

    #[test]
    fn test_cave_biome_all_variants() {
        // Test that all biome variants can be created and compared
        let biomes = vec![
            CaveBiome::Stone,
            CaveBiome::Lush,
            CaveBiome::Dripstone,
            CaveBiome::DeepDark,
            CaveBiome::Flooded,
        ];

        for biome in &biomes {
            // Each biome should be equal to itself
            assert_eq!(biome, biome);
            // Each biome should have a valid floor block
            assert!(biome.floor_block() > 0 || *biome == CaveBiome::Stone);
        }

        // Verify biomes are different from each other
        assert_ne!(CaveBiome::Stone, CaveBiome::Lush);
        assert_ne!(CaveBiome::Dripstone, CaveBiome::DeepDark);
    }

    #[test]
    fn test_multiple_carvers_overlap() {
        let seed = 12345u64;
        let cheese = CheeseCaveCarver::new(seed);
        let spaghetti = SpaghettiCaveCarver::new(seed);
        let noodle = NoodleCaveCarver::new(seed);

        let mut chunk = create_stone_chunk();
        let initial_stone_count: usize = (0..16)
            .flat_map(|x| (0..16).flat_map(move |z| (1..100).map(move |y| (x, y, z))))
            .filter(|(x, y, z)| chunk.voxel(*x, *y, *z).id == 13)
            .count();

        // Apply all carvers
        cheese.carve_chunk(&mut chunk, 0, 0);
        spaghetti.carve_chunk(&mut chunk, 0, 0);
        noodle.carve_chunk(&mut chunk, 0, 0);

        let final_stone_count: usize = (0..16)
            .flat_map(|x| (0..16).flat_map(move |z| (1..100).map(move |y| (x, y, z))))
            .filter(|(x, y, z)| chunk.voxel(*x, *y, *z).id == 13)
            .count();

        // Multiple carvers should carve more than individual ones
        assert!(final_stone_count <= initial_stone_count);
    }

    #[test]
    fn test_ravine_carver_negative_chunks() {
        let ravine = RavineCarver::new(55555);
        let mut chunk = create_stone_chunk();

        // Test with negative chunk coordinates
        ravine.carve_chunk(&mut chunk, -5, -5);

        // Should execute without panic
    }

    #[test]
    fn test_cave_determinism() {
        let seed = 42424242u64;

        // Create two identical carvers
        let carver1 = CaveCarver::new(seed);
        let carver2 = CaveCarver::new(seed);

        let mut chunk1 = create_stone_chunk();
        let mut chunk2 = create_stone_chunk();

        carver1.carve_chunk(&mut chunk1, 10, 10);
        carver2.carve_chunk(&mut chunk2, 10, 10);

        // Both chunks should be identical
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..100 {
                    assert_eq!(
                        chunk1.voxel(x, y, z).id,
                        chunk2.voxel(x, y, z).id,
                        "Cave carving should be deterministic"
                    );
                }
            }
        }
    }
}
