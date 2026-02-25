// Chunk fragment shader
// Samples textures from atlas and applies basic lighting

struct FragmentInput {
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) block_id: u32,
};

@group(1) @binding(0)
var t_atlas: texture_2d<f32>;

@group(1) @binding(1)
var s_atlas: sampler;

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    // Calculate UV coordinates based on position and normal
    // This is a simple planar mapping - faces are mapped to their dominant axis
    var uv: vec2<f32>;
    let abs_normal = abs(in.normal);

    if abs_normal.x > abs_normal.y && abs_normal.x > abs_normal.z {
        // X-dominant face
        uv = vec2<f32>(in.world_position.z, in.world_position.y);
    } else if abs_normal.y > abs_normal.z {
        // Y-dominant face
        uv = vec2<f32>(in.world_position.x, in.world_position.z);
    } else {
        // Z-dominant face
        uv = vec2<f32>(in.world_position.x, in.world_position.y);
    }

    // Fractional part for tiling
    uv = fract(uv);

    // For now, sample the texture at the UV coordinates
    // In a full implementation, we would use block_id to look up the correct texture region
    // For testing, we'll just use the UVs directly to sample the atlas
    let texture_color = textureSample(t_atlas, s_atlas, uv);

    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let diffuse = max(dot(normalize(in.normal), light_dir), 0.0);
    let ambient = 0.3;
    let light = ambient + diffuse * (1.0 - ambient);

    return vec4<f32>(texture_color.rgb * light, texture_color.a);
}
