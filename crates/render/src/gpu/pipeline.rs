//! Render pipeline creation and management.
//!
//! This module provides builder-pattern pipeline creation with sensible defaults
//! for chunk mesh rendering.

use anyhow::{Context as AnyhowContext, Result};
use wgpu::{BindGroupLayout, Device};

use super::GpuContext;

/// Information about a render pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineInfo {
    /// Debug label.
    pub label: String,
    /// Whether depth testing is enabled.
    pub depth_test: bool,
    /// Depth comparison function.
    pub depth_compare: wgpu::CompareFunction,
    /// Cull mode.
    pub cull_mode: Option<wgpu::Face>,
}

/// Render pipeline for drawing geometry.
pub struct RenderPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layouts: Vec<BindGroupLayout>,
    info: PipelineInfo,
}

impl RenderPipeline {
    /// Create a new render pipeline.
    ///
    /// This is a simplified constructor for tests. Use RenderPipelineBuilder for full control.
    pub fn new(_device: &Device) -> Result<Self> {
        // Not implemented - use RenderPipelineBuilder instead
        anyhow::bail!("Use RenderPipelineBuilder::new() instead of RenderPipeline::new()")
    }

    /// Get pipeline information.
    pub fn info(&self) -> &PipelineInfo {
        &self.info
    }

    /// Get the underlying wgpu pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Get bind group layout pointer (for testing).
    pub fn bind_group_layout(&self) -> *const () {
        if self.bind_group_layouts.is_empty() {
            std::ptr::null()
        } else {
            &self.bind_group_layouts[0] as *const _ as *const ()
        }
    }
}

/// Builder for creating render pipelines.
pub struct RenderPipelineBuilder<'a> {
    _context: &'a GpuContext,
    vertex_shader: Option<String>,
    fragment_shader: Option<String>,
    depth_test: bool,
    depth_compare: wgpu::CompareFunction,
    topology: wgpu::PrimitiveTopology,
    cull_mode: Option<wgpu::Face>,
    blend_mode: Option<wgpu::BlendState>,
    sample_count: u32,
    label: Option<String>,
    front_face: wgpu::FrontFace,
    polygon_mode: wgpu::PolygonMode,
    vertex_entry: String,
    fragment_entry: String,
    bind_group_layouts: Vec<Vec<wgpu::BindGroupLayoutEntry>>,
}

impl<'a> RenderPipelineBuilder<'a> {
    /// Create a new pipeline builder.
    pub fn new(context: &'a GpuContext) -> Self {
        Self {
            _context: context,
            vertex_shader: None,
            fragment_shader: None,
            depth_test: false,
            depth_compare: wgpu::CompareFunction::Less,
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            blend_mode: None,
            sample_count: 1,
            label: None,
            front_face: wgpu::FrontFace::Ccw,
            polygon_mode: wgpu::PolygonMode::Fill,
            vertex_entry: "vs_main".to_string(),
            fragment_entry: "fs_main".to_string(),
            bind_group_layouts: Vec::new(),
        }
    }

    /// Set vertex shader source.
    pub fn with_vertex_shader(mut self, source: &str) -> Self {
        self.vertex_shader = Some(source.to_string());
        self
    }

    /// Set fragment shader source.
    pub fn with_fragment_shader(mut self, source: &str) -> Self {
        self.fragment_shader = Some(source.to_string());
        self
    }

    /// Set vertex format (type parameter for compile-time checking).
    #[allow(clippy::extra_unused_type_parameters)]
    pub fn with_vertex_format<T>(self) -> Self {
        // Type parameter T is for compile-time type checking
        // Actual vertex layout is determined in build()
        self
    }

    /// Enable/disable depth testing.
    pub fn with_depth_test(mut self, enabled: bool) -> Self {
        self.depth_test = enabled;
        self
    }

    /// Set depth compare function.
    pub fn with_depth_compare(mut self, compare: wgpu::CompareFunction) -> Self {
        self.depth_compare = compare;
        self
    }

