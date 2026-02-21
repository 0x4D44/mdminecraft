// Skybox shader for gradient sky rendering with time-of-day
// Uses full-screen triangle technique (no vertex buffer needed)

// Time uniform
struct TimeUniform {
    time: vec4<f32>,
    sun_dir: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
    sky_color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> time_uniform: TimeUniform;

struct CameraUniform {
    forward: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    // x: tan_half_fov (vertical), y: aspect, z/w: reserved
    params: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> camera_uniform: CameraUniform;

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
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
    // Map clip-space [-1,1] across the screen to UV [0,1]. The full-screen triangle uses
    // vertices outside [-1,1] (x/y = 3), so UVs outside [0,1] are expected at vertices and
    // will interpolate correctly across the covered area.
    out.uv = vec2<f32>(x, y) * 0.5 + vec2<f32>(0.5, 0.5);

    return out;
}

struct SkyColors {
    horizon: vec3<f32>,
    zenith: vec3<f32>,
}

// Get sky colors based on time of day
fn get_sky_colors(time: f32) -> SkyColors {

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

    return SkyColors(horizon, zenith);
}

// Fragment shader - creates gradient with time-of-day
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let ndc = in.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let tan_half_fov = camera_uniform.params.x;
    let aspect = camera_uniform.params.y;
    let dir = normalize(
        camera_uniform.forward.xyz
            + camera_uniform.right.xyz * (ndc.x * tan_half_fov * aspect)
            + camera_uniform.up.xyz * (ndc.y * tan_half_fov)
    );

    let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    let env = time_uniform.time.y;

    if (env > 0.5) {
        // Dimension-specific sky approximations:
        // - Nether: dark red haze
        // - End: near-black with sparse star-like specks
        if (env < 1.5) {
            // Nether
            let base = vec3<f32>(0.12, 0.02, 0.01);
            let haze = vec3<f32>(0.35, 0.07, 0.04);
            var color = mix(base, haze, smoothstep(0.0, 1.0, t) * 0.35);
            let noise_uv = dir.xz * 3.0
                + vec2<f32>(time_uniform.time.x * 0.03, time_uniform.time.x * 0.025);
            let n = cloud_noise(noise_uv);
            color += vec3<f32>(0.02, 0.01, 0.0) * (n - 0.5) * 0.6;
            return vec4<f32>(color, 1.0);
        } else {
            // End
            var color = vec3<f32>(0.01, 0.0, 0.02);
            let p = dir.xz * 2.0
                + vec2<f32>(time_uniform.time.x * 0.02, -time_uniform.time.x * 0.017);
            let cell = floor(p * 40.0);
            let n = hash(cell);
            let star = step(0.992, n);
            let twinkle = 0.4 + 0.6 * (0.5 + 0.5 * sin(time_uniform.time.x * 200.0 + n * 6.2831853));
            color += vec3<f32>(0.55, 0.3, 0.9) * star * twinkle;
            return vec4<f32>(color, 1.0);
        }
    }

    // Get colors for current time of day
    let colors = get_sky_colors(time_uniform.time.x);
    var horizon_color = colors.horizon;
    var zenith_color = colors.zenith;

    // Biome-tinted sky: blend the horizon toward fog (vanilla-ish),
    // and tint the zenith toward a biome-provided sky color during daytime.
    let day_weight = smoothstep(0.2, 0.3, time_uniform.time.x)
        * (1.0 - smoothstep(0.7, 0.8, time_uniform.time.x));
    horizon_color = mix(horizon_color, time_uniform.fog_color.rgb, 0.35);
    zenith_color = mix(zenith_color, time_uniform.sky_color.rgb, 0.35 * day_weight);

    // Smooth interpolation from horizon to zenith
    let gradient_t = smoothstep(0.0, 1.0, t);
    let sky_color = mix(horizon_color, zenith_color, gradient_t);

    // Sun disc
    let sun_dir = normalize(time_uniform.sun_dir.xyz);
    let sun_dot = max(dot(dir, sun_dir), 0.0);
    let sun_intensity = smoothstep(0.997, 1.0, sun_dot);
    let sun_color = vec3<f32>(1.0, 0.95, 0.85);

    // Moon disc opposite the sun
    let moon_dir = -sun_dir;
    let moon_dot = max(dot(dir, moon_dir), 0.0);
    let moon_intensity = smoothstep(0.997, 1.0, moon_dot);
    let moon_color = vec3<f32>(0.8, 0.85, 1.0);

    var color = sky_color
        + sun_color * sun_intensity
        + moon_color * moon_intensity * 0.4;

    // Stars (Overworld): simple procedural star field visible at night.
    let night_weight = 1.0 - day_weight;
    if (night_weight > 0.001) {
        // Fade stars toward the horizon for a more vanilla-ish look.
        let star_fade = night_weight * smoothstep(0.2, 0.7, t);
        // Project the view direction onto a pseudo-sky plane for stable star noise.
        let star_p = dir.xz / max(0.15, abs(dir.y)) * 0.5;
        let cell = floor(star_p * 120.0);
        let n = hash(cell);
        let star = step(0.996, n);
        // Twinkle based on deterministic sim-time (time.w).
        let twinkle = 0.5 + 0.5 * sin(time_uniform.time.w * 4.0 + n * 6.2831853);
        // Weather suppresses star visibility.
        let precipitation = time_uniform.fog_params.w;
        let weather_fade = 1.0 - clamp(precipitation * 1.2, 0.0, 1.0);
        color += vec3<f32>(1.0, 1.0, 1.0) * star * (0.2 + 0.8 * twinkle) * star_fade * weather_fade;
    }

    // Simple procedural clouds
    let uv = dir.xz * 0.5 + vec2<f32>(time_uniform.time.x * 0.02, time_uniform.time.x * 0.015);
    let cloud = cloud_noise(uv * 4.0);
    let cloud_mask = smoothstep(0.5, 0.7, cloud);
    color = mix(color, color + vec3<f32>(0.1, 0.1, 0.12), cloud_mask * 0.3);

    let precipitation = time_uniform.fog_params.w;
    if (precipitation > 0.001) {
        let overcast = vec3<f32>(0.55, 0.58, 0.65);
        color = mix(color, overcast, clamp(precipitation * 0.7, 0.0, 1.0));
        color -= vec3<f32>(0.0, 0.0, 0.05) * precipitation * 0.3;
    }

    let underwater = time_uniform.time.z;
    let thunder = time_uniform.fog_color.w;
    if (env < 0.5 && underwater < 0.5 && thunder > 0.001) {
        let gloom = vec3<f32>(0.18, 0.18, 0.22);
        color = mix(color, gloom, clamp(thunder, 0.0, 1.0) * 0.55);
    }
    let lightning = time_uniform.sky_color.w;
    if (env < 0.5 && underwater < 0.5 && lightning > 0.001) {
        let flash = clamp(lightning, 0.0, 1.0);
        color = mix(color, vec3<f32>(1.0, 1.0, 1.0), flash * 0.65);
    }
    if (underwater > 0.5) {
        // Vanilla applies strong underwater color grading. Approximate with a tint toward the
        // current fog color (which is set to biome water-fog color while underwater).
        color = mix(color, time_uniform.fog_color.rgb, 0.85);
        color *= vec3<f32>(0.85, 0.9, 0.95);
    }

    return vec4<f32>(color, 1.0);
}

fn hash(p: vec2<f32>) -> f32 {
    let h = sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453;
    return fract(h);
}

fn cloud_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}
