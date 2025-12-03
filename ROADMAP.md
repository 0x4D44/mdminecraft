# mdminecraft Development Roadmap

This document outlines the systematic plan to improve and expand mdminecraft from its current MVP state to a feature-complete voxel sandbox engine.

## Overview

The roadmap is organized into 6 phases, ordered by dependency and priority:

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Technical Foundation | 🔄 In Progress |
| 2 | Multiplayer Completion | ⏳ Pending |
| 3 | Core Gameplay Features | ⏳ Pending |
| 4 | Rendering Enhancements | ⏳ Pending |
| 5 | World Generation Expansion | ⏳ Pending |
| 6 | Modding & Extensibility | ⏳ Pending |

---

## Phase 1: Technical Foundation

**Goal:** Solidify the codebase, reduce technical debt, and prepare for feature development.

### 1.1 Error Handling Audit
- [ ] Audit all `unwrap()` and `expect()` calls in production code
- [ ] Replace with proper `Result` handling or `anyhow` context
- [ ] Add graceful degradation for non-critical failures
- [ ] Create error types for each crate where appropriate

### 1.2 Documentation
- [ ] Generate and host cargo docs for public API
- [ ] Document the networking protocol specification
- [ ] Document the physics system
- [ ] Add architecture decision records (ADRs) for key decisions
- [ ] Expand CONTRIBUTING.md with development guidelines

### 1.3 Asset Pipeline
- [ ] Create default texture pack with actual textures
- [ ] Complete block→texture mappings in `config/blocks.json`
- [ ] Add texture pack validation tooling
- [ ] Document texture pack creation process

### 1.4 Complete Existing TODOs
- [ ] `crates/world/src/drop_item.rs:72` - Complete block→item mappings
- [ ] `crates/world/src/lighting.rs:180` - Queue cross-chunk light updates
- [ ] `crates/world/src/crafting.rs:56` - Implement crafting rollback
- [ ] `src/menu.rs:220` - Settings menu (basic implementation)
- [ ] `src/game.rs:1733` - Death screen implementation

### 1.5 Test Infrastructure
- [ ] Add benchmark suite for performance regression testing
- [ ] Add integration tests for the main game loop
- [ ] Add network protocol fuzz testing
- [ ] Set up code coverage reporting

---

## Phase 2: Multiplayer Completion

**Goal:** Complete the multiplayer system to enable networked gameplay.

### 2.1 Server Implementation
- [ ] Implement server tick loop in `mdminecraft-server`
- [ ] Add player connection handling and authentication
- [ ] Implement server-side world management
- [ ] Add player spawn point selection
- [ ] Implement server console commands

### 2.2 Client-Server Synchronization
- [ ] Fix chunk data application (`multiplayer.rs:246` TODO)
- [ ] Implement entity replication for mobs
- [ ] Add player position synchronization
- [ ] Implement block change propagation
- [ ] Add inventory synchronization

### 2.3 Prediction & Reconciliation
- [ ] Test and validate client prediction system
- [ ] Implement server reconciliation for desync
- [ ] Add latency compensation
- [ ] Implement input buffering

### 2.4 Network Robustness
- [ ] Add connection timeout handling
- [ ] Implement reconnection logic
- [ ] Add bandwidth throttling
- [ ] Implement chunk streaming priority (near player first)

### 2.5 Multiplayer Testing
- [ ] Create multiplayer integration tests
- [ ] Add network simulation (latency, packet loss)
- [ ] Test with multiple concurrent players
- [ ] Validate determinism across network

---

## Phase 3: Core Gameplay Features

**Goal:** Add essential gameplay mechanics for an engaging experience.

### 3.1 Combat System
- [ ] Add hostile mob types (Zombie, Skeleton, Spider, Creeper)
- [ ] Implement mob AI for hostility (detection, pathfinding, attack)
- [ ] Add player attack mechanics (melee, cooldown)
- [ ] Implement damage calculation and knockback
- [ ] Add death drops for mobs
- [ ] Implement mob spawning rules (light level, biome)

### 3.2 Tools & Equipment
- [ ] Implement tool tiers (wood, stone, iron, diamond)
- [ ] Add tool durability system
- [ ] Implement mining speed based on tool tier
- [ ] Add armor system with damage reduction
- [ ] Implement equipment slots (helmet, chest, legs, boots)

### 3.3 Full Inventory System
- [ ] Design and implement inventory UI (27 slots + hotbar)
- [ ] Add chest block with storage
- [ ] Implement item drag-and-drop
- [ ] Add shift-click quick transfer
- [ ] Implement stack splitting

### 3.4 Crafting System Completion
- [ ] Implement 3x3 crafting grid UI
- [ ] Add crafting table block
- [ ] Expand recipe definitions
- [ ] Add recipe book / discovery system
- [ ] Implement furnace with smelting

### 3.5 Additional Block Types
- [ ] Add door blocks (wood, iron) with open/close state
- [ ] Implement ladder blocks with climbing
- [ ] Add fence and fence gate blocks
- [ ] Implement slab and stair blocks
- [ ] Add torch block with light emission
- [ ] Implement bed with spawn point setting

