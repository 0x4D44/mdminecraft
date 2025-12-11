# mdminecraft

## Project Overview

**mdminecraft** is a production-ready, deterministic voxel sandbox engine built in Rust. It features server-authoritative multiplayer networking with client prediction, modern 3D rendering using wgpu, and a complete deterministic replay system.

### Key Technologies
*   **Language:** Rust
*   **Rendering:** wgpu (Vulkan/DirectX 12/Metal)
*   **ECS:** bevy_ecs (adapted for deterministic scheduling)
*   **Networking:** quinn (QUIC), postcard (serialization)
*   **Compression:** zstd (Region file persistence)
*   **Testing:** proptest (Property-based testing), custom Worldtest infrastructure

### Architecture

The project is structured as a workspace of modular crates:

*   **Application Layer:**
    *   `mdminecraft`: Main binary, game loop, menu, input handling.
    *   `crates/server`: Dedicated multiplayer server (authoritative simulation).
    *   `crates/client`: Multiplayer client (prediction, reconciliation).
*   **Presentation Layer:**
    *   `crates/render`: GPU rendering, chunk meshing, shader pipelines.
    *   `crates/ui3d`: 3D billboard and text system.
*   **Simulation Layer:**
    *   `crates/world`: World generation, chunk storage, lighting, persistence (.rg files).
    *   `crates/ecs`: Entity-Component-System wrapper.
    *   `crates/physics`: AABB collision detection, raycasting.
*   **Foundation Layer:**
    *   `crates/core`: Fundamental types (`SimTick`, `Voxel`, coordinates).
    *   `crates/net`: QUIC networking, protocol, prediction/reconciliation logic.
    *   `crates/assets`: Texture atlas, block registry.
*   **Tools:**
    *   `crates/testkit`: Testing infrastructure, metrics export.
    *   `crates/cli`: Command-line tools (`debug-world`, `metrics-diff`).
    *   `tools/atlas_packer`: Texture atlas generator.
    *   `tools/ecs_compare`: ECS benchmarking tool.

## Building and Running

### Prerequisites
*   Rust 1.75+
*   GPU with Vulkan/DirectX 12/Metal support

### Build Commands
```bash
# Development build
cargo build

# Release build (recommended for performance testing/play)
cargo build --release

# Build specific crate
cargo build --package mdminecraft-world
```

### Run Commands
```bash
# Run standalone client with auto-play (skips menu)
cargo run -- --auto-play

# Run dedicated server
cargo run --bin mdminecraft-server --release

# Run multiplayer client
cargo run --bin mdminecraft-client --release

# Run 3D viewer demo
cargo run --example viewer --package mdminecraft-render

# Run with scripted input (for automated demos/testing)
cargo run -- --auto-play --scripted-input config/scripts/demo.json
```

### Test Commands
```bash
# Run all tests
cargo test --all

# Run specific crate tests
cargo test --package mdminecraft-world

# Run Worldtests (Large-scale integration/performance tests)
cargo test --package mdminecraft-world --test '*worldtest' -- --nocapture

# Run specific Worldtest for performance metrics
cargo test --release --package mdminecraft-world --test large_scale_terrain_worldtest -- --nocapture
```

### Tool Commands
```bash
# Performance regression detection
cargo run --bin metrics-diff -- baseline.json current.json

# World generation debugging (heightmaps, biomes)
cargo run --bin debug-world -- heightmap --seed 12345 --region -2,-2,2,2
```

## Development Conventions

### Determinism
**CRITICAL:** The engine must remain 100% deterministic.
*   **Randomness:** Use custom deterministic PRNG seeded from `world_seed` + `position` + `SimTick`. NEVER use `rand::thread_rng()` or system entropy in simulation code.
*   **Iteration:** Use `BTreeMap` or sorted `Vec` for deterministic iteration. NEVER iterate over `HashMap` in simulation/generation code.
*   **Time:** Use `SimTick` (u64) for all simulation timing. NEVER use `std::time::SystemTime`.
*   **Floating Point:** Be cautious; avoid platform-specific math intrinsics.

### Code Style & Quality
*   **Formatting:** Run `cargo fmt --all` before committing.
*   **Linting:** Run `cargo clippy --all-targets --all-features`.
*   **Testing:**
    *   Write unit tests for new logic.
    *   Write property tests (`proptest`) for invariants (especially world gen/physics).
    *   Use `worldtest` for large-scale system validation.

### Performance
*   **Targets:** 60 FPS, <30ms chunk generation, <30ms reconciliation error.
*   **Metrics:** Performance metrics are exported to `target/metrics/*.json` during worldtests.
*   **Optimization:** Profile before optimizing. Maintain correctness/determinism.

### Networking
*   **Transport:** QUIC (via `quinn`).
*   **Channels:**
    *   `Input` (Unreliable Datagram): Player input.
    *   `EntityDelta` (Unreliable Datagram): Server state updates.
    *   `ChunkStream` (Reliable Ordered): Chunk data.
    *   `Chat` (Reliable Ordered): Chat/Commands.
*   **Protocol:** Use `postcard` for serialization.

## Key Documentation
*   `wrk_docs/2025.11.15 - DOC - Architecture Overview.md`: Comprehensive system design.
*   `wrk_docs/2025.11.15 - DOC - Worldtest Usage Guide.md`: Guide to running large-scale tests.
*   `CONTRIBUTING.md`: detailed contribution guidelines.
*   `CLAUDE.md`: Quick reference for AI agents.
