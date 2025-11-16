//! Main application entry point with 3D rendering.

use anyhow::Result;
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
use mdminecraft_world::{Chunk, ChunkPos};
use mdminecraft_assets::BlockRegistry;

/// Main application state.
struct App {
    renderer: Renderer,
    camera: Camera,
    input: InputState,
    last_frame: Instant,
    registry: BlockRegistry,

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
        create_test_world(&mut renderer, &registry);

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
    }

    fn render(&mut self, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
        // Get render statistics
        let render_stats = self.renderer.get_render_stats();

        // Run egui
        let raw_input = self.egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            self.ui_state.render(
                ctx,
                self.camera.position.to_array(),
                self.camera.yaw,
                self.camera.pitch,
                self.current_speed,
                render_stats,
            );
        });

        // Handle egui output
        self.egui_state.handle_platform_output(window, full_output.platform_output.clone());

        // Render 3D scene + UI
        self.renderer.render_with_ui(&self.camera, Some((self.egui_ctx.clone(), full_output)))
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

        // Handle our input
        self.input.handle_event(event);

        // Check for F3 toggle
        if let WindowEvent::KeyboardInput { .. } = event {
            if self.input.key_just_pressed(winit::keyboard::KeyCode::F3) {
                self.ui_state.toggle_debug();
            }
        }

        false
    }

    fn handle_device_event(&mut self, event: &winit::event::DeviceEvent) {
        self.input.handle_device_event(event);
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
    ])
}

/// Simple 2D noise function for terrain generation.
/// Uses a combination of sine waves for smooth, rolling hills.
fn terrain_height(world_x: i32, world_z: i32) -> usize {
    let x = world_x as f32;
    let z = world_z as f32;

    // Multiple octaves of sine waves for more natural terrain
    let octave1 = ((x * 0.05).sin() + (z * 0.05).sin()) * 8.0;
    let octave2 = ((x * 0.1).sin() + (z * 0.1).cos()) * 4.0;
    let octave3 = ((x * 0.2 + z * 0.1).sin()) * 2.0;

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
fn should_spawn_tree(world_x: i32, world_z: i32) -> bool {
    let hash = simple_hash(world_x, world_z);
    (hash % 100) < 3 // ~3% tree spawn rate
}

/// Place a simple tree at the given world coordinates in the chunk.
fn place_tree(chunk: &mut Chunk, x: usize, y: usize, z: usize) {
    use mdminecraft_world::Voxel;

    // Tree dimensions
    let trunk_height = 4;
    let leaf_radius = 2;

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

    // Place leaves (simple sphere-ish shape)
    let leaf_start = y + trunk_height - 1;
    for dy in 0..=3 {
        let y_pos = leaf_start + dy;
        if y_pos >= 256 {
            break;
        }

        let radius = if dy == 0 || dy == 3 { 1 } else { 2 };

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

fn create_test_world(renderer: &mut Renderer, registry: &BlockRegistry) {
    use mdminecraft_world::Voxel;

    // Create a 7x7 grid of chunks with smooth terrain
    for cx in -3..=3 {
        for cz in -3..=3 {
            let pos = ChunkPos::new(cx, cz);
            let mut chunk = Chunk::new(pos);

            // Fill bottom layers with blocks
            for x in 0usize..16 {
                for z in 0usize..16 {
                    // Calculate world coordinates
                    let world_x = x as i32 + cx * 16;
                    let world_z = z as i32 + cz * 16;

                    // Get terrain height from noise function
                    let height = terrain_height(world_x, world_z);

                    for y in 0usize..height {
                        let block_id = if y == height - 1 {
                            2 // grass
                        } else if y > height - 4 {
                            3 // dirt
                        } else {
                            1 // stone
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

                    // Spawn trees on surface
                    if should_spawn_tree(world_x, world_z) {
                        // Place tree on top of terrain
                        if height < 250 {
                            // Ensure we have room for the tree
                            place_tree(&mut chunk, x, height, z);
                        }
                    }
                }
            }

            // Generate and upload mesh
            let mesh = mesh_chunk(&chunk, registry);
            renderer.upload_chunk_mesh(pos, &mesh);
        }
    }

    tracing::info!("created test world with 49 chunks (7×7 grid) + trees");
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

    // Lock cursor for first-person camera
    window.set_cursor_visible(false);
    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Confined);

    // Create app
    let mut app = App::new(window.clone())?;
    app.input.cursor_locked = true;

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
                        // Check for ESC to exit
                        if app.input.key_just_pressed(winit::keyboard::KeyCode::Escape) {
                            tracing::info!("escape pressed, shutting down");
                            target.exit();
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let frame_start = Instant::now();

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

