# mdminecraft - Ore Generation Guide

## 🪨 Ore Generation System

This guide documents the ore generation mechanics in mdminecraft's voxel world.

---

## 📊 Ore Types and Distribution

### Coal Ore (Block ID: 18)
**Appearance:** Black/dark gray speckles in stone
**Generation:**
- **Spawn Range**: Y levels 0-128
- **Spawn Chance**: 2.0% per stone block
- **Distribution**: Common, found throughout most underground areas
- **Typical Veins**: Individual blocks scattered naturally
- **Best Mining Levels**: Y 30-90 (abundant anywhere)

**Uses:**
- Primary fuel source for furnaces
- Burns longer than wood or sticks
- Essential for smelting ores into ingots
- Currently drops as coal ore block (future: coal item)

---

### Iron Ore (Block ID: 17)
**Appearance:** Brown/rust-colored speckles in stone
**Generation:**
- **Spawn Range**: Y levels 0-64
- **Spawn Chance**: 1.5% per stone block
- **Distribution**: Less common, restricted to lower elevations
- **Typical Veins**: Individual blocks with occasional clusters
- **Best Mining Levels**: Y 10-50 (peak around Y 32)

**Uses:**
- Smelts into Iron Ingots
- Crafts iron-tier tools (pickaxe, axe, sword, shovel)
- Essential mid-game progression material
- Requires stone pickaxe or better to mine
- Currently drops as iron ore block (future: raw iron)

---

## 🔍 How Ore Generation Works

### Generation Pipeline

```
1. Terrain Generation (stone, dirt, grass placement)
         ↓
2. **Ore Generation** (replace stone with ore)
         ↓
3. Cave Carving (expose ores in caves)
         ↓
4. Tree Population (surface decoration)
```

**Placement:** Ores are generated **before** cave carving, ensuring natural exposure in cave walls.

### Algorithm Details

**Hash-Based Deterministic Noise:**
```rust
fn ore_noise(x: i32, y: i32, z: i32, salt: u64) -> f32 {
    let seed = world_seed + salt;
    let hash = (x * 73856093 + y * 19349663 + z * 83492791 + seed) % 1000000;
    return (hash as f32) / 1000000.0;
}
```

**Key Features:**
- **Deterministic**: Same world seed produces identical ore distribution
- **Position-based**: Each block coordinate has consistent ore chance
- **Salt-separated**: Coal (salt=1000) and Iron (salt=2000) use different RNG streams
- **Fast**: Simple hash calculation, no expensive 3D Perlin noise

**Replacement Logic:**
- Only replaces `blocks::STONE` (ID 1)
- Preserves dirt, grass, sand, water, bedrock, etc.
- Evaluates each stone block independently
- No clustering algorithm (future enhancement)

---

## 📏 Spawn Statistics

### Expected Ore Counts per Chunk

**Chunk Size:** 16×256×16 = 65,536 blocks

**Coal Ore (Y 0-128):**
- Stone blocks in range: ~20,000 (varies by terrain)
- Expected coal ore: 20,000 × 0.02 = **~400 coal ore blocks**
- Actual visible (after caves): ~250-300 blocks

**Iron Ore (Y 0-64):**
- Stone blocks in range: ~10,000 (varies by terrain)
- Expected iron ore: 10,000 × 0.015 = **~150 iron ore blocks**
- Actual visible (after caves): ~100-120 blocks

**Note:** Caves remove ~30-40% of underground blocks, making remaining ores easier to find.

---

## ⛏️ Mining Recommendations

### Optimal Mining Strategies

**For Coal:**
```
Depth: Y 40-80 (anywhere is fine)
Strategy: Branch mining or cave exploration
Abundance: Very common, don't need to mine specifically for it
Tool Required: Any pickaxe (wood or better)
```

**For Iron:**
```
Depth: Y 10-50 (optimal around Y 32)
Strategy: Strip mining at Y 32 for maximum efficiency
Pattern: 2-block tall tunnels, 3 blocks apart
Tool Required: Stone pickaxe or better
```

