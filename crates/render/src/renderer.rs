//! High-level renderer that integrates GPU context, texture atlas, and chunk mesh cache.
//!
//! This module provides the main `Renderer` struct that manages the complete rendering pipeline
//! for the voxel world.

use anyhow::{Context as AnyhowContext, Result};
use mdminecraft_assets::BlockRegistry;
use mdminecraft_world::{Chunk, ChunkPos, DirtyFlags};
use std::collections::HashMap;
use wgpu::{BindGroup, Buffer, Texture, TextureView};

use crate::{
    gpu::{
        create_texture_layout, create_view_projection_layout, look_at_matrix, perspective_matrix,
        GpuContext, RenderPipeline, RenderPipelineBuilder, TextureAtlas,
        UniformBuffer, ViewProjectionUniforms,
    },
    ChunkMeshCache, MeshBuffers,
};

/// Configuration for the renderer.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Target width in pixels.
    pub width: u32,
    /// Target height in pixels.
    pub height: u32,
    /// Whether to run in headless mode.
    pub headless: bool,
    /// Field of view in degrees.
    pub fov_degrees: f32,
    /// Near clipping plane.
    pub near_plane: f32,
    /// Far clipping plane.
    pub far_plane: f32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            headless: false,
            fov_degrees: 70.0,
            near_plane: 0.1,
            far_plane: 1000.0,
        }
    }
}

/// Camera state for rendering.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// Camera position in world space.
    pub position: [f32; 3],
    /// Point the camera is looking at.
    pub target: [f32; 3],
    /// Up direction.
    pub up: [f32; 3],
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: [0.0, 20.0, 30.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
        }
    }
}

/// GPU resources for a single chunk mesh.
struct ChunkGpuMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

/// Main renderer for voxel chunks.
pub struct Renderer {
    gpu: GpuContext,
    mesh_cache: ChunkMeshCache,
    texture_atlas: TextureAtlas,
    pipeline: RenderPipeline,
    view_projection_uniform: UniformBuffer<ViewProjectionUniforms>,
    texture_bind_group: BindGroup,
    camera: Camera,
    config: RendererConfig,
    gpu_meshes: HashMap<ChunkPos, ChunkGpuMesh>,
    #[allow(dead_code)] // Held for lifetime management of depth_texture_view
    depth_texture: Texture,
    depth_texture_view: TextureView,
}

impl Renderer {
    /// Create a new renderer with the given configuration.
    ///
    /// # Arguments
    /// * `config` - Renderer configuration
    /// * `registry` - Block registry for texture loading
    ///
    /// # Returns
    /// A new renderer instance, or an error if initialization fails.
    pub fn new(config: RendererConfig, _registry: &BlockRegistry) -> Result<Self> {
        // Create GPU context
        let gpu_config = crate::gpu::GpuContextConfig {
            width: config.width,
            height: config.height,
            headless: config.headless,
            power_preference: wgpu::PowerPreference::HighPerformance,
            present_mode: wgpu::PresentMode::Fifo,
        };
        let gpu = GpuContext::new(gpu_config).context("Failed to create GPU context")?;

        // Create mesh cache
        let mesh_cache = ChunkMeshCache::new();

        // Create texture atlas with placeholder textures
        let mut texture_atlas = TextureAtlas::new();

        // Add some placeholder textures for testing
        // In a real implementation, these would be loaded from the asset system
        let white_texture = vec![255u8; 16 * 16 * 4];
        texture_atlas
            .add_texture("white", &white_texture, 16, 16)
            .context("Failed to add white texture")?;

        let stone_texture = create_checkerboard_texture();
        texture_atlas
            .add_texture("stone", &stone_texture, 16, 16)
            .context("Failed to add stone texture")?;

        texture_atlas.build().context("Failed to build texture atlas")?;

        // Create GPU texture from atlas
        let gpu_texture = texture_atlas
            .create_gpu_texture(gpu.device(), gpu.queue())
            .context("Failed to create GPU texture")?;

        let texture_view = gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = gpu.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layouts
        let view_proj_layout = create_view_projection_layout(gpu.device());
        let texture_layout = create_texture_layout(gpu.device());

        // Create texture bind group
        let texture_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture_bind_group"),
            layout: &texture_layout,
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

        // Load shaders
        let vertex_shader = include_str!("../shaders/chunk_vertex.wgsl");
        let fragment_shader = include_str!("../shaders/chunk_fragment.wgsl");

        // Create render pipeline with bind group layouts
        let view_proj_bind_layout = &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];

