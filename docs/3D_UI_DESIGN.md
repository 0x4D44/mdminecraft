# 3D UI Design and Implementation

## Overview

This document describes the 3D user interface implementation for mdminecraft, built entirely in Rust using modern GPU rendering techniques.

## Architecture

### Component Hierarchy

```
┌─────────────────────────────────────────────────┐
│         Application Layer                       │
│  ┌──────────────────────────────────────────┐   │
│  │  Window Manager (winit)                  │   │
│  │  - Event handling                        │   │
│  │  - Input state tracking                  │   │
│  └──────────────────────────────────────────┘   │
├─────────────────────────────────────────────────┤
│         Renderer Layer                          │
│  ┌──────────────┐  ┌──────────────────────┐    │
│  │   Camera     │  │  Render Pipeline     │    │
│  │   System     │  │  (WGSL Shaders)      │    │
│  └──────────────┘  └──────────────────────┘    │
├─────────────────────────────────────────────────┤
│         GPU Layer (wgpu)                        │
│  ┌──────────────┐  ┌──────────────────────┐    │
│  │   Surface    │  │  Device & Queue      │    │
│  └──────────────┘  └──────────────────────┘    │
├─────────────────────────────────────────────────┤
│         Data Layer                              │
│  ┌──────────────┐  ┌──────────────────────┐    │
│  │ Mesh         │  │  Chunk System        │    │
│  │ Generation   │  │  (Greedy Meshing)    │    │
│  └──────────────┘  └──────────────────────┘    │
└─────────────────────────────────────────────────┘
```

## Core Components

### 1. Window Management (`window.rs`)

**Purpose:** Manages the application window and input events.

**Key Features:**
- Cross-platform window creation via winit
- Input state tracking (keyboard, mouse)
- Cursor grab/release for FPS-style camera control
- Event loop management

**API:**
```rust
let window_manager = WindowManager::new(WindowConfig {
    title: "mdminecraft".to_string(),
    width: 1280,
    height: 720,
    vsync: true,
})?;
```

### 2. Camera System (`camera.rs`)

**Purpose:** First-person camera with perspective projection.

**Key Features:**
- Position and rotation in 3D space
- View and projection matrix generation
- FPS-style movement (WASD + mouse look)
- Pitch clamping to prevent gimbal lock

**Mathematics:**
- Field of view: 60 degrees (π/3 radians)
- Near/far clip planes: 0.1 to 1000.0
- Yaw/pitch Euler angles
- Right-handed coordinate system

**API:**
```rust
let mut camera = Camera::new(aspect_ratio);
camera.position = Vec3::new(0.0, 100.0, 0.0);
camera.rotate(yaw_delta, pitch_delta);
camera.translate(movement_vector);
```

### 3. GPU Pipeline (`pipeline.rs`)

**Purpose:** Manages GPU resources and rendering pipeline.

**Components:**

#### RenderContext
- wgpu Surface, Device, and Queue
- Surface configuration (resolution, format, VSync)
- Resource initialization and resizing

#### VoxelPipeline
- Shader compilation (WGSL)
- Render pipeline configuration
- Camera uniform buffer management
- Depth testing (reversed-Z)
- Backface culling

#### ChunkMeshBuffer
- Vertex/index buffer management
- GPU upload for chunk meshes

**Rendering Flow:**
```rust
// Initialize
let context = RenderContext::new(window).await?;
let pipeline = VoxelPipeline::new(&context)?;

// Per-frame
let frame = renderer.begin_frame()?;
pipeline.update_camera(&queue, &camera);

let mut render_pass = pipeline.begin_render_pass(&mut encoder, &frame.view);
render_pass.set_pipeline(pipeline.pipeline());
render_pass.set_bind_group(0, pipeline.camera_bind_group(), &[]);
render_pass.set_vertex_buffer(0, mesh_buffer.vertex_buffer.slice(..));
render_pass.set_index_buffer(mesh_buffer.index_buffer.slice(..), IndexFormat::Uint32);
render_pass.draw_indexed(0..mesh_buffer.index_count, 0, 0..1);

frame.present();
```

