use wgpu::util::DeviceExt;
use mdminecraft_camera::Camera;
use glam::Mat4;

/// Camera uniform data sent to GPU.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CameraUniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _padding1: f32,
    sky_horizon: [f32; 3],  // Sky color at horizon
    _padding2: f32,
    sky_zenith: [f32; 3],   // Sky color at zenith
    _padding3: f32,
}

unsafe impl bytemuck::Pod for CameraUniforms {}
unsafe impl bytemuck::Zeroable for CameraUniforms {}

/// Chunk uniform data sent to GPU.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ChunkUniforms {
    /// Offset in world coordinates for this chunk.
    pub offset: [f32; 3],
    pub _padding: f32,
}

unsafe impl bytemuck::Pod for ChunkUniforms {}
unsafe impl bytemuck::Zeroable for ChunkUniforms {}

/// Render pipeline for chunk rendering.
pub struct ChunkPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub camera_bind_group: wgpu::BindGroup,
    pub chunk_bind_group_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
}

impl ChunkPipeline {
    /// Create a new chunk rendering pipeline.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Chunk Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk.wgsl").into()),
        });

        // Create camera uniform buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[CameraUniforms {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                camera_pos: [0.0; 3],
                _padding1: 0.0,
                sky_horizon: [0.70, 0.85, 0.95],  // Default: Forest biome
                _padding2: 0.0,
                sky_zenith: [0.30, 0.50, 0.85],   // Default: Forest biome
                _padding3: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layout
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
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

        // Create bind group
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Create chunk bind group layout
        let chunk_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Chunk Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Chunk Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &chunk_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Chunk Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::MeshVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // Position
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // Normal
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // Block ID
                        wgpu::VertexAttribute {
                            offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        // Light
                        wgpu::VertexAttribute {
                            offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<u32>())
                                as wgpu::BufferAddress,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            render_pipeline,
            camera_bind_group,
            chunk_bind_group_layout,
            camera_buffer,
        }
    }

    /// Update the camera uniform buffer.
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera, sky_horizon: [f32; 3], sky_zenith: [f32; 3]) {
        let uniforms = CameraUniforms {
            view_proj: camera.view_projection_matrix().to_cols_array_2d(),
            camera_pos: camera.position.to_array(),
            _padding1: 0.0,
            sky_horizon,
            _padding2: 0.0,
            sky_zenith,
            _padding3: 0.0,
        };

        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
}
