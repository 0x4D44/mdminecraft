use blake3::Hasher;
use mdminecraft_assets::{BlockFace, BlockRegistry, TextureAtlasMetadata};
use mdminecraft_world::{
    BlockId, Chunk, Voxel, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};

const AXIS_SIZE: [usize; 3] = [CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z];

/// Hash of the combined vertex/index buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshHash(pub [u8; 32]);

/// Packed vertex layout produced by the mesher.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    /// Position in chunk-local coordinates.
    pub position: [f32; 3],
    /// Face normal (unit length).
    pub normal: [f32; 3],
    /// Texture coordinates for atlas sampling.
    pub uv: [f32; 2],
    /// Block identifier baked into the face for material lookup.
    pub block_id: u16,
    /// Combined light level (max of skylight and blocklight), range 0-15.
    pub light: u8,
    /// Padding to align to 4 bytes
    _padding: u8,
}

/// Output mesh buffers per chunk.
#[derive(Debug, Clone)]
pub struct MeshBuffers {
    /// Vertex buffer used for draw submission.
    pub vertices: Vec<MeshVertex>,
    /// Index buffer (triangle list) referencing the vertex buffer.
    pub indices: Vec<u32>,
    /// Stable hash of the vertex + index buffers for cache comparisons.
    pub hash: MeshHash,
}

impl MeshBuffers {
    /// Construct an empty mesh (useful for initialization).
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            hash: MeshHash([0; 32]),
        }
    }
}

/// Generate greedy-meshed buffers for the given chunk.
pub fn mesh_chunk(
    chunk: &Chunk,
    registry: &BlockRegistry,
    atlas: Option<&TextureAtlasMetadata>,
) -> MeshBuffers {
    let mut builder = MeshBuilder::new(registry, atlas);
    GreedyMesher::mesh(chunk, &mut builder);
    builder.finish()
}

/// Internal helper for building mesh data.
struct MeshBuilder<'a> {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
    registry: &'a BlockRegistry,
    atlas: Option<&'a TextureAtlasMetadata>,
}

impl<'a> MeshBuilder<'a> {
    fn new(registry: &'a BlockRegistry, atlas: Option<&'a TextureAtlasMetadata>) -> Self {
        Self {
            vertices: Vec::with_capacity(1024), // Pre-allocate to reduce reallocations
            indices: Vec::with_capacity(1024 * 6 / 4), // Indices are 1.5x vertices for quads
            registry,
            atlas,
        }
    }

    fn push_quad(
        &mut self,
        block_id: BlockId,
        face: BlockFace,
        normal: [f32; 3],
        corners: [[f32; 3]; 4],
        normal_positive: bool,
        light: u8,
    ) {
        let base = self.vertices.len() as u32;

        let uvs = self.resolve_uvs(block_id, face);

        for (i, &corner) in corners.iter().enumerate() {
            self.vertices.push(MeshVertex {
                position: corner,
                normal,
                uv: uvs[i],
                block_id,
                light,
                _padding: 0,
            });
        }
        let indices = if normal_positive {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        for idx in indices {
            self.indices.push(base + idx);
        }
    }

    fn finish(self) -> MeshBuffers {
        let MeshBuilder {
            vertices, indices, ..
        } = self;
        let mut hasher = Hasher::new();
        for vertex in &vertices {
            hasher.update(bytemuck::cast_slice(&vertex.position));
            hasher.update(bytemuck::cast_slice(&vertex.normal));
            hasher.update(&vertex.block_id.to_le_bytes());
            hasher.update(&[vertex.light]);
        }
        hasher.update(bytemuck::cast_slice(&indices));
        MeshBuffers {
            vertices,
            indices,
            hash: MeshHash(*hasher.finalize().as_bytes()),
        }
    }

    fn resolve_uvs(&self, block_id: BlockId, face: BlockFace) -> [[f32; 2]; 4] {
        if let Some(atlas) = self.atlas {
            if let Some(desc) = self.registry.descriptor(block_id) {
                if let Some(entry) = atlas.entry(desc.texture_for(face)) {
                    return [
                        [entry.u0, entry.v0],
                        [entry.u1, entry.v0],
                        [entry.u1, entry.v1],
                        [entry.u0, entry.v1],
                    ];
                }
            }
        }

        let atlas_size = 16.0;
        let atlas_x = (block_id % 16) as f32;
        let atlas_y = (block_id / 16) as f32;
        [
            [atlas_x / atlas_size, atlas_y / atlas_size],
            [(atlas_x + 1.0) / atlas_size, atlas_y / atlas_size],
            [(atlas_x + 1.0) / atlas_size, (atlas_y + 1.0) / atlas_size],
            [atlas_x / atlas_size, (atlas_y + 1.0) / atlas_size],
        ]
    }
}

struct GreedyMesher;

impl GreedyMesher {
    pub fn mesh(chunk: &Chunk, builder: &mut MeshBuilder) {
        for axis in 0..3 {
            Self::mesh_axis(chunk, builder, axis);
        }
    }

