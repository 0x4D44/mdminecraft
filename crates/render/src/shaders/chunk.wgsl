// Chunk rendering shader for voxel terrain

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
}

struct ChunkUniforms {
    offset: vec3<f32>,
}

// Fog parameters
const FOG_START: f32 = 64.0;
const FOG_END: f32 = 128.0;

// Sky gradient colors
const SKY_HORIZON: vec3<f32> = vec3<f32>(0.7, 0.85, 0.95); // Light blue/white at horizon
const SKY_ZENITH: vec3<f32> = vec3<f32>(0.3, 0.5, 0.85);   // Deeper blue at zenith

// Calculate sky color based on view direction
fn sky_color(view_dir: vec3<f32>) -> vec3<f32> {
    // Normalize y component to [0, 1] where 0 = horizontal, 1 = straight up
    let t = clamp(view_dir.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(SKY_HORIZON, SKY_ZENITH, t);
}

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

    // Color palette for different block types (improved colors)
    switch in.block_id {
        case 0u: { base_color = vec3<f32>(0.0, 0.0, 0.0); } // Air (shouldn't be rendered)
        case 1u: { base_color = vec3<f32>(0.55, 0.55, 0.55); } // Stone - slightly lighter gray
        case 2u: { base_color = vec3<f32>(0.45, 0.75, 0.35); } // Grass - vibrant green
        case 3u: { base_color = vec3<f32>(0.55, 0.35, 0.20); } // Dirt - warm brown
        case 4u: { base_color = vec3<f32>(0.85, 0.75, 0.55); } // Sand - warm beige
        case 5u: { base_color = vec3<f32>(0.45, 0.30, 0.15); } // Wood - rich brown bark
        case 6u: { base_color = vec3<f32>(0.25, 0.65, 0.25); } // Leaves - forest green
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

    // Calculate sky color based on view direction (creates gradient)
    let view_dir = normalize(in.world_pos - camera.camera_pos);
    let fog_color = sky_color(view_dir);

    // Mix lit color with fog color based on distance
    let final_color = mix(lit_color, fog_color, fog_factor);

    return vec4<f32>(final_color, 1.0);
}
