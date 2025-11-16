//! Main application entry point with 3D rendering.

use anyhow::Result;
use std::cell::Cell;
use std::sync::Arc;
use std::time::Instant;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use mdminecraft_camera::Camera;
use mdminecraft_input::InputState;
use mdminecraft_ui::UiState;
use mdminecraft_render::{mesh_chunk, Renderer, RendererConfig};
use mdminecraft_world::{Chunk, ChunkPos, Voxel, BLOCK_AIR};
use mdminecraft_assets::BlockRegistry;
use mdminecraft_physics::raycast_voxel;
use std::collections::HashMap;

/// Game state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameState {
    MainMenu,
    InGame,
    Paused,
}

/// Main application state.
struct App {
    renderer: Renderer,
    camera: Camera,
    input: InputState,
    last_frame: Instant,
    registry: BlockRegistry,

    // Game state
    state: GameState,

    // World data
    chunks: HashMap<ChunkPos, Chunk>,

    // Block interaction
    targeted_block: Option<([i32; 3], [i32; 3])>, // (block_pos, normal)
    selected_block: u16, // Block ID in hotbar (for placing)
    last_mouse_left: bool,  // Track previous frame state for click detection
    last_mouse_right: bool,

    // UI state
    ui_state: UiState,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,

    // Performance tracking
    frame_count: u32,
    fps_timer: Instant,
    last_fps: f32,

    // Camera speed
    current_speed: f32,
}

impl App {
    fn new(window: Arc<winit::window::Window>) -> Result<Self> {
        let size = window.inner_size();

        let config = RendererConfig {
            width: size.width,
            height: size.height,
            headless: false,
        };

        let mut renderer = Renderer::new(window.clone(), config);

        // Create a simple test world with a few chunks
        let registry = create_test_registry();
        let chunks = create_test_world(&mut renderer, &registry);

        // Position camera at ground level for first-person view
        // Terrain height is ~64-72, so place camera at y=72 (on top of terrain)
        let mut camera = Camera::new(glam::Vec3::new(0.0, 72.0, 0.0));
        camera.set_aspect(size.width, size.height);

        // Look forward (slightly down to see terrain ahead)
        camera.yaw = 0.0;  // Looking in +X direction
        camera.pitch = -0.1;  // Slight downward angle

        tracing::info!(
            "camera initialized at pos=({:.1}, {:.1}, {:.1}) yaw={:.2} pitch={:.2}",
            camera.position.x, camera.position.y, camera.position.z,
            camera.yaw, camera.pitch
        );

        // Initialize egui
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
        );

        Ok(Self {
            renderer,
            camera,
            input: InputState::new(),
            last_frame: Instant::now(),
            registry,
            state: GameState::MainMenu,  // Start in main menu
            chunks,
            targeted_block: None,
            selected_block: 2,  // Start with grass block
            last_mouse_left: false,
            last_mouse_right: false,
            ui_state: UiState::new(),
            egui_ctx,
            egui_state,
            frame_count: 0,
            fps_timer: Instant::now(),
            last_fps: 0.0,
            current_speed: 20.0,
        })
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Only update game logic when in-game
        if self.state != GameState::InGame {
            return;
        }

        // Base movement speed (blocks per second)
        let base_speed = 20.0;

        // Apply speed modifiers
        let speed_multiplier = if self.input.key_pressed(winit::keyboard::KeyCode::ShiftLeft) ||
                                   self.input.key_pressed(winit::keyboard::KeyCode::ShiftRight) {
            4.0 // Sprint mode (80 blocks/sec)
        } else if self.input.key_pressed(winit::keyboard::KeyCode::ControlLeft) ||
                  self.input.key_pressed(winit::keyboard::KeyCode::ControlRight) {
            0.25 // Slow mode (5 blocks/sec)
        } else {
            1.0 // Normal speed (20 blocks/sec)
        };

        let move_speed = base_speed * speed_multiplier * dt;
        let look_speed = 0.002; // radians per pixel

        // Track current speed for display
        self.current_speed = base_speed * speed_multiplier;

        // Handle mouse look
        let (mouse_dx, mouse_dy) = self.input.mouse_delta;
        if self.input.cursor_locked {
            self.camera.rotate(
                -(mouse_dx as f32) * look_speed,
                -(mouse_dy as f32) * look_speed,
            );
        }

