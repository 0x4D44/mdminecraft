//! Window management and event handling with winit.

use anyhow::Result;
use std::collections::HashSet;
use tracing::warn;
use winit::{
    event::{DeviceEvent, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::EventLoop,
    keyboard::KeyCode,
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
    pub fn new_with_event_loop(
        config: WindowConfig,
        event_loop: &winit::event_loop::EventLoopWindowTarget<()>,
    ) -> Result<Self> {
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
        let event_loop = self
            .event_loop
            .take()
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

/// Active input context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputContext {
    /// Menu UI owns input. Gameplay should pause.
    #[default]
    Menu,
    /// Gameplay (first-person control) owns input.
    Gameplay,
    /// HUD/overlay UI is focused but gameplay may still preview input.
    UiOverlay,
}

/// Snapshot of per-frame input data.
#[derive(Debug, Clone)]
pub struct InputSnapshot {
    /// Context at the time of snapshot.
    pub context: InputContext,
    /// Keys that were held when the snapshot was taken.
    pub keys_pressed: HashSet<KeyCode>,
    /// Keys that were pressed at least once during the frame.
    pub keys_just_pressed: HashSet<KeyCode>,
    /// Mouse buttons held at snapshot time.
    pub mouse_buttons: HashSet<MouseButton>,
    /// Mouse buttons clicked during the frame.
    pub mouse_clicks: HashSet<MouseButton>,
    /// Absolute cursor position.
    pub mouse_pos: (f64, f64),
    /// Window-relative cursor delta accumulated this frame.
    pub mouse_delta: (f64, f64),
    /// Raw device delta accumulated this frame.
    pub raw_mouse_delta: (f64, f64),
    /// Scroll delta accumulated this frame.
    pub scroll_delta: f32,
    /// Whether the cursor was captured/hidden.
    pub cursor_captured: bool,
}

/// Input state tracking.
#[derive(Debug, Clone)]
pub struct InputState {
    /// Keys currently pressed
    pub keys_pressed: HashSet<KeyCode>,
    /// Keys pressed this frame
    pub keys_just_pressed: HashSet<KeyCode>,
    /// Mouse position (x, y) in pixels
    pub mouse_pos: (f64, f64),
    /// Mouse delta since last frame (x, y)
    pub mouse_delta: (f64, f64),
    /// Raw mouse delta reported by DeviceEvents
    pub raw_mouse_delta: (f64, f64),
    /// Mouse buttons pressed
    pub mouse_buttons: HashSet<MouseButton>,
    /// Mouse buttons clicked this frame
    pub mouse_clicks: HashSet<MouseButton>,
    /// Scroll delta accumulated this frame
    pub scroll_delta: f32,
    /// Whether cursor is currently captured/hidden
    pub cursor_captured: bool,
    /// Whether the window currently has focus
    pub focused: bool,
    /// Whether cursor should be captured when focus is regained
    pub wants_cursor_capture: bool,
    /// Current context
    pub context: InputContext,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            keys_just_pressed: HashSet::new(),
            mouse_pos: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            raw_mouse_delta: (0.0, 0.0),
            mouse_buttons: HashSet::new(),
            mouse_clicks: HashSet::new(),
            scroll_delta: 0.0,
            cursor_captured: false,
            focused: true,
            wants_cursor_capture: false,
            context: InputContext::default(),
        }
    }
}

impl InputState {
    /// Create a new input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return an immutable snapshot and reset per-frame state.
    pub fn snapshot(&mut self) -> InputSnapshot {
        let snapshot = self.snapshot_view();
        self.reset_frame();
        snapshot
    }

