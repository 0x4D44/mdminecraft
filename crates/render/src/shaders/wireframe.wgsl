// Wireframe shader for block selection highlight

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view_pos: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct HighlightUniform {
    position: vec3<f32>,
    padding: f32,
    color: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> highlight: HighlightUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;

    // Transform vertex to world position (centered on highlighted block)
    let world_pos = position + highlight.position;

    // Transform to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = highlight.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
