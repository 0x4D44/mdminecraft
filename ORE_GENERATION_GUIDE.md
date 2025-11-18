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
- Drops Raw Iron when mined
- Smelts into Iron Ingots in furnace
- Crafts iron-tier tools (pickaxe, axe, sword, shovel)
- Essential mid-game progression material
- Requires stone pickaxe or better to mine

---

### Diamond Ore (Block ID: 19)
**Appearance:** Cyan/light blue diamond crystals in stone
**Generation:**
- **Spawn Range**: Y levels 1-16 (very deep underground, near bedrock)
- **Spawn Chance**: 0.5% per stone block (very rare!)
- **Distribution**: Extremely rare, only in deepest layers
- **Typical Veins**: Individual blocks, widely scattered
- **Best Mining Levels**: Y 5-12 (safest above bedrock, below lava)

**Uses:**
- Drops Diamonds when mined (no smelting required!)
- Crafts diamond-tier tools (pickaxe, axe, sword, shovel)
- **Highest tier tools in game** (8× speed, 1561 durability, 7-9 damage)
- End-game progression material
- **Requires iron pickaxe or better to mine**
- Cannot be mined with stone tools (ore breaks, no drop)

**Rarity:**
- Approximately **1 diamond ore per chunk** in Y 1-16 range
- Much rarer than iron or coal
- Valuable and precious resource
- Strip mining at Y 11 recommended for optimal discovery

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
- **Salt-separated**: Coal (salt=1000), Iron (salt=2000), and Diamond (salt=3000) use different RNG streams
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

**Diamond Ore (Y 1-16):**
- Stone blocks in range: ~2,500 (varies by terrain)
- Expected diamond ore: 2,500 × 0.005 = **~12-13 diamond ore blocks**
- Actual visible (after caves): ~8-10 blocks
- **Very rare**: Only ~1 diamond per chunk on average

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

**For Diamond:**
```
Depth: Y 5-12 (optimal around Y 11)
Strategy: Strip mining at Y 11 (safest level above lava)
Pattern: 2-block tall tunnels, 3 blocks apart (same as iron)
Tool Required: Iron pickaxe or better (REQUIRED!)
Caution: Deep underground, near bedrock and lava lakes
```

**Diamond Mining Tips:**
- **Y 11 is safest**: Above most lava lakes but still in diamond range
- **Bring iron pickaxe**: Stone tools cannot harvest diamond ore!
- **Watch for lava**: Keep water bucket for emergencies
- **Be patient**: Diamonds are very rare (~1 per chunk)
- **Mark found diamonds**: Note coordinates for future reference
- **Strip mine systematically**: Cover large areas methodically

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
   - Wood Pickaxe: Can mine stone and coal ore
   - Stone Pickaxe: Required for iron ore
   - Iron Pickaxe: Required for diamond ore

2. **Find a Cave**
   - Caves naturally expose ore veins
   - Look for speckles in stone walls:
     * Black/dark gray = Coal Ore
     * Brown/rust = Iron Ore
     * Cyan/light blue = Diamond Ore (very rare!)
   - Explore different Y levels to find specific ores

3. **Start Mining**
   - Break ore blocks with appropriate pickaxe
   - Ores automatically drop items:
     * Coal Ore → Coal (Item 3)
     * Iron Ore → Raw Iron (Item 4) - needs smelting
     * Diamond Ore → Diamond (Item 5) - ready to use!
   - Items automatically added to hotbar

4. **Check Your Depth**
   - Press F3 to show debug HUD
   - Your Y coordinate is displayed
   - Coal: Y 0-128 (anywhere underground)
   - Iron: Y 0-64 (mid-depth)
   - Diamond: Y 1-16 (very deep, near bedrock!)

---

## 🔬 Technical Details

### Block ID Mapping

| Block Type | ID | Used In |
|------------|-----|---------|
| Stone | 1 | Terrain generation |
| Coal Ore | 18 | Ore generation |
| Iron Ore | 17 | Ore generation |
| Diamond Ore | 19 | Ore generation |
| Bedrock | 10 | Bottom layer (Y 0-5) |

### Generation Performance

**Per-Chunk Cost:**
- Ore evaluation: ~65,536 checks
- Stone blocks only: ~30,000 checks (typical)
- Ore placements: ~560 replacements (coal + iron + diamond)
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
- Gold Ore (Y 0-32, 1.0% chance)
- Redstone Ore (Y 0-16, 1.5% chance)
- Lapis Ore (Y 0-32, 0.8% chance)
- Emerald Ore (Y 0-32, 0.3% chance, mountain biomes only)

**3. Fortune Enchantment**
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

**2. Fixed Spawn Rates**
- 2% coal, 1.5% iron, 0.5% diamond are hardcoded
- No configuration options
- Can't adjust for difficulty

**3. Limited Variety**
- Only coal, iron, and diamond ores
- No other rare ores (gold, emerald)
- No decorative ores (lapis, redstone)

---

## 📖 Related Documentation

- **CRAFTING_RECIPES.md** - Furnace and iron tool recipes
- **PROJECT_SUMMARY.md** - Technical architecture
- **DEMO_GUIDE.md** - Complete gameplay guide

---

**Ore generation is now live! Explore caves and dig deep to find coal, iron, and diamond ores throughout the underground world. Complete tool progression: Wood → Stone → Iron → Diamond!**