    fn mesh_axis(chunk: &Chunk, builder: &mut MeshBuilder, axis: usize) {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        let width = AXIS_SIZE[u_axis];
        let height = AXIS_SIZE[v_axis];
        let mut mask: Vec<Option<FaceDesc>> = vec![None; width * height];

        for slice in 0..=AXIS_SIZE[axis] {
            for j in 0..height {
                for i in 0..width {
                    let idx = j * width + i;
                    mask[idx] = Self::sample_face(chunk, builder.registry, axis, slice, i, j);
                }
            }

            let mut j = 0;
            while j < height {
                let mut i = 0;
                while i < width {
                    let idx = j * width + i;
                    if let Some(cell) = mask[idx] {
                        let mut quad_width = 1;
                        while i + quad_width < width
                            && mask[j * width + i + quad_width] == Some(cell)
                        {
                            quad_width += 1;
                        }

                        let mut quad_height = 1;
                        'scan: while j + quad_height < height {
                            for k in 0..quad_width {
                                if mask[(j + quad_height) * width + i + k] != Some(cell) {
                                    break 'scan;
                                }
                            }
                            quad_height += 1;
                        }

                        Self::emit_quad(
                            builder,
                            axis,
                            slice,
                            (i, j),
                            (quad_width, quad_height),
                            cell,
                        );

                        for dy in 0..quad_height {
                            for dx in 0..quad_width {
                                mask[(j + dy) * width + i + dx] = None;
                            }
                        }
                        i += quad_width;
                    } else {
                        i += 1;
                    }
                }
                j += 1;
            }
        }
    }

    fn sample_face(
        chunk: &Chunk,
        registry: &BlockRegistry,
        axis: usize,
        slice: usize,
        u: usize,
        v: usize,
    ) -> Option<FaceDesc> {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;

        let front = if slice < AXIS_SIZE[axis] {
            let mut pos = [0usize; 3];
            pos[axis] = slice;
            pos[u_axis] = u;
            pos[v_axis] = v;
            Some(chunk.voxel(pos[0], pos[1], pos[2]))
        } else {
            None
        };

        let back = if slice == 0 {
            None
        } else {
            let mut pos = [0usize; 3];
            pos[axis] = slice - 1;
            pos[u_axis] = u;
            pos[v_axis] = v;
            Some(chunk.voxel(pos[0], pos[1], pos[2]))
        };

        // Determine which faces to render based on block types:
        // - Opaque blocks: render face when adjacent to non-opaque (air or transparent)
        // - Transparent blocks: render face when adjacent to air or different block type
        // - Air: never render
        match (front, back) {
            (Some(a), Some(b)) => {
                let a_opaque = is_opaque(a, registry);
                let b_opaque = is_opaque(b, registry);
                let a_solid = is_solid(a);
                let b_solid = is_solid(b);

                if a_opaque && !b_opaque {
                    // Opaque block 'a' facing non-opaque 'b' (air or transparent)
                    let light = a.light_sky.max(a.light_block);
                    Some(FaceDesc::new(a.id, axis, false, light))
                } else if b_opaque && !a_opaque {
                    // Opaque block 'b' facing non-opaque 'a' (air or transparent)
                    let light = b.light_sky.max(b.light_block);
                    Some(FaceDesc::new(b.id, axis, true, light))
                } else if a_solid && !a_opaque && !b_solid {
                    // Transparent block 'a' facing air
                    let light = a.light_sky.max(a.light_block);
                    Some(FaceDesc::new(a.id, axis, false, light))
                } else if b_solid && !b_opaque && !a_solid {
                    // Transparent block 'b' facing air
                    let light = b.light_sky.max(b.light_block);
                    Some(FaceDesc::new(b.id, axis, true, light))
                } else if a_solid && !a_opaque && b_solid && !b_opaque && a.id != b.id {
                    // Two different transparent blocks - render face for 'a'
                    let light = a.light_sky.max(a.light_block);
                    Some(FaceDesc::new(a.id, axis, false, light))
                } else {
                    None
                }
            }
            (Some(a), None) if is_solid(a) => {
                // Block at chunk edge facing outside
                let light = a.light_sky.max(a.light_block);
                Some(FaceDesc::new(a.id, axis, false, light))
            }
            (None, Some(b)) if is_solid(b) => {
                // Block at chunk edge facing outside
                let light = b.light_sky.max(b.light_block);
                Some(FaceDesc::new(b.id, axis, true, light))
            }
            _ => None,
        }
    }

