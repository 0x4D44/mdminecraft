// Billboard pipeline (instanced camera-facing quads)
// Feature-gated via `ui3d_billboards`.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(1) @binding(1)
var atlas_sampler: sampler;

struct VSOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) light: f32,
    @location(3) flags: u32,
}

fn rotate(local: vec2<f32>, angle: f32) -> vec2<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec2<f32>(local.x * c - local.y * s, local.x * s + local.y * c);
}

@vertex
fn vs_main(
    @location(0) quad_pos: vec2<f32>,
    @location(1) position: vec3<f32>,
    @location(2) size: vec2<f32>,
    @location(3) rot: f32,
    @location(4) uv_min: vec2<f32>,
    @location(5) uv_max: vec2<f32>,
    @location(6) color: vec4<f32>,
    @location(7) light: f32,
    @location(8) layer_flags: vec2<i32>,
) -> VSOut {
    // Build facing basis from camera -> instance vector
    let forward = normalize(camera.camera_pos.xyz - position);
    let world_up = vec3<f32>(0.0, 1.0, 0.0);
    // If camera is exactly above/below, right may degenerate; fall back to X axis in that rare case.
    var right = cross(world_up, forward);
    if (length(right) < 1e-3) {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    right = normalize(right);
    let up = normalize(cross(forward, right));

    // Apply per-instance rotation about camera-forward
    let rotated = rotate(quad_pos, rot);
    let world = position + right * rotated.x * size.x + up * rotated.y * size.y;

    var out: VSOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    out.uv = mix(uv_min, uv_max, quad_pos * 0.5 + vec2<f32>(0.5));
    out.color = color;
    out.light = light;
    // second component carries flags packed into lower 16 bits (sign-extended by vertex fetch)
    out.flags = bitcast<u32>(layer_flags.y);
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    var c = textureSample(atlas_texture, atlas_sampler, in.uv) * in.color;
    if ((in.flags & 0x1u) == 0u) {
        let l = clamp(in.light, 0.0, 1.0);
        let lit = mix(0.35, 1.0, l);
        c = vec4<f32>(c.rgb * lit, c.a);
    }
    return c;
}
