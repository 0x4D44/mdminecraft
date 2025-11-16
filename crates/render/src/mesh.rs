use blake3::Hasher;
use mdminecraft_assets::BlockRegistry;
use mdminecraft_world::{
    BlockId, Chunk, Voxel, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};

const AXIS_SIZE: [usize; 3] = [CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z];

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
    /// Block identifier baked into the face for material lookup.
    pub block_id: u16,
    /// Combined light level (max of skylight and blocklight), range 0-15.
    pub light: u8,
    /// Padding to align to 4 bytes
    _padding: u8,
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
pub fn mesh_chunk(chunk: &Chunk, registry: &BlockRegistry) -> MeshBuffers {
    let mut builder = MeshBuilder::default();
    GreedyMesher::mesh(chunk, registry, &mut builder);
    builder.finish()
}

#[derive(Default)]
struct MeshBuilder {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    fn push_quad(
        &mut self,
        block_id: BlockId,
        normal: [f32; 3],
        corners: [[f32; 3]; 4],
        normal_positive: bool,
        light: u8,
    ) {
        let base = self.vertices.len() as u32;
        for &corner in &corners {
            self.vertices.push(MeshVertex {
                position: corner,
                normal,
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
        let MeshBuilder { vertices, indices } = self;
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
}

struct GreedyMesher;

impl GreedyMesher {
    pub fn mesh(chunk: &Chunk, registry: &BlockRegistry, builder: &mut MeshBuilder) {
        for axis in 0..3 {
            Self::mesh_axis(chunk, registry, builder, axis);
        }
    }

    fn mesh_axis(chunk: &Chunk, registry: &BlockRegistry, builder: &mut MeshBuilder, axis: usize) {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        let width = AXIS_SIZE[u_axis];
        let height = AXIS_SIZE[v_axis];
        let mut mask: Vec<Option<FaceDesc>> = vec![None; width * height];

        for slice in 0..=AXIS_SIZE[axis] {
            for j in 0..height {
                for i in 0..width {
                    let idx = j * width + i;
                    mask[idx] = Self::sample_face(chunk, registry, axis, slice, i, j);
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

                        Self::emit_quad(builder, axis, slice, i, j, quad_width, quad_height, cell);

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

        match (front, back) {
            (Some(a), Some(b)) => match (is_opaque(a, registry), is_opaque(b, registry)) {
                (true, false) => {
                    let light = a.light_sky.max(a.light_block);
                    Some(FaceDesc::new(a.id, axis, true, light))
                }
                (false, true) => {
                    let light = b.light_sky.max(b.light_block);
                    Some(FaceDesc::new(b.id, axis, false, light))
                }
                _ => None,
            },
            (Some(a), None) if is_opaque(a, registry) => {
                let light = a.light_sky.max(a.light_block);
                Some(FaceDesc::new(a.id, axis, true, light))
            }
            (None, Some(b)) if is_opaque(b, registry) => {
                let light = b.light_sky.max(b.light_block);
                Some(FaceDesc::new(b.id, axis, false, light))
            }
            _ => None,
        }
    }

    fn emit_quad(
        builder: &mut MeshBuilder,
        axis: usize,
        slice: usize,
        u: usize,
        v: usize,
        quad_width: usize,
        quad_height: usize,
        cell: FaceDesc,
    ) {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;

        let mut origin = [0f32; 3];
        origin[u_axis] = u as f32;
        origin[v_axis] = v as f32;
        origin[axis] = slice as f32;
        if cell.normal[axis] < 0 {
            origin[axis] -= 1.0;
        }

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

#[cfg(test)]
mod tests {
    use mdminecraft_assets::{BlockDescriptor, BlockRegistry};
    use mdminecraft_world::{Chunk, ChunkPos, Voxel};

    use super::*;

    fn registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor {
                name: "air".into(),
                opaque: false,
            },
            BlockDescriptor {
                name: "stone".into(),
                opaque: true,
            },
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
        let mesh = mesh_chunk(&chunk, &registry());
        assert!(!mesh.vertices.is_empty());
        assert_eq!(mesh.indices.len(), 36); // 6 faces * 2 tris * 3 indices
    }

    #[test]
    fn mesh_hash_changes_with_voxel_updates() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();
        let mesh = mesh_chunk(&chunk, &registry);
        let hash_empty = mesh.hash;

        let voxel = Voxel {
            id: 2,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        chunk.set_voxel(0, 0, 0, voxel);
        let mesh_updated = mesh_chunk(&chunk, &registry);
        assert_ne!(hash_empty, mesh_updated.hash);
    }
}
