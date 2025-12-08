# Ralph Wiggum Prompt: Texture Compositing Support

## Usage

```bash
/ralph-wiggum:ralph-loop "$(cat RALPH_TEXTURE_COMPOSITING.md)" --max-iterations 30 --completion-promise "Texture compositing is implemented and grass block sides render correctly with green grass on top and dirt below"
```

---

## TASK: Add Texture Compositing Support

You are adding texture compositing support to **mdminecraft**. Currently, textures with alpha transparency render with black where transparent pixels are.

### JOURNAL REQUIREMENT

Maintain a journal in `wrk_journals` folder. Name: `YYYY.MM.DD - JRN - Texture Compositing.md`

### THE PROBLEM

The grass_side.png texture has green grass on top and transparency below. It is designed as an overlay meant to be composited over the dirt texture. But the renderer shows black instead of dirt where pixels are transparent.

### COMPLETION CRITERIA

1. `cargo build` compiles with zero errors and zero warnings
2. `cargo clippy --all-targets --all-features` passes with zero warnings
3. `cargo fmt --all -- --check` passes
4. `cargo test --all` passes ALL tests
5. Grass block sides render correctly - green grass on top, dirt showing through below, NO BLACK AREAS

### IMPLEMENTATION APPROACH

The recommended approach is to pre-composite overlay textures during atlas generation:

1. Add a "base" field to BlockTextureConfig in crates/assets/src/lib.rs
2. Modify tools/atlas_packer to detect overlays and composite them onto base textures
3. Update config/blocks.json grass block to specify base texture for grass_side

Example blocks.json change:
```json
{
  "name": "grass",
  "opaque": true,
  "textures": {
    "top": "blocks/grass_top",
    "bottom": "blocks/dirt",
    "side": { "overlay": "blocks/grass_side", "base": "blocks/dirt" }
  }
}
```

Or simpler: just composite grass_side onto dirt during atlas packing.

### FILES TO EXAMINE

- tools/atlas_packer/src/main.rs - atlas generation code
- crates/assets/src/lib.rs - BlockTextureConfig struct
- crates/assets/src/registry.rs - texture resolution
- config/blocks.json - block definitions
- assets/textures/src/blocks/grass_side.png - overlay texture with transparency
- assets/textures/src/blocks/dirt.png - base texture
- assets/atlas/atlas.json - generated atlas metadata

### CURRENT TEXTURE LAYOUT

grass_side.png: 16x16 RGBA - green grass strip at top, transparent below
dirt.png: 16x16 - solid brown dirt texture

### ALTERNATIVE APPROACHES

If atlas compositing is complex:

1. Create a pre-composited grass_side_full.png manually
2. Modify the shader to sample two textures and blend by alpha
3. Add a preprocessing step that composites textures before atlas packing

Choose the simplest approach that works.