        let texture_bind_layout = &[
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
        ];

        let pipeline = RenderPipelineBuilder::new(&gpu)
            .with_vertex_shader(vertex_shader)
            .with_fragment_shader(fragment_shader)
            .with_vertex_format::<crate::MeshVertex>()
            .with_depth_test(true)
            .with_depth_compare(wgpu::CompareFunction::Less)
            .with_cull_mode(Some(wgpu::Face::Back))
            .with_bind_group_layout(0, view_proj_bind_layout)
            .with_bind_group_layout(1, texture_bind_layout)
            .with_label("chunk_pipeline")
            .build()
            .context("Failed to create render pipeline")?;

        // Create view-projection uniform buffer
        let view_proj_data = ViewProjectionUniforms::identity();
        let view_projection_uniform =
            UniformBuffer::new(gpu.device(), &view_proj_data, "view_projection", &view_proj_layout);

        let camera = Camera::default();

        // Create depth texture
        let depth_texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self {
            gpu,
            mesh_cache,
            texture_atlas,
            pipeline,
            view_projection_uniform,
            texture_bind_group,
            camera,
            config,
            gpu_meshes: HashMap::new(),
            depth_texture,
            depth_texture_view,
        })
    }

    /// Update the camera position and recalculate view-projection matrix.
    pub fn update_camera(&mut self, camera: Camera) {
        self.camera = camera;

        // Calculate view matrix
        let view = look_at_matrix(self.camera.position, self.camera.target, self.camera.up);

        // Calculate projection matrix
        let aspect = self.config.width as f32 / self.config.height as f32;
        let fov_radians = self.config.fov_degrees.to_radians();
        let projection = perspective_matrix(fov_radians, aspect, self.config.near_plane, self.config.far_plane);

        // Update uniform buffer
        let uniforms = ViewProjectionUniforms::from_view_projection(view, projection);
        self.view_projection_uniform.update(self.gpu.queue(), &uniforms);
    }

    /// Update a chunk in the mesh cache and upload to GPU if needed.
    ///
    /// # Arguments
    /// * `chunk` - The chunk to update
    /// * `dirty` - Dirty flags indicating what changed
    /// * `registry` - Block registry for meshing
    pub fn update_chunk(&mut self, chunk: &Chunk, dirty: DirtyFlags, registry: &BlockRegistry) -> Result<()> {
        let pos = chunk.position();

        // Update mesh in cache - returns a reference
        let mesh = self.mesh_cache.update_chunk(chunk, dirty, registry);

        // Check if we need to upload
        let should_upload = !mesh.vertices.is_empty() && !mesh.indices.is_empty();
        let should_remove = !should_upload;

        // Clone mesh data for upload to avoid borrow issues
        if should_upload {
            let mesh_clone = mesh.clone();
            self.upload_chunk_mesh(pos, &mesh_clone)?;
        } else if should_remove {
            // Remove GPU mesh if chunk became empty
            self.gpu_meshes.remove(&pos);
        }

        Ok(())
    }

    /// Upload a chunk mesh to the GPU.
    fn upload_chunk_mesh(&mut self, pos: ChunkPos, mesh: &MeshBuffers) -> Result<()> {
        use wgpu::util::DeviceExt;

        // Create vertex buffer
        let vertex_buffer = self.gpu.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("chunk_vertex_buffer_{:?}", pos)),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create index buffer
        let index_buffer = self.gpu.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("chunk_index_buffer_{:?}", pos)),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let gpu_mesh = ChunkGpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        };

        self.gpu_meshes.insert(pos, gpu_mesh);

        Ok(())
    }

    /// Get the number of chunks currently loaded in GPU memory.
    pub fn loaded_chunk_count(&self) -> usize {
        self.gpu_meshes.len()
    }

    /// Get camera position.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Get reference to GPU context.
    pub fn gpu(&self) -> &GpuContext {
        &self.gpu
    }

    /// Get reference to texture atlas.
    pub fn texture_atlas(&self) -> &TextureAtlas {
        &self.texture_atlas
    }

    /// Get reference to mesh cache.
    pub fn mesh_cache(&self) -> &ChunkMeshCache {
        &self.mesh_cache
    }

    /// Render a frame to the provided texture view.
    ///
    /// # Arguments
    /// * `target_view` - The texture view to render to (e.g., from a surface or off-screen texture)
    ///
    /// # Returns
    /// Ok(()) if rendering succeeds, or an error if any GPU operation fails.
    pub fn render_frame(&self, target_view: &TextureView) -> Result<()> {
        // Create command encoder
        let mut encoder = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        // Create render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set pipeline and bind groups
            render_pass.set_pipeline(self.pipeline.pipeline());
            render_pass.set_bind_group(0, self.view_projection_uniform.bind_group(), &[]);
            render_pass.set_bind_group(1, &self.texture_bind_group, &[]);

            // Draw all uploaded chunk meshes
            for gpu_mesh in self.gpu_meshes.values() {
                render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(
                    gpu_mesh.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
            }
        }

        // Submit command buffer
        self.gpu.queue().submit(Some(encoder.finish()));

        Ok(())
    }
}

