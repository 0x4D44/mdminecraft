//! UI3D Manager - Manages all 3D UI elements in the game

use crate::components::{Text3D, UIComponent};
use crate::render::{FontAtlas, FontAtlasBuilder, TextRenderer};
use anyhow::Result;
use glam::Vec3;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// Handle to a UI element for updates/removal
pub type UIElementHandle = u64;

/// A managed UI element with its buffers
struct UIElement {
    text: Text3D,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    needs_update: bool,
}

/// Manages all 3D UI elements in the game
pub struct UI3DManager {
    text_renderer: TextRenderer,
    elements: HashMap<UIElementHandle, UIElement>,
    next_handle: u64,
}

impl UI3DManager {
    /// Create a new UI3D manager
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        font_atlas: FontAtlas,
    ) -> Result<Self> {
        let text_renderer = TextRenderer::new(
            device,
            queue,
            surface_format,
            camera_bind_group_layout,
            font_atlas,
        )?;

        Ok(Self {
            text_renderer,
            elements: HashMap::new(),
            next_handle: 1,
        })
    }

    /// Create a UI3D manager with a system font
    pub fn with_system_font(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        font_path: &str,
        font_size: f32,
    ) -> Result<Self> {
        let atlas = FontAtlasBuilder::from_file(font_path)?
            .with_font_size(font_size)
            .build()?;

        Self::new(device, queue, surface_format, camera_bind_group_layout, atlas)
    }

    /// Add a new text element to the UI
    pub fn add_text(&mut self, device: &wgpu::Device, text: Text3D) -> UIElementHandle {
        let handle = self.next_handle;
        self.next_handle += 1;

        let (vertex_buffer, index_buffer, index_count) = self.create_text_buffers(device, &text);

        self.elements.insert(
            handle,
            UIElement {
                text,
                vertex_buffer,
                index_buffer,
                index_count,
                needs_update: false,
            },
        );

        handle
    }

    /// Update an existing text element
    pub fn update_text(&mut self, device: &wgpu::Device, handle: UIElementHandle, text: Text3D) {
        let buffers = self.create_text_buffers(device, &text);
        if let Some(element) = self.elements.get_mut(&handle) {
            element.text = text;
            element.vertex_buffer = buffers.0;
            element.index_buffer = buffers.1;
            element.index_count = buffers.2;
            element.needs_update = false;
        }
    }

    /// Update just the text content (more efficient than full update)
    pub fn set_text_content(&mut self, device: &wgpu::Device, handle: UIElementHandle, content: impl Into<String>) {
        // Create a temporary text object to generate buffers
        let buffers = if let Some(element) = self.elements.get(&handle) {
            let mut temp_text = element.text.clone();
            temp_text.set_text(content);
            Some((self.create_text_buffers(device, &temp_text), temp_text))
        } else {
            None
        };

        // Now update the element
        if let Some(((vertex_buffer, index_buffer, index_count), text)) = buffers {
            if let Some(element) = self.elements.get_mut(&handle) {
                element.text = text;
                element.vertex_buffer = vertex_buffer;
                element.index_buffer = index_buffer;
                element.index_count = index_count;
            }
        }
    }

    /// Update just the position of a text element
    pub fn set_text_position(&mut self, device: &wgpu::Device, handle: UIElementHandle, position: Vec3) {
        // Create a temporary text object to generate buffers
        let buffers = if let Some(element) = self.elements.get(&handle) {
            let mut temp_text = element.text.clone();
            temp_text.set_position(position);
            Some((self.create_text_buffers(device, &temp_text), temp_text))
        } else {
            None
        };

        // Now update the element
        if let Some(((vertex_buffer, index_buffer, index_count), text)) = buffers {
            if let Some(element) = self.elements.get_mut(&handle) {
                element.text = text;
                element.vertex_buffer = vertex_buffer;
                element.index_buffer = index_buffer;
                element.index_count = index_count;
            }
        }
    }

    /// Remove a text element
    pub fn remove_text(&mut self, handle: UIElementHandle) {
        self.elements.remove(&handle);
    }

    /// Remove all text elements
    pub fn clear(&mut self) {
        self.elements.clear();
    }

    /// Render all UI elements
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, camera_bind_group: &'a wgpu::BindGroup) {
        for element in self.elements.values() {
            if !element.text.is_visible() {
                continue;
            }

            // Select pipeline based on billboard mode
            render_pass.set_pipeline(self.text_renderer.pipeline(element.text.billboard));
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_bind_group(1, self.text_renderer.font_bind_group(), &[]);
            render_pass.set_vertex_buffer(0, element.vertex_buffer.slice(..));
            render_pass.set_index_buffer(element.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..element.index_count, 0, 0..1);
        }
    }

    /// Get the number of active UI elements
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    /// Helper to create GPU buffers for a text element
    fn create_text_buffers(&self, device: &wgpu::Device, text: &Text3D) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        let (vertices, indices) = self.text_renderer.generate_text_mesh(text);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        (vertex_buffer, index_buffer, indices.len() as u32)
    }
}
