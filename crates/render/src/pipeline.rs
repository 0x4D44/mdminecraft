//! GPU rendering pipeline using wgpu.

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::camera::{Camera, CameraUniform};
use crate::mesh::MeshVertex;
use crate::texture_atlas::{warn_missing_atlas, RuntimeAtlas};
use mdminecraft_assets::TextureAtlasMetadata;

/// GPU rendering context.
pub struct RenderContext {
    /// Window surface the renderer presents into.
    pub surface: wgpu::Surface<'static>,
    /// Logical GPU device used for issuing commands.
    pub device: wgpu::Device,
    /// Command queue for submitting work to the GPU.
    pub queue: wgpu::Queue,
    /// Surface configuration describing swapchain parameters.
    pub config: wgpu::SurfaceConfiguration,
    /// Current backbuffer dimensions in pixels (width, height).
    pub size: (u32, u32),
}

impl RenderContext {
    /// Create a new render context from a window.
    pub async fn new(window: std::sync::Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find suitable GPU adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("mdminecraft device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // VSync
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        tracing::info!(
            width = size.width,
            height = size.height,
            format = ?surface_format,
            "GPU rendering context initialized"
        );

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: (size.width, size.height),
        })
    }

    /// Resize the surface.
    pub fn resize(&mut self, new_size: (u32, u32)) {
        if new_size.0 > 0 && new_size.1 > 0 {
            self.size = new_size;
            self.config.width = new_size.0;
            self.config.height = new_size.1;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Get current aspect ratio.
    pub fn aspect_ratio(&self) -> f32 {
        self.size.0 as f32 / self.size.1 as f32
    }
}

/// Chunk uniform data sent to GPU.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ChunkUniform {
    /// Chunk offset in world coordinates
    pub chunk_offset: [f32; 3],
    /// Padding for alignment
    pub _padding: f32,
}

impl ChunkUniform {
    /// Create chunk uniform from chunk position.
    pub fn from_chunk_pos(chunk_pos: mdminecraft_world::ChunkPos) -> Self {
        // Convert chunk coordinates to world coordinates
        // Each chunk is 16×256×16 voxels
        let x = (chunk_pos.x * 16) as f32;
        let z = (chunk_pos.z * 16) as f32;

        Self {
            chunk_offset: [x, 0.0, z],
            _padding: 0.0,
        }
    }
}

/// Create a procedural debug texture atlas (16×16 grid).
///
/// Returns RGBA pixels and the atlas dimension in pixels.
fn create_debug_texture_atlas() -> (Vec<u8>, u32) {
    const ATLAS_SIZE: u32 = 256; // 16×16 grid of 16×16 textures
    const TILE_SIZE: u32 = 16;
    const GRID_SIZE: u32 = 16;

    let mut data = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];

    for y in 0..ATLAS_SIZE {
        for x in 0..ATLAS_SIZE {
            let tile_x = x / TILE_SIZE;
            let tile_y = y / TILE_SIZE;
            let tile_id = tile_y * GRID_SIZE + tile_x;

            // Generate color based on tile_id
            let r = ((tile_id * 37) % 256) as u8;
            let g = ((tile_id * 73) % 256) as u8;
            let b = ((tile_id * 109) % 256) as u8;

            let idx = ((y * ATLAS_SIZE + x) * 4) as usize;
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = 255; // Alpha
        }
    }

    (data, ATLAS_SIZE)
}

fn upload_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    pixels: &[u8],
    label: &str,
) -> wgpu::Texture {
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let bytes_per_pixel = 4usize;
    let row_bytes = width as usize * bytes_per_pixel;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded_row_bytes = row_bytes.div_ceil(alignment) * alignment;

    if padded_row_bytes == row_bytes {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes as u32),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    } else {
        let mut padded = vec![0u8; padded_row_bytes * height as usize];
        for row in 0..height as usize {
            let src_start = row * row_bytes;
            let dst_start = row * padded_row_bytes;
            padded[dst_start..dst_start + row_bytes]
                .copy_from_slice(&pixels[src_start..src_start + row_bytes]);
        }
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes as u32),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    texture
}

/// Voxel rendering pipeline.
pub struct VoxelPipeline {
    render_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    time_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    chunk_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group: wgpu::BindGroup,
    atlas_view: wgpu::TextureView,
    atlas_sampler: wgpu::Sampler,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    atlas_metadata: Option<TextureAtlasMetadata>,
}

