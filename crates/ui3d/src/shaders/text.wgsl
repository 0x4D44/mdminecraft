// Text Rendering Shader for 3D UI
//
// This shader renders text using a font atlas texture with signed distance fields (SDF).
// It supports billboarding, text color, and smooth edges at any scale.

// Camera uniforms (shared with other pipelines)
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    camera_position: vec3<f32>,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// Font atlas texture
@group(1) @binding(0)
var font_texture: texture_2d<f32>;
@group(1) @binding(1)
var font_sampler: sampler;

// Vertex input
struct VertexInput {
    @location(0) position: vec3<f32>,     // World position of quad corner
    @location(1) uv: vec2<f32>,           // Texture coordinates in atlas
    @location(2) color: vec4<f32>,        // Text color with alpha
    @location(3) billboard_center: vec3<f32>, // Center of the billboard
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Calculate billboard orientation
    // Make the quad face the camera
    let to_camera = normalize(camera.camera_position - vertex.billboard_center);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(up, to_camera));
    let billboard_up = cross(to_camera, right);

    // Offset from billboard center
    let offset = vertex.position - vertex.billboard_center;

    // Transform offset to face camera
    let world_offset = offset.x * right + offset.y * billboard_up;
    let world_position = vertex.billboard_center + world_offset;

    // Transform to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_position, 1.0);
    out.uv = vertex.uv;
    out.color = vertex.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the font atlas
    let distance = textureSample(font_texture, font_sampler, in.uv).r;

    // SDF-based alpha
    // For now, we're using regular bitmap, so just use the sampled value as alpha
    // TODO: When we implement true SDF, use:
    // let alpha = smoothstep(0.45, 0.55, distance);
    let alpha = distance;

    // Output color with alpha
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

// Variant without billboarding for fixed-orientation text
@vertex
fn vs_main_fixed(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Use position directly without billboarding
    out.clip_position = camera.view_proj * vec4<f32>(vertex.position, 1.0);
    out.uv = vertex.uv;
    out.color = vertex.color;

    return out;
}