### 3.6 Hunger & Food System
- [ ] Add hunger bar (20 points)
- [ ] Implement hunger depletion over time
- [ ] Add food items with saturation values
- [ ] Implement health regeneration when fed
- [ ] Add starvation damage when hungry

---

## Phase 4: Rendering Enhancements

**Goal:** Improve visual quality and add modern rendering features.

### 4.1 Shadow Mapping
- [ ] Implement cascaded shadow maps for sun/moon
- [ ] Add shadow filtering (PCF or VSM)
- [ ] Implement shadow distance culling
- [ ] Add shadow quality settings

### 4.2 Water Rendering
- [ ] Implement water transparency with sorting
- [ ] Add water surface animation
- [ ] Implement basic reflections (planar or SSR)
- [ ] Add underwater fog and tint
- [ ] Implement caustics effect

### 4.3 Block Animations
- [ ] Add animated texture support in atlas
- [ ] Implement flowing water/lava animation
- [ ] Add fire animation
- [ ] Implement foliage sway (grass, leaves)

### 4.4 Level of Detail (LOD)
- [ ] Implement chunk LOD system
- [ ] Add distance-based mesh simplification
- [ ] Implement impostor rendering for far chunks
- [ ] Add LOD transition blending

### 4.5 Post-Processing
- [ ] Add bloom effect for bright blocks
- [ ] Implement SSAO (Screen Space Ambient Occlusion)
- [ ] Add motion blur (optional)
- [ ] Implement color grading / tone mapping
- [ ] Add vignette effect

### 4.6 UI3D Completion
- [ ] Complete billboard rendering pipeline
- [ ] Implement proper SDF font rendering
- [ ] Add floating damage numbers
- [ ] Implement name tags for players/mobs
- [ ] Add 3D item rendering in hand

---

## Phase 5: World Generation Expansion

**Goal:** Create more diverse and interesting worlds.

### 5.1 Underground Biomes
- [ ] Add lush caves with glow berries
- [ ] Implement dripstone caves
- [ ] Add deep dark biome
- [ ] Implement underground lakes and rivers
- [ ] Add ore veins by depth

### 5.2 Surface Improvements
- [ ] Add mountain biome with peaks
- [ ] Implement river generation
- [ ] Add beach transitions
- [ ] Implement swamp biome
- [ ] Add jungle biome with large trees

### 5.3 Structures
- [ ] Implement structure placement system
- [ ] Add villages with buildings
- [ ] Implement dungeons with spawners
- [ ] Add mineshafts
- [ ] Implement temples (desert, jungle)
- [ ] Add strongholds

### 5.4 Weather Effects
- [ ] Implement snow accumulation on blocks
- [ ] Add rain effects on water surfaces
- [ ] Implement thunder and lightning
- [ ] Add weather persistence per chunk
- [ ] Implement biome-specific weather

### 5.5 Ocean Content
- [ ] Add ocean monuments
- [ ] Implement coral reefs
- [ ] Add kelp and seagrass
- [ ] Implement shipwrecks
- [ ] Add buried treasure

---

## Phase 6: Modding & Extensibility

**Goal:** Enable community content creation and customization.

### 6.1 WASM Scripting API
- [ ] Design mod API surface
- [ ] Implement WASM runtime integration (wasmtime)
- [ ] Create script context with world access
- [ ] Add event hooks (block break, mob spawn, etc.)
- [ ] Implement mod loading system

### 6.2 Data-Driven Content
- [ ] Add JSON schema validation for blocks
- [ ] Implement custom biome definitions
- [ ] Add custom mob definitions
- [ ] Implement custom recipe format
- [ ] Add custom structure definitions

### 6.3 Resource Pack System
- [ ] Implement resource pack loading
- [ ] Add texture override support
- [ ] Implement sound pack support
- [ ] Add language pack support
- [ ] Create resource pack documentation

### 6.4 Server Plugins
- [ ] Design server plugin API
- [ ] Implement plugin loading
- [ ] Add permission system
- [ ] Implement plugin configuration
- [ ] Create plugin development guide

### 6.5 Developer Tools
- [ ] Add in-game debug console
- [ ] Implement world editing tools
- [ ] Add structure editor
- [ ] Implement mob spawner tool
- [ ] Create development documentation

---

## Quality Gates

Each phase must meet these criteria before moving to the next:

1. **All tests pass** (cargo test --all)
2. **No clippy warnings** (cargo clippy --all-targets --all-features)
3. **Code formatted** (cargo fmt --all --check)
4. **Documentation updated** for new features
5. **Performance targets maintained** (no regression in worldtests)

---

## Progress Tracking

This roadmap will be updated as work progresses. Each completed item should be marked with:
- [x] Completed item
- Date of completion in commit message

Work journals should be maintained in `docs/work-journals/` for significant features.

---

*Last updated: 2024-12-03*