impl VoxelPipeline {
    /// Create a new voxel rendering pipeline.
    pub fn new(ctx: &RenderContext) -> Result<Self> {
        let device = &ctx.device;

        // Create camera buffer
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create time buffer
        let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Time Buffer (Voxel)"),
            size: std::mem::size_of::<crate::time::TimeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create camera bind group layout (includes time)
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: time_buffer.as_entire_binding(),
                },
            ],
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

        // Load runtime texture atlas if available, otherwise fall back to debug atlas
        let (atlas_texture, atlas_metadata) = match RuntimeAtlas::load_from_disk() {
            Ok(runtime_atlas) => {
                let texture = upload_rgba_texture(
                    device,
                    &ctx.queue,
                    runtime_atlas.metadata.atlas_width,
                    runtime_atlas.metadata.atlas_height,
                    &runtime_atlas.pixels,
                    "Texture Atlas",
                );
                (texture, Some(runtime_atlas.metadata))
            }
            Err(err) => {
                warn_missing_atlas(&err);
                let (atlas_data, size) = create_debug_texture_atlas();
                let texture = upload_rgba_texture(
                    device,
                    &ctx.queue,
                    size,
                    size,
                    &atlas_data,
                    "Debug Texture Atlas",
                );
                (texture, None)
            }
        };

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest, // Pixel-perfect rendering
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create texture bind group layout
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Voxel Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/voxel.wgsl").into()),
        });

        // Create render pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Voxel Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &chunk_bind_group_layout,
                &texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // Create depth texture
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: ctx.config.width,
                height: ctx.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Voxel Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // position
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // normal
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // uv
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // block_id (u16) and light (u8) packed as u32
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
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

        Ok(Self {
            render_pipeline,
            camera_buffer,
            time_buffer,
            camera_bind_group,
            camera_bind_group_layout,
            chunk_bind_group_layout,
            texture_bind_group,
            atlas_view,
            atlas_sampler,
            depth_texture,
            depth_view,
            atlas_metadata,
        })
    }

    /// Get the texture bind group for rendering.
    pub fn texture_bind_group(&self) -> &wgpu::BindGroup {
        &self.texture_bind_group
    }

    /// Get the atlas texture view used by voxel rendering.
    pub fn atlas_view(&self) -> &wgpu::TextureView {
        &self.atlas_view
    }

    /// Get the atlas sampler used by voxel rendering.
    pub fn atlas_sampler(&self) -> &wgpu::Sampler {
        &self.atlas_sampler
    }

    /// Access atlas metadata if a runtime atlas was loaded.
    pub fn atlas_metadata(&self) -> Option<&TextureAtlasMetadata> {
        self.atlas_metadata.as_ref()
    }

    /// Update camera uniform buffer.
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera) {
        let uniform = CameraUniform::from_camera(camera);
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Update atmospheric time/weather uniform buffer.
    pub fn update_time(
        &self,
        queue: &wgpu::Queue,
        time: &crate::time::TimeOfDay,
        weather_intensity: f32,
        night_vision: f32,
    ) {
        let uniform =
            crate::time::TimeUniform::from_time_of_day(time, weather_intensity, night_vision);
        queue.write_buffer(&self.time_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Create a chunk bind group for a specific chunk position.
    pub fn create_chunk_bind_group(
        &self,
        device: &wgpu::Device,
        chunk_pos: mdminecraft_world::ChunkPos,
    ) -> wgpu::BindGroup {
        let chunk_uniform = ChunkUniform::from_chunk_pos(chunk_pos);
        let chunk_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Uniform Buffer"),
            contents: bytemuck::cast_slice(&[chunk_uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chunk Bind Group"),
            layout: &self.chunk_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: chunk_buffer.as_entire_binding(),
            }],
        })
    }

    /// Resize depth texture.
    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        self.depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        self.depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    /// Begin a render pass.
    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        view: &'a wgpu::TextureView,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Voxel Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load existing content (skybox already rendered)
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        })
    }

    /// Get the render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.render_pipeline
    }

    /// Get the camera bind group.
    pub fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    /// Get the camera bind group layout.
    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    /// Get the depth texture view.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }
}

/// Skybox rendering pipeline.
pub struct SkyboxPipeline {
    render_pipeline: wgpu::RenderPipeline,
    time_buffer: wgpu::Buffer,
    time_bind_group: wgpu::BindGroup,
}

impl SkyboxPipeline {
    /// Create a new skybox rendering pipeline.
    pub fn new(ctx: &RenderContext) -> Result<Self> {
        let device = &ctx.device;

        // Create time buffer
        let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Time Buffer"),
            size: std::mem::size_of::<crate::time::TimeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create time bind group layout
        let time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Time Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Time Bind Group"),
            layout: &time_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: time_buffer.as_entire_binding(),
            }],
        });

        // Load skybox shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/skybox.wgsl").into()),
        });

        // Create render pipeline layout with time bind group
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox Pipeline Layout"),
            bind_group_layouts: &[&time_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[], // No vertex buffers (generated in shader)
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for skybox
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None, // No depth testing for skybox
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Ok(Self {
            render_pipeline,
            time_buffer,
            time_bind_group,
        })
    }

    /// Update time uniform buffer.
    pub fn update_time(
        &self,
        queue: &wgpu::Queue,
        time: &crate::time::TimeOfDay,
        weather_intensity: f32,
        night_vision: f32,
    ) {
        let uniform =
            crate::time::TimeUniform::from_time_of_day(time, weather_intensity, night_vision);
        queue.write_buffer(&self.time_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Get the time bind group.
    pub fn time_bind_group(&self) -> &wgpu::BindGroup {
        &self.time_bind_group
    }

    /// Begin a skybox render pass.
    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        view: &'a wgpu::TextureView,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skybox Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        })
    }

    /// Get the render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.render_pipeline
    }
}