    /// Set primitive topology.
    pub fn with_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    /// Set cull mode.
    pub fn with_cull_mode(mut self, cull_mode: Option<wgpu::Face>) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    /// Set blend mode.
    pub fn with_blend_mode(mut self, blend: wgpu::BlendState) -> Self {
        self.blend_mode = Some(blend);
        self
    }

    /// Set sample count for MSAA.
    pub fn with_sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    /// Set debug label.
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Set front face winding order.
    pub fn with_front_face(mut self, front_face: wgpu::FrontFace) -> Self {
        self.front_face = front_face;
        self
    }

    /// Set polygon mode (fill, line, point).
    pub fn with_polygon_mode(mut self, mode: wgpu::PolygonMode) -> Self {
        self.polygon_mode = mode;
        self
    }

    /// Set vertex shader entry point.
    pub fn with_vertex_entry(mut self, entry: &str) -> Self {
        self.vertex_entry = entry.to_string();
        self
    }

    /// Set fragment shader entry point.
    pub fn with_fragment_entry(mut self, entry: &str) -> Self {
        self.fragment_entry = entry.to_string();
        self
    }

    /// Add a bind group layout.
    pub fn with_bind_group_layout(
        mut self,
        _group: u32,
        entries: &[wgpu::BindGroupLayoutEntry],
    ) -> Self {
        self.bind_group_layouts.push(entries.to_vec());
        self
    }

    /// Build the render pipeline.
    pub fn build(self) -> Result<RenderPipeline> {
        use wgpu::{
            BindGroupLayoutDescriptor, ColorTargetState, ColorWrites, DepthBiasState,
            DepthStencilState, FragmentState, MultisampleState, PrimitiveState,
            RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, StencilState,
            TextureFormat, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
            VertexStepMode,
        };

        // Validate shaders are set
        let vertex_shader = self.vertex_shader.context("Vertex shader source not set")?;
        let fragment_shader = self
            .fragment_shader
            .context("Fragment shader source not set")?;

        let device = self._context.device();

        // Create shader modules
        let vs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("vertex_shader"),
            source: ShaderSource::Wgsl(vertex_shader.into()),
        });

        let fs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("fragment_shader"),
            source: ShaderSource::Wgsl(fragment_shader.into()),
        });

        // Create bind group layouts
        let bind_group_layouts: Vec<BindGroupLayout> = self
            .bind_group_layouts
            .iter()
            .enumerate()
            .map(|(i, entries)| {
                device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: Some(&format!("bind_group_layout_{}", i)),
                    entries,
                })
            })
            .collect();

        let bind_group_layout_refs: Vec<&BindGroupLayout> = bind_group_layouts.iter().collect();

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &bind_group_layout_refs,
            push_constant_ranges: &[],
        });

        // Define vertex buffer layout (hardcoded for MeshVertex)
        const MESH_VERTEX_ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Uint32,
            },
        ];

        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::MeshVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: MESH_VERTEX_ATTRIBUTES,
        };

        // Create depth stencil state
        let depth_stencil = if self.depth_test {
            Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: self.depth_compare,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            })
        } else {
            None
        };

        // Create the render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: self.label.as_deref(),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &vs_module,
                entry_point: &self.vertex_entry,
                buffers: &[vertex_buffer_layout],
            },
            fragment: Some(FragmentState {
                module: &fs_module,
                entry_point: &self.fragment_entry,
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: self.blend_mode,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: self.topology,
                strip_index_format: None,
                front_face: self.front_face,
                cull_mode: self.cull_mode,
                polygon_mode: self.polygon_mode,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil,
            multisample: MultisampleState {
                count: self.sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let info = PipelineInfo {
            label: self.label.unwrap_or_else(|| "pipeline".to_string()),
            depth_test: self.depth_test,
            depth_compare: self.depth_compare,
            cull_mode: self.cull_mode,
        };

        Ok(RenderPipeline {
            pipeline,
            bind_group_layouts,
            info,
        })
    }
}
