# mdminecraft

**A deterministic voxel sandbox engine built in Rust**

[![Tests](https://img.shields.io/badge/tests-159%20passing-brightgreen)]()
[![Determinism](https://img.shields.io/badge/determinism-100%25-brightgreen)]()
[![Performance](https://img.shields.io/badge/performance-6--166×%20faster-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()

---

## Overview

mdminecraft is a production-ready voxel sandbox engine featuring deterministic world generation, server-authoritative multiplayer networking with client prediction, and complete deterministic replay capability. Built entirely in Rust for safety, performance, and reliability.

**Current Status:** ✅ MVP Complete - Production Ready

### Key Features

- 🎨 **Modern 3D Rendering** - GPU-accelerated voxel rendering with wgpu (Vulkan/DirectX 12/Metal)
- 🌍 **Deterministic World Generation** - Same seed produces identical worlds every time
- 🌐 **QUIC-Based Multiplayer** - Low-latency networking with client prediction and reconciliation
- 🔄 **Complete Replay System** - Record and replay gameplay deterministically
- 🎮 **14 Biome Types** - Diverse terrain with seamless biome transitions
- 💾 **Efficient Persistence** - 498× compression ratio with region file format
- 🧪 **Comprehensive Testing** - 159 tests covering all major subsystems
- ⚡ **High Performance** - 6-166× faster than performance targets
- 📊 **Metrics Infrastructure** - Automated performance tracking and regression detection

---

## Quick Start

### Prerequisites

- Rust 1.75+ (with cargo)
- GPU with Vulkan/DirectX 12/Metal support (for rendering)

### Building

```bash
# Clone the repository
git clone https://github.com/0x4D44/mdminecraft.git
cd mdminecraft

# Build all crates
cargo build --release

# Run tests
cargo test --all
```

### Running

**TLS / Networking modes**
- Secure by default (client uses system roots). For local dev against self-signed servers, set `MDM_INSECURE_TLS=1` (unsafe for prod).
- Provide real certs for the server via `MDM_SERVER_CERT_PATH` and `MDM_SERVER_KEY_PATH` (PEM, PKCS8). Otherwise the server generates a self-signed dev cert.

```bash
# Start a local server
cargo run --bin mdminecraft-server --release

# Start a client (in another terminal)
cargo run --bin mdminecraft-client --release

# Run the 3D viewer demo (new!)
cargo run --example viewer --package mdminecraft-render

# Launch the standalone client directly into gameplay
cargo run -- --auto-play

# Launch with a scripted input sequence (useful for CI/headless demos)
cargo run -- --auto-play --scripted-input config/scripts/demo.json
```

The scripted-input format is a simple JSON file describing a list of timed steps (see `config/scripts/demo.json`). Each step specifies a duration along with movement/look deltas so you can automate smoke tests without manual input.

Additional samples live under `config/scripts/`:
- `demo.json` – gentle forward walk with a camera sweep.
- `rotation_demo.json` – pure camera rotation showcase.
- `walk_square.json` – walks a square path to stress movement transitions.

### Headless Automation Harness (screenshots + remote input)

Run a headless instance with a TCP automation server:

```bash
cargo run -- --headless --no-audio --no-save --world-seed 1 \
  --automation-listen 127.0.0.1:4242 --automation-step \
  --screenshot-dir target/harness/run1 --screenshot-every-ticks 1
```

The automation protocol is newline-delimited JSON (NDJSON). Example client using `nc`:

```bash
{ \
  echo '{"op":"hello","id":1,"version":1}'; \
  echo '{"op":"get_state","id":2}'; \
  echo '{"op":"step","id":3,"ticks":10}'; \
  echo '{"op":"screenshot","id":4,"tag":"overlook"}'; \
  echo '{"op":"shutdown","id":5}'; \
} | nc 127.0.0.1 4242
```

There is also a small reference client at `tools/harness_client/mdm_harness_client.py` for running NDJSON scripts.

Notes:
- `--automation-step` blocks until `step` requests (deterministic tick stepping).
- `set_view` expects yaw/pitch in radians (yaw=0 looks toward +X; positive pitch looks up; pitch is clamped to just under ±π/2).
- `--no-render` runs simulation without a GPU; `screenshot` returns `unsupported`.
- `--exit-when-script-finished` exits headless once `--command-script` completes.
- On unix, you can use `--automation-uds /path/to/socket` instead of `--automation-listen`.

**3D Viewer Controls:**
- **WASD** - Move camera
- **Mouse** - Look around (Tab to grab cursor)
- **Space/Shift** - Move up/down
- **Escape** - Exit

See [docs/3D_UI_DESIGN.md](docs/3D_UI_DESIGN.md) for full 3D rendering architecture details.

---

## Developer Tools

### Performance Regression Detection

**metrics-diff** - Automated performance comparison for CI/CD integration:

```bash
# Compare worldtest metrics
cargo run --bin metrics-diff -- baseline.json current.json

# With custom thresholds
cargo run --bin metrics-diff -- baseline.json current.json \
  --threshold-warning 0.05 --threshold-failure 0.10

# JSON output for automation
cargo run --bin metrics-diff -- baseline.json current.json --format json
```

See [Metrics Diff Tool Usage](wrk_docs/2025.11.15%20-%20DOC%20-%20Metrics%20Diff%20Tool%20Usage.md) for complete documentation.

### Save Migration

**save-upgrade** - Upgrade on-disk save data to the latest supported formats:

```bash
# Upgrade a save in place (creates a timestamped world.state.bak.* by default)
cargo run --bin save-upgrade -- --world saves/default

# Supply a seed when creating a missing world.meta
cargo run --bin save-upgrade -- --world saves/default --seed 12345

# Disable backups
cargo run --bin save-upgrade -- --world saves/default --no-backup
```

### World Generation Debugging

**debug-world** - Visual debugging and validation tools:

```bash
# Visualize heightmap (ASCII art)
cargo run --bin debug-world -- heightmap --seed 12345 --region -2,-2,2,2

# Display biome distribution
cargo run --bin debug-world -- biomes --seed 12345 --region -5,-5,5,5

# Validate chunk seams
cargo run --bin debug-world -- validate-seams --seed 12345 --region -10,-10,10,10
```

Features:
- Heightmap visualization (5 height levels: █ ▓ ▒ ░ ·)
- Biome map display (14 biome types)
- Seam validation (chunk boundary continuity)
- File export support

### Texture Atlas Packing (Phase 0 Tooling)

**atlas_packer** – packs authored textures into a runtime-ready atlas and emits JSON metadata for UV lookup:

```bash
# Build atlas.png + atlas.json from a directory of square textures
cargo run --package atlas_packer --bin atlas_packer -- \
  --input assets/textures/base \
  --output-image build/atlas.png \
  --output-meta build/atlas.json \
  --tile-size 32 --padding 2

# Resize mismatched textures automatically and limit atlas columns
cargo run -p atlas_packer -- \
  --input assets/textures/dev \
  --allow-mixed-sizes --columns 8
```

Metadata schema:

```json
{
  "tile_size": 32,
  "padding": 2,
  "columns": 8,
  "rows": 8,
  "atlas_width": 272,
  "atlas_height": 272,
  "entries": [
    { "name": "blocks/stone", "x": 2, "y": 2, "width": 32, "height": 32, "u0": 0.007, "v0": 0.007, "u1": 0.125, "v1": 0.125 }
  ]
}
```

Use this tool during Phase 1 rendering work to generate the authoritative atlas consumed by the runtime renderer.
Place the resulting `atlas.png`/`atlas.json` under `assets/atlas/` (or set `MDM_ATLAS_IMAGE` / `MDM_ATLAS_META`) so the client automatically loads them at startup. Missing files fall back to the color-coded debug atlas.

Block packs live in `config/blocks.json`. Each entry can specify a single `texture` or a `textures` object:

```json
[
  { "name": "air", "opaque": false },
  { "name": "stone", "opaque": true, "texture": "blocks/stone" },
  {
    "name": "grass",
    "opaque": true,
    "textures": {
      "top": "blocks/grass_top",
      "bottom": "blocks/dirt",
      "side": "blocks/grass_side"
    }
  }
]
```

These keys are resolved against the atlas metadata at load time; missing textures fall back to the debug gradient.

**Current texture sources (CC0):**
- [Assorted Minecraft-style textures](https://opengameart.org/content/assorted-minecraft-style-textures) by Joe Enderman — used for stone, dirt, sand, gravel, logs, planks, glass, and bedrock stand-ins.
- Solid-color CC0 tiles (generated locally) for water, ice, snow, and clay.
- All textures are repacked via `atlas_packer` into `assets/atlas/atlas.png` and referenced by name.

### ECS Benchmarking

**ecs_compare** – quick-and-dirty benchmark that compares `bevy_ecs` vs `hecs` for mdminecraft-style workloads (position + velocity updates). Helps validate Phase 0 ECS decisions.

```bash
# Compare runtimes for 50k entities over 200 ticks
cargo run --package ecs_compare -- --entities 50000 --ticks 200

# Custom seed / entity count
cargo run -p ecs_compare -- --entities 200000 --ticks 400 --seed 42
```

Output includes total duration and per-tick averages for each backend so we can track regressions when tweaking schedules/components.

---

## Project Structure

```
mdminecraft/
├── crates/
│   ├── core/          # Fundamental types (SimTick, Voxel, coordinates)
│   ├── world/         # World generation, chunks, persistence
│   ├── ecs/           # Entity-Component-System (bevy_ecs wrapper)
│   ├── physics/       # Collision detection, raycasting
│   ├── render/        # GPU rendering (wgpu)
│   ├── net/           # QUIC networking, protocol, prediction
│   ├── server/        # Dedicated server
│   ├── client/        # Game client
│   ├── testkit/       # Testing infrastructure, metrics
│   ├── assets/        # Asset loading and management
│   ├── scripting/     # Mod API (planned)
│   └── cli/           # Command-line tools
├── wrk_docs/          # Documentation (architecture, guides, plans)
├── wrk_journals/      # Development journals
└── README.md          # This file
```

---

## Documentation

### Getting Started

- **[Examples and FAQ](wrk_docs/2025.11.15%20-%20GUI%20-%20Examples%20and%20FAQ.md)** - Real-world examples, troubleshooting, and common questions
- **[Error Handling Best Practices](wrk_docs/2025.11.15%20-%20GUI%20-%20Error%20Handling%20Best%20Practices.md)** - Guidelines for contributors

### For Developers

- **[Architecture Overview](wrk_docs/2025.11.15%20-%20DOC%20-%20Architecture%20Overview.md)** - System design and technical details
- **[Worldtest Usage Guide](wrk_docs/2025.11.15%20-%20DOC%20-%20Worldtest%20Usage%20Guide.md)** - Running and writing large-scale tests
- **[Performance Baselines](wrk_docs/2025.11.15%20-%20BAS%20-%20Performance%20Baselines.md)** - Performance targets and regression thresholds

### Tool Documentation

- **[Metrics Diff Tool Usage](wrk_docs/2025.11.15%20-%20DOC%20-%20Metrics%20Diff%20Tool%20Usage.md)** - Performance regression detection for CI/CD

### Project Milestones

- **[Stage 5 Completion Summary](wrk_docs/2025.11.15%20-%20SUM%20-%20Stage%205%20Completion%20Summary.md)** - MVP completion
- **[Polish Sprint Complete](wrk_docs/2025.11.15%20-%20SUM%20-%20Polish%20Sprint%20Complete.md)** - Developer tools and documentation
- **[Project Status](wrk_docs/2025.11.15%20-%20SUM%20-%20Project%20Status%20and%20Next%20Steps.md)** - Current state and roadmap

### Planning Documents

- **[High-Level Design](wrk_docs/2025.11.12%20-%20HLD%20-%20Deterministic%20Voxel%20Sandbox.md)** - System requirements and design
- **[Implementation Plan](wrk_docs/2025.11.12%20-%20PLN%20-%20Deterministic%20Voxel%20Sandbox%20Implementation.md)** - Multi-stage development plan

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific test suite
cargo test --package mdminecraft-world

# Run worldtests (large-scale integration tests)
cargo test --package mdminecraft-world --test '*worldtest' -- --nocapture

# Run with release optimizations (for accurate performance metrics)
cargo test --release --package mdminecraft-world --test large_scale_terrain_worldtest -- --nocapture
```

### Test Coverage

| Test Type | Count | Coverage |
|-----------|-------|----------|
| Unit Tests | 117 | Core functionality |
| Property Tests | 37 | Invariant validation (25,600 test cases) |
| Worldtests | 5 | Large-scale integration |
| **Total** | **159** | **All subsystems** |

**Pass Rate:** 100% (0 flaky tests, 0 known bugs)

### Available Worldtests

1. **Large-Scale Terrain** - 2,601 chunks, seam continuity validation
2. **Persistence Round-Trip** - Save/load fidelity, compression testing
3. **Mob Lifecycle** - 80k mobs, 6k tick simulation
4. **Determinism Validation** - 18.9M voxels, perfect reproducibility
5. **Stage 4 Metrics** - Full system integration

See [Worldtest Usage Guide](wrk_docs/2025.11.15%20-%20DOC%20-%20Worldtest%20Usage%20Guide.md) for details.

---

## Performance

All performance targets exceeded with significant margin:

| System | Metric | Performance | Target | Margin |
|--------|--------|-------------|--------|--------|
| Terrain Generation | Avg time | 4.4ms/chunk | <30ms | **6.8× faster** |
| Mob Simulation | Per-mob | 0.016μs | <1μs | **63× faster** |
| Item Updates | Per-item | 0.007μs | <1μs | **147× faster** |
| Persistence | Compression | 498× | >3× | **166× better** |

**Quality Metrics:**
- Determinism: 100% (18.9M voxels verified)
- Data Fidelity: 100% (persistence round-trip)
- Seam Continuity: 100% (81,600 seams validated)

See [Performance Baselines](wrk_docs/2025.11.15%20-%20BAS%20-%20Performance%20Baselines.md) for complete benchmarks.

---

## Architecture

### Core Principles

1. **Determinism First** - Same seed + same inputs = same outputs, always
2. **Server Authority** - Server is source of truth, clients predict and reconcile
3. **Performance** - Target 60 FPS with hundreds of entities
4. **Modularity** - Clean crate boundaries with minimal dependencies
5. **Testability** - Headless worldtests validate systems at scale

### Key Systems

**World Generation:**
- Multi-octave Perlin noise with biome-based modulation
- 14 biome types with deterministic assignment
- Heightmap-based terrain with features (trees, ores)
- ~4-5ms per chunk generation time

**Networking:**
- QUIC transport (UDP-based with reliability)
- 5 channel types (Input, EntityDelta, ChunkStream, Chat, Diagnostics)
- Client prediction with server reconciliation
- ≤30ms reconciliation error at 100ms RTT

**Persistence:**
- Region file format (.rg) with 32×32 chunk regions
- zstd compression (498× ratio on typical chunks)
- CRC32 validation for data integrity
- Async I/O for non-blocking saves

See [Architecture Overview](wrk_docs/2025.11.15%20-%20DOC%20-%20Architecture%20Overview.md) for complete details.

---

## Development

### Building for Development

```bash
# Development build (faster compilation, includes debug info)
cargo build

# Release build (optimizations enabled, slower compilation)
cargo build --release

# Build specific crate
cargo build --package mdminecraft-world
```

### Code Quality

```bash
# Run linter
cargo clippy --all-targets --all-features

# Format code
cargo fmt --all

# Check without building
cargo check --all
```

### Metrics

Tests export performance metrics to `target/metrics/*.json`:

```bash
# Run a worldtest and view metrics
cargo test --package mdminecraft-world --test large_scale_terrain_worldtest -- --nocapture
cat target/metrics/large_scale_terrain_worldtest.json | jq .
```

---

## Roadmap

### Completed Stages (MVP)

- ✅ **Stage 0:** Foundations & Tooling
- ✅ **Stage 1:** Engine Core & World Primitives
- ✅ **Stage 2:** Lighting, Environmental Systems, Persistence
- ✅ **Stage 3:** Networking & Multiplayer
- ✅ **Stage 4:** Biomes, Structures, Environmental Content
- ✅ **Stage 5:** Hardening, CI Automation, Release Prep

### Future Work (Post-MVP)

**Stage 6: Content & Polish** (Potential)
- More biomes (jungle, savanna, mushroom, etc.)
- Cave and underground generation
- Structures (villages, dungeons)
- More mob types and behaviors
- Mod API and scripting support
- Advanced rendering features

**Stage 7: Advanced Features** (Long-term)
- Redstone-like logic system
- Advanced AI (pathfinding, combat)
- Distributed server architecture
- Cross-platform mobile support
- GPU terrain generation

See [Project Status](wrk_docs/2025.11.15%20-%20SUM%20-%20Project%20Status%20and%20Next%20Steps.md) for detailed next steps.

---

## Contributing

### Development Setup

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test --all`)
5. Ensure code is formatted (`cargo fmt --all`)
6. Ensure linter passes (`cargo clippy --all-targets`)
7. Commit your changes (`git commit -m 'Add amazing feature'`)
8. Push to the branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

### Guidelines

- Write tests for new functionality
- Maintain 100% determinism for world generation
- Document public APIs
- Follow existing code style
- Update documentation for significant changes

### Testing Your Changes

```bash
# Run all tests
cargo test --all

# Run property tests (validates invariants)
cargo test --package mdminecraft-world -- --ignored

# Run worldtests (validates at scale)
cargo test --package mdminecraft-world --test '*worldtest' -- --nocapture
```

---

## Technical Specifications

### System Requirements

**Minimum:**
- CPU: Dual-core 2.0 GHz
- RAM: 2 GB
- GPU: DirectX 11 / Vulkan 1.1 / Metal compatible
- Storage: 500 MB

**Recommended:**
- CPU: Quad-core 3.0 GHz+
- RAM: 4 GB+
- GPU: DirectX 12 / Vulkan 1.2 / Metal 2 compatible
- Storage: 1 GB

### Network Requirements

**Client:**
- Minimum: 128 kbps upload/download
- Recommended: 1 Mbps upload/download
- Latency: <200ms RTT for good experience

**Server:**
- Per player: ~50-100 kbps bandwidth
- 10 players: ~1 Mbps
- 100 players: ~10 Mbps

---

## License

MIT License - See LICENSE file for details

---

## Acknowledgments

- Built with [wgpu](https://wgpu.rs/) for cross-platform GPU rendering
- Networking powered by [quinn](https://github.com/quinn-rs/quinn) (QUIC implementation)
- ECS using [bevy_ecs](https://github.com/bevyengine/bevy)
- Compression via [zstd](https://github.com/facebook/zstd)
- Testing with [proptest](https://github.com/proptest-rs/proptest)

---

## Links

- **Documentation:** [wrk_docs/](wrk_docs/)
- **Architecture:** [Architecture Overview](wrk_docs/2025.11.15%20-%20DOC%20-%20Architecture%20Overview.md)
- **Issues:** [GitHub Issues](https://github.com/0x4D44/mdminecraft/issues)
- **Discussions:** [GitHub Discussions](https://github.com/0x4D44/mdminecraft/discussions)

---

## Status Badges

![Tests](https://img.shields.io/badge/tests-159%20passing-brightgreen)
![Coverage](https://img.shields.io/badge/coverage-all%20subsystems-brightgreen)
![Determinism](https://img.shields.io/badge/determinism-100%25-brightgreen)
![Performance](https://img.shields.io/badge/performance-6--166×%20faster-brightgreen)
![Quality](https://img.shields.io/badge/flaky%20tests-0-brightgreen)
![Bugs](https://img.shields.io/badge/known%20bugs-0-brightgreen)

**Production Ready:** ✅

---

*Last Updated: 2025-11-15*
*Version: 0.1.0-mvp*