    /// Peek at the current snapshot without resetting state.
    pub fn snapshot_view(&self) -> InputSnapshot {
        InputSnapshot {
            context: self.context,
            keys_pressed: self.keys_pressed.clone(),
            keys_just_pressed: self.keys_just_pressed.clone(),
            mouse_buttons: self.mouse_buttons.clone(),
            mouse_clicks: self.mouse_clicks.clone(),
            mouse_pos: self.mouse_pos,
            mouse_delta: self.mouse_delta,
            raw_mouse_delta: self.raw_mouse_delta,
            scroll_delta: self.scroll_delta,
            cursor_captured: self.cursor_captured,
        }
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
        self.raw_mouse_delta = (0.0, 0.0);
        self.scroll_delta = 0.0;
        self.mouse_clicks.clear();
        self.keys_just_pressed.clear();
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
                            self.keys_just_pressed.insert(keycode);
                        }
                        ElementState::Released => {
                            self.keys_pressed.remove(&keycode);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    self.mouse_buttons.insert(*button);
                    self.mouse_clicks.insert(*button);
                }
                ElementState::Released => {
                    self.mouse_buttons.remove(button);
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = (position.x, position.y);
                let delta = (new_pos.0 - self.mouse_pos.0, new_pos.1 - self.mouse_pos.1);
                self.mouse_pos = new_pos;

                // Accumulate delta; caller decides if cursor is captured
                self.mouse_delta.0 += delta.0;
                self.mouse_delta.1 += delta.1;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match *delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y / 120.0) as f32,
                };
                self.scroll_delta += scroll;
            }
            WindowEvent::Focused(focused) => {
                self.focused = *focused;
                if !focused {
                    // Remember if we wanted cursor capture when we lost focus
                    self.wants_cursor_capture = self.cursor_captured;
                }
            }
            _ => {}
        }
    }

    /// Handle device-level events (raw mouse motion).
    pub fn handle_device_event(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.raw_mouse_delta.0 += delta.0;
            self.raw_mouse_delta.1 += delta.1;
        }
    }

    /// Set the current input context.
    pub fn set_context(&mut self, context: InputContext) {
        self.context = context;
    }

    /// Toggle cursor grab state.
    pub fn toggle_cursor_grab(&mut self, window: &Window) -> Result<()> {
        let capture = !self.cursor_captured;
        self.set_cursor_capture(window, capture)
    }

    /// Explicitly set cursor capture state.
    pub fn set_cursor_capture(&mut self, window: &Window, capture: bool) -> Result<()> {
        use winit::window::CursorGrabMode;

        self.wants_cursor_capture = capture;

        if capture {
            // On Linux/WSL2, Locked mode breaks DeviceEvent delivery and CursorMoved stops.
            // Use Confined mode which keeps CursorMoved events working.
            // On Windows/macOS, Locked mode works best.
            #[cfg(target_os = "linux")]
            let grab_result = window.set_cursor_grab(CursorGrabMode::Confined);

            #[cfg(not(target_os = "linux"))]
            let grab_result = {
                let locked = window.set_cursor_grab(CursorGrabMode::Locked);
                if locked.is_err() {
                    window.set_cursor_grab(CursorGrabMode::Confined)
                } else {
                    locked
                }
            };

            if let Err(err) = grab_result {
                warn!("Failed to capture cursor: {err}");
                self.cursor_captured = false;
                return Ok(());
            }
            window.set_cursor_visible(false);
            self.cursor_captured = true;
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                warn!("Failed to release cursor grab: {err}");
            }
            window.set_cursor_visible(true);
            self.cursor_captured = false;
        }

        Ok(())
    }

    /// Handle focus changes - call this when focus is regained to recapture cursor if needed.
    pub fn handle_focus_regained(&mut self, window: &Window) -> Result<()> {
        if self.wants_cursor_capture && !self.cursor_captured {
            self.set_cursor_capture(window, true)?;
        }
        Ok(())
    }

    /// Enter gameplay context (captures cursor and resets frame state).
    pub fn enter_gameplay(&mut self, window: &Window) -> Result<()> {
        self.context = InputContext::Gameplay;
        self.set_cursor_capture(window, true)?;
        self.reset_frame();
        Ok(())
    }

    /// Enter menu context (releases cursor and resets frame state).
    pub fn enter_menu(&mut self, window: &Window) -> Result<()> {
        self.context = InputContext::Menu;
        self.set_cursor_capture(window, false)?;
        self.reset_frame();
        Ok(())
    }

    /// Enter UI overlay context (releases cursor but keeps gameplay state).
    pub fn enter_ui_overlay(&mut self, window: &Window) -> Result<()> {
        self.context = InputContext::UiOverlay;
        self.set_cursor_capture(window, false)?;
        self.reset_frame();
        Ok(())
    }
}
