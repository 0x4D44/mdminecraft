// Chunk rendering shader for voxel terrain

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
}

struct ChunkUniforms {
    offset: vec3<f32>,
}

// Fog parameters
const FOG_START: f32 = 48.0;
const FOG_END: f32 = 96.0;
const FOG_COLOR: vec3<f32> = vec3<f32>(0.53, 0.81, 0.92); // Sky blue

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(1) @binding(0)
var<uniform> chunk: ChunkUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) block_id: u32,
    @location(3) light: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) block_id: u32,
    @location(3) light: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply chunk offset to position
    let world_pos = in.position + chunk.offset;

    // Transform position to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.normal = in.normal;
    out.block_id = in.block_id;

    // Normalize light level to [0, 1]
    out.light = f32(in.light) / 15.0;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple block coloring based on block_id (placeholder until texture atlas)
    var base_color: vec3<f32>;

    // Color palette for different block types
    switch in.block_id {
        case 0u: { base_color = vec3<f32>(0.0, 0.0, 0.0); } // Air (shouldn't be rendered)
        case 1u: { base_color = vec3<f32>(0.5, 0.5, 0.5); } // Stone
        case 2u: { base_color = vec3<f32>(0.4, 0.8, 0.3); } // Grass
        case 3u: { base_color = vec3<f32>(0.6, 0.4, 0.2); } // Dirt
        case 4u: { base_color = vec3<f32>(0.8, 0.7, 0.5); } // Sand
        case 5u: { base_color = vec3<f32>(0.3, 0.2, 0.1); } // Wood
        case 6u: { base_color = vec3<f32>(0.2, 0.6, 0.2); } // Leaves
        default: { base_color = vec3<f32>(0.9, 0.2, 0.9); } // Magenta for unknown blocks
    }

    // Simple diffuse lighting based on normal
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let diffuse = max(dot(in.normal, light_dir), 0.0);

    // Combine ambient, diffuse, and voxel lighting
    let ambient = 0.3;
    let lighting = ambient + diffuse * 0.5 + in.light * 0.2;

    let lit_color = base_color * lighting;

    // Calculate distance-based fog
    let distance = length(in.world_pos - camera.camera_pos);
    let fog_factor = clamp((distance - FOG_START) / (FOG_END - FOG_START), 0.0, 1.0);

    // Mix lit color with fog color based on distance
    let final_color = mix(lit_color, FOG_COLOR, fog_factor);

    return vec4<f32>(final_color, 1.0);
}