        // Handle movement input
        let (forward, right) = self.input.movement_input();
        if forward != 0.0 {
            if forward > 0.0 {
                self.camera.move_forward(move_speed);
            } else {
                self.camera.move_backward(move_speed);
            }
        }
        if right != 0.0 {
            if right > 0.0 {
                self.camera.move_right(move_speed);
            } else {
                self.camera.move_left(move_speed);
            }
        }

        // Handle vertical movement
        let vertical = self.input.vertical_input();
        if vertical != 0.0 {
            if vertical > 0.0 {
                self.camera.move_up(move_speed);
            } else {
                self.camera.move_down(move_speed);
            }
        }

        // Handle W-axis movement (4D)
        // Q = backward in W, E = forward in W
        if self.input.key_pressed(winit::keyboard::KeyCode::KeyQ) {
            self.camera.move_w_backward(move_speed);
        }
        if self.input.key_pressed(winit::keyboard::KeyCode::KeyE) {
            self.camera.move_w_forward(move_speed);
        }

        // Perform raycast to find targeted block
        self.perform_raycast();

        // Handle hotbar selection (1-8 keys)
        self.handle_hotbar_input();

        // Detect mouse clicks (not holds)
        let mouse_left = self.input.mouse_button_pressed(winit::event::MouseButton::Left);
        let mouse_right = self.input.mouse_button_pressed(winit::event::MouseButton::Right);

        let left_click = mouse_left && !self.last_mouse_left;
        let right_click = mouse_right && !self.last_mouse_right;

        // Update previous state
        self.last_mouse_left = mouse_left;
        self.last_mouse_right = mouse_right;

        // Handle block breaking (left click)
        if left_click {
            self.break_block();
        }