/// GPU buffer for a chunk mesh.
pub struct ChunkMeshBuffer {
    /// GPU vertex buffer containing chunk geometry.
    pub vertex_buffer: wgpu::Buffer,
    /// GPU index buffer for chunk geometry.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw from the index buffer.
    pub index_count: u32,
}

impl ChunkMeshBuffer {
    /// Create a chunk mesh buffer from vertices and indices.
    pub fn new(device: &wgpu::Device, vertices: &[MeshVertex], indices: &[u32]) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }
}

/// Highlight uniform data for wireframe rendering.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HighlightUniform {
    /// Position of the highlighted block
    pub position: [f32; 3],
    /// Padding for alignment
    pub padding: f32,
    /// Color of the highlight
    pub color: [f32; 4],
}

/// Wireframe rendering pipeline for block selection highlight.
pub struct WireframePipeline {
    render_pipeline: wgpu::RenderPipeline,
    highlight_buffer: wgpu::Buffer,
    highlight_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
}

/// Particle billboard rendering pipeline.
pub struct ParticlePipeline {
    render_pipeline: wgpu::RenderPipeline,
}

impl ParticlePipeline {
    /// Create the particle render pipeline that draws camera-facing point sprites.
    pub fn new(
        ctx: &RenderContext,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<Self> {
        let device = &ctx.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particles.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::particles::ParticleVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Float32, 3 => Float32],
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Render Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
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

        Ok(Self { render_pipeline })
    }

    /// Access the underlying `wgpu::RenderPipeline` for particles.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.render_pipeline
    }

    /// Begin a particle render pass that reuses the voxel depth buffer so billboards depth test correctly.
    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        color_view: &'a wgpu::TextureView,
        depth_view: &'a wgpu::TextureView,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Particle Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        })
    }
}

impl WireframePipeline {
    /// Create a new wireframe rendering pipeline.
    pub fn new(
        ctx: &RenderContext,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<Self> {
        let device = &ctx.device;

        // Create highlight buffer
        let highlight_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Highlight Buffer"),
            size: std::mem::size_of::<HighlightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create highlight bind group layout
        let highlight_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Highlight Bind Group Layout"),
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

        let highlight_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Highlight Bind Group"),
            layout: &highlight_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: highlight_buffer.as_entire_binding(),
            }],
        });

        // Create cube wireframe vertices (12 edges, 2 vertices per edge = 24 vertices)
        let cube_vertices: Vec<[f32; 3]> = vec![
            // Bottom face edges
            [-0.501, -0.001, -0.501],
            [0.501, -0.001, -0.501], // Front
            [0.501, -0.001, -0.501],
            [0.501, -0.001, 0.501], // Right
            [0.501, -0.001, 0.501],
            [-0.501, -0.001, 0.501], // Back
            [-0.501, -0.001, 0.501],
            [-0.501, -0.001, -0.501], // Left
            // Top face edges
            [-0.501, 0.999, -0.501],
            [0.501, 0.999, -0.501], // Front
            [0.501, 0.999, -0.501],
            [0.501, 0.999, 0.501], // Right
            [0.501, 0.999, 0.501],
            [-0.501, 0.999, 0.501], // Back
            [-0.501, 0.999, 0.501],
            [-0.501, 0.999, -0.501], // Left
            // Vertical edges
            [-0.501, -0.001, -0.501],
            [-0.501, 0.999, -0.501], // Front-left
            [0.501, -0.001, -0.501],
            [0.501, 0.999, -0.501], // Front-right
            [0.501, -0.001, 0.501],
            [0.501, 0.999, 0.501], // Back-right
            [-0.501, -0.001, 0.501],
            [-0.501, 0.999, 0.501], // Back-left
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Wireframe Vertex Buffer"),
            contents: bytemuck::cast_slice(&cube_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Load wireframe shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Wireframe Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/wireframe.wgsl").into()),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Wireframe Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &highlight_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 3]>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        };

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Wireframe Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_buffer_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Don't write depth for wireframe
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1, // Slight offset to avoid z-fighting
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Ok(Self {
            render_pipeline,
            highlight_buffer,
            highlight_bind_group,
            vertex_buffer,
        })
    }

    /// Update highlight uniform (position and color).
    pub fn update_highlight(&self, queue: &wgpu::Queue, position: [f32; 3], color: [f32; 4]) {
        let uniform = HighlightUniform {
            position,
            padding: 0.0,
            color,
        };
        queue.write_buffer(&self.highlight_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Begin a wireframe render pass (uses existing depth from voxel pass).
    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        view: &'a wgpu::TextureView,
        depth_view: &'a wgpu::TextureView,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Wireframe Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Load existing content
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Load existing depth
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        })
    }

    /// Get the render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.render_pipeline
    }

    /// Get the vertex buffer.
    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    /// Get the highlight bind group.
    pub fn highlight_bind_group(&self) -> &wgpu::BindGroup {
        &self.highlight_bind_group
    }
}
