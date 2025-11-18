# mdminecraft - Crafting Recipes Guide

## 🔨 Complete Recipe Reference

This guide documents all available crafting recipes in mdminecraft's 3D crafting system.

---

## 📋 How Crafting Works

### Crafting Grid
- Press `C` to open the 3D Crafting Table
- Your **hotbar becomes the crafting grid** (9 slots arranged as 3×3)
- Slots are mapped as:
  ```
  [1] [2] [3]    (hotbar slots 0, 1, 2)
  [4] [5] [6]    (hotbar slots 3, 4, 5)
  [7] [8] [9]    (hotbar slots 6, 7, 8)
  ```

### Pattern Matching
- **Shapeless Positioning**: Recipes can be placed anywhere in the grid
- The system automatically detects valid patterns
- Unused items remain in your hotbar after crafting
- Required items are consumed automatically

### Crafting Process
1. Open crafting table (`C` key)
2. Arrange items in your hotbar to match a recipe pattern
3. Result preview appears on the right showing what you can craft
4. Aim at the **CRAFT** button and click
5. Crafted item appears in your hotbar
6. Required materials are consumed

---

## 🪵 Basic Resources

### Recipe #1: Planks from Wood
**Input:** 1 Wood
**Output:** 4 Planks
**Pattern:** Shapeless (place wood anywhere)

```
Wood anywhere in grid → 4 Planks
```

**Example:**
```
[ ] [ ] [ ]
[ ] [W] [ ]  →  4 Planks
[ ] [ ] [ ]
```

**Usage:**
- Most fundamental recipe
- Wood logs obtained by breaking trees
- Planks used for tools, sticks, and crafting table

---

### Recipe #2: Sticks from Planks
**Input:** 2 Planks (vertical)
**Output:** 4 Sticks
**Pattern:** 2 planks vertically adjacent

```
[P]           OR    [ ] [P] [ ]    OR    [ ] [ ] [P]
[P]                 [ ] [P] [ ]          [ ] [ ] [P]
[ ]                 [ ] [ ] [ ]          [ ] [ ] [ ]
```

**Examples:**
```
Slots 1-4:        Slots 2-5:        Slots 3-6:
[P] [ ] [ ]       [ ] [P] [ ]       [ ] [ ] [P]
[P] [ ] [ ]  OR   [ ] [P] [ ]  OR   [ ] [ ] [P]
[ ] [ ] [ ]       [ ] [ ] [ ]       [ ] [ ] [ ]
```

**Usage:**
- Essential crafting component
- Used for all tools
- 2 planks make 4 sticks (2:1 efficiency)

---

## ⛏️ Tools

### Recipe #3: Wood Pickaxe
**Input:** 3 Planks + 2 Sticks
**Output:** 1 Wood Pickaxe
**Pattern:** T-shape (3 planks on top, 2 sticks vertical below)

```
[P] [P] [P]
[ ] [S] [ ]
[ ] [S] [ ]
```

**Exact Pattern (can be placed anywhere):**
```
[Plank] [Plank] [Plank]
[     ] [Stick] [     ]
[     ] [Stick] [     ]
```

**Usage:**
- Basic mining tool
- Mines stone blocks
- Damage: 2 HP (in combat)
- Required to obtain cobblestone

---

### Recipe #4: Stone Pickaxe
**Input:** 3 Cobblestone + 2 Sticks
**Output:** 1 Stone Pickaxe
**Pattern:** T-shape (3 cobblestone on top, 2 sticks vertical below)

```
[C] [C] [C]
[ ] [S] [ ]
[ ] [S] [ ]
```

**Exact Pattern (can be placed anywhere):**
```
[Cobble] [Cobble] [Cobble]
[      ] [Stick ] [      ]
[      ] [Stick ] [      ]
```

**Usage:**
- Upgraded mining tool
- Faster than wood pickaxe
- Damage: 3 HP (in combat)
- More durable than wood tools

---

## 🛠️ Utility Blocks

