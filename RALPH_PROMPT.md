# Ralph Wiggum Prompt for mdminecraft Development

## Usage

```bash
/ralph-wiggum:ralph-loop "$(cat RALPH_PROMPT.md)" --max-iterations 50 --completion-promise "PHASE 5 COMPLETE AND ALL TESTS PASS"
```

---

## TASK: Complete Near-Minecraft Experience Implementation

You are continuing development of **mdminecraft**, a deterministic voxel sandbox engine written in Rust. The project has completed Phases 0-3 and most of Phase 4. Your task is to complete Phase 4 (Entities & Combat) and implement Phase 5 (Audio & UX Polish).

### JOURNAL REQUIREMENT

**Maintain a journal while you work.** Regularly update and save to disk in a `wrk_journals` folder under the repo root.

- Naming: `YYYY.MM.DD - JRN - <desc>.md`
- Log what you're working on, decisions made, problems encountered, and solutions applied
- Update at least every major milestone or significant code change
- Include timestamps for entries within the journal

### COMPLETION CRITERIA (ALL must be true to finish)

1. `cargo build` compiles with zero errors and zero warnings
2. `cargo clippy --all-targets --all-features` passes with zero warnings
3. `cargo fmt --all -- --check` passes (no formatting issues)
4. `cargo test --all` passes ALL tests
5. Phase 4 entities & combat is feature-complete (see checklist below)
6. Phase 5 audio & UX polish is feature-complete (see checklist below)

### CURRENT STATE

Check the current state by running:
- `cargo build 2>&1` - see compilation status
- `cargo test --all 2>&1` - see test status
- `cargo clippy --all-targets --all-features 2>&1` - see lint status
- `git status` - see what's been modified
- `git log --oneline -10` - see recent commits

Review existing documentation:
- `wrk_docs/2025.11.19 - PLN - Near Minecraft Experience.md` - Phase plan
- `wrk_docs/2025.11.26 - CR - Comprehensive Code Review.md` - Code review findings
- `CLAUDE.md` - Build commands and architecture overview

### WHAT'S ALREADY IMPLEMENTED

**Phase 0-3 (Complete):**
- Texture atlas system with per-face UVs
- Skylight & block light propagation
- Day/night cycle with sun/moon/clouds
- Weather system (rain/snow particles)
- Inventory, hotbar, crafting grid
- Furnace smelting system
- Hunger, health, fall damage
- Tool durability and mining speeds
- World persistence (region files)
- Chunk streaming

**Phase 4 (Mostly Complete):**
- Passive mobs: cow, pig, sheep, chicken (spawning, wandering, drops)
- Hostile mobs: zombie, skeleton, spider, creeper (spawning, basic AI)
- Player armor system (leather, iron, diamond)
- Bow & arrow projectiles with charging
- Basic melee combat

### PHASE 4 REMAINING TASKS

Review and complete any gaps:

1. **Mob AI Polish**
   - Hostile mob aggro and pathfinding toward player
   - Skeleton ranged attacks (shoot arrows)
   - Creeper explosion behavior
   - Spider climbing ability (optional)
   - Mob damage and knockback feedback

2. **Combat Feedback**
   - Damage numbers or hit indicators
   - Death animations or effects
   - Loot drops from hostile mobs

3. **Entity Performance**
   - Verify 500+ entities maintain 60 FPS
   - Entity despawn at distance
   - Spawn rate balancing

### PHASE 5 IMPLEMENTATION TASKS

1. **Audio Engine Integration**
   - Add audio crate dependency (recommend `kira` or `rodio`)
   - Create `AudioManager` for sound playback
   - Implement positional audio (3D sound)
   - Add volume controls (master, music, SFX, ambient)

2. **Sound Effects**
   - Block break/place sounds
   - Footstep sounds (vary by surface)
   - Tool swing sounds
   - Mob sounds (idle, hurt, death)
   - Ambient sounds (wind, cave drips)
   - Combat sounds (bow draw, arrow hit, damage)
   - UI sounds (inventory open/close, crafting success)

3. **Music System**
   - Background music with smooth crossfades
   - Context-aware music (day, night, combat, caves)
   - Music toggle in settings

4. **HUD Polish**
   - XP bar (visual only, XP not functional)
   - Status effect icons placeholder
   - Improved hotbar visuals
   - Better health/hunger/armor display
   - Crosshair improvements

5. **Menu Polish**
   - Main menu with animated background
   - Pause menu with resume/settings/quit
   - Settings menu (video, audio, controls)
   - World selection/creation UI
   - Death screen with respawn button (already exists, polish it)

6. **Accessibility**
   - Font scaling option
   - Colorblind mode placeholder
   - Remappable controls (already exists via config, add UI)

### CODE QUALITY REQUIREMENTS

- Follow existing code patterns in the codebase
- Add doc comments for public APIs
- Write unit tests for new functionality
- Keep functions focused and under 50 lines where practical
- Use `anyhow` for error handling with `.context()`
- No `unwrap()` or `expect()` in production paths
- Run `cargo fmt` and `cargo clippy` before committing

### IMPLEMENTATION ORDER (suggested)

1. **Audit Phase 4** - Review mob/combat code, identify gaps, fix issues
2. **Audio foundation** - Add audio crate, create AudioManager, basic playback
3. **Core SFX** - Block sounds, footsteps, combat sounds
4. **HUD improvements** - Polish existing UI elements
5. **Menu system** - Settings UI, improved pause menu
6. **Ambient audio** - Background music, environmental sounds
7. **Final polish** - Testing, bug fixes, performance verification

### RESOURCES

Existing audio assets can be sourced from:
- OpenGameArt.org (CC0/CC-BY)
- Kenney.nl (CC0)
- Freesound.org (various licenses - check each)

For placeholder sounds, simple synthesized tones are acceptable.

### COMMIT GUIDELINES

- Commit frequently with descriptive messages
- Format: `[Feature/Fix/Refactor] Brief description`
- Example: `[Feature] Add AudioManager with positional sound support`
- Run tests before committing

### KNOWN ISSUES TO ADDRESS

From the code review (2025.11.26):
- O(n) texture lookup in atlas - use HashMap (Major)
- Dead code warnings - clean up or annotate
- Magic numbers - extract to named constants
- Test cleanup - ensure temp files are removed

---

## SUCCESS METRICS

When complete, the game should:
- Have atmospheric audio that enhances immersion
- Display a polished HUD with clear health/hunger/armor indicators
- Provide smooth menu navigation with settings persistence
- Run at 60 FPS with 500+ entities and audio playing
- Pass all automated tests with zero warnings
