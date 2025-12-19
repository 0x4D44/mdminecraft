use mdminecraft_core::DimensionId;
use std::fmt;

/// Chunk width (X axis) in voxels.
pub const CHUNK_SIZE_X: usize = 16;
/// Chunk height (Y axis) in voxels.
pub const CHUNK_SIZE_Y: usize = 256;
/// Chunk depth (Z axis) in voxels.
pub const CHUNK_SIZE_Z: usize = 16;
/// Total voxel count per chunk.
pub const CHUNK_VOLUME: usize = CHUNK_SIZE_X * CHUNK_SIZE_Y * CHUNK_SIZE_Z;

/// Block identifier referencing the registry.
pub type BlockId = u16;
/// Block state metadata bits.
pub type BlockState = u16;

/// Reserved ID for air.
pub const BLOCK_AIR: BlockId = 0;

/// ID for stone block.
pub const BLOCK_STONE: BlockId = 1;

/// ID for dirt.
pub const BLOCK_DIRT: BlockId = 2;

/// ID for grass.
pub const BLOCK_GRASS: BlockId = 3;

/// ID for sand.
pub const BLOCK_SAND: BlockId = 4;

/// ID for gravel.
pub const BLOCK_GRAVEL: BlockId = 5;

/// ID for water source block.
pub const BLOCK_WATER: BlockId = 6;

/// ID for ice.
pub const BLOCK_ICE: BlockId = 7;

/// ID for snow.
pub const BLOCK_SNOW: BlockId = 8;

/// ID for clay.
pub const BLOCK_CLAY: BlockId = 9;

/// ID for bedrock.
pub const BLOCK_BEDROCK: BlockId = 10;

/// ID for oak log.
pub const BLOCK_OAK_LOG: BlockId = 11;

/// ID for oak planks.
pub const BLOCK_OAK_PLANKS: BlockId = 12;

/// ID for crafting table (from blocks.json index).
pub const BLOCK_CRAFTING_TABLE: BlockId = 13;

/// ID for cobblestone.
pub const BLOCK_COBBLESTONE: BlockId = 24;

/// ID for glass.
pub const BLOCK_GLASS: BlockId = 25;

/// ID for coal ore (spawns y: 0-128).
pub const BLOCK_COAL_ORE: BlockId = 14;

/// ID for iron ore (spawns y: 0-64).
pub const BLOCK_IRON_ORE: BlockId = 15;

/// ID for gold ore (spawns y: 0-32).
pub const BLOCK_GOLD_ORE: BlockId = 16;

/// ID for diamond ore (spawns y: 0-16).
pub const BLOCK_DIAMOND_ORE: BlockId = 17;

/// ID for furnace block.
pub const BLOCK_FURNACE: BlockId = 18;

/// ID for lit furnace block.
pub const BLOCK_FURNACE_LIT: BlockId = 19;

/// ID for obsidian.
pub const BLOCK_OBSIDIAN: BlockId = 23;

/// ID for lapis ore (spawns y: 0-32).
pub const BLOCK_LAPIS_ORE: BlockId = 98;

/// ID for enchanting table.
pub const BLOCK_ENCHANTING_TABLE: BlockId = 99;

/// ID for brewing stand.
pub const BLOCK_BREWING_STAND: BlockId = 100;

/// ID for nether wart block.
pub const BLOCK_NETHER_WART_BLOCK: BlockId = 101;

/// ID for soul sand.
pub const BLOCK_SOUL_SAND: BlockId = 102;

/// ID for bookshelf.
pub const BLOCK_BOOKSHELF: BlockId = 103;

/// ID for sugar cane.
pub const BLOCK_SUGAR_CANE: BlockId = 104;

/// ID for brown mushroom.
pub const BLOCK_BROWN_MUSHROOM: BlockId = 105;

/// ID for magma cream ore (Overworld proxy block).
pub const BLOCK_MAGMA_CREAM_ORE: BlockId = 106;

/// ID for moss block.
pub const BLOCK_MOSS_BLOCK: BlockId = 75;

/// ID for deepslate.
pub const BLOCK_DEEPSLATE: BlockId = 76;

/// ID for glow lichen.
pub const BLOCK_GLOW_LICHEN: BlockId = 77;

/// ID for pointed dripstone.
pub const BLOCK_POINTED_DRIPSTONE: BlockId = 78;

/// ID for sculk.
pub const BLOCK_SCULK: BlockId = 79;

/// ID for magma block.
pub const BLOCK_MAGMA_BLOCK: BlockId = 80;

/// ID for cave vines.
pub const BLOCK_CAVE_VINES: BlockId = 86;

