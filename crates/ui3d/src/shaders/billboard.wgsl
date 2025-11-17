// Billboard Rendering Shader for 3D UI
//
// This shader renders camera-facing quads with optional textures.
// Supports multiple billboard orientations and depth modes.

// Camera uniforms
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    camera_position: vec3<f32>,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// Texture (optional - can render solid colors)
@group(1) @binding(0)
var billboard_texture: texture_2d<f32>;
@group(1) @binding(1)
var billboard_sampler: sampler;

// Vertex input
struct VertexInput {
    @location(0) position: vec3<f32>,     // World position of quad corner
    @location(1) uv: vec2<f32>,           // Texture coordinates
    @location(2) color: vec4<f32>,        // Tint color
    @location(3) center: vec3<f32>,       // Billboard center
    @location(4) orientation_mode: u32,   // 0=Full, 1=YAxis, 2=Fixed
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

    var world_position: vec3<f32>;

    if (vertex.orientation_mode == 0u) {
        // Full billboard - face camera completely
        world_position = billboard_full(vertex.position, vertex.center);
    } else if (vertex.orientation_mode == 1u) {
        // Y-axis billboard - rotate around Y only
        world_position = billboard_y_axis(vertex.position, vertex.center);
    } else {
        // Fixed orientation - no billboarding
        world_position = vertex.position;
    }

    out.clip_position = camera.view_proj * vec4<f32>(world_position, 1.0);
    out.uv = vertex.uv;
    out.color = vertex.color;

    return out;
}

fn billboard_full(position: vec3<f32>, center: vec3<f32>) -> vec3<f32> {
    let to_camera = normalize(camera.camera_position - center);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(up, to_camera));
    let billboard_up = cross(to_camera, right);

    let offset = position - center;
    let world_offset = offset.x * right + offset.y * billboard_up;
    return center + world_offset;
}

fn billboard_y_axis(position: vec3<f32>, center: vec3<f32>) -> vec3<f32> {
    var to_camera = camera.camera_position - center;
    to_camera.y = 0.0; // Project to XZ plane
    to_camera = normalize(to_camera);

    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_camera));
    let up = vec3<f32>(0.0, 1.0, 0.0);

    let offset = position - center;
    let world_offset = offset.x * right + offset.y * up;
    return center + world_offset;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture and apply color tint
    let tex_color = textureSample(billboard_texture, billboard_sampler, in.uv);
    return tex_color * in.color;
}

// Variant for solid color (no texture)
@fragment
fn fs_main_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
