//! Window management and event handling with winit.

use anyhow::Result;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

/// Window configuration.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window title
    pub title: String,
    /// Initial width
    pub width: u32,
    /// Initial height
    pub height: u32,
    /// Enable VSync
    pub vsync: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "mdminecraft".to_string(),
            width: 1280,
            height: 720,
            vsync: true,
        }
    }
}

/// Window manager wrapping winit.
pub struct WindowManager {
    window: std::sync::Arc<Window>,
    event_loop: Option<EventLoop<()>>,
}

impl WindowManager {
    /// Create a new window with the given configuration.
    pub fn new(config: WindowConfig) -> Result<Self> {
        let event_loop = EventLoop::new()?;

        let window = WindowBuilder::new()
            .with_title(config.title)
            .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
            .build(&event_loop)?;

        Ok(Self {
            window: std::sync::Arc::new(window),
            event_loop: Some(event_loop),
        })
    }

    /// Create a new window with an existing event loop.
    pub fn new_with_event_loop(config: WindowConfig, event_loop: &winit::event_loop::EventLoopWindowTarget<()>) -> Result<Self> {
        let window = WindowBuilder::new()
            .with_title(config.title)
            .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
            .build(event_loop)?;

        Ok(Self {
            window: std::sync::Arc::new(window),
            event_loop: None,
        })
    }

    /// Convert into just the window (consuming self).
    pub fn into_window(self) -> std::sync::Arc<Window> {
        self.window
    }

    /// Get Arc reference to the window.
    pub fn window(&self) -> std::sync::Arc<Window> {
        self.window.clone()
    }

    /// Get the current window size.
    pub fn size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width, size.height)
    }

    /// Run the event loop with a callback.
    ///
    /// The callback receives events and returns whether to continue running.
    pub fn run<F>(mut self, mut callback: F) -> Result<()>
    where
        F: FnMut(Event<()>, &Window) -> bool + 'static,
    {
        let event_loop = self.event_loop.take()
            .ok_or_else(|| anyhow::anyhow!("Event loop already consumed"))?;

        let window = self.window;

        event_loop.run(move |event, elwt| {
            let should_continue = callback(event, &window);

            if !should_continue {
                elwt.exit();
            }
        })?;

        Ok(())
    }
}

/// Input state tracking.
#[derive(Debug, Default, Clone)]
pub struct InputState {
    /// Keys currently pressed
    pub keys_pressed: std::collections::HashSet<winit::keyboard::KeyCode>,
    /// Mouse position (x, y) in pixels
    pub mouse_pos: (f64, f64),
    /// Mouse delta since last frame (x, y)
    pub mouse_delta: (f64, f64),
    /// Mouse buttons pressed
    pub mouse_buttons: std::collections::HashSet<winit::event::MouseButton>,
    /// Mouse buttons clicked this frame
    pub mouse_clicks: std::collections::HashSet<winit::event::MouseButton>,
    /// Whether cursor is grabbed
    pub cursor_grabbed: bool,
}

impl InputState {
    /// Create a new input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a key is currently pressed.
    pub fn is_key_pressed(&self, key: winit::keyboard::KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Check if a mouse button is currently pressed.
    pub fn is_mouse_pressed(&self, button: winit::event::MouseButton) -> bool {
        self.mouse_buttons.contains(&button)
    }

    /// Check if a mouse button was clicked this frame.
    pub fn is_mouse_clicked(&self, button: winit::event::MouseButton) -> bool {
        self.mouse_clicks.contains(&button)
    }

    /// Reset per-frame state (like mouse delta and clicks).
    pub fn reset_frame(&mut self) {
        self.mouse_delta = (0.0, 0.0);
        self.mouse_clicks.clear();
    }

    /// Handle a window event and update state.
    pub fn handle_event(&mut self, event: &WindowEvent) {
        use winit::event::ElementState;
        use winit::keyboard::PhysicalKey;

        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            self.keys_pressed.insert(keycode);
                        }
                        ElementState::Released => {
                            self.keys_pressed.remove(&keycode);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                match state {
                    ElementState::Pressed => {
                        self.mouse_buttons.insert(*button);
                        self.mouse_clicks.insert(*button);
                    }
                    ElementState::Released => {
                        self.mouse_buttons.remove(button);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = (position.x, position.y);
                let delta = (
                    new_pos.0 - self.mouse_pos.0,
                    new_pos.1 - self.mouse_pos.1,
                );
                self.mouse_pos = new_pos;

                // Only update delta if cursor is grabbed
                if self.cursor_grabbed {
                    self.mouse_delta = delta;
                }
            }
            _ => {}
        }
    }

    /// Toggle cursor grab state.
    pub fn toggle_cursor_grab(&mut self, window: &Window) -> Result<()> {
        use winit::window::CursorGrabMode;

        self.cursor_grabbed = !self.cursor_grabbed;

        if self.cursor_grabbed {
            window.set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))?;
            window.set_cursor_visible(false);
        } else {
            window.set_cursor_grab(CursorGrabMode::None)?;
            window.set_cursor_visible(true);
        }

        Ok(())
    }
}