### Strip Mining Layout
```
Y=32 Level (Top-Down View):

[T]====[T]====[T]====[T]
 |      |      |      |
 |      |      |      |
[T]====[T]====[T]====[T]

T = Tunnel entrance
= = Mined tunnel (2 blocks tall)
| = Connecting passages (optional)

Spacing: 3 blocks between tunnels
Length: 20-30 blocks per tunnel
Height: 2 blocks (can see floor and ceiling ores)
```

---

## 🎮 Finding Your First Ores

### Starting Out

1. **Craft a Pickaxe**
   - Wood Pickaxe: Can mine stone and coal
   - Stone Pickaxe: Required for iron ore

2. **Find a Cave**
   - Caves naturally expose ore veins
   - Look for brown (iron) or black (coal) speckles in stone walls
   - Explore different Y levels to find iron below Y=64

3. **Start Mining**
   - Break ore blocks with appropriate pickaxe
   - Currently: Ores drop as ore blocks (can be placed)
   - Future: Will drop ore items for furnace smelting

4. **Check Your Depth**
   - Press F3 to show debug HUD
   - Your Y coordinate is displayed
   - Iron only spawns below Y=64

---

## 🔬 Technical Details

### Block ID Mapping

| Block Type | ID | Used In |
|------------|-----|---------|
| Stone | 1 | Terrain generation |
| Coal Ore | 18 | Ore generation |
| Iron Ore | 17 | Ore generation |
| Bedrock | 10 | Bottom layer (Y 0-5) |

### Generation Performance

**Per-Chunk Cost:**
- Ore evaluation: ~65,536 checks
- Stone blocks only: ~30,000 checks (typical)
- Ore placements: ~550 replacements (typical)
- **Time:** <1ms per chunk (negligible)

**World Generation Order:**
1. Heightmap calculation
2. Column filling (bedrock → stone → dirt → grass)
3. **Ore generation** ← New step
4. Cave carving
5. Tree placement

---

## 🚀 Future Enhancements

### Planned Improvements

**1. Ore Clustering Algorithm**
- Generate blob-shaped veins (3-10 blocks)
- More realistic distribution
- Better visual mining experience

**2. Additional Ore Types**
- Diamond Ore (Y 0-16, 0.5% chance, very rare)
- Gold Ore (Y 0-32, 1.0% chance)
- Redstone Ore (Y 0-16, 1.5% chance)
- Lapis Ore (Y 0-32, 0.8% chance)

**3. Ore Item Drops**
- Mining ore blocks drops ore items (not blocks)
- Stack in inventory up to 64
- Must be smelted in furnace to create ingots

**4. Fortune Enchantment**
- Higher chance for multiple drops
- Especially valuable for diamond and coal

**5. Biome-Specific Variants**
- Mesa: Extra gold ore
- Mountains: Extra emerald ore
- Ocean: No ores underwater

---

## 🐛 Known Limitations

### Current Implementation

**1. No Clustering**
- Ores spawn as individual blocks
- No natural vein formations
- Can feel scattered rather than clustered

**2. No Item Drops**
- Breaking ore block removes it completely
- No items added to inventory
- Must be fixed for full progression

**3. Fixed Spawn Rates**
- 2% coal, 1.5% iron are hardcoded
- No configuration options
- Can't adjust for difficulty

**4. Limited Variety**
- Only coal and iron ores
- No rare ores (diamond, gold, emerald)
- No decorative ores (lapis, redstone)

---

## 📖 Related Documentation

- **CRAFTING_RECIPES.md** - Furnace and iron tool recipes
- **PROJECT_SUMMARY.md** - Technical architecture
- **DEMO_GUIDE.md** - Complete gameplay guide

---

**Ore generation is now live! Explore caves and dig deep to find iron and coal ores throughout the underground world.**
