//! UI3D Manager - Manages all 3D UI elements in the game

use crate::components::{Button3D, ButtonState, Text3D, UIComponent};
use crate::interaction::{raycast_billboard_quad, UIRaycastHit};
use crate::render::{FontAtlas, FontAtlasBuilder, TextRenderer};
use anyhow::Result;
use glam::{Vec3, Mat4};
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

/// A managed button element
struct ButtonElement {
    button: Button3D,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

/// Manages all 3D UI elements in the game
pub struct UI3DManager {
    text_renderer: TextRenderer,
    elements: HashMap<UIElementHandle, UIElement>,
    buttons: HashMap<UIElementHandle, ButtonElement>,
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
            buttons: HashMap::new(),
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
        // Render text elements
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

        // Render buttons
        self.render_buttons(render_pass, camera_bind_group);
    }

    /// Get the number of active UI elements
    pub fn element_count(&self) -> usize {
        self.elements.len() + self.buttons.len()
    }

    // === Button Management ===

    /// Add a new button to the UI
    pub fn add_button(&mut self, device: &wgpu::Device, button: Button3D) -> UIElementHandle {
        let handle = self.next_handle;
        self.next_handle += 1;

        let text = button.to_text3d();
        let (vertex_buffer, index_buffer, index_count) = self.create_text_buffers(device, &text);

        self.buttons.insert(
            handle,
            ButtonElement {
                button,
                vertex_buffer,
                index_buffer,
                index_count,
            },
        );

        handle
    }

    /// Update a button's state (hover, pressed, etc.)
    pub fn set_button_state(&mut self, device: &wgpu::Device, handle: UIElementHandle, state: ButtonState) {
        // Create buffers first
        let buffers = if let Some(element) = self.buttons.get(&handle) {
            let mut temp_button = element.button.clone();
            temp_button.set_state(state);
            let text = temp_button.to_text3d();
            Some((self.create_text_buffers(device, &text), temp_button))
        } else {
            None
        };

        // Now update the element
        if let Some(((vertex_buffer, index_buffer, index_count), button)) = buffers {
            if let Some(element) = self.buttons.get_mut(&handle) {
                element.button = button;
                element.vertex_buffer = vertex_buffer;
                element.index_buffer = index_buffer;
                element.index_count = index_count;
            }
        }
    }

    /// Get a button's current state
    pub fn get_button_state(&self, handle: UIElementHandle) -> Option<ButtonState> {
        self.buttons.get(&handle).map(|e| e.button.state)
    }

    /// Update a button's text content
    pub fn set_button_text(&mut self, device: &wgpu::Device, handle: UIElementHandle, new_text: String) {
        // Create buffers first
        let buffers = if let Some(element) = self.buttons.get(&handle) {
            let mut temp_button = element.button.clone();
            temp_button.text = new_text;
            let text = temp_button.to_text3d();
            Some((self.create_text_buffers(device, &text), temp_button))
        } else {
            None
        };

        // Now update the element
        if let Some(((vertex_buffer, index_buffer, index_count), button)) = buffers {
            if let Some(element) = self.buttons.get_mut(&handle) {
                element.button = button;
                element.vertex_buffer = vertex_buffer;
                element.index_buffer = index_buffer;
                element.index_count = index_count;
            }
        }
    }

    /// Remove a button
    pub fn remove_button(&mut self, handle: UIElementHandle) {
        self.buttons.remove(&handle);
    }

    /// Raycast against all buttons to find which one was clicked
    /// Returns (handle, hit) if a button was hit
    pub fn raycast_buttons(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        camera_pos: Vec3,
    ) -> Option<(UIElementHandle, UIRaycastHit)> {
        let mut closest_hit: Option<(UIElementHandle, UIRaycastHit)> = None;
        let mut closest_distance = f32::MAX;

        for (handle, element) in &self.buttons {
            if !element.button.is_interactable() {
                continue;
            }

            let (pos, size) = element.button.bounds();
            if let Some(hit) = raycast_billboard_quad(ray_origin, ray_dir, pos, size, camera_pos) {
                if hit.distance < closest_distance {
                    closest_distance = hit.distance;
                    closest_hit = Some((*handle, hit));
                }
            }
        }

        closest_hit
    }

    /// Get button callback ID if it exists
    pub fn get_button_callback(&self, handle: UIElementHandle) -> Option<u32> {
        self.buttons.get(&handle).and_then(|e| e.button.callback_id)
    }

    /// Render buttons (called during render pass)
    fn render_buttons<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, camera_bind_group: &'a wgpu::BindGroup) {
        for element in self.buttons.values() {
            if !element.button.visible {
                continue;
            }

            render_pass.set_pipeline(self.text_renderer.pipeline(element.button.billboard));
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_bind_group(1, self.text_renderer.font_bind_group(), &[]);
            render_pass.set_vertex_buffer(0, element.vertex_buffer.slice(..));
            render_pass.set_index_buffer(element.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..element.index_count, 0, 0..1);
        }
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