### 4. Voxel Shader (`shaders/voxel.wgsl`)

**Purpose:** WGSL shader for rendering voxel chunks.

**Vertex Shader:**
- Transforms positions to clip space
- Unpacks block ID and light from packed u32
- Passes normals and lighting data

**Fragment Shader:**
- Block-based color palette (10+ block types)
- Diffuse lighting from directional sun
- Ambient occlusion
- Voxel light integration (0-15 levels)

**Lighting Model:**
```
final_color = base_color * (ambient + diffuse + voxel_light)
  ambient = 0.3
  diffuse = max(dot(normal, sun_dir), 0.0) * 0.5
  voxel_light = light_level / 15.0 * 0.4
```

### 5. Mesh Generation (`mesh.rs`)

**Purpose:** Convert voxel chunks to GPU-ready meshes.

**Algorithm:** Greedy meshing
- Culls hidden faces between opaque blocks
- Merges adjacent coplanar faces
- 60-90% vertex reduction vs. naive meshing

**Vertex Layout (28 bytes):**
```rust
struct MeshVertex {
    position: [f32; 3],    // 12 bytes
    normal: [f32; 3],      // 12 bytes
    block_id: u16,         // 2 bytes
    light: u8,             // 1 byte
    _padding: u8,          // 1 byte (alignment)
}
```

## Renderer Public API

### Initialization

```rust
use mdminecraft_render::{Renderer, RendererConfig};

let mut renderer = Renderer::new(RendererConfig {
    width: 1280,
    height: 720,
    headless: false,
});

// Async GPU initialization
pollster::block_on(renderer.initialize_gpu(window))?;
```

### Per-Frame Rendering

```rust
// Update camera from input
update_camera(renderer.camera_mut(), &input, delta_time);

// Begin frame
if let Some(frame) = renderer.begin_frame() {
    let resources = renderer.render_resources().unwrap();

    let mut encoder = resources.device.create_command_encoder(&desc);
    {
        let mut pass = resources.pipeline.begin_render_pass(&mut encoder, &frame.view);
        // Draw calls here...
    }

    resources.queue.submit(std::iter::once(encoder.finish()));
    frame.present();
}
```

### Camera Controls

```rust
fn update_camera(camera: &mut Camera, input: &InputState, dt: f32) {
    // Mouse look
    if input.cursor_grabbed {
        camera.rotate(
            input.mouse_delta.0 as f32 * 0.002,
            -input.mouse_delta.1 as f32 * 0.002,
        );
    }

    // WASD movement
    let speed = 10.0 * dt;
    if input.is_key_pressed(KeyCode::KeyW) {
        camera.translate(camera.forward() * speed);
    }
    // ... other keys
}
```

## Demo Application

Run the included viewer example:

```bash
cargo run --example viewer --package mdminecraft-render
```

**Controls:**
- **WASD** - Move camera
- **Mouse** - Look around (when cursor grabbed)
- **Space** - Move up
- **Shift** - Move down
- **Tab** - Toggle cursor grab
- **Escape** - Exit

## Performance Characteristics

### Target Performance
- **Frame Rate:** 60 FPS
- **Resolution:** 1280×720 (default)
- **Render Distance:** Configurable (currently single chunk demo)

### Optimizations Implemented
1. **Greedy Meshing:** 60-90% fewer vertices
2. **Backface Culling:** 50% fewer fragments
3. **Depth Testing:** Early-Z rejection
4. **VSync:** Prevents tearing, caps at 60 FPS

### Future Optimizations
- Frustum culling for multi-chunk rendering
- LOD (Level of Detail) system
- Occlusion culling
- Chunk mesh caching (already implemented, needs GPU integration)

## Technical Specifications