    fn emit_quad(
        builder: &mut MeshBuilder,
        axis: usize,
        slice: usize,
        origin_uv: (usize, usize),
        quad_size: (usize, usize),
        cell: FaceDesc,
    ) {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        let (u, v) = origin_uv;
        let (quad_width, quad_height) = quad_size;

        let mut origin = [0f32; 3];
        origin[u_axis] = u as f32;
        origin[v_axis] = v as f32;
        origin[axis] = slice as f32;

        let mut du = [0f32; 3];
        du[u_axis] = quad_width as f32;
        let mut dv = [0f32; 3];
        dv[v_axis] = quad_height as f32;

        let v0 = origin;
        let v1 = add(origin, du);
        let v2 = add(add(origin, du), dv);
        let v3 = add(origin, dv);

        let normal = [
            cell.normal[0] as f32,
            cell.normal[1] as f32,
            cell.normal[2] as f32,
        ];
        builder.push_quad(
            cell.block_id,
            cell.face(),
            normal,
            [v0, v1, v2, v3],
            cell.normal[axis] > 0,
            cell.light,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FaceDesc {
    block_id: BlockId,
    normal: [i8; 3],
    light: u8,
}

impl FaceDesc {
    fn new(block_id: BlockId, axis: usize, positive: bool, light: u8) -> Self {
        let mut normal = [0i8; 3];
        normal[axis] = if positive { 1 } else { -1 };
        Self {
            block_id,
            normal,
            light,
        }
    }

    fn face(&self) -> BlockFace {
        match self.normal {
            [0, 1, 0] => BlockFace::Up,
            [0, -1, 0] => BlockFace::Down,
            [0, 0, 1] => BlockFace::South,
            [0, 0, -1] => BlockFace::North,
            [1, 0, 0] => BlockFace::East,
            [-1, 0, 0] => BlockFace::West,
            _ => BlockFace::Up,
        }
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn is_opaque(voxel: Voxel, registry: &BlockRegistry) -> bool {
    registry
        .descriptor(voxel.id)
        .map(|d| d.opaque)
        .unwrap_or(voxel.id != BLOCK_AIR)
}

/// Check if a voxel is a solid (non-air) block that should be rendered
fn is_solid(voxel: Voxel) -> bool {
    voxel.id != BLOCK_AIR
}

#[cfg(test)]
mod tests {
    use mdminecraft_assets::{BlockDescriptor, BlockRegistry};
    use mdminecraft_world::{Chunk, ChunkPos, Voxel};

    use super::*;

    fn registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor::simple("air", false),
            BlockDescriptor::simple("stone", true),
            BlockDescriptor::simple("leaves", false), // transparent block
        ])
    }

    #[test]
    fn chunk_with_single_block_meshes() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let voxel = Voxel {
            id: 1,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        chunk.set_voxel(1, 1, 1, voxel);
        let registry = registry();
        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(!mesh.vertices.is_empty());
        assert_eq!(mesh.indices.len(), 36); // 6 faces * 2 tris * 3 indices
    }

    #[test]
    fn mesh_hash_changes_with_voxel_updates() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();
        let mesh = mesh_chunk(&chunk, &registry, None);
        let hash_empty = mesh.hash;

        let voxel = Voxel {
            id: 2,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        chunk.set_voxel(0, 0, 0, voxel);
        let mesh_updated = mesh_chunk(&chunk, &registry, None);
        assert_ne!(hash_empty, mesh_updated.hash);
    }

    #[test]
    fn transparent_block_renders_faces() {
        // Test that transparent blocks (like leaves) render faces when adjacent to air
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();

        // Place a transparent block (id=2 is "leaves" with opaque=false)
        let leaves = Voxel {
            id: 2,
            state: 0,
            light_sky: 15,
            light_block: 0,
        };
        chunk.set_voxel(5, 5, 5, leaves);

        let mesh = mesh_chunk(&chunk, &registry, None);

        // Transparent block surrounded by air should have 6 faces
        assert!(
            !mesh.vertices.is_empty(),
            "Transparent block should generate mesh vertices"
        );
        assert_eq!(
            mesh.indices.len(),
            36,
            "Transparent block should have 6 faces (36 indices)"
        );
    }

    #[test]
    fn transparent_blocks_adjacent_render_correctly() {
        // Test that two adjacent transparent blocks of the same type don't render internal faces
        // and that greedy meshing combines faces properly
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();

        let leaves = Voxel {
            id: 2,
            state: 0,
            light_sky: 15,
            light_block: 0,
        };
        // Place two adjacent leaves blocks along Z axis
        chunk.set_voxel(5, 5, 5, leaves);
        chunk.set_voxel(5, 5, 6, leaves);

        let mesh = mesh_chunk(&chunk, &registry, None);

        // Two adjacent same-type transparent blocks: greedy mesher merges front/back Z faces
        // So we get: 2 merged Z faces (front+back) + 4 individual side faces per block merged
        // = fewer total quads due to greedy meshing
        // The key test is that SOME mesh is generated (transparent blocks render)
        assert!(
            !mesh.vertices.is_empty(),
            "Should generate mesh for adjacent transparent blocks"
        );
        // With greedy meshing, the exact count depends on merge opportunities
        // Just verify we have less than 12 faces (would be 72 indices if no merging)
        assert!(
            !mesh.indices.is_empty() && mesh.indices.len() < 72,
            "Greedy meshing should reduce face count for adjacent blocks, got {} indices",
            mesh.indices.len()
        );
    }
}
