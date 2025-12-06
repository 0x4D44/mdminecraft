//! Instanced billboard rendering pipeline for 3D UI.

use anyhow::Result;
use bitflags::bitflags;
use tracing::warn;
use wgpu::util::DeviceExt;

const INITIAL_CAPACITY: usize = 1_024;
const MAX_INSTANCES: usize = 32_768;

bitflags! {
    /// Per-instance feature flags.
    pub struct BillboardFlags: u16 {
        /// Skip light modulation (treat as emissive).
        const EMISSIVE = 0b0001;
        /// Render in overlay pass without depth testing.
        const OVERLAY_NO_DEPTH = 0b0010;
    }
}

/// GPU-facing instance data. Packed to 64 bytes for cache-friendly uploads.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BillboardInstance {
    pub position: [f32; 3],
    pub size: [f32; 2],
    pub rot: f32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub color: [f32; 4],
    pub light: f32,
    /// Sort key (higher draws later).
    pub layer: i16,
    /// Lower bits follow [`BillboardFlags`].
    pub flags: u16,
}

impl Default for BillboardInstance {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            size: [1.0, 1.0],
            rot: 0.0,
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            light: 1.0,
            layer: 0,
            flags: BillboardFlags::empty().bits(),
        }
    }
}

impl BillboardInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        1 => Float32x3, // position
        2 => Float32x2, // size
        3 => Float32,   // rot
        4 => Float32x2, // uv_min
        5 => Float32x2, // uv_max
        6 => Float32x4, // color
        7 => Float32,   // light
        8 => Sint16x2,  // layer (x), flags (y)
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Submission containing a stable ID for deterministic ordering.
#[derive(Clone, Debug)]
pub struct BillboardSubmission {
    pub id: u32,
    pub instance: BillboardInstance,
}

/// Frame-local collection of billboards. Not thread-safe by design.
#[derive(Default, Debug)]
pub struct BillboardEmitter {
    entries: Vec<BillboardSubmission>,
}

impl BillboardEmitter {
    pub fn submit(&mut self, id: u32, instance: BillboardInstance) {
        self.entries.push(BillboardSubmission { id, instance });
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn sort_by_layer_then_id(&mut self) {
        self.entries.sort_by(|a, b| {
            a.instance
                .layer
                .cmp(&b.instance.layer)
                .then(a.id.cmp(&b.id))
        });
    }
}

/// Runtime statistics for a draw call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BillboardStats {
    pub instances: usize,
    pub overlay_instances: usize,
    pub draw_calls: u32,
}

/// GPU renderer for billboards.
pub struct BillboardRenderer {
    pipeline_depth: wgpu::RenderPipeline,
    pipeline_overlay: wgpu::RenderPipeline,
    atlas_bind_group: wgpu::BindGroup,
    quad_vertex: wgpu::Buffer,
    quad_index: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    depth_instances: Vec<BillboardInstance>,
    overlay_instances: Vec<BillboardInstance>,
    combined: Vec<BillboardInstance>,
    stats: BillboardStats,
}