        // Handle block placing (right click)
        if right_click {
            self.place_block();
        }
    }

    /// Query if a block at world coordinates (x, y, z) is solid.
    /// Uses camera's current W slice for the query.
    fn is_block_solid(&self, x: i32, y: i32, z: i32) -> bool {
        // Convert world coordinates to chunk position and local position
        let chunk_x = x.div_euclid(16);
        let chunk_z = z.div_euclid(16);
        let chunk_w = self.camera.w_slice();

        let local_x = x.rem_euclid(16) as usize;
        let local_y = y as usize;
        let local_z = z.rem_euclid(16) as usize;

        // Check bounds
        if local_y >= 256 {
            return false;
        }

        // Get the chunk
        let chunk_pos = ChunkPos::new_4d(chunk_x, chunk_z, chunk_w);
        if let Some(chunk) = self.chunks.get(&chunk_pos) {
            let voxel = chunk.voxel(local_x, local_y, local_z);
            voxel.is_opaque()
        } else {
            false
        }
    }

    /// Perform raycast from camera to find targeted block.
    fn perform_raycast(&mut self) {
        let origin = self.camera.position;
        let direction = self.camera.forward();
        let max_distance = 10.0; // Reach distance

        self.targeted_block = raycast_voxel(
            origin,
            direction,
            max_distance,
            |x, y, z| self.is_block_solid(x, y, z),
        ).map(|hit| (hit.block_pos, hit.normal));
    }

    /// Break the currently targeted block.
    fn break_block(&mut self) {
        if let Some((block_pos, _normal)) = self.targeted_block {
            let [x, y, z] = block_pos;

            // Convert to chunk position and local position
            let chunk_x = x.div_euclid(16);
            let chunk_z = z.div_euclid(16);
            let chunk_w = self.camera.w_slice();

            let local_x = x.rem_euclid(16) as usize;
            let local_y = y as usize;
            let local_z = z.rem_euclid(16) as usize;

            let chunk_pos = ChunkPos::new_4d(chunk_x, chunk_z, chunk_w);

            // Modify the chunk
            if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                // Set block to air
                chunk.set_voxel(local_x, local_y, local_z, Voxel {
                    id: BLOCK_AIR,
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                });

                // Regenerate mesh
                let mesh = mesh_chunk(chunk, &self.registry);
                self.renderer.upload_chunk_mesh(chunk_pos, &mesh);

                tracing::info!("broke block at ({}, {}, {}) in chunk {}", x, y, z, chunk_pos);
            }
        }
    }

    /// Place a block adjacent to the targeted block.
    fn place_block(&mut self) {
        if let Some((block_pos, normal)) = self.targeted_block {
            // Calculate placement position (adjacent to targeted block, in direction of normal)
            let [x, y, z] = block_pos;
            let [nx, ny, nz] = normal;

            let place_x = x + nx;
            let place_y = y + ny;
            let place_z = z + nz;

            // Don't place blocks where the player is standing
            let player_pos = self.camera.position;
            let player_block_x = player_pos.x.floor() as i32;
            let player_block_y = player_pos.y.floor() as i32;
            let player_block_z = player_pos.z.floor() as i32;

            // Check if placement would collide with player (check feet and head)
            if place_x == player_block_x && place_z == player_block_z &&
                (place_y == player_block_y || place_y == player_block_y + 1) {
                return; // Don't place block inside player
            }

            // Convert to chunk position and local position
            let chunk_x = place_x.div_euclid(16);
            let chunk_z = place_z.div_euclid(16);
            let chunk_w = self.camera.w_slice();

            let local_x = place_x.rem_euclid(16) as usize;
            let local_y = place_y as usize;
            let local_z = place_z.rem_euclid(16) as usize;

            // Check bounds
            if local_y >= 256 {
                return;
            }

            let chunk_pos = ChunkPos::new_4d(chunk_x, chunk_z, chunk_w);

            // Place the block
            if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                // Only place if target position is air
                let current = chunk.voxel(local_x, local_y, local_z);
                if current.id == BLOCK_AIR {
                    chunk.set_voxel(local_x, local_y, local_z, Voxel {
                        id: self.selected_block,
                        state: 0,
                        light_sky: 15,
                        light_block: 0,
                    });

                    // Regenerate mesh
                    let mesh = mesh_chunk(chunk, &self.registry);
                    self.renderer.upload_chunk_mesh(chunk_pos, &mesh);

                    tracing::info!("placed block {} at ({}, {}, {}) in chunk {}",
                                   self.selected_block, place_x, place_y, place_z, chunk_pos);
                }
            }
        }
    }

    /// Handle hotbar selection input (number keys 1-8).
    fn handle_hotbar_input(&mut self) {
        use winit::keyboard::KeyCode;

        // Map number keys to block IDs
        // 1=stone, 2=grass, 3=dirt, 4=sand, 5=wood, 6=leaves, 7=snow
        if self.input.key_just_pressed(KeyCode::Digit1) {
            self.selected_block = 1; // stone
            tracing::info!("selected block: stone");
        } else if self.input.key_just_pressed(KeyCode::Digit2) {
            self.selected_block = 2; // grass
            tracing::info!("selected block: grass");
        } else if self.input.key_just_pressed(KeyCode::Digit3) {
            self.selected_block = 3; // dirt
            tracing::info!("selected block: dirt");
        } else if self.input.key_just_pressed(KeyCode::Digit4) {
            self.selected_block = 4; // sand
            tracing::info!("selected block: sand");
        } else if self.input.key_just_pressed(KeyCode::Digit5) {
            self.selected_block = 5; // wood
            tracing::info!("selected block: wood");
        } else if self.input.key_just_pressed(KeyCode::Digit6) {
            self.selected_block = 6; // leaves
            tracing::info!("selected block: leaves");
        } else if self.input.key_just_pressed(KeyCode::Digit7) {
            self.selected_block = 7; // snow
            tracing::info!("selected block: snow");
        }
    }

    /// Track previous state to detect transitions
    fn check_state_transition(&mut self, window: &winit::window::Window, prev_state: GameState) {
        if prev_state != self.state {
            match self.state {
                GameState::InGame => {
                    // Entering game - lock cursor
                    window.set_cursor_visible(false);
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
                    self.input.cursor_locked = true;
                }
                GameState::MainMenu | GameState::Paused => {
                    // Entering menu - unlock cursor
                    window.set_cursor_visible(true);
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                    self.input.cursor_locked = false;
                }
            }
        }
    }

    /// Render main menu UI (returns new state if changed)
    fn render_main_menu(ctx: &egui::Context) -> Option<GameState> {
        let mut new_state = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 200)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);

                    // Title
                    ui.heading(egui::RichText::new("mdminecraft").size(64.0).color(egui::Color32::WHITE));
                    ui.label(egui::RichText::new("4D Voxel Engine").size(24.0).color(egui::Color32::LIGHT_GRAY));

                    ui.add_space(80.0);

                    // Menu buttons
                    if ui.add_sized([200.0, 50.0], egui::Button::new(
                        egui::RichText::new("New Game").size(20.0)
                    )).clicked() {
                        new_state = Some(GameState::InGame);
                    }

                    ui.add_space(10.0);

                    if ui.add_sized([200.0, 50.0], egui::Button::new(
                        egui::RichText::new("Settings").size(20.0)
                    )).clicked() {
                        // TODO: Settings menu
                    }

                    ui.add_space(10.0);

                    if ui.add_sized([200.0, 50.0], egui::Button::new(
                        egui::RichText::new("Quit").size(20.0)
                    )).clicked() {
                        std::process::exit(0);
                    }

                    ui.add_space(40.0);

                    // Footer info
                    ui.label(egui::RichText::new("Press ESC in-game to pause").size(14.0).color(egui::Color32::GRAY));
                });
            });
        new_state
    }

    /// Render pause menu UI (returns new state if changed)
    fn render_pause_menu(ctx: &egui::Context) -> Option<GameState> {
        let mut new_state = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 150)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(150.0);

                    // Title
                    ui.heading(egui::RichText::new("Paused").size(48.0).color(egui::Color32::WHITE));

                    ui.add_space(60.0);

                    // Menu buttons
                    if ui.add_sized([200.0, 50.0], egui::Button::new(
                        egui::RichText::new("Resume").size(20.0)
                    )).clicked() {
                        new_state = Some(GameState::InGame);
                    }

                    ui.add_space(10.0);

                    if ui.add_sized([200.0, 50.0], egui::Button::new(
                        egui::RichText::new("Settings").size(20.0)
                    )).clicked() {
                        // TODO: Settings menu
                    }

                    ui.add_space(10.0);

                    if ui.add_sized([200.0, 50.0], egui::Button::new(
                        egui::RichText::new("Main Menu").size(20.0)
                    )).clicked() {
                        new_state = Some(GameState::MainMenu);
                    }
                });
            });
        new_state
    }

    fn render(&mut self, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
        // Collect data needed for UI rendering
        let camera_w_slice = self.camera.w_slice();
        let render_stats = self.renderer.get_slice_stats(camera_w_slice);
        let camera_pos = self.camera.position.to_array();
        let camera_w = self.camera.w;
        let camera_yaw = self.camera.yaw;
        let camera_pitch = self.camera.pitch;
        let current_speed = self.current_speed;

        // Get biome-specific sky colors
        let biome = Biome::from_w(camera_w_slice);
        let sky_horizon = biome.sky_horizon();
        let sky_zenith = biome.sky_zenith();

        // Use Cell to capture state changes from within egui closure
        let requested_state = Cell::new(None);

        // Run egui based on game state
        let raw_input = self.egui_state.take_egui_input(window);
        let ui_state_ref = &mut self.ui_state;  // Borrow ui_state separately
        let current_state = self.state;

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            match current_state {
                GameState::MainMenu => {
                    if let Some(new_state) = Self::render_main_menu(ctx) {
                        requested_state.set(Some(new_state));
                    }
                }
                GameState::InGame => {
                    ui_state_ref.render(
                        ctx,
                        camera_pos,
                        camera_w,
                        camera_yaw,
                        camera_pitch,
                        current_speed,
                        render_stats,
                    );
                }
                GameState::Paused => {
                    if let Some(new_state) = Self::render_pause_menu(ctx) {
                        requested_state.set(Some(new_state));
                    }
                }
            }
        });

        // Apply state change if menu requested it
        if let Some(new_state) = requested_state.get() {
            self.state = new_state;
        }

        // Handle egui output
        self.egui_state.handle_platform_output(window, full_output.platform_output.clone());

        // Render 3D scene + UI (or just UI for menu)
        match self.state {
            GameState::MainMenu | GameState::Paused => {
                // Render just the UI for menus
                self.renderer.render_with_ui(&self.camera, sky_horizon, sky_zenith, Some((self.egui_ctx.clone(), full_output)))
            }
            GameState::InGame => {
                // Render full 3D scene + UI
                self.renderer.render_with_ui(&self.camera, sky_horizon, sky_zenith, Some((self.egui_ctx.clone(), full_output)))
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.camera.set_aspect(width, height);
    }

    fn handle_event(&mut self, event: &WindowEvent, window: &winit::window::Window) -> bool {
        // Let egui handle the event first
        let response = self.egui_state.on_window_event(window, event);

        // If egui consumed the event, don't pass to input
        if response.consumed {
            return true;
        }

        // Handle our input (only when in-game)
        if self.state == GameState::InGame {
            self.input.handle_event(event);
        }

        // Handle keyboard events based on state
        if let WindowEvent::KeyboardInput { .. } = event {
            match self.state {
                GameState::InGame => {
                    // F3 toggle debug panel
                    if self.input.key_just_pressed(winit::keyboard::KeyCode::F3) {
                        self.ui_state.toggle_debug();
                    }
                    // ESC to pause
                    if self.input.key_just_pressed(winit::keyboard::KeyCode::Escape) {
                        self.state = GameState::Paused;
                    }
                }
                GameState::Paused => {
                    // ESC to resume
                    if self.input.key_just_pressed(winit::keyboard::KeyCode::Escape) {
                        self.state = GameState::InGame;
                    }
                }
                GameState::MainMenu => {
                    // ESC to quit from main menu
                    if self.input.key_just_pressed(winit::keyboard::KeyCode::Escape) {
                        std::process::exit(0);
                    }
                }
            }
        }

        false
    }

    fn handle_device_event(&mut self, event: &winit::event::DeviceEvent) {
        // Only handle device events (mouse movement) when in-game
        if self.state == GameState::InGame {
            self.input.handle_device_event(event);
        }
    }

    /// Update FPS counter and return current FPS.
    fn update_fps(&mut self, frame_time_ms: f32) -> f32 {
        self.frame_count += 1;

        let elapsed = self.fps_timer.elapsed();
        if elapsed.as_secs_f32() >= 1.0 {
            self.last_fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.frame_count = 0;
            self.fps_timer = Instant::now();
        }

        // Update UI state with FPS and frame time
        self.ui_state.update_fps(self.last_fps, frame_time_ms);

        self.last_fps
    }

    /// Get debug info string for display.
    fn debug_info(&self) -> String {
        format!(
            "FPS: {:.0} | Pos: ({:.1}, {:.1}, {:.1}) | Chunks: 49",
            self.last_fps,
            self.camera.position.x,
            self.camera.position.y,
            self.camera.position.z
        )
    }
}