### Dependencies
- **winit 0.29** - Cross-platform windowing
- **wgpu 0.19** - GPU abstraction (Vulkan/DirectX 12/Metal)
- **glam 0.25** - SIMD vector/matrix math
- **egui 0.26** - Immediate-mode GUI (prepared, not yet integrated)
- **pollster 0.3** - Async executor for GPU init

### Shader Language
- **WGSL** (WebGPU Shading Language)
- Compiled at runtime via wgpu/naga

### Coordinate Systems
- **World Space:** Right-handed, Y-up
- **Chunk Size:** 16×256×16 voxels
- **Clip Space:** Right-handed, reversed-Z

## Current Limitations

1. **Single Chunk Rendering:** Demo only renders one chunk
2. **No Texture Atlas:** Uses procedural colors
3. **No Entity Rendering:** Only voxel terrain
4. **No UI Overlay:** egui integration pending

## Roadmap

### Phase 1: Foundation ✅
- [x] Window management
- [x] Camera system
- [x] GPU pipeline
- [x] Basic shaders
- [x] Single chunk rendering

### Phase 2: Multi-Chunk (Next)
- [ ] Frustum culling
- [ ] Chunk streaming
- [ ] View distance configuration
- [ ] Mesh cache integration

### Phase 3: Visuals
- [ ] Texture atlas system
- [ ] Block texture mapping
- [ ] Improved lighting (shadows, AO)
- [ ] Skybox rendering

### Phase 4: UI & Polish
- [ ] egui integration
- [ ] Debug HUD (FPS, position, chunk stats)
- [ ] Settings menu
- [ ] Performance profiling overlay

### Phase 5: Advanced Features
- [ ] Entity rendering (mobs, items)
- [ ] Particle systems
- [ ] Transparent blocks (water, glass)
- [ ] Post-processing effects

## Development Notes

### Building
```bash
# Check compilation
cargo check --all

# Build everything
cargo build --all

# Run tests
cargo test --all

# Run viewer demo
cargo run --example viewer --package mdminecraft-render
```

### Code Organization
```
crates/render/
├── src/
│   ├── lib.rs           # Public API, Renderer struct
│   ├── camera.rs        # Camera math and controls
│   ├── window.rs        # Window & input management
│   ├── pipeline.rs      # GPU pipeline & resources
│   ├── mesh.rs          # Voxel meshing algorithm
│   ├── cache.rs         # Mesh caching (existing)
│   ├── driver.rs        # Mesh update driver (existing)
│   └── shaders/
│       └── voxel.wgsl   # Voxel rendering shader
├── examples/
│   └── viewer.rs        # Demo application
└── Cargo.toml
```

### Testing Strategy
- Unit tests for camera math
- Integration tests for mesh generation
- Visual tests via example applications
- Property tests for determinism (existing testkit)

## Troubleshooting

### Common Issues

**Black screen on startup:**
- Ensure GPU drivers are up to date
- Check that wgpu can find a suitable adapter
- Verify window is requesting correct surface format

**Low frame rate:**
- Check if VSync is enabled (intentionally caps at 60 FPS)
- Profile with `cargo flamegraph` to find bottlenecks
- Reduce render distance or chunk count

**Cursor not grabbing:**
- Some platforms require focus before cursor grab works
- Try clicking window before pressing Tab

**Shader compilation errors:**
- Ensure wgpu and naga versions match workspace
- Check WGSL syntax against spec

## References

- [wgpu Tutorial](https://sotrh.github.io/learn-wgpu/)
- [WGSL Specification](https://www.w3.org/TR/WGSL/)
- [winit Documentation](https://docs.rs/winit/)
- [glam Documentation](https://docs.rs/glam/)

## Credits

This 3D UI implementation was designed and built from scratch for mdminecraft, leveraging:
- Existing voxel meshing algorithms
- Modern GPU API best practices
- FPS game camera control conventions