### Recipe #5: Crafting Table
**Input:** 4 Planks
**Output:** 1 Crafting Table block
**Pattern:** 2×2 square of planks

```
[P] [P] [ ]
[P] [P] [ ]
[ ] [ ] [ ]
```

**Alternative Placements:**
```
Top-left:         Top-right:        Bottom-left:
[P] [P] [ ]       [ ] [P] [P]       [ ] [ ] [ ]
[P] [P] [ ]  OR   [ ] [P] [P]  OR   [P] [P] [ ]
[ ] [ ] [ ]       [ ] [ ] [ ]       [P] [P] [ ]

Bottom-right:     Center:
[ ] [ ] [ ]       [ ] [ ] [ ]
[ ] [P] [P]  OR   [ ] [P] [P]
[ ] [P] [P]       [ ] [P] [P]
```

**Usage:**
- Placeable decorative block
- Block ID: 58
- Can be placed in world like any block
- Currently cosmetic (not functional as workstation)

---

## ⚔️ Combat Tools

### Recipe #6: Wood Axe
**Input:** 3 Planks + 2 Sticks
**Output:** 1 Wood Axe
**Pattern:** L-shape (3 planks, 2 sticks)

```
[P] [P] [ ]
[P] [S] [ ]
[ ] [S] [ ]
```

**Usage:**
- Combat weapon: 3 HP damage
- Better than wood pickaxe (2 HP)
- Fast attack speed
- Can also chop wood blocks faster

---

### Recipe #7: Stone Axe
**Input:** 3 Cobblestone + 2 Sticks
**Output:** 1 Stone Axe
**Pattern:** L-shape (3 cobblestone, 2 sticks)

```
[C] [C] [ ]
[C] [S] [ ]
[ ] [S] [ ]
```

**Usage:**
- Combat weapon: 4 HP damage
- Strong mid-game weapon
- More durable than wood axe
- Recommended for combat before swords

---

### Recipe #8: Wood Sword
**Input:** 2 Planks + 1 Stick
**Output:** 1 Wood Sword
**Pattern:** Vertical line (2 planks, 1 stick)

```
[ ] [P] [ ]
[ ] [P] [ ]
[ ] [S] [ ]
```

**Usage:**
- Dedicated combat weapon: 4 HP damage
- Same damage as stone axe
- Cheaper than axe (only 2 planks vs 3)
- Best early-game weapon for damage-per-cost

---

### Recipe #9: Stone Sword
**Input:** 2 Cobblestone + 1 Stick
**Output:** 1 Stone Sword
**Pattern:** Vertical line (2 cobblestone, 1 stick)

```
[ ] [C] [ ]
[ ] [C] [ ]
[ ] [S] [ ]
```

**Usage:**
- Strong combat weapon: 5 HP damage
- Second-best weapon (iron sword is 6 HP)
- Very durable
- Recommended for fighting zombies (20 HP)

---

## ⛏️ Utility Tools

### Recipe #10: Wood Shovel
**Input:** 1 Plank + 2 Sticks
**Output:** 1 Wood Shovel
**Pattern:** Vertical line (1 plank, 2 sticks)

```
[ ] [P] [ ]
[ ] [S] [ ]
[ ] [S] [ ]
```

**Usage:**
- Digging tool for dirt, sand, gravel
- Cheaper than pickaxe (only 1 plank)
- Faster dirt/sand breaking speed
- Basic utility tool

---

### Recipe #11: Stone Shovel
**Input:** 1 Cobblestone + 2 Sticks
**Output:** 1 Stone Shovel
**Pattern:** Vertical line (1 cobblestone, 2 sticks)

```
[ ] [C] [ ]
[ ] [S] [ ]
[ ] [S] [ ]
```

**Usage:**
- Upgraded digging tool
- Faster and more durable than wood
- Efficient for terraforming
- Recommended for large excavation projects

---

## 🏭 Advanced Crafting

### Recipe #12: Furnace
**Input:** 8 Cobblestone
**Output:** 1 Furnace Block
**Pattern:** Hollow 3×3 square (empty center)

