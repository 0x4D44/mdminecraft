// Voxel rendering shader with per-face texture atlas, water animation, fog, and ambient occlusion.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct TimeUniform {
    time: vec4<f32>,
    sun_dir: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
}

@group(0) @binding(1)
var<uniform> time_uniform: TimeUniform;

struct ChunkUniform {
    chunk_offset: vec3<f32>,
    _padding: f32,
}

@group(1) @binding(0)
var<uniform> chunk: ChunkUniform;

@group(2) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(2) @binding(1)
var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) packed_data: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) block_id: u32,
    @location(4) light: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = in.position + chunk.chunk_offset;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.normal = in.normal;
    out.uv = in.uv;
    out.block_id = in.packed_data & 0xFFFFu;
    let light_value = (in.packed_data >> 16u) & 0xFFu;
    out.light = f32(light_value) / 15.0;
    return out;
}

fn apply_fog(color: vec3<f32>, dist: f32) -> vec3<f32> {
    let fog_start = time_uniform.fog_params.x;
    let fog_end = time_uniform.fog_params.y;
    let fog_factor = clamp((dist - fog_start) / max(fog_end - fog_start, 0.0001), 0.0, 1.0);
    return mix(color, time_uniform.fog_color.rgb, fog_factor);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(atlas_texture, atlas_sampler, in.uv).rgb;
    let sun_dir = normalize(time_uniform.sun_dir.xyz);
    let precipitation = time_uniform.fog_params.w;
    let diffuse = max(dot(in.normal, sun_dir), 0.0);
    let time_factor = smoothstep(0.0, 0.3, time_uniform.time.x) * (1.0 - smoothstep(0.7, 1.0, time_uniform.time.x));
    let ambient = mix(0.1, 0.3, time_factor);
    let sun_contrib = (ambient + diffuse * 0.5) * mix(1.0, 0.65, precipitation);
    let artificial_light = in.light * mix(0.4, 0.55, precipitation);
    color *= sun_contrib + artificial_light;
    color = mix(color, color * vec3<f32>(0.85, 0.9, 0.95), precipitation * 0.2);

    // Water animation and tint
    var alpha = 1.0;
    if (in.block_id == 6u) {
        let wave = sin(time_uniform.time.x * 20.0 + in.world_pos.x * 0.3 + in.world_pos.z * 0.3);
        let water_tint = mix(vec3<f32>(0.2, 0.5, 0.9), vec3<f32>(0.15, 0.25, 0.45), precipitation * 0.8);
        color = mix(color, water_tint, 0.6 + wave * 0.05);
        alpha = 0.75;
    }

    let dist = distance(in.world_pos, camera.camera_pos.xyz);
    color = apply_fog(color, dist);

    return vec4<f32>(color, alpha);
}
