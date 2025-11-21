//! Text Renderer - GPU-accelerated text rendering in 3D

use super::font_atlas::FontAtlas;
use crate::components::Text3D;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};

/// Text rendering pipeline and resources
pub struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    pipeline_fixed: wgpu::RenderPipeline,
    font_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    // Texture must stay alive for the bind group even if unused directly.
    font_texture: wgpu::Texture,
    atlas: FontAtlas,
}

/// Vertex format for text rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TextVertex {
    /// World position of this vertex
    pub position: [f32; 3],
    /// UV coordinates in font atlas
    pub uv: [f32; 2],
    /// Text color (RGBA)
    pub color: [f32; 4],
    /// Billboard center position
    pub billboard_center: [f32; 3],
}

impl TextVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4, 3 => Float32x3];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

impl TextRenderer {
    /// Create a new text renderer
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        atlas: FontAtlas,
    ) -> Result<Self> {
        // Create font texture
        let texture_size = wgpu::Extent3d {
            width: atlas.width,
            height: atlas.height,
            depth_or_array_layers: 1,
        };

        let font_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Font Atlas Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload atlas data to GPU
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &font_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas.texture_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(atlas.width),
                rows_per_image: Some(atlas.height),
            },
            texture_size,
        );

        // Create texture view and sampler
        let texture_view = font_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Font Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout for font texture
        let font_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Font Bind Group Layout"),
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

        let font_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Font Bind Group"),
            layout: &font_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/text.wgsl").into()),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &font_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline (billboard variant)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline (Billboard)"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[TextVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Don't cull for billboards
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // UI doesn't write depth
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Create fixed orientation pipeline
        let pipeline_fixed = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline (Fixed)"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main_fixed",
                buffers: &[TextVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Ok(Self {
            pipeline,
            pipeline_fixed,
            font_bind_group,
            font_texture,
            atlas,
        })
    }

    /// Generate mesh for a Text3D component
    pub fn generate_text_mesh(&self, text: &Text3D) -> (Vec<TextVertex>, Vec<u32>) {
        let layouts = self.atlas.layout_text(&text.text, text.font_size);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let center = text.transform.position;
        let color = text.color;

        for layout in layouts {
            let base_vertex = vertices.len() as u32;

            // Create quad for this glyph
            let x0 = layout.position_x;
            let y0 = layout.position_y;
            let x1 = x0 + layout.width;
            let y1 = y0 + layout.height;

            let (u0, v0) = layout.uv_min;
            let (u1, v1) = layout.uv_max;

            // Four corners of the quad (in local space, will be billboarded)
            vertices.push(TextVertex {
                position: [center.x + x0, center.y + y0, center.z],
                uv: [u0, v0],
                color,
                billboard_center: [center.x, center.y, center.z],
            });
            vertices.push(TextVertex {
                position: [center.x + x1, center.y + y0, center.z],
                uv: [u1, v0],
                color,
                billboard_center: [center.x, center.y, center.z],
            });
            vertices.push(TextVertex {
                position: [center.x + x1, center.y + y1, center.z],
                uv: [u1, v1],
                color,
                billboard_center: [center.x, center.y, center.z],
            });
            vertices.push(TextVertex {
                position: [center.x + x0, center.y + y1, center.z],
                uv: [u0, v1],
                color,
                billboard_center: [center.x, center.y, center.z],
            });

            // Two triangles for the quad
            indices.extend_from_slice(&[
                base_vertex,
                base_vertex + 1,
                base_vertex + 2,
                base_vertex,
                base_vertex + 2,
                base_vertex + 3,
            ]);
        }

        (vertices, indices)
    }

    /// Get the render pipeline
    pub fn pipeline(&self, billboard: bool) -> &wgpu::RenderPipeline {
        if billboard {
            &self.pipeline
        } else {
            &self.pipeline_fixed
        }
    }

    /// Get the font bind group
    pub fn font_bind_group(&self) -> &wgpu::BindGroup {
        &self.font_bind_group
    }

    /// Get the font atlas
    pub fn atlas(&self) -> &FontAtlas {
        &self.atlas
    }
}