fn create_test_registry() -> BlockRegistry {
    use mdminecraft_assets::BlockDescriptor;

    BlockRegistry::new(vec![
        BlockDescriptor {
            name: "air".into(),
            opaque: false,
        },
        BlockDescriptor {
            name: "stone".into(),
            opaque: true,
        },
        BlockDescriptor {
            name: "grass".into(),
            opaque: true,
        },
        BlockDescriptor {
            name: "dirt".into(),
            opaque: true,
        },
        BlockDescriptor {
            name: "sand".into(),
            opaque: true,
        },
        BlockDescriptor {
            name: "wood".into(),
            opaque: true,
        },
        BlockDescriptor {
            name: "leaves".into(),
            opaque: true,
        },
        BlockDescriptor {
            name: "snow".into(),
            opaque: true,
        },
    ])
}

/// Biome types for different W slices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Biome {
    Desert,   // W=-2: Sandy, sparse vegetation
    Plains,   // W=-1: Grassy, moderate vegetation
    Forest,   // W=0:  Grassy, dense vegetation
    Taiga,    // W=1:  Grassy, dense vegetation
    Tundra,   // W=2:  Snowy, sparse vegetation
}

impl Biome {
    /// Select biome based on W coordinate
    fn from_w(w: i32) -> Self {
        match w {
            -2 => Biome::Desert,
            -1 => Biome::Plains,
            0 => Biome::Forest,
            1 => Biome::Taiga,
            2 => Biome::Tundra,
            _ => Biome::Forest, // Default to forest for any other W values
        }
    }

