struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct ParticleGlobals {
    viewport_size: vec2<f32>,
    _padding: vec2<f32>,
}

@group(1) @binding(0)
var<uniform> globals: ParticleGlobals;

struct ParticleVertexInput {
    // Per-vertex quad corner in clip-space pixel units (expanded in shader).
    @location(0) corner: vec2<f32>,

    // Per-instance particle attributes.
    @location(1) position: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) lifetime: f32,
    @location(4) scale: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: ParticleVertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world = vec4<f32>(in.position, 1.0);
    let center_clip = camera.view_proj * world;

    // Convert a pixel-sized quad into clip space. Multiply by `clip.w` so the quad
    // stays approximately constant-size in screen space regardless of depth.
    let pixel_to_ndc = vec2<f32>(
        2.0 / globals.viewport_size.x,
        2.0 / globals.viewport_size.y,
    );
    let half_size = in.scale * 0.5;
    let ndc_offset = in.corner * half_size * pixel_to_ndc;

    out.clip_position = center_clip + vec4<f32>(ndc_offset * center_clip.w, 0.0, 0.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