/// Create a simple checkerboard texture for testing.
fn create_checkerboard_texture() -> Vec<u8> {
    let mut data = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            let checker = ((x / 4) + (y / 4)) % 2 == 0;
            let gray = if checker { 128 } else { 192 };
            data.extend_from_slice(&[gray, gray, gray, 255]);
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_assets::{BlockDescriptor, BlockRegistry};
    use mdminecraft_world::{Chunk, ChunkPos, Voxel};

    fn test_registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor {
                name: "air".into(),
                opaque: false,
            },
            BlockDescriptor {
                name: "stone".into(),
                opaque: true,
            },
        ])
    }

    #[test]
    fn renderer_creation() -> Result<()> {
        let config = RendererConfig {
            width: 800,
            height: 600,
            headless: true,
            ..Default::default()
        };
        let registry = test_registry();

        let _renderer = Renderer::new(config, &registry)?;

        Ok(())
    }

    #[test]
    fn renderer_camera_update() -> Result<()> {
        let config = RendererConfig {
            width: 800,
            height: 600,
            headless: true,
            ..Default::default()
        };
        let registry = test_registry();

        let mut renderer = Renderer::new(config, &registry)?;

        let camera = Camera {
            position: [10.0, 10.0, 10.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
        };

        renderer.update_camera(camera);

        assert_eq!(renderer.camera().position, [10.0, 10.0, 10.0]);

        Ok(())
    }

    #[test]
    fn renderer_chunk_upload() -> Result<()> {
        let config = RendererConfig {
            width: 800,
            height: 600,
            headless: true,
            ..Default::default()
        };
        let registry = test_registry();

        let mut renderer = Renderer::new(config, &registry)?;

        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);

        // Add a block to create geometry
        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: 1,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let dirty = chunk.take_dirty_flags();
        renderer.update_chunk(&chunk, dirty, &registry)?;

        assert_eq!(renderer.loaded_chunk_count(), 1);

        Ok(())
    }

    #[test]
    fn renderer_empty_chunk_not_uploaded() -> Result<()> {
        let config = RendererConfig {
            width: 800,
            height: 600,
            headless: true,
            ..Default::default()
        };
        let registry = test_registry();

        let mut renderer = Renderer::new(config, &registry)?;

        let pos = ChunkPos::new(0, 0);
        let chunk = Chunk::new(pos);

        let dirty = DirtyFlags::MESH;
        renderer.update_chunk(&chunk, dirty, &registry)?;

        assert_eq!(renderer.loaded_chunk_count(), 0);

        Ok(())
    }
}
