# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build
cargo build                          # Development build
cargo build --release                # Release build (for performance testing)

# Run tests
cargo test --all                     # All tests
cargo test --package mdminecraft-world  # Single crate
cargo test --package mdminecraft-world --test '*worldtest' -- --nocapture  # Worldtests (large-scale)
cargo test --release --package mdminecraft-world --test large_scale_terrain_worldtest -- --nocapture  # Performance metrics

# Code quality
cargo clippy --all-targets --all-features
cargo fmt --all

# Run the game
cargo run -- --auto-play             # Skip menu, enter game directly
cargo run --bin mdminecraft-server --release   # Dedicated server
cargo run --bin mdminecraft-client --release   # Multiplayer client
cargo run --example viewer --package mdminecraft-render  # 3D viewer demo
```

## Architecture Overview

**mdminecraft** is a deterministic voxel sandbox engine with server-authoritative multiplayer, client prediction, and GPU rendering.

### Crate Dependency Flow

```
Application Layer:
  mdminecraft (main binary) - game loop, menu, input handling
  mdminecraft-server        - dedicated multiplayer server
  mdminecraft-client        - multiplayer client with prediction

Presentation Layer:
  mdminecraft-render        - wgpu GPU rendering, mesh generation
  mdminecraft-ui3d          - 3D billboard/text system (optional feature)

Simulation Layer:
  mdminecraft-world         - chunks, terrain generation, lighting, persistence
  mdminecraft-ecs           - bevy_ecs wrapper for deterministic scheduling
  mdminecraft-physics       - AABB collision detection

Foundation Layer:
  mdminecraft-core          - SimTick, ItemStack, coordinates
  mdminecraft-net           - QUIC networking, protocol, prediction
  mdminecraft-assets        - texture atlas, block registry

Tools:
  mdminecraft-testkit       - metrics, event logging
  mdminecraft-cli           - debug-world, metrics-diff
  tools/atlas_packer        - texture atlas generation
  tools/ecs_compare         - ECS benchmarking
```

### Key Architectural Patterns

1. **Determinism**: All gameplay uses `SimTick` (u64) at 20 TPS. Same seed + inputs = same outputs. Scoped RNG seeded from `world_seed XOR chunk_hash XOR tick`.

2. **Chunk-Based World**: 16×256×16 voxel chunks. `Voxel` stores BlockId + BlockState + skylight + blocklight. SoA layout for efficient serialization.

3. **Multiplayer Model**: Server-authoritative with client prediction. `ClientPredictor` handles rollback/reconciliation. QUIC transport via quinn with postcard serialization.

4. **Rendering Pipeline**: `Renderer` owns VoxelPipeline, SkyboxPipeline, ParticlePipeline. `ChunkManager` handles mesh caching and frustum culling. `MeshVertex` = position + normal + texcoord + AO.

### Central Types

| Type | Location | Purpose |
|------|----------|---------|
| `SimTick` | core | Deterministic time unit (20 TPS) |
| `Voxel` | world | Block data (id, state, lighting) |
| `Chunk` / `ChunkPos` | world | 16×256×16 voxel array |
| `BlockRegistry` | assets | Block ID ↔ name/texture mapping |
| `ServerSnapshot` | net | Tick + entity deltas + chunk data |
| `InputBundle` | net | Player movement + actions |
| `GameWorld` | src/game.rs | Main game state (renderer, chunks, player) |

### Key File Locations

- **Game loop & state**: `src/game.rs` (GameWorld struct)
- **World generation**: `crates/world/src/terrain.rs`, `noise.rs`, `biome.rs`
- **Chunk storage**: `crates/world/src/chunk.rs`
- **Mesh generation**: `crates/render/src/mesh.rs`
- **Networking protocol**: `crates/net/src/protocol.rs`
- **Client prediction**: `crates/net/src/prediction.rs`
- **Block definitions**: `config/blocks.json`
- **Controls config**: `config/controls.toml`

## Environment Variables

- `MDM_INSECURE_TLS=1` - Disable TLS validation (dev only, for self-signed certs)
- `MDM_SERVER_CERT_PATH` / `MDM_SERVER_KEY_PATH` - Server TLS certificates (PEM, PKCS8)
- `MDM_ATLAS_IMAGE` / `MDM_ATLAS_META` - Custom texture atlas paths

## Testing Notes

- **Worldtests** (`*worldtest`) are large-scale integration tests that validate determinism and performance
- Metrics are written to `target/metrics/*.json` for CI regression detection
- Property tests use `proptest` for fuzzing invariants
- 100% determinism is required - all world generation must be reproducible from seed
