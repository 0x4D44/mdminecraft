#[cfg(not(feature = "ui3d_billboards"))]
fn main() {
    println!("Enable the `ui3d_billboards` feature to run this example.");
}

#[cfg(feature = "ui3d_billboards")]
fn main() {
    pollster::block_on(run());
}

#[cfg(feature = "ui3d_billboards")]
async fn run() {
    use glam::Mat4;
    use mdminecraft_render::CameraUniform;
    use mdminecraft_ui3d::render::{
        BillboardEmitter, BillboardFlags, BillboardInstance, BillboardRenderer,
    };

    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        })
        .await
        .expect("adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .expect("device");

    // Camera uniform + layout
    let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Billboard Demo Camera BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Billboard Demo Camera UB"),
        size: std::mem::size_of::<CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Billboard Demo Camera BG"),
        layout: &camera_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
    });

    let cam = CameraUniform {
        view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        camera_pos: [0.0, 0.0, 4.0, 1.0],
    };
    queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&cam));

    // 2×2 atlas (solid colors)
    let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Billboard Demo Atlas"),
        size: wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let pixels: [u8; 16] = [
        255, 0, 0, 255, // red
        0, 255, 0, 255, // green
        0, 0, 255, 255, // blue
        255, 255, 0, 255, // yellow
    ];

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &atlas_tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * 2),
            rows_per_image: Some(2),
        },
        wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
    );

    let atlas_view = atlas_tex.create_view(&Default::default());
    let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

    let mut renderer = BillboardRenderer::new(
        &device,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        &camera_layout,
        &atlas_view,
        &atlas_sampler,
    )
    .expect("renderer");

    // Targets
    let color_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Billboard Demo Color"),
        size: wgpu::Extent3d {
            width: 512,
            height: 512,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Billboard Demo Depth"),
        size: wgpu::Extent3d {
            width: 512,
            height: 512,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    let color_view = color_tex.create_view(&Default::default());
    let depth_view = depth_tex.create_view(&Default::default());

    let mut emitter = BillboardEmitter::default();
    emitter.submit(
        1,
        BillboardInstance {
            position: [0.0, 0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            ..Default::default()
        },
    );
    emitter.submit(
        2,
        BillboardInstance {
            position: [0.5, 0.3, 0.0],
            size: [0.6, 0.6],
            rot: 0.3,
            uv_min: [0.0, 0.0],
            uv_max: [0.5, 0.5],
            color: [0.9, 0.8, 1.0, 0.8],
            layer: 1,
            ..Default::default()
        },
    );
    emitter.submit(
        3,
        BillboardInstance {
            position: [-0.7, 0.2, 0.3],
            size: [0.4, 0.8],
            uv_min: [0.5, 0.0],
            uv_max: [1.0, 0.5],
            color: [0.4, 1.0, 0.6, 0.7],
            layer: 2,
            flags: BillboardFlags::OVERLAY_NO_DEPTH.bits(),
            ..Default::default()
        },
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Billboard Demo Encoder"),
    });

    let stats = renderer
        .render(
            &device,
            &queue,
            &mut encoder,
            &color_view,
            &depth_view,
            &camera_bg,
            &mut emitter,
        )
        .expect("render");

    queue.submit(Some(encoder.finish()));
    device.poll(wgpu::Maintain::Wait);

    println!(
        "Rendered {} billboards ({} overlay) across {} draw calls",
        stats.instances, stats.overlay_instances, stats.draw_calls
    );
}