    /// Get surface block ID for this biome
    fn surface_block(&self) -> u16 {
        match self {
            Biome::Desert => 4,  // sand
            Biome::Plains => 2,  // grass
            Biome::Forest => 2,  // grass
            Biome::Taiga => 2,   // grass
            Biome::Tundra => 7,  // snow
        }
    }

    /// Get subsurface block ID for this biome (below surface layer)
    fn subsurface_block(&self) -> u16 {
        match self {
            Biome::Desert => 4,  // sand (deeper)
            Biome::Plains => 3,  // dirt
            Biome::Forest => 3,  // dirt
            Biome::Taiga => 3,   // dirt
            Biome::Tundra => 7,  // snow (deeper layers)
        }
    }

    /// Get tree spawn rate (0-100, percentage)
    fn tree_density(&self) -> u32 {
        match self {
            Biome::Desert => 1,   // 1% - very sparse
            Biome::Plains => 2,   // 2% - sparse
            Biome::Forest => 5,   // 5% - dense
            Biome::Taiga => 4,    // 4% - moderate-dense
            Biome::Tundra => 1,   // 1% - very sparse
        }
    }

    /// Get sky horizon color (RGB, 0.0-1.0 range)
    fn sky_horizon(&self) -> [f32; 3] {
        match self {
            Biome::Desert => [0.95, 0.85, 0.65],  // Warm sandy yellow
            Biome::Plains => [0.75, 0.88, 0.97],  // Light clear blue
            Biome::Forest => [0.70, 0.85, 0.95],  // Medium blue (default)
            Biome::Taiga => [0.65, 0.80, 0.93],   // Cool crisp blue
            Biome::Tundra => [0.85, 0.88, 0.90],  // Cold gray-white
        }
    }