impl BillboardRenderer {
    /// Create pipelines and static buffers. Caller provides existing camera bind group layout and atlas resources.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        atlas_view: &wgpu::TextureView,
        atlas_sampler: &wgpu::Sampler,
    ) -> Result<Self> {
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("UI3D Billboard Atlas BGL"),
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

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI3D Billboard Atlas BG"),
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI3D Billboard Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/billboard.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI3D Billboard Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &atlas_bind_group_layout],
            push_constant_ranges: &[],
        });

        let quad_vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI3D Billboard Quad Vertices"),
            contents: bytemuck::cast_slice(&QUAD_POSITIONS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let quad_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI3D Billboard Quad Indices"),
            contents: bytemuck::cast_slice(&QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_buffer = Self::create_instance_buffer(device, INITIAL_CAPACITY);

        let premul_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let depth_state = Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: -2,
                slope_scale: -1.5,
                clamp: 0.0,
            },
        });

        let pipeline_depth = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI3D Billboard Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[quad_layout(), BillboardInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(premul_blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: depth_state.clone(),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let pipeline_overlay = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI3D Billboard Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[quad_layout(), BillboardInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(premul_blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Ok(Self {
            pipeline_depth,
            pipeline_overlay,
            atlas_bind_group,
            quad_vertex,
            quad_index,
            instance_buffer,
            instance_capacity: INITIAL_CAPACITY,
            depth_instances: Vec::new(),
            overlay_instances: Vec::new(),
            combined: Vec::new(),
            stats: BillboardStats::default(),
        })
    }

    fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI3D Billboard Instances"),
            size: (capacity * std::mem::size_of::<BillboardInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn ensure_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.instance_capacity {
            return;
        }

        let mut new_cap = self.instance_capacity.max(1);
        while new_cap < needed && new_cap < MAX_INSTANCES {
            new_cap = (new_cap * 2).min(MAX_INSTANCES);
        }

        warn!(
            current = self.instance_capacity,
            requested = needed,
            new_capacity = new_cap,
            "Growing billboard instance buffer"
        );

        self.instance_buffer = Self::create_instance_buffer(device, new_cap);
        self.instance_capacity = new_cap;
    }

    /// Render billboards. Consumes the emitter for this frame and clears it.
    #[allow(clippy::too_many_arguments)]
    pub fn render<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &'a mut wgpu::CommandEncoder,
        color_view: &'a wgpu::TextureView,
        depth_view: &'a wgpu::TextureView,
        camera_bind_group: &'a wgpu::BindGroup,
        emitter: &mut BillboardEmitter,
    ) -> Result<BillboardStats> {
        if emitter.is_empty() {
            self.stats = BillboardStats::default();
            return Ok(self.stats);
        }

        emitter.sort_by_layer_then_id();

        self.depth_instances.clear();
        self.overlay_instances.clear();

        for submission in emitter.entries.iter() {
            if BillboardFlags::from_bits_truncate(submission.instance.flags)
                .contains(BillboardFlags::OVERLAY_NO_DEPTH)
            {
                self.overlay_instances.push(submission.instance);
            } else {
                self.depth_instances.push(submission.instance);
            }
        }

        let mut depth_count = self.depth_instances.len();
        let mut overlay_count = self.overlay_instances.len();
        let mut total = depth_count + overlay_count;

        if total > MAX_INSTANCES {
            let mut drop = total - MAX_INSTANCES;
            if overlay_count >= drop {
                overlay_count -= drop;
                self.overlay_instances.truncate(overlay_count);
            } else {
                drop -= overlay_count;
                overlay_count = 0;
                if depth_count > drop {
                    depth_count -= drop;
                    self.depth_instances.truncate(depth_count);
                } else {
                    depth_count = 0;
                    self.depth_instances.clear();
                }
            }

            total = depth_count + overlay_count;
            warn!(
                requested = emitter.len(),
                kept = total,
                cap = MAX_INSTANCES,
                "Dropping billboards above cap"
            );
        }

        self.combined.clear();
        self.combined.reserve(total);
        self.combined
            .extend_from_slice(&self.depth_instances[..depth_count]);
        self.combined
            .extend_from_slice(&self.overlay_instances[..overlay_count]);

        self.ensure_capacity(device, total);
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.combined[..total]),
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI3D Billboard Pass"),
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
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_vertex_buffer(0, self.quad_vertex.slice(..));
            pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
            pass.set_bind_group(0, camera_bind_group, &[]);
            pass.set_bind_group(1, &self.atlas_bind_group, &[]);

            if depth_count > 0 {
                pass.set_pipeline(&self.pipeline_depth);
                pass.draw_indexed(0..QUAD_INDICES.len() as u32, 0, 0..depth_count as u32);
            }

            if overlay_count > 0 {
                pass.set_pipeline(&self.pipeline_overlay);
                pass.draw_indexed(
                    0..QUAD_INDICES.len() as u32,
                    0,
                    depth_count as u32..(depth_count + overlay_count) as u32,
                );
            }
        }

        emitter.clear();
        self.stats = BillboardStats {
            instances: total,
            overlay_instances: overlay_count,
            draw_calls: ((depth_count > 0) as u32) + ((overlay_count > 0) as u32),
        };

        Ok(self.stats)
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> BillboardStats {
        self.stats
    }
}

fn quad_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 2]>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

const QUAD_POSITIONS: [[f32; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];

const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;
    use mdminecraft_render::CameraUniform;

    fn test_device() -> (wgpu::Instance, wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        }))
        .expect("adapter");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .expect("device");

        (instance, device, queue)
    }

    fn make_camera_bind_group(
        device: &wgpu::Device,
    ) -> (wgpu::BindGroupLayout, wgpu::BindGroup, wgpu::Buffer) {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Test Camera BGL"),
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

        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Test Camera BG"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });

        (layout, bind_group, camera_buf)
    }

    fn make_targets(device: &wgpu::Device) -> (wgpu::TextureView, wgpu::TextureView) {
        let color_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Color"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Depth"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        (
            color_tex.create_view(&Default::default()),
            depth_tex.create_view(&Default::default()),
        )
    }

    fn write_camera(queue: &wgpu::Queue, buffer: &wgpu::Buffer) {
        let cam = CameraUniform {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 5.0, 1.0],
        };
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(&cam));
    }

    #[test]
    fn instance_layout_is_packed() {
        assert_eq!(std::mem::size_of::<BillboardInstance>(), 64);
    }

    #[test]
    fn sort_is_stable() {
        let mut emitter = BillboardEmitter::default();
        emitter.submit(
            2,
            BillboardInstance {
                layer: 0,
                ..Default::default()
            },
        );
        emitter.submit(
            1,
            BillboardInstance {
                layer: 0,
                ..Default::default()
            },
        );
        emitter.submit(
            3,
            BillboardInstance {
                layer: 1,
                ..Default::default()
            },
        );
        emitter.sort_by_layer_then_id();

        let ids: Vec<u32> = emitter.entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn smoke_renders_single_billboard() {
        let (_instance, device, queue) = test_device();
        let (camera_layout, camera_bind_group, camera_buf) = make_camera_bind_group(&device);
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &atlas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
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

        let (color_view, depth_view) = make_targets(&device);

        let mut emitter = BillboardEmitter::default();
        emitter.submit(0, BillboardInstance::default());

        write_camera(&queue, &camera_buf);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Billboard Test Encoder"),
        });

        let stats = renderer
            .render(
                &device,
                &queue,
                &mut encoder,
                &color_view,
                &depth_view,
                &camera_bind_group,
                &mut emitter,
            )
            .expect("render");

        queue.submit(Some(encoder.finish()));

        assert_eq!(stats.instances, 1);
        assert_eq!(stats.overlay_instances, 0);
        assert_eq!(stats.draw_calls, 1);
    }
}