```
[C] [C] [C]
[C] [ ] [C]
[C] [C] [C]
```

**Usage:**
- Essential for smelting ores into ingots
- Cooks raw meat into cooked meat
- Requires fuel (wood, planks, coal, sticks)
- Place with right-click in world
- Interactive block for future smelting UI

**Crafting Notes:**
- Requires 8 cobblestone blocks
- Center slot must be empty
- Pattern can be placed anywhere in grid (shapeless positioning)

---

## ⚒️ Iron Tier Tools

### Recipe #13: Iron Pickaxe
**Input:** 3 Iron Ingots + 2 Sticks
**Output:** 1 Iron Pickaxe
**Pattern:** T-shape (3 ingots top, 2 sticks center column)

```
[I] [I] [I]
[ ] [S] [ ]
[ ] [S] [ ]
```

**Stats:**
- **Damage**: 4.0 (vs Stone: 3.0, Wood: 2.0)
- **Mining Speed**: 6.0× multiplier (vs Stone: 4.0×, Wood: 2.0×)
- **Durability**: 250 uses (vs Stone: 131, Wood: 59)
- **Mining Tier**: Can mine diamond-level blocks

**Usage:**
- Superior mining tool for stone, ores, and hard materials
- Much faster than stone tier
- Required for advanced ore mining
- Excellent combat weapon (4 damage)

---

### Recipe #14: Iron Axe
**Input:** 3 Iron Ingots + 2 Sticks
**Output:** 1 Iron Axe
**Pattern:** L-shape (ingots in L-formation, sticks vertical)

```
[I] [I] [ ]
[I] [S] [ ]
[ ] [S] [ ]
```

**Stats:**
- **Damage**: 5.0-6.0 (highest damage tool)
- **Mining Speed**: 6.0× for wood blocks
- **Durability**: 250 uses
- **Specialty**: Wood chopping

**Usage:**
- Best weapon for combat (highest damage)
- Extremely fast wood harvesting
- Durable and efficient
- Recommended for mob fighting

---

### Recipe #15: Iron Sword
**Input:** 2 Iron Ingots + 1 Stick
**Output:** 1 Iron Sword
**Pattern:** Vertical line (2 ingots, 1 stick)

```
[ ] [I] [ ]
[ ] [I] [ ]
[ ] [S] [ ]
```

**Stats:**
- **Damage**: 4.5-5.0
- **Durability**: 251 uses (sword durability formula)
- **Attack Speed**: Faster than axes
- **Combat Role**: Primary weapon

**Usage:**
- Dedicated combat weapon
- Higher attack speed than axes
- Less resource cost than axe (2 ingots vs 3)
- Efficient for mob fighting

---

### Recipe #16: Iron Shovel
**Input:** 1 Iron Ingot + 2 Sticks
**Output:** 1 Iron Shovel
**Pattern:** Vertical line (1 ingot, 2 sticks)

```
[ ] [I] [ ]
[ ] [S] [ ]
[ ] [S] [ ]
```

**Stats:**
- **Mining Speed**: 6.0× for dirt, sand, gravel
- **Durability**: 250 uses
- **Efficiency**: Best digging tool

**Usage:**
- Fastest dirt/sand/gravel excavation
- Essential for large-scale terraforming
- Very durable for extended use
- Cheapest iron tool (only 1 ingot)

---

## 📊 Recipe Summary Table