    /// Get sky zenith color (RGB, 0.0-1.0 range)
    fn sky_zenith(&self) -> [f32; 3] {
        match self {
            Biome::Desert => [0.85, 0.70, 0.45],  // Deeper sandy orange
            Biome::Plains => [0.40, 0.60, 0.90],  // Bright blue
            Biome::Forest => [0.30, 0.50, 0.85],  // Deep blue (default)
            Biome::Taiga => [0.25, 0.45, 0.80],   // Deeper cool blue
            Biome::Tundra => [0.70, 0.73, 0.75],  // Overcast gray
        }
    }
}

/// 4D noise function for terrain generation.
/// Uses a combination of sine waves with W-based variation.
/// Each W slice generates unique terrain patterns.
fn terrain_height(world_x: i32, world_z: i32, world_w: i32) -> usize {
    let x = world_x as f32;
    let z = world_z as f32;
    let w = world_w as f32;

    // Use W to create phase shifts in the terrain generation
    // This makes each W slice have a completely different terrain pattern
    let w_offset1 = w * 100.0;  // Large offset for major terrain differences
    let w_offset2 = w * 50.0;   // Medium offset for variation
    let w_offset3 = w * 25.0;   // Small offset for detail

    // Multiple octaves of sine waves with W-based phase shifts
    let octave1 = ((x * 0.05 + w_offset1).sin() + (z * 0.05 + w_offset1).sin()) * 8.0;
    let octave2 = ((x * 0.1 + w_offset2).sin() + (z * 0.1 + w_offset2).cos()) * 4.0;
    let octave3 = ((x * 0.2 + z * 0.1 + w_offset3).sin()) * 2.0;

    let height = 68.0 + octave1 + octave2 + octave3;
    height.max(60.0).min(80.0) as usize
}

/// Simple hash function for deterministic random placement.
fn simple_hash(x: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374761393);
    h = h.wrapping_add((z as u32).wrapping_mul(668265263));
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    h
}

/// Check if a tree should spawn at this location (deterministic).
/// Spawn rate varies by biome.
fn should_spawn_tree(world_x: i32, world_z: i32, biome: Biome) -> bool {
    let hash = simple_hash(world_x, world_z);
    (hash % 100) < biome.tree_density()
}

