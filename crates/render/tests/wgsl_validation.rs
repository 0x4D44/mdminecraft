use wgpu::naga;

fn validate_wgsl(source: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|err| err.emit_to_string(source))?;

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    validator
        .validate(&module)
        .map(|_| ())
        .map_err(|err| err.emit_to_string(source))
}

#[test]
fn voxel_shader_is_valid_wgsl() {
    validate_wgsl(include_str!("../src/shaders/voxel.wgsl")).unwrap();
}

#[test]
fn skybox_shader_is_valid_wgsl() {
    validate_wgsl(include_str!("../src/shaders/skybox.wgsl")).unwrap();
}

#[test]
fn particles_shader_is_valid_wgsl() {
    validate_wgsl(include_str!("../src/shaders/particles.wgsl")).unwrap();
}
