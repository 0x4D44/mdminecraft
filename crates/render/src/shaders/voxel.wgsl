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
    sky_color: vec4<f32>,
}

@group(0) @binding(1)
var<uniform> time_uniform: TimeUniform;

struct ChunkUniform {
    chunk_offset: vec3<f32>,
    _padding0: f32,
    grass_tint: vec3<f32>,
    _padding1: f32,
    foliage_tint: vec3<f32>,
    _padding2: f32,
    water_tint: vec3<f32>,
    _padding3: f32,
}

@group(1) @binding(0)
var<uniform> chunk: ChunkUniform;

@group(2) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(2) @binding(1)
var atlas_sampler: sampler;

struct AtlasParams {
    atlas_width: f32,
    atlas_height: f32,
    tile_size: f32,
    padding: f32,
}

@group(2) @binding(2)
var<uniform> atlas_params: AtlasParams;

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
    @location(5) extra: u32,
    @location(6) ao: f32,
}

struct ShadeInput {
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    uv: vec2<f32>,
    uv_dx: vec2<f32>,
    uv_dy: vec2<f32>,
    light: f32,
    extra: u32,
    ao: f32,
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
    let light_ao = (in.packed_data >> 16u) & 0xFFu;
    let light_value = light_ao & 0x0Fu;
    let ao_value = (light_ao >> 4u) & 0x0Fu;
    out.light = f32(light_value) / 15.0;
    out.ao = f32(min(ao_value, 3u)) / 3.0;
    out.extra = (in.packed_data >> 24u) & 0xFFu;
    return out;
}

fn apply_fog(color: vec3<f32>, dist: f32) -> vec3<f32> {
    let fog_start = time_uniform.fog_params.x;
    let fog_end = time_uniform.fog_params.y;
    let fog_factor = clamp((dist - fog_start) / max(fog_end - fog_start, 0.0001), 0.0, 1.0);
    var fog_color = time_uniform.fog_color.rgb;
    let env = time_uniform.time.y;
    if (env < 0.5) {
        // Overworld: tint the distance haze slightly toward the biome sky tint.
        fog_color = mix(fog_color, time_uniform.sky_color.rgb, 0.35);
    }
    return mix(color, fog_color, fog_factor);
}

fn decode_flow_dir(extra: u32) -> vec2<f32> {
    let diag = 0.70710678;
    switch(extra) {
        case 1u: { return vec2<f32>(0.0, -1.0); } // north
        case 2u: { return vec2<f32>(0.0, 1.0); }  // south
        case 3u: { return vec2<f32>(-1.0, 0.0); } // west
        case 4u: { return vec2<f32>(1.0, 0.0); }  // east
        case 5u: { return vec2<f32>(-diag, -diag); } // northwest
        case 6u: { return vec2<f32>(diag, -diag); }  // northeast
        case 7u: { return vec2<f32>(-diag, diag); }  // southwest
        case 8u: { return vec2<f32>(diag, diag); }   // southeast
        default: { return vec2<f32>(0.0, 0.0); }
    }
}

