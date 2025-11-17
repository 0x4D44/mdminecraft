// Skybox shader for gradient sky rendering with time-of-day
// Uses full-screen triangle technique (no vertex buffer needed)

// Time uniform
struct TimeUniform {
    time: f32,
    sun_dir: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> time_uniform: TimeUniform;

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

// Get sky colors based on time of day
fn get_sky_colors(time: f32) -> vec4<vec3<f32>> {
    // Returns: (horizon_color, zenith_color)

    // Night (0.0-0.2, 0.8-1.0)
    let night_horizon = vec3<f32>(0.1, 0.1, 0.2);
    let night_zenith = vec3<f32>(0.0, 0.0, 0.1);

    // Dawn (0.2-0.3)
    let dawn_horizon = vec3<f32>(1.0, 0.6, 0.4);  // Orange
    let dawn_zenith = vec3<f32>(0.4, 0.5, 0.8);   // Light blue

    // Day (0.3-0.7)
    let day_horizon = vec3<f32>(0.9, 0.9, 0.7);   // Light yellow
    let day_zenith = vec3<f32>(0.4, 0.6, 1.0);    // Sky blue

    // Dusk (0.7-0.8)
    let dusk_horizon = vec3<f32>(1.0, 0.5, 0.3);  // Deep orange
    let dusk_zenith = vec3<f32>(0.3, 0.4, 0.7);   // Purple-blue

    var horizon: vec3<f32>;
    var zenith: vec3<f32>;

    if (time < 0.2) {
        // Night to dawn transition
        let t = smoothstep(0.15, 0.2, time);
        horizon = mix(night_horizon, dawn_horizon, t);
        zenith = mix(night_zenith, dawn_zenith, t);
    } else if (time < 0.3) {
        // Dawn to day transition
        let t = smoothstep(0.2, 0.3, time);
        horizon = mix(dawn_horizon, day_horizon, t);
        zenith = mix(dawn_zenith, day_zenith, t);
    } else if (time < 0.7) {
        // Day (stable)
        horizon = day_horizon;
        zenith = day_zenith;
    } else if (time < 0.8) {
        // Day to dusk transition
        let t = smoothstep(0.7, 0.8, time);
        horizon = mix(day_horizon, dusk_horizon, t);
        zenith = mix(day_zenith, dusk_zenith, t);
    } else {
        // Dusk to night transition
        let t = smoothstep(0.8, 0.85, time);
        horizon = mix(dusk_horizon, night_horizon, t);
        zenith = mix(dusk_zenith, night_zenith, t);
    }

    return vec4<vec3<f32>>(horizon, zenith, vec3<f32>(0.0), vec3<f32>(0.0));
}

// Fragment shader - creates gradient with time-of-day
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Normalize Y coordinate from [-1, 1] to [0, 1]
    // -1 (bottom) -> 0, +1 (top) -> 1
    let t = (in.view_dir.y + 1.0) * 0.5;

    // Get colors for current time of day
    let colors = get_sky_colors(time_uniform.time);
    let horizon_color = colors.x;
    let zenith_color = colors.y;

    // Smooth interpolation from horizon to zenith
    let gradient_t = smoothstep(0.0, 1.0, t);
    let sky_color = mix(horizon_color, zenith_color, gradient_t);

    return vec4<f32>(sky_color, 1.0);
}
