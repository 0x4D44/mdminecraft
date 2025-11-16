// Skybox shader for gradient sky rendering
// Uses full-screen triangle technique (no vertex buffer needed)

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_dir: vec2<f32>,
}

// Vertex shader - generates full-screen triangle
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate full-screen triangle using vertex index
    // This technique covers the entire screen with just 3 vertices
    let x = f32((vertex_index & 1u) << 2u) - 1.0;  // -1 or 3
    let y = f32((vertex_index & 2u) << 1u) - 1.0;  // -1 or 3

    out.clip_position = vec4<f32>(x, y, 1.0, 1.0);  // Far plane (z=1)
    out.view_dir = vec2<f32>(x, y);

    return out;
}

// Fragment shader - creates gradient
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Normalize Y coordinate from [-1, 1] to [0, 1]
    // -1 (bottom) -> 0, +1 (top) -> 1
    let t = (in.view_dir.y + 1.0) * 0.5;

    // Sky gradient colors
    let horizon_color = vec3<f32>(0.9, 0.9, 0.7);  // Light yellow/white
    let zenith_color = vec3<f32>(0.4, 0.6, 1.0);   // Sky blue

    // Smooth interpolation from horizon to zenith
    // Use smoothstep for more natural gradient
    let gradient_t = smoothstep(0.0, 1.0, t);
    let sky_color = mix(horizon_color, zenith_color, gradient_t);

    return vec4<f32>(sky_color, 1.0);
}