fn hash21(p: vec2<f32>) -> f32 {
    // Deterministic hash for cheap procedural detail (not cryptographic).
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn tile_local_uv(uv: vec2<f32>) -> vec2<f32> {
    let stride = atlas_params.tile_size + atlas_params.padding * 2.0;
    let px = uv.x * atlas_params.atlas_width;
    let py = uv.y * atlas_params.atlas_height;
    let cell_x = floor(px / stride);
    let cell_y = floor(py / stride);
    let local_x = (px - cell_x * stride - atlas_params.padding) / atlas_params.tile_size;
    let local_y = (py - cell_y * stride - atlas_params.padding) / atlas_params.tile_size;
    return clamp(vec2<f32>(local_x, local_y), vec2<f32>(0.0), vec2<f32>(1.0));
}

fn tile_atlas_uv(uv: vec2<f32>, local: vec2<f32>) -> vec2<f32> {
    let stride = atlas_params.tile_size + atlas_params.padding * 2.0;
    let px = uv.x * atlas_params.atlas_width;
    let py = uv.y * atlas_params.atlas_height;
    let cell_x = floor(px / stride);
    let cell_y = floor(py / stride);
    let local_clamped = clamp(local, vec2<f32>(0.0), vec2<f32>(1.0));
    let out_px = cell_x * stride + atlas_params.padding + local_clamped.x * atlas_params.tile_size;
    let out_py = cell_y * stride + atlas_params.padding + local_clamped.y * atlas_params.tile_size;
    return vec2<f32>(out_px / atlas_params.atlas_width, out_py / atlas_params.atlas_height);
}

fn shade_voxel(in: ShadeInput) -> vec4<f32> {
    let sample = textureSample(atlas_texture, atlas_sampler, in.uv);
    var color = sample.rgb;
    var alpha = sample.a;
    let alpha_pass = (in.extra & 0x80u) != 0u;
    let kind = (in.extra >> 4u) & 0x7u;

    if (!alpha_pass) {
        // Opaque pass: kind encodes biome tint selection.
        if (kind == 1u) {
            color *= chunk.grass_tint;
        } else if (kind == 2u) {
            color *= chunk.foliage_tint;
        } else if (kind == 3u) {
            let local = tile_local_uv(in.uv);
            let overlay = 1.0 - step(0.25, local.y);
            color = mix(color, color * chunk.grass_tint, overlay);
        } else if (kind == 4u) {
            // Birch leaves use a fixed tint in vanilla.
            color *= vec3<f32>(0.2158605, 0.38642943, 0.09084171);
        } else if (kind == 5u) {
            // Spruce leaves use a fixed tint in vanilla.
            color *= vec3<f32>(0.11953843, 0.31854677, 0.11953843);
        }
    }
    let env = time_uniform.time.y;
    let is_overworld = env < 0.5;
    let is_nether = env >= 0.5 && env < 1.5;

    let sun_dir = normalize(time_uniform.sun_dir.xyz);
    let precipitation = time_uniform.fog_params.w;
    let day_weight = smoothstep(0.2, 0.3, time_uniform.time.x)
        * (1.0 - smoothstep(0.7, 0.8, time_uniform.time.x));
    let diffuse = select(
        0.0,
        max(dot(in.normal, sun_dir), 0.0) * day_weight,
        is_overworld,
    );
    let ambient = select(0.15, mix(0.08, 0.3, day_weight), is_overworld);
    let sun_contrib = select(
        select(0.12, 0.14, !is_nether),
        (ambient + diffuse * 0.5) * mix(1.0, 0.65, precipitation),
        is_overworld,
    );
    let artificial_light = in.light * mix(0.4, 0.55, precipitation);
    let night_vision = clamp(time_uniform.fog_params.z, 0.0, 1.0);
    let brightness = sun_contrib + artificial_light;
    let boosted = max(brightness, 0.6);
    let final_brightness = mix(brightness, boosted, night_vision);
    color *= final_brightness;
    color = mix(color, color * vec3<f32>(0.85, 0.9, 0.95), precipitation * 0.2);
    let ao_factor = mix(0.6, 1.0, clamp(in.ao, 0.0, 1.0));
    color *= ao_factor;

    // Alpha pass shading: kind encodes the material class for common translucent blocks.
    if (alpha_pass && kind == 1u) {
        // Water.
        let flow_dir = decode_flow_dir(in.extra & 0x0Fu);
        let flow_strength = step(0.0001, length(flow_dir));
        let anim_time = time_uniform.time.w;
        let base_coord = in.world_pos.x * 0.3 + in.world_pos.z * 0.3;
        let flow_coord = dot(in.world_pos.xz, flow_dir) * 0.45;
        let coord = mix(base_coord, flow_coord, flow_strength);
        let wave_phase = anim_time * 6.2831853 * 0.08;
        let wave = sin(wave_phase + coord);

        // Vanilla-ish water motion: blend two scrolling samples to approximate animated textures.
        let local = tile_local_uv(in.uv);
        let base_dir = normalize(flow_dir + vec2<f32>(0.6, 0.8));
        let ortho_dir = vec2<f32>(-base_dir.y, base_dir.x);
        let uv1 = tile_atlas_uv(in.uv, fract(local + base_dir * anim_time * 0.035));
        let uv2 = tile_atlas_uv(in.uv, fract(local + ortho_dir * anim_time * 0.028));
        let s1 = textureSampleGrad(atlas_texture, atlas_sampler, uv1, in.uv_dx, in.uv_dy).rgb;
        let s2 = textureSampleGrad(atlas_texture, atlas_sampler, uv2, in.uv_dx, in.uv_dy).rgb;
        color = mix(s1, s2, 0.5);

        let water_tint = mix(
            chunk.water_tint,
            chunk.water_tint * vec3<f32>(0.5, 0.55, 0.65),
            precipitation * 0.8,
        );
        color = mix(color, water_tint, 0.6 + wave * 0.05);
        alpha = 0.75;
    } else if (alpha_pass && kind == 2u) {
        // Lava.
        let flow_dir = decode_flow_dir(in.extra & 0x0Fu);
        let flow_strength = step(0.0001, length(flow_dir));
        let anim_time = time_uniform.time.w;
        let base_coord = in.world_pos.x * 0.2 + in.world_pos.z * 0.2;
        let flow_coord = dot(in.world_pos.xz, flow_dir) * 0.35;
        let coord = mix(base_coord, flow_coord, flow_strength);
        let wave_phase = anim_time * 6.2831853 * 0.05;
        let wave = sin(wave_phase + coord);

        // Lava motion: slower, heavier scrolling.
        let local = tile_local_uv(in.uv);
        let base_dir = normalize(flow_dir + vec2<f32>(0.35, 0.15));
        let ortho_dir = vec2<f32>(-base_dir.y, base_dir.x);
        let uv1 = tile_atlas_uv(in.uv, fract(local + base_dir * anim_time * 0.02));
        let uv2 = tile_atlas_uv(in.uv, fract(local + ortho_dir * anim_time * 0.017));
        let s1 = textureSampleGrad(atlas_texture, atlas_sampler, uv1, in.uv_dx, in.uv_dy).rgb;
        let s2 = textureSampleGrad(atlas_texture, atlas_sampler, uv2, in.uv_dx, in.uv_dy).rgb;
        color = mix(s1, s2, 0.55);

        let lava_tint = vec3<f32>(1.0, 0.5, 0.1);
        color = mix(color, lava_tint, 0.55 + wave * 0.05);
        alpha = 0.9;
    } else if (alpha_pass && kind == 3u) {
        // Glass: treat texture alpha as an edge mask, but keep a base translucency so the
        // full block reads as glass instead of a cutout frame.
        let edge = alpha;
        var base = vec3<f32>(0.55, 0.75, 0.9) * final_brightness;
        base = mix(base, base * vec3<f32>(0.85, 0.9, 0.95), precipitation * 0.2);
        color = mix(base, color, edge);
        alpha = mix(0.25, 0.65, edge);
    } else if (alpha_pass && kind == 4u) {
        // Nether portal (visual-only approximation).
        let t = time_uniform.time.w * 6.2831853 * 0.35;
        let swirl = sin(t + in.world_pos.y * 2.2 + in.world_pos.x * 1.1) * cos(t + in.world_pos.z * 1.3);
        let pulse = 0.5 + 0.5 * sin(t * 0.15);
        color = mix(vec3<f32>(0.12, 0.02, 0.18), vec3<f32>(0.65, 0.25, 0.85), 0.55 + swirl * 0.12);
        color *= 0.9 + pulse * 0.2;
        alpha = 0.75;
    } else if (alpha_pass && kind == 5u) {
        // End portal (starfield-ish surface).
        let t = time_uniform.time.w * 6.2831853 * 0.25;
        let p = in.world_pos.xz * 0.35 + vec2<f32>(t * 0.02, -t * 0.017);
        let cell = floor(p * 8.0);
        let n = hash21(cell);
        let star = step(0.985, n);
        let twinkle = 0.4 + 0.6 * (0.5 + 0.5 * sin(t + n * 6.2831853));
        let base = vec3<f32>(0.02, 0.0, 0.04);
        color = base + vec3<f32>(0.55, 0.8, 1.0) * star * twinkle;
        alpha = 0.92;
    } else if (alpha_pass && kind == 6u) {
        // Fire (visual-only approximation).
        let t = time_uniform.time.w * 6.2831853 * 0.7;
        let height = fract(in.world_pos.y);
        let p = in.world_pos.xz * 1.1 + vec2<f32>(t * 0.03, -t * 0.02);
        let cell = floor(p * 6.0);
        let n = hash21(cell);
        let flicker = 0.6 + 0.4 * sin(t + n * 6.2831853);
        let rise = clamp(1.0 - height, 0.0, 1.0);
        let intensity = clamp(rise * flicker, 0.0, 1.0);

        color = mix(vec3<f32>(1.0, 0.35, 0.05), vec3<f32>(1.0, 0.9, 0.25), intensity);
        alpha = mix(0.25, 0.85, intensity);
    }

    let dist = distance(in.world_pos, camera.camera_pos.xyz);
    color = apply_fog(color, dist);

    let underwater = time_uniform.time.z;
    if (underwater > 0.5) {
        // Vanilla underwater has a strong full-screen tint. Apply a small near-field tint here
        // (fog already handles far-field absorption).
        color = mix(color, time_uniform.fog_color.rgb, 0.18);
    }

    let thunder = time_uniform.fog_color.w;
    if (is_overworld && underwater < 0.5 && thunder > 0.001) {
        let gloom = clamp(thunder, 0.0, 1.0);
        color *= 1.0 - gloom * 0.18;
    }

    let lightning = time_uniform.sky_color.w;
    if (is_overworld && underwater < 0.5 && lightning > 0.001) {
        let flash = clamp(lightning, 0.0, 1.0);
        color = mix(color, vec3<f32>(1.0, 1.0, 1.0), flash * 0.35);
    }

    return vec4<f32>(color, alpha);
}

@fragment
fn fs_main_opaque(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv_dx = dpdx(in.uv);
    let uv_dy = dpdy(in.uv);
    if ((in.extra & 0x80u) != 0u) {
        discard;
    }
    let shaded = shade_voxel(ShadeInput(in.world_pos, in.normal, in.uv, uv_dx, uv_dy, in.light, in.extra, in.ao));
    if (shaded.a < 0.5) {
        discard;
    }
    return vec4<f32>(shaded.rgb, 1.0);
}

@fragment
fn fs_main_fluid(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv_dx = dpdx(in.uv);
    let uv_dy = dpdy(in.uv);
    if ((in.extra & 0x80u) == 0u) {
        discard;
    }
    return shade_voxel(ShadeInput(in.world_pos, in.normal, in.uv, uv_dx, uv_dy, in.light, in.extra, in.ao));
}
