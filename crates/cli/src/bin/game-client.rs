//! Interactive game client with window, rendering, and input.
//!
//! This binary integrates all game systems into a playable client.

use anyhow::{Context, Result};
use mdminecraft_assets::{BlockDescriptor, BlockRegistry};
use mdminecraft_client::{
    camera::CameraController,
    game_loop::GameState,
    input::{InputEvent, InputState},
    player::PlayerController,
    window::{WindowConfig, WindowManager},
};
use mdminecraft_render::{Renderer, RendererConfig};
use mdminecraft_world::{ChunkPos, ChunkStorage, DirtyFlags, Voxel};
use tracing::Level;
use tracing_subscriber::fmt;
use winit::{
    event::{DeviceEvent, ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
};

/// Physics state for the game.
struct PhysicsState {
    storage: ChunkStorage,
    #[allow(dead_code)] // Will be used in later physics integration
    player: PlayerController,
    camera: CameraController,
    input: InputState,
}

impl PhysicsState {
    fn new() -> Self {
        // Create storage with test chunks
        let mut storage = ChunkStorage::new(64);

        // Create a simple ground plane
        let stone_id = 1;
        for cx in -2..=2 {
            for cz in -2..=2 {
                let pos = ChunkPos::new(cx, cz);
                let chunk = storage.ensure_chunk(pos);

                // Fill bottom layer with stone
                for x in 0..16 {
                    for z in 0..16 {
                        for y in 0..4 {
                            chunk.set_voxel(
                                x,
                                y,
                                z,
                                Voxel {
                                    id: stone_id,
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

        Self {
            storage,
            player: PlayerController::default(),
            camera: CameraController::default(),
            input: InputState::new(),
        }
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        // Get movement direction from input
        let (forward, right) = self.input.movement_direction();

        // Convert to world space using camera direction
        let camera_forward = self.camera.forward();
        let camera_right = self.camera.right();

        // Calculate movement vector
        let move_x = camera_forward[0] * forward + camera_right[0] * right;
        let move_z = camera_forward[2] * forward + camera_right[2] * right;

        // Apply movement to camera position
        let speed = 10.0 * _dt;
        self.camera.position[0] += move_x * speed;
        self.camera.position[2] += move_z * speed;

        // Handle vertical movement (jump/crouch)
        if self.input.jump {
            self.camera.position[1] += 5.0 * _dt;
        }
        if self.input.crouch {
            self.camera.position[1] -= 5.0 * _dt;
        }

        // Clamp Y position to stay above ground
        if self.camera.position[1] < 5.0 {
            self.camera.position[1] = 5.0;
        }

        Ok(())
    }
}

fn main() {
    // Initialize logging
    let _ = fmt().with_max_level(Level::INFO).try_init();
    tracing::info!("Starting MDMinecraft game client");

    if let Err(e) = run() {
        tracing::error!("Fatal error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {

    // Create block registry
    let registry = BlockRegistry::new(vec![
        BlockDescriptor {
            name: "air".into(),
            opaque: false,
        },
        BlockDescriptor {
            name: "stone".into(),
            opaque: true,
        },
    ]);

    // Create event loop and window
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let window_config = WindowConfig::default();
    let mut window_manager = WindowManager::new(&event_loop, window_config)
        .context("Failed to create window")?;

    let (width, height) = window_manager.size();
    tracing::info!("Window created: {}x{}", width, height);

    // Create renderer
    let renderer_config = RendererConfig {
        width,
        height,
        headless: false,
        ..Default::default()
    };

    let renderer = Renderer::new(renderer_config, &registry)
        .context("Failed to create renderer")?;

    // Upload initial chunks to renderer before we move it
    let mut temp_renderer = renderer;
    for pos in [-2, -1, 0, 1, 2].iter().flat_map(|&x| {
        [-2, -1, 0, 1, 2]
            .iter()
            .map(move |&z| ChunkPos::new(x, z))
    }) {
        let physics = PhysicsState::new();
        if let Some(chunk) = physics.storage.get(pos) {
            temp_renderer
                .update_chunk(chunk, DirtyFlags::MESH, &registry)
                .context("Failed to upload chunk")?;
        }
    }

    tracing::info!("Renderer initialized with {} chunks", temp_renderer.loaded_chunk_count());

    // Create physics state
    let physics = PhysicsState::new();

    // Create game state
    let mut game_state = GameState::new(physics, temp_renderer);

    // Clone the window Arc for surface creation
    let window_arc = window_manager.window().clone();

    // Create surface before event loop starts
    // SAFETY: window Arc is cloned and outlives the surface
    let surface = game_state.renderer.gpu().instance().create_surface(
        wgpu::SurfaceTarget::from(window_arc)
    ).context("Failed to create surface")?;

    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    surface.configure(game_state.renderer.gpu().device(), &surface_config);

    tracing::info!("Starting game loop");

    let mut cursor_grabbed = false;

    // Run event loop
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        // Grab cursor on first iteration
        if !cursor_grabbed {
            if let Err(e) = window_manager.grab_cursor() {
                tracing::error!("Failed to grab cursor: {}", e);
            } else {
                cursor_grabbed = true;
            }
        }

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    tracing::info!("Close requested, shutting down");
                    elwt.exit();
                }
                WindowEvent::Resized(new_size) => {
                    tracing::info!("Window resized: {}x{}", new_size.width, new_size.height);
                    let new_config = wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        width: new_size.width,
                        height: new_size.height,
                        present_mode: wgpu::PresentMode::Fifo,
                        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                        view_formats: vec![],
                        desired_maximum_frame_latency: 2,
                    };
                    surface.configure(game_state.renderer.gpu().device(), &new_config);
                }
                WindowEvent::KeyboardInput {
                    event: key_event,
                    ..
                } => {
                    let pressed = key_event.state == ElementState::Pressed;

                    if let PhysicalKey::Code(keycode) = key_event.physical_key {
                        // Handle ESC to release cursor
                        if keycode == KeyCode::Escape && pressed {
                            if cursor_grabbed {
                                if let Err(e) = window_manager.release_cursor() {
                                    tracing::error!("Failed to release cursor: {}", e);
                                } else {
                                    cursor_grabbed = false;
                                    tracing::info!("Cursor released");
                                }
                            } else {
                                if let Err(e) = window_manager.grab_cursor() {
                                    tracing::error!("Failed to grab cursor: {}", e);
                                } else {
                                    cursor_grabbed = true;
                                    tracing::info!("Cursor grabbed");
                                }
                            }
                        }

                        // Map keyboard input to game input events
                        let event = match keycode {
                            KeyCode::KeyW => Some(InputEvent::MoveForward(pressed)),
                            KeyCode::KeyS => Some(InputEvent::MoveBack(pressed)),
                            KeyCode::KeyA => Some(InputEvent::MoveLeft(pressed)),
                            KeyCode::KeyD => Some(InputEvent::MoveRight(pressed)),
                            KeyCode::Space => Some(InputEvent::Jump(pressed)),
                            KeyCode::ShiftLeft => Some(InputEvent::Crouch(pressed)),
                            KeyCode::ControlLeft => Some(InputEvent::Sprint(pressed)),
                            _ => None,
                        };

                        if let Some(event) = event {
                            game_state.physics.input.apply_event(event);
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    // Update game state
                    let result = game_state.update(
                        |physics, dt| physics.update(dt),
                        |renderer, physics| {
                            // Update camera
                            renderer.update_camera(physics.camera.get_view_matrix());

                            // Render frame
                            match surface.get_current_texture() {
                                Ok(frame) => {
                                    let view = frame
                                        .texture
                                        .create_view(&wgpu::TextureViewDescriptor::default());

                                    if let Err(e) = renderer.render_frame(&view) {
                                        tracing::error!("Render error: {}", e);
                                        return Err(e);
                                    }

                                    frame.present();
                                    Ok(())
                                }
                                Err(e) => {
                                    tracing::error!("Failed to get surface texture: {}", e);
                                    Err(e.into())
                                }
                            }
                        },
                    );

                    if let Err(e) = result {
                        tracing::error!("Game update error: {}", e);
                        elwt.exit();
                    }

                    // Frame pacing
                    game_state.sleep_for_frame_pacing();

                    // Request redraw for next frame
                    window_manager.request_redraw();
                }
                _ => {}
            },
            Event::DeviceEvent { event, .. } => {
                if let DeviceEvent::MouseMotion { delta } = event {
                    // Only process mouse motion if cursor is grabbed
                    if cursor_grabbed {
                        let (dx, dy) = delta;
                        game_state.physics.camera.process_mouse(dx as f32, dy as f32);
                    }
                }
            }
            Event::AboutToWait => {
                // Request a redraw to keep the game loop running
                window_manager.request_redraw();
            }
            _ => {}
        }
    })?;

    Ok(())
}