/// Determine tree type based on position hash
fn tree_type(world_x: i32, world_z: i32) -> TreeType {
    let hash = simple_hash(world_x, world_z);
    if (hash % 100) < 40 {
        TreeType::Pine  // 40% pine
    } else {
        TreeType::Oak   // 60% oak
    }
}

#[derive(Debug, Clone, Copy)]
enum TreeType {
    Oak,
    Pine,
}

/// Place a tree at the given world coordinates in the chunk.
fn place_tree(chunk: &mut Chunk, x: usize, y: usize, z: usize, world_x: i32, world_z: i32) {
    let tree_type = tree_type(world_x, world_z);
    match tree_type {
        TreeType::Oak => place_oak_tree(chunk, x, y, z, world_x, world_z),
        TreeType::Pine => place_pine_tree(chunk, x, y, z, world_x, world_z),
    }
}

/// Place an oak tree (broad, round canopy)
fn place_oak_tree(chunk: &mut Chunk, x: usize, y: usize, z: usize, world_x: i32, world_z: i32) {
    use mdminecraft_world::Voxel;

    // Vary trunk height based on position
    let hash = simple_hash(world_x, world_z);
    let trunk_height = 4 + (hash % 3) as usize; // 4-6 blocks tall
    let max_leaf_radius = 2;

    // Place trunk (wood blocks)
    for dy in 0..trunk_height {
        if y + dy < 256 {
            chunk.set_voxel(
                x,
                y + dy,
                z,
                Voxel {
                    id: 5, // wood
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                },
            );
        }
    }

    // Place leaves (sphere-ish shape)
    let leaf_start = y + trunk_height - 1;
    for dy in 0..=3 {
        let y_pos = leaf_start + dy;
        if y_pos >= 256 {
            break;
        }

        let radius = if dy == 0 || dy == 3 { 1 } else { max_leaf_radius };

        for dx in -(radius as i32)..=(radius as i32) {
            for dz in -(radius as i32)..=(radius as i32) {
                let nx = x as i32 + dx;
                let nz = z as i32 + dz;

                // Check if within chunk bounds
                if nx >= 0 && nx < 16 && nz >= 0 && nz < 16 {
                    // Skip center column where trunk is (except top)
                    if !(dx == 0 && dz == 0 && dy < 3) {
                        chunk.set_voxel(
                            nx as usize,
                            y_pos,
                            nz as usize,
                            Voxel {
                                id: 6, // leaves
                                state: 0,
                                light_sky: 15,
                                light_block: 0,
                            },
                        );
                    }
                }
            }
        }
    }
}

/// Place a pine tree (tall, narrow, conical shape)
fn place_pine_tree(chunk: &mut Chunk, x: usize, y: usize, z: usize, world_x: i32, world_z: i32) {
    use mdminecraft_world::Voxel;

    // Pine trees are taller than oak
    let hash = simple_hash(world_x, world_z);
    let trunk_height = 6 + (hash % 4) as usize; // 6-9 blocks tall

    // Place trunk (wood blocks)
    for dy in 0..trunk_height {
        if y + dy < 256 {
            chunk.set_voxel(
                x,
                y + dy,
                z,
                Voxel {
                    id: 5, // wood
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                },
            );
        }
    }

    // Place conical leaves (narrow, triangular profile)
    let leaf_layers = 5;
    for dy in 0..leaf_layers {
        let y_pos = y + trunk_height - leaf_layers + dy;
        if y_pos >= 256 {
            break;
        }

        // Radius decreases as we go up (conical shape)
        let radius = if dy == 0 {
            2  // Bottom layer widest
        } else if dy < leaf_layers - 1 {
            1  // Middle layers
        } else {
            0  // Top is just center
        };

        for dx in -(radius as i32)..=(radius as i32) {
            for dz in -(radius as i32)..=(radius as i32) {
                let nx = x as i32 + dx;
                let nz = z as i32 + dz;

                // Check if within chunk bounds
                if nx >= 0 && nx < 16 && nz >= 0 && nz < 16 {
                    // Pine trees have leaves closer to trunk
                    if dx.abs() + dz.abs() <= radius {
                        chunk.set_voxel(
                            nx as usize,
                            y_pos,
                            nz as usize,
                            Voxel {
                                id: 6, // leaves
                                state: 0,
                                light_sky: 15,
                                light_block: 0,
                            },
                        );
                    }
                }
            }
        }
    }
}