| Recipe | Inputs | Output | Ratio | Pattern Type |
|--------|--------|--------|-------|--------------|
| Planks | 1 Wood | 4 Planks | 1:4 | Shapeless |
| Sticks | 2 Planks | 4 Sticks | 2:4 | Vertical line |
| Wood Pickaxe | 3 Planks, 2 Sticks | 1 Tool | 5:1 | T-shape |
| Stone Pickaxe | 3 Cobble, 2 Sticks | 1 Tool | 5:1 | T-shape |
| Crafting Table | 4 Planks | 1 Block | 4:1 | 2×2 square |
| Wood Axe | 3 Planks, 2 Sticks | 1 Tool | 5:1 | L-shape |
| Stone Axe | 3 Cobble, 2 Sticks | 1 Tool | 5:1 | L-shape |
| Wood Sword | 2 Planks, 1 Stick | 1 Tool | 3:1 | Vertical line |
| Stone Sword | 2 Cobble, 1 Stick | 1 Tool | 3:1 | Vertical line |
| Wood Shovel | 1 Plank, 2 Sticks | 1 Tool | 3:1 | Vertical line |
| Stone Shovel | 1 Cobble, 2 Sticks | 1 Tool | 3:1 | Vertical line |
| **Furnace** | **8 Cobblestone** | **1 Block** | **8:1** | **Hollow square** |
| **Iron Pickaxe** | **3 Iron Ingots, 2 Sticks** | **1 Tool** | **5:1** | **T-shape** |
| **Iron Axe** | **3 Iron Ingots, 2 Sticks** | **1 Tool** | **5:1** | **L-shape** |
| **Iron Sword** | **2 Iron Ingots, 1 Stick** | **1 Tool** | **3:1** | **Vertical line** |
| **Iron Shovel** | **1 Iron Ingot, 2 Sticks** | **1 Tool** | **3:1** | **Vertical line** |

**Total Recipes**: 16 (11 basic + 5 advanced)

---

## 🎯 Crafting Progression

### Starting Out (Initial Hotbar)
You start with:
- Wood Pickaxe ×1
- Stone Pickaxe ×1
- **Iron Ingot ×16** ⭐ (slot 2) - For testing iron tool crafting
- Wood Shovel ×1
- Dirt ×64
- **Wood ×64** ⭐ (slot 5)
- Stone ×64
- Cobblestone ×64
- **Planks ×64** ⭐ (slot 8)

### Quick Start Crafting Chain

**Option 1: Make Sticks**
1. Already have Planks (slot 8)
2. Move 2 planks to adjacent vertical slots (e.g., slots 0 and 3)
3. Open crafting (`C`)
4. Craft → Get 4 Sticks

**Option 2: Get More Planks**
1. Use Wood from slot 5
2. Open crafting (`C`)
3. Craft → Get 4 more Planks per Wood

**Option 3: Craft a Tool**
1. Craft Sticks first (2 planks → 4 sticks)
2. Arrange 3 planks in top row (slots 0-2)
3. Place 2 sticks in middle and bottom center (slots 4 and 7)
4. Craft → Get Wood Pickaxe

**Option 4: Make Crafting Table**
1. Arrange 4 planks in 2×2 pattern
2. Craft → Get Crafting Table block
3. Select it in hotbar
4. Right-click to place in world

---

## 💡 Crafting Tips

### Pattern Recognition
- The system finds the **smallest bounding box** around your pattern
- Pattern can be shifted anywhere within the 3×3 grid
- Empty slots outside the pattern don't matter
- All non-pattern slots must be empty

### Efficiency Tips
1. **Keep materials organized**: Planks in one slot, sticks in another
2. **Craft in bulk**: Each Wood makes 4 Planks
3. **Progression path**: Wood → Planks → Sticks → Tools
4. **Combat use**: Stone Pickaxe (3 damage) > Wood Pickaxe (2 damage)

### Common Mistakes
❌ **Pattern too spread out** - Keep pattern components adjacent
❌ **Extra items in grid** - Only required items should be present
❌ **Wrong orientation** - Vertical sticks pattern must be vertical, not horizontal
✅ **Shapeless positioning works** - Pattern can be anywhere in grid
✅ **Stack multiple** - Extra materials stay in hotbar for multiple crafts

---

## 🔍 Testing Recipes

### How to Test Each Recipe

**Test 1: Wood → Planks**
```
1. Open inventory (E) - verify Wood in slot 5
2. Open crafting (C)
3. Should see "Result: Planks x4 (Click CRAFT)"
4. Click CRAFT button
5. Verify: 4 planks appear, 1 wood consumed
```

