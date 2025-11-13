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

/// Chunk-local position (X, Y, Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Per-voxel data stored in the SoA arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
