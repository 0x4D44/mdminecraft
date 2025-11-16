# mdminecraft - 3D Voxel Engine

A complete 3D voxel rendering engine built with Rust and wgpu, featuring first-person controls, procedural terrain generation, and a professional debug UI.

## Features

### Rendering
- **wgpu 0.20** - Modern GPU rendering pipeline
- **WGSL shaders** - Custom vertex and fragment shaders
- **Greedy meshing** - Efficient geometry optimization
- **Depth testing** - Proper z-ordering
- **Distance fog** - Atmospheric depth perception (64-128 blocks)
- **Simple lighting** - Directional light with ambient

### World
- **7×7 chunk grid** - 112×112 block playable area (49 chunks)
- **Smooth terrain** - Multi-octave sine-wave based height map
- **Tree generation** - Deterministic placement (~3% spawn rate)
- **Height variation** - Rolling hills from 60-80 blocks
- **Block types** - Stone, dirt, grass, sand, wood, leaves

### Controls
- **First-person camera** - Full 6DOF movement
- **WASD** - Horizontal movement
- **Space** - Move up
- **Ctrl** - Move down (also slow mode)
- **Mouse** - Look around
- **Shift** - Sprint mode (4× speed, 80 blocks/sec)
- **Ctrl** - Slow mode (0.25× speed, 5 blocks/sec)
- **F3** - Toggle debug panel
- **ESC** - Exit

### UI
- **Crosshair** - Always-visible center reticle
- **F3 Debug Panel** - Comprehensive performance and world info
  - FPS stats (current, average, min, max)
  - Frame time (ms)
  - Camera position and rotation
  - Movement speed
  - Render statistics (chunks, triangles, indices)
  - World size and render distance
  - Controls reference

## Building

### Prerequisites
- Rust 1.70+ (2021 edition)
- A GPU with Vulkan, Metal, or DirectX 12 support

### Build Release Binary
```bash
cargo build --release -p mdminecraft-app
```

The binary will be located at: `target/release/mdminecraft` (or `mdminecraft.exe` on Windows)

### Build Debug Binary (faster compile, slower runtime)
```bash
cargo build -p mdminecraft-app
```

## Running

```bash
# Run release build
./target/release/mdminecraft

# Or run directly with cargo
cargo run --release -p mdminecraft-app
```

### Requirements
- A display/window system (X11, Wayland, Windows, or macOS)
- OpenGL, Vulkan, Metal, or DirectX 12 capable GPU

### First Launch
On first launch, you'll see:
- A 112×112 block world with rolling terrain
- Trees scattered across the landscape
- A white crosshair in the center
- Camera positioned at ground level

Press **F3** to see the debug panel with detailed statistics.

## Performance

- **Target:** 60 FPS
- **World size:** 49 chunks (7×7 grid)
- **Typical triangle count:** 50,000-150,000 (depends on terrain)
- **Optimization:** Greedy meshing reduces geometry significantly

## Architecture

The application is built with a modular architecture:
- `mdminecraft-render` - Rendering pipeline and GPU management
- `mdminecraft-camera` - First-person camera system
- `mdminecraft-input` - Keyboard and mouse input handling
- `mdminecraft-ui` - egui-based UI overlays
- `mdminecraft-world` - Chunk and voxel data structures
- `mdminecraft-assets` - Block registry and definitions

## Development

### Enable Logging
```bash
RUST_LOG=info cargo run --release -p mdminecraft-app
```

Log levels: `error`, `warn`, `info`, `debug`, `trace`

### Debug Panel
Press **F3** to toggle the debug panel, which shows:
- Real-time FPS and frame time
- Camera position and orientation
- Current movement speed
- Chunks rendered
- Triangle and index counts
- World size and render distance
- Full controls reference

## Troubleshooting

### Low FPS
- Ensure you're running the release build (`--release`)
- Check GPU drivers are up to date
- Lower the world size if needed (edit `create_test_world()` in `main.rs`)

### Window doesn't open
- Ensure you have a display/window system running
- Check that your GPU supports modern graphics APIs
- Try running with `RUST_LOG=debug` to see detailed logs

### Controls not working
- Ensure the window has focus
- Mouse look requires cursor lock (happens automatically)
- Press ESC to exit cleanly

## License

MIT OR Apache-2.0 (same as workspace)

## Credits

Built as part of the mdminecraft project - a deterministic voxel sandbox engine.