/// ID for moss carpet.
pub const BLOCK_MOSS_CARPET: BlockId = 87;

/// ID for spore blossom.
pub const BLOCK_SPORE_BLOSSOM: BlockId = 88;

/// ID for hanging roots.
pub const BLOCK_HANGING_ROOTS: BlockId = 91;

/// ID for sculk sensor.
pub const BLOCK_SCULK_SENSOR: BlockId = 92;

/// ID for sculk shrieker.
pub const BLOCK_SCULK_SHRIEKER: BlockId = 93;

/// ID for sculk catalyst.
pub const BLOCK_SCULK_CATALYST: BlockId = 94;

/// ID for sculk vein.
pub const BLOCK_SCULK_VEIN: BlockId = 95;

/// ID for smooth basalt.
pub const BLOCK_SMOOTH_BASALT: BlockId = 82;

/// ID for calcite.
pub const BLOCK_CALCITE: BlockId = 83;

/// ID for amethyst block.
pub const BLOCK_AMETHYST_BLOCK: BlockId = 84;

/// ID for budding amethyst.
pub const BLOCK_BUDDING_AMETHYST: BlockId = 85;

/// Chunk-local position (X, Y, Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalPos {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl LocalPos {
    /// Convert to a linear index within the SoA arrays.
    pub fn index(self) -> usize {
        debug_assert!(self.x < CHUNK_SIZE_X);
        debug_assert!(self.y < CHUNK_SIZE_Y);
        debug_assert!(self.z < CHUNK_SIZE_Z);
        (self.y * CHUNK_SIZE_Z + self.z) * CHUNK_SIZE_X + self.x
    }
}

/// Chunk coordinate (X,Z) in chunk space.
/// Implements Ord for deterministic iteration in BTreeMap/BTreeSet (sorts by x, then z).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

impl fmt::Display for ChunkPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.z)
    }
}

/// Dimension-scoped chunk key.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct ChunkKey {
    pub dimension: DimensionId,
    pub pos: ChunkPos,
}

impl ChunkKey {
    pub const fn new(dimension: DimensionId, x: i32, z: i32) -> Self {
        Self {
            dimension,
            pos: ChunkPos::new(x, z),
        }
    }

    pub const fn from_pos(dimension: DimensionId, pos: ChunkPos) -> Self {
        Self { dimension, pos }
    }
}

impl fmt::Display for ChunkKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.dimension.as_str(), self.pos)
    }
}

/// Per-voxel data stored in the SoA arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Voxel {
    pub id: BlockId,
    pub state: BlockState,
    pub light_sky: u8,
    pub light_block: u8,
}

impl Default for Voxel {
    fn default() -> Self {
        Self {
            id: BLOCK_AIR,
            state: 0,
            light_sky: 0,
            light_block: 0,
        }
    }
}

impl Voxel {
    #[inline]
    pub fn is_air(&self) -> bool {
        self.id == BLOCK_AIR
    }

    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.id != BLOCK_AIR
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    /// Dirty flags set whenever chunk data changes.
    pub struct DirtyFlags: u8 {
        const MESH = 0b0000_0001;
        const LIGHT = 0b0000_0010;
    }
}

impl Default for DirtyFlags {
    fn default() -> Self {
        DirtyFlags::empty()
    }
}

/// Chunk storing voxel data in SoA form plus dirty flags.
pub struct Chunk {
    position: ChunkPos,
    voxels: Vec<Voxel>,
    dirty: DirtyFlags,
}

impl Chunk {
    /// Allocate a fresh chunk filled with air.
    pub fn new(position: ChunkPos) -> Self {
        Self {
            position,
            voxels: vec![Voxel::default(); CHUNK_VOLUME],
            dirty: DirtyFlags::all(),
        }
    }

    #[inline]
    pub fn position(&self) -> ChunkPos {
        self.position
    }

    fn index(x: usize, y: usize, z: usize) -> usize {
        LocalPos { x, y, z }.index()
    }

    /// Fetch a voxel copy.
    pub fn voxel(&self, x: usize, y: usize, z: usize) -> Voxel {
        let idx = Self::index(x, y, z);
        self.voxels[idx]
    }

    /// Set a voxel and mark the relevant dirty flags.
    pub fn set_voxel(&mut self, x: usize, y: usize, z: usize, voxel: Voxel) {
        let idx = Self::index(x, y, z);
        if self.voxels[idx] != voxel {
            self.voxels[idx] = voxel;
            self.dirty.insert(DirtyFlags::MESH | DirtyFlags::LIGHT);
        }
    }

