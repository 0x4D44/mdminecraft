// Voxel rendering shader for mdminecraft

// Camera uniform binding
struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// Chunk offset (passed via push constants or uniform)
struct ChunkUniform {
    chunk_offset: vec3<f32>,
    _padding: f32,
}

@group(1) @binding(0)
var<uniform> chunk: ChunkUniform;

// Vertex input from mesh generation
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) packed_data: u32,  // block_id (u16) and light (u8) packed
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) block_id: u32,
    @location(3) light: f32,
}

// Vertex shader
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply chunk offset to position
    let world_pos = in.position + chunk.chunk_offset;

    // Transform position to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.normal = in.normal;

    // Unpack block_id and light from packed u32
    // Layout: [block_id: u16][light: u8][padding: u8]
    out.block_id = in.packed_data & 0xFFFFu;
    let light_value = (in.packed_data >> 16u) & 0xFFu;

    // Convert light level (0-15) to 0.0-1.0 range
    out.light = f32(light_value) / 15.0;

    return out;
}

// Simple color palette for different block types
fn get_block_color(block_id: u32) -> vec3<f32> {
    // Basic color mapping based on block ID
    switch block_id {
        case 0u: { return vec3<f32>(0.0, 0.0, 0.0); }      // Air (shouldn't render)
        case 1u: { return vec3<f32>(0.5, 0.5, 0.5); }      // Stone
        case 2u: { return vec3<f32>(0.3, 0.6, 0.2); }      // Grass top
        case 3u: { return vec3<f32>(0.4, 0.3, 0.2); }      // Dirt
        case 4u: { return vec3<f32>(0.6, 0.6, 0.6); }      // Cobblestone
        case 5u: { return vec3<f32>(0.7, 0.5, 0.3); }      // Wood planks
        case 6u: { return vec3<f32>(0.2, 0.7, 0.3); }      // Sapling
        case 7u: { return vec3<f32>(0.3, 0.3, 0.3); }      // Bedrock
        case 8u: { return vec3<f32>(0.2, 0.4, 0.8); }      // Water
        case 9u: { return vec3<f32>(0.9, 0.3, 0.1); }      // Lava
        case 10u: { return vec3<f32>(0.9, 0.9, 0.5); }     // Sand
        default: { return vec3<f32>(0.8, 0.2, 0.8); }      // Unknown (magenta)
    }
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Get base color from block ID
    var base_color = get_block_color(in.block_id);

    // Simple diffuse lighting using normal
    let sun_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let diffuse = max(dot(in.normal, sun_dir), 0.0);

    // Ambient + diffuse + voxel light
    let ambient = 0.3;
    let lighting = ambient + diffuse * 0.5 + in.light * 0.4;

    // Apply lighting to base color
    let final_color = base_color * lighting;

    return vec4<f32>(final_color, 1.0);
}
