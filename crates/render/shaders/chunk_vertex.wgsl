// Chunk vertex shader
// Transforms vertices from chunk-local space to clip space

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) block_id: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) block_id: u32,
};

struct ViewProjection {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> view_projection: ViewProjection;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform position to clip space
    out.position = view_projection.view_proj * vec4<f32>(in.position, 1.0);

    // Pass through world position for fragment shader
    out.world_position = in.position;

    // Pass through normal and block_id
    out.normal = in.normal;
    out.block_id = in.block_id;

    return out;
}
