---
active: true
iteration: 1
max_iterations: 100
completion_promise: "GAMEPLAY INTEGRATED"
started_at: "2025-12-05T12:24:29Z"
---


## Mission: Integrate Core Gameplay Systems

mdminecraft has inventory, crafting, mobs, and dropped items implemented but NOT integrated into the game. Your job is to connect them.

### Success Criteria (ALL must pass):
1. Breaking blocks drops items (use existing drop_item.rs system)
2. Player can pick up dropped items into inventory
3. Inventory UI opens with E key (full 36-slot grid, not just hotbar)
4. Crafting table block exists and opens 3x3 crafting UI
5. Passive mobs spawn in world using existing mob.rs system
6. All tests pass: cargo test --all
7. Game runs without crashes: cargo run -- --auto-play

### Constraints:
- Use EXISTING systems in crates/world/src/{inventory,crafting,mob,drop_item}.rs
- Follow patterns in src/game.rs for UI integration
- Maintain determinism (use SimTick, seeded RNG)
- Small incremental commits after each feature

### Current State Check:
Run 'git log --oneline -3' and 'cargo test --all' to see progress.

When ALL criteria pass, output: <promise>GAMEPLAY INTEGRATED</promise>