    /// Borrow raw voxel storage for meshing.
    #[allow(dead_code)]
    pub(crate) fn voxels(&self) -> &[Voxel] {
        &self.voxels
    }

    /// Consume and return the current dirty flags.
    pub fn take_dirty_flags(&mut self) -> DirtyFlags {
        let flags = self.dirty;
        self.dirty = DirtyFlags::empty();
        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_voxel_marks_dirty() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        assert!(chunk.take_dirty_flags().contains(DirtyFlags::MESH));
        let voxel = Voxel {
            id: 5,
            state: 1,
            light_sky: 15,
            light_block: 0,
        };
        chunk.set_voxel(1, 2, 3, voxel);
        assert_eq!(chunk.voxel(1, 2, 3).id, 5);
        assert!(chunk.take_dirty_flags().contains(DirtyFlags::MESH));
    }

    #[test]
    fn test_local_pos_index() {
        // Test index calculation
        let pos1 = LocalPos { x: 0, y: 0, z: 0 };
        assert_eq!(pos1.index(), 0);

        let pos2 = LocalPos { x: 15, y: 0, z: 0 };
        assert_eq!(pos2.index(), 15);

        let pos3 = LocalPos { x: 0, y: 1, z: 0 };
        // y=1 means z*x + x offset, then y layer
        let expected = CHUNK_SIZE_Z * CHUNK_SIZE_X;
        assert_eq!(pos3.index(), expected);
    }

    #[test]
    fn test_chunk_pos_display() {
        let pos = ChunkPos::new(5, -3);
        let display = format!("{}", pos);
        assert_eq!(display, "(5, -3)");
    }

    #[test]
    fn test_voxel_default() {
        let voxel = Voxel::default();
        assert_eq!(voxel.id, BLOCK_AIR);
        assert_eq!(voxel.state, 0);
        assert_eq!(voxel.light_sky, 0);
        assert_eq!(voxel.light_block, 0);
    }

    #[test]
    fn test_voxel_is_air() {
        let air = Voxel::default();
        assert!(air.is_air());
        assert!(!air.is_opaque());

        let stone = Voxel {
            id: BLOCK_STONE,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        assert!(!stone.is_air());
        assert!(stone.is_opaque());
    }

    #[test]
    fn test_dirty_flags_default() {
        let flags = DirtyFlags::default();
        assert!(flags.is_empty());
    }

    #[test]
    fn test_chunk_position() {
        let pos = ChunkPos::new(10, 20);
        let chunk = Chunk::new(pos);
        assert_eq!(chunk.position(), pos);
    }

    #[test]
    fn test_chunk_new_is_air() {
        let chunk = Chunk::new(ChunkPos::new(0, 0));

        // All voxels should be air
        for x in 0..CHUNK_SIZE_X {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    assert!(chunk.voxel(x, y, z).is_air());
                }
            }
        }
    }

    #[test]
    fn test_set_same_voxel_no_dirty() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        chunk.take_dirty_flags(); // Clear initial flags

        // Set air to air - should not set dirty
        let air = Voxel::default();
        chunk.set_voxel(0, 0, 0, air);
        assert!(chunk.take_dirty_flags().is_empty());
    }

    #[test]
    fn test_chunk_pos_ordering() {
        // ChunkPos implements Ord for BTreeMap determinism
        let pos1 = ChunkPos::new(0, 0);
        let pos2 = ChunkPos::new(1, 0);
        let pos3 = ChunkPos::new(0, 1);

        assert!(pos1 < pos2);
        assert!(pos1 < pos3);
        assert!(pos2 > pos1);
    }

    #[test]
    fn test_dirty_flags_all() {
        let chunk = Chunk::new(ChunkPos::new(0, 0));
        // New chunk should have all flags set
        assert!(chunk.dirty.contains(DirtyFlags::MESH));
        assert!(chunk.dirty.contains(DirtyFlags::LIGHT));
    }

    #[test]
    fn test_voxel_serialization() {
        let voxel = Voxel {
            id: 42,
            state: 7,
            light_sky: 15,
            light_block: 10,
        };

        let serialized = serde_json::to_string(&voxel).unwrap();
        let deserialized: Voxel = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, 42);
        assert_eq!(deserialized.state, 7);
        assert_eq!(deserialized.light_sky, 15);
        assert_eq!(deserialized.light_block, 10);
    }

    #[test]
    fn test_chunk_pos_serialization() {
        let pos = ChunkPos::new(-5, 10);

        let serialized = serde_json::to_string(&pos).unwrap();
        let deserialized: ChunkPos = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.x, -5);
        assert_eq!(deserialized.z, 10);
    }
}
