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

        // Position camera above the test world
        let mut camera = Camera::new(glam::Vec3::new(8.0, 100.0, 8.0));
        camera.set_aspect(size.width, size.height);

        // Look down slightly
        camera.pitch = -0.3;

        Ok(Self {
            renderer,
            camera,
            input: InputState::new(),
            last_frame: Instant::now(),
            registry,
        })
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Movement speed
        let move_speed = 20.0 * dt; // blocks per second
        let look_speed = 0.002; // radians per pixel

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

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.renderer.render(&self.camera)
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.camera.set_aspect(width, height);
    }

    fn handle_event(&mut self, event: &WindowEvent) {
        self.input.handle_event(event);
    }

    fn handle_device_event(&mut self, event: &winit::event::DeviceEvent) {
        self.input.handle_device_event(event);
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

fn create_test_world(renderer: &mut Renderer, registry: &BlockRegistry) {
    use mdminecraft_world::Voxel;

    // Create a simple 3x3 grid of chunks with some terrain
    for cx in -1..=1 {
        for cz in -1..=1 {
            let pos = ChunkPos::new(cx, cz);
            let mut chunk = Chunk::new(pos);

            // Fill bottom layers with blocks
            for x in 0usize..16 {
                for z in 0usize..16 {
                    // Create a simple height map
                    let height = (64 + ((x as i32 + cx * 16) % 8) + ((z as i32 + cz * 16) % 8)) as usize;

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
                }
            }

            // Generate and upload mesh
            let mesh = mesh_chunk(&chunk, registry);
            renderer.upload_chunk_mesh(pos, &mesh);
        }
    }

    tracing::info!("created test world with 9 chunks");
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
                app.handle_event(&event);

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
                        app.input.begin_frame();
                        app.update();

                        match app.render() {
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