fn create_test_world(renderer: &mut Renderer, registry: &BlockRegistry) -> HashMap<ChunkPos, Chunk> {
    use mdminecraft_world::Voxel;

    let mut chunks = HashMap::new();

    // Create a 7x7 grid of chunks across multiple W slices
    // W slices from -2 to +2 (5 total slices)
    for cw in -2..=2 {
        // Get biome for this W slice
        let biome = Biome::from_w(cw);

        for cx in -3..=3 {
            for cz in -3..=3 {
                let pos = ChunkPos::new_4d(cx, cz, cw);
                let mut chunk = Chunk::new(pos);

            // Fill bottom layers with blocks
            for x in 0usize..16 {
                for z in 0usize..16 {
                    // Calculate world coordinates
                    let world_x = x as i32 + cx * 16;
                    let world_z = z as i32 + cz * 16;

                    // Get terrain height from 4D noise function (includes W coordinate)
                    let height = terrain_height(world_x, world_z, cw);

                    for y in 0usize..height {
                        // Use biome-specific blocks
                        let block_id = if y == height - 1 {
                            biome.surface_block() // surface (grass/sand/snow)
                        } else if y > height - 4 {
                            biome.subsurface_block() // subsurface (dirt/sand/snow)
                        } else {
                            1 // stone (deep layers same for all biomes)
                        };

                        chunk.set_voxel(
                            x,
                            y,
                            z,
                            Voxel {
                                id: block_id,
                                state: 0,
                                light_sky: 15,
                                light_block: 0,
                            },
                        );
                    }

                    // Spawn trees on surface (rate varies by biome)
                    if should_spawn_tree(world_x, world_z, biome) {
                        // Place tree on top of terrain
                        if height < 250 {
                            // Ensure we have room for the tree
                            place_tree(&mut chunk, x, height, z, world_x, world_z);
                        }
                    }
                }
            }

                // Generate and upload mesh
                let mesh = mesh_chunk(&chunk, registry);
                renderer.upload_chunk_mesh(pos, &mesh);

                // Store chunk in HashMap
                chunks.insert(pos, chunk);
            }
        }
    }

    tracing::info!("created test world with 245 chunks (7×7 grid × 5 W slices) + trees");
    chunks
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    tracing::info!("mdminecraft starting...");

    // Create event loop and window
    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("mdminecraft - 3D Voxel Engine")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .build(&event_loop)?,
    );

    // Start with cursor visible (we're in main menu)
    window.set_cursor_visible(true);
    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);

    // Create app
    let mut app = App::new(window.clone())?;
    app.input.cursor_locked = false;

    tracing::info!("initialization complete, entering event loop");

    // Run event loop
    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, .. } => {
                app.handle_event(&event, &window);

                match event {
                    WindowEvent::CloseRequested => {
                        tracing::info!("close requested, shutting down");
                        target.exit();
                    }
                    WindowEvent::Resized(size) => {
                        app.resize(size.width, size.height);
                    }
                    WindowEvent::KeyboardInput { .. } => {
                        // State transitions are handled in handle_event()
                    }
                    WindowEvent::RedrawRequested => {
                        let frame_start = Instant::now();

                        // Track state for transition detection
                        let prev_state = app.state;

                        app.input.begin_frame();
                        app.update();

                        // Calculate frame time
                        let frame_time_ms = frame_start.elapsed().as_secs_f32() * 1000.0;

                        // Update FPS counter
                        app.update_fps(frame_time_ms);

                        // Update window title with debug info
                        window.set_title(&format!("mdminecraft | {}", app.debug_info()));

                        match app.render(&window) {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost) => app.resize(1280, 720),
                            Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                            Err(e) => tracing::error!("render error: {:?}", e),
                        }

                        // Check for state transitions and update cursor accordingly
                        app.check_state_transition(&window, prev_state);
                    }
                    _ => {}
                }
            }
            Event::DeviceEvent { event, .. } => {
                app.handle_device_event(&event);
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    })?;

    Ok(())
}