**Test 2: Planks → Sticks**
```
1. Clear slots 0-1 (move items if needed)
2. Put 2 planks in slots 0 and 3 (vertical)
3. Open crafting (C)
4. Should see "Result: Sticks x4 (Click CRAFT)"
5. Click CRAFT
6. Verify: 4 sticks appear, 2 planks consumed
```

**Test 3: Wood Pickaxe**
```
1. Ensure you have 3 planks and 2 sticks
2. Arrange: [P][P][P] in slots 0-2
3. Arrange: [S] in slots 4 and 7 (T-shape)
4. Open crafting (C)
5. Should see "Result: Wood Pickaxe x1 (Click CRAFT)"
6. Click CRAFT
7. Verify: 1 Wood Pickaxe appears, materials consumed
```

**Test 4: Stone Pickaxe**
```
1. Have 3 cobblestone and 2 sticks
2. Arrange: [C][C][C] in slots 0-2
3. Arrange: [S] in slots 4 and 7
4. Open crafting (C)
5. Should see "Result: Stone Pickaxe x1 (Click CRAFT)"
6. Click CRAFT
```

**Test 5: Crafting Table**
```
1. Have 4 planks available
2. Arrange 2×2: [P][P] in slots 0-1, [P][P] in slots 3-4
3. Open crafting (C)
4. Should see "Result: Crafting Table x1 (Click CRAFT)"
5. Click CRAFT
6. Verify: Crafting Table block appears
7. Select it and right-click to place in world
```

---

## 🚀 Advanced Usage

### Hotbar as Crafting Grid
The innovative design uses your hotbar directly as the crafting input:
- No separate inventory management needed
- Visual feedback in inventory UI (press `E`)
- Real-time recipe detection as you arrange items
- Immediate crafting from your active hotbar

### Pattern Matching Algorithm
- **Bounding box calculation**: Finds minimal rectangle containing pattern
- **Offset testing**: Tries all 9 possible positions for pattern placement
- **Validation**: Ensures no extra items outside pattern area
- **Shapeless support**: Pattern can be anywhere, not just top-left

---

## 📝 Future Recipe Ideas

### Planned for Future Updates
- **Smelting System**: Active furnace UI for ore → ingot conversion
- **Diamond Tools**: All tool types with diamond (requires diamond ore)
- **Armor**: Helmets, chestplates, leggings, boots (leather, iron, diamond)
- **Advanced Blocks**: Chest, Door, Trapdoor, Fence
- **Utility Items**: Torches (lighting), Ladders (climbing), Beds (sleeping)
- **Food Recipes**: Bread from wheat, cake, cookies
- **Decorative**: Colored wool, stained glass, stairs, slabs

### Recipe Book UI (Concept)
Future enhancement could include:
- Visual recipe catalog
- Unlock system based on collected materials
- Recipe hints and patterns
- Ingredient highlighting

---

## 🎮 Integration with Gameplay

### Combat
- Crafted pickaxes can be used as weapons
- Stone Pickaxe: 3 damage (better than Wood: 2 damage)
- Axes deal even more damage when implemented

### Building
- Crafting Table blocks can be placed decoratively
- Planks can be placed as building blocks
- Sticks cannot be placed (tool component only)

### Resource Loop
```
Mine Trees → Wood
↓
Craft Planks (1:4)
↓
Craft Sticks (2:4) + Keep Planks
↓
Craft Tools (3P + 2S)
↓
Better Mining → More Resources
```

---

## 🐛 Known Limitations

### Current Implementation
- Crafting grid is hotbar (all 9 slots)
- No separate crafting inventory
- Pattern must fit within available items
- Can't craft if hotbar is full (no output space)

### Workarounds
- Clear hotbar slots before complex recipes
- Craft basic items first to free up space
- Use inventory UI (`E`) to see current arrangement

---

**For more information, see:**
- `DEMO_GUIDE.md` - Complete game guide
- `COMBAT_GUIDE.md` - Combat mechanics
- `PROJECT_SUMMARY.md` - Technical details

**Happy Crafting!** 🎉
