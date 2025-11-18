# mdminecraft - Furnace Smelting Guide

## 🔥 Furnace Smelting System

Complete guide to smelting ores and materials in the furnace.

---

## 📋 Overview

The furnace is a functional block that converts raw materials into refined products using fuel. It's essential for the iron progression and allows you to create iron ingots from raw iron ore.

**Key Features:**
- Automatic fuel consumption
- Progress tracking (10 seconds per item)
- Multiple fuel types with different burn times
- Stackable output collection
- Continuous operation while fuel lasts

---

## 🏗️ Crafting a Furnace

**Recipe: Furnace (8 Cobblestone)**

```
[C] [C] [C]
[C] [ ] [C]
[C] [C] [C]
```

**Materials Required:**
- 8× Cobblestone (mined from stone blocks)

**Pattern:**
- Hollow 3×3 square (center must be empty)
- Can be placed anywhere in crafting grid (shapeless positioning)

**How to Craft:**
1. Mine stone blocks → Get cobblestone
2. Collect 8 cobblestone blocks
3. Press `C` to open crafting table
4. Arrange 8 cobblestone in hollow square pattern
5. Click CRAFT button
6. Furnace block appears in hotbar

---

## 🔧 Using the Furnace

### Placing the Furnace

1. Select furnace block in hotbar
2. Right-click on ground or surface
3. Furnace block is placed in world

### Opening the Furnace

**Right-click on placed furnace block:**
- Opens 3D furnace UI to your right
- Toggles UI open/closed with subsequent clicks
- UI displays current furnace state in real-time

**3D Furnace UI Elements:**
- **[Input]** slot (top-left): Place items to smelt
- **[Fuel]** slot (bottom-left): Place fuel items
- **→ Progress** (center): Shows smelting progress (0-100%)
- **[Output]** slot (right): Collect smelted results
- **Title:** "Furnace" displayed at top

### Using the Furnace UI

**Adding Items to Slots:**
1. Select item in hotbar (use number keys 1-9)
2. Aim at furnace slot button (crosshair in center of screen)
3. Click the slot to transfer 1 item from hotbar

**Input Slot:**
- Only accepts smeltable items (Raw Iron, ore blocks)
- Shows item name and quantity (e.g., "Raw Iron x10")
- Click again to return ALL items to hotbar

**Fuel Slot:**
- Only accepts fuel items (Coal, wood, sticks, planks)
- Shows 🔥 icon when furnace is actively burning
- Click again to return ALL fuel to hotbar

**Output Slot:**
- Displays smelted results (e.g., "Iron Ingot x5")
- Click to collect ALL items to hotbar
- If hotbar is full, items remain in furnace

**Progress Indicator:**
- Shows percentage complete (e.g., "→ 35%")
- Shows fuel timer when burning (e.g., "🔥 42.5s")
- Updates in real-time every frame

### Testing Interface (V Key - Optional)

For quick testing, press **V** while furnace is open:
- **First press:** Automatically adds 10 Raw Iron + 5 Coal
- **Watch:** Furnace begins smelting automatically
- **Subsequent presses:** Collects all output to hotbar
- Shows status logs in console

*Note: The V key is a debug feature and not required for normal gameplay.*

---

## 🔬 Smelting Recipes

### Raw Iron → Iron Ingot

**Input:** Raw Iron (Item ID 4)
**Output:** Iron Ingot (Item ID 2)
**Time:** 10 seconds per ingot
**Fuel Required:** ~1.25 seconds of burn time per ingot

**Usage:**
- Primary smelting recipe
- Essential for iron tool progression
- Each Raw Iron yields exactly 1 Iron Ingot

**Example:**
```
Input: 10 Raw Iron + 5 Coal
Result: 10 Iron Ingots
Fuel Used: 2 coal (16 seconds burn time / 100 seconds total smelting)
Time: 100 seconds total
```

### Legacy Ore Block Recipes

**Iron Ore Block → Iron Ingot**
- Input: Iron Ore block (Block ID 17)
- Output: Iron Ingot
- Same timing as Raw Iron

**Coal Ore Block → Coal**
- Input: Coal Ore block (Block ID 18)
- Output: Coal item
- Time: 10 seconds

*Note: These recipes support ore blocks if mined with Silk Touch (future feature)*

---

## ⛽ Fuel Types

### Fuel Efficiency Table

| Fuel Type | Burn Time | Items Smelted | Efficiency Score |
|-----------|-----------|---------------|------------------|
| **Coal** | 80 seconds | 8 items | ⭐⭐⭐⭐⭐ Best |
| **Oak/Birch/Pine Log** | 15 seconds | 1.5 items | ⭐⭐⭐ Good |
| **Planks** | 7.5 seconds | 0.75 items | ⭐⭐ Okay |
| **Stick** | 5 seconds | 0.5 items | ⭐ Poor |

### Fuel Details

**Coal (Item 3) - 80 seconds**
- **Best fuel source**
- Smelts 8 items per coal
- Found by mining coal ore
- Recommended for large smelting jobs
- Example: 5 coal = 40 items smelted

**Logs (Blocks 11, 13, 15) - 15 seconds**
- Oak Log (Block 11)
- Birch Log (Block 13)
- Pine Log (Block 15)
- Good alternative when coal is scarce
- Smelts 1.5 items per log

**Planks (Block 7) - 7.5 seconds**
- Half the efficiency of logs
- Better to use logs directly
- Emergency fuel option
- Smelts 0.75 items per plank

**Sticks (Item 1) - 5 seconds**
- Least efficient fuel
- Use only when desperate
- Smelts 0.5 items per stick
- Better to save for tool crafting

---

## 🔄 Smelting Process

### How Automatic Smelting Works

```
1. Place Raw Iron in INPUT slot
2. Place Coal in FUEL slot
         ↓
3. Furnace checks for valid recipe → Found!
         ↓
4. Furnace checks for fuel → Available!
         ↓
5. AUTO-CONSUME: Takes 1 coal from fuel slot
         ↓
6. START BURNING: 80-second timer begins
         ↓
7. SMELTING PROGRESS: 10% per second
         ↓
8. AFTER 10 SECONDS: First Iron Ingot complete!
         ↓
9. Output slot receives Iron Ingot
10. Input slot loses 1 Raw Iron
11. Smelting progress resets to 0%
         ↓
12. Process repeats for remaining items
         ↓
13. STILL BURNING: 70 seconds of fuel remain
         ↓
14. Smelts 7 more items on same coal
         ↓
15. When fuel depletes: Auto-consumes next coal
         ↓
16. Continues until input or fuel is empty
```

### Smart Fuel Management

**Automatic Fuel Consumption:**
- Furnace only consumes fuel when needed
- Checks fuel every frame while smelting
- Never wastes fuel (only lights when input is ready)
- Seamlessly transitions between fuel items

**Multi-Item Smelting:**
- One coal (80s) can smelt 8 Raw Iron (8 × 10s)
- Fuel efficiency maximized automatically
- No manual intervention needed

**Output Handling:**
- Iron Ingots stack up to 64 in output slot
- If output slot is full/blocked: smelting pauses
- Warning logged: "Furnace output blocked!"
- Resume automatically when output is cleared

---

## 📊 Smelting Examples

### Example 1: Basic Iron Smelting

**Goal:** Smelt 3 Raw Iron

```
Setup:
  Input: 3 Raw Iron
  Fuel: 1 Coal (80s burn time)

Timeline:
  0s:  Furnace starts, consumes 1 coal
  10s: First Iron Ingot complete (output: 1)
  20s: Second Iron Ingot complete (output: 2)
  30s: Third Iron Ingot complete (output: 3)
  30s: Smelting stops (no more input)

Remaining: 50 seconds of coal burn time (wasted)
Efficiency: 30s used / 80s available = 37.5%
```

**Tip:** Always smelt in batches to maximize fuel efficiency!

### Example 2: Efficient Batch Smelting

**Goal:** Smelt 16 Raw Iron

```
Setup:
  Input: 16 Raw Iron
  Fuel: 2 Coal (160s burn time total)

Timeline:
  0s:   Start, consume 1st coal (80s)
  10s:  Ingot #1 (output: 1)
  20s:  Ingot #2 (output: 2)
  30s:  Ingot #3 (output: 3)
  40s:  Ingot #4 (output: 4)
  50s:  Ingot #5 (output: 5)
  60s:  Ingot #6 (output: 6)
  70s:  Ingot #7 (output: 7)
  80s:  Ingot #8 (output: 8)
  80s:  Auto-consume 2nd coal (80s)
  90s:  Ingot #9 (output: 9)
  100s: Ingot #10 (output: 10)
  ...
  160s: Ingot #16 (output: 16)

Result: All 16 items smelted
Fuel Used: 2 coal (exactly right!)
Efficiency: 160s used / 160s available = 100%
```

### Example 3: Using Logs as Fuel

**Goal:** Smelt 10 Raw Iron with logs

```
Setup:
  Input: 10 Raw Iron
  Fuel: 7 Oak Logs (15s each = 105s total)

Required Time: 10 items × 10s = 100 seconds
Available Time: 7 logs × 15s = 105 seconds

Timeline:
  0s-15s:   Log #1 → 1.5 ingots (2 complete, progress 50%)
  15s-30s:  Log #2 → +1.5 ingots (4 complete)
  30s-45s:  Log #3 → +1.5 ingots (5 complete, progress 50%)
  45s-60s:  Log #4 → +1.5 ingots (7 complete)
  60s-75s:  Log #5 → +1.5 ingots (8 complete, progress 50%)
  75s-90s:  Log #6 → +1.5 ingots (10 complete)

Result: All 10 items smelted with 1 log left over
```

---

## 🎯 Optimal Strategies

### Fuel Selection Priority

1. **Use Coal First**
   - Most efficient (8 items per fuel)
   - Ideal for large batches
   - Stockpile coal for major smelting sessions

2. **Use Logs for Medium Batches**
   - Good for 2-5 items
   - Readily available from trees
   - Don't waste coal on small jobs

3. **Avoid Planks**
   - Half the efficiency of logs
   - Only use if no logs available
   - Better to keep for crafting

4. **Never Use Sticks**
   - Extremely inefficient
   - Takes 2 sticks per item smelted
   - Save for tool handles instead

### Batch Size Recommendations

**For Coal Fuel:**
- Smelt in multiples of 8 (1 coal = 8 items)
- Ideal batches: 8, 16, 24, 32, 40, 48, 56, 64

**For Log Fuel:**
- Each log smelts ~1.5 items
- Odd numbers are fine (log efficiency varies)
- Example: 3 items = 2 logs, 5 items = 4 logs

**General Rule:**
- Larger batches = better fuel efficiency
- Always load full stacks when possible
- Collect output regularly to avoid blocking

---

## 🔍 Current Implementation Details

### Testing Mode (V Key)

**Press V once:**
```
Action: Initialize furnace with test materials
Result:
  - Input slot: 10 Raw Iron
  - Fuel slot: 5 Coal
  - Console: "Added test items: 10 Raw Iron + 5 Coal"
```

**Automatic smelting begins:**
```
Every 10 seconds: 1 Raw Iron → 1 Iron Ingot
Fuel consumption: 1 coal every 80 seconds
Total time: 100 seconds for all 10 items
Total fuel used: 2 coal (remaining: 3)
```

**Press V again:**
```
Action: Collect all output to hotbar
Result:
  - If output exists: "Collected from furnace: Iron Ingot x10"
  - If no output: "No output to collect yet."
  - If inventory full: "Inventory full! Could not collect output."
```

### Console Logging

**Furnace Status (Press V):**
```
=== Furnace Smelting Test ===
Current furnace state:
  Input: Some(("Raw Iron", 8))
  Fuel: Some(("Coal", 4))
  Output: Some(("Iron Ingot", 2))
  Progress: 35.0%
  Fuel burning: 52.3s / 80.0s
```

**Fuel Events:**
```
Furnace: Started burning fuel (80.0s)  ← Coal consumed
```

**Smelting Events:**
```
Furnace: Smelting complete!  ← Item finished
```

**Blocking Events:**
```
Furnace output blocked!  ← Output slot full
```

---

## 🚀 Iron Progression Workflow

### Complete Iron Tier Unlock

**Step 1: Gather Materials**
```
1. Mine underground (Y 10-50)
2. Find iron ore veins
3. Mine iron ore → Get Raw Iron
4. Find coal ore veins
5. Mine coal ore → Get Coal
6. Collect 8 cobblestone for furnace
```

**Step 2: Craft Furnace**
```
1. Press C (open crafting)
2. Place 8 cobblestone in hollow square
3. Click CRAFT
4. Place furnace block in world
```

**Step 3: Smelt Iron**
```
1. Right-click furnace block to open 3D UI
2. Select Raw Iron in hotbar (number keys 1-9)
3. Click [Input] slot to add Raw Iron
4. Select Coal in hotbar
5. Click [Fuel] slot to add Coal
6. Watch progress bar: "→ 0%" → "→ 100%"
7. Click [Output] slot to collect Iron Ingots
```

*Alternative: Press V for quick test (adds 10 Raw Iron + 5 Coal automatically)*

**Step 4: Craft Iron Tools**
```
With Iron Ingots, craft:
  - Iron Pickaxe (3 ingots + 2 sticks)
  - Iron Axe (3 ingots + 2 sticks)
  - Iron Sword (2 ingots + 1 stick)
  - Iron Shovel (1 ingot + 2 sticks)
```

**Step 5: Dominate**
```
Iron tools provide:
  - 6.0× mining speed
  - 250 durability
  - 4.0-6.0 damage
  - Can mine all current blocks
```

---

## 🐛 Known Limitations

### Current System

**1. Single Furnace Instance**
- Only one global furnace state
- All furnace blocks share same state
- Can't have multiple furnaces running
- Future: Per-block furnace states

**2. No Persistent Storage**
- Furnace state resets on game restart
- Items in furnace are lost
- Future: Save/load furnace contents

**3. Manual Item Transfer**
- Must click slots individually to add items (1 at a time)
- No shift-click for bulk transfer
- Future: Batch transfer options

### Tips for Efficient Use

**Quick Item Transfer:**
1. Select item stack in hotbar
2. Click furnace slot multiple times
3. Each click transfers 1 item
4. Watch slot quantity update

**Monitoring Progress:**
- Progress bar shows percentage (0-100%)
- Fuel timer shows remaining burn time
- 🔥 icon appears when actively burning
- Real-time updates every frame

**Bulk Smelting:**
- Click Input slot repeatedly to add multiple items
- Add enough fuel for all items
- Leave furnace open to watch progress
- Or close UI and come back later

**Quick Testing (V Key):**
- Use V for instant 10 Raw Iron + 5 Coal setup
- Great for testing or quick iron needs
- Press V again to collect all output

---

## 📈 Future Enhancements

### Planned Features

**1. Per-Block Furnace States** ✨ PRIORITY
- Each furnace block has unique state
- Run multiple furnaces simultaneously
- Distributed smelting for efficiency

**2. Batch Item Transfer**
- Shift-click to transfer entire stacks
- Drag-and-drop item movement
- Faster bulk smelting setup

**3. Visual Furnace States**
- Lit texture when burning fuel
- Smoke particles from chimney
- Orange glow at night
- Crackling sound effects

**4. Advanced Recipes**
- Sand → Glass (decorative)
- Raw Meat → Cooked Meat (food)
- Wet Sponge → Dry Sponge (utility)
- Cactus → Green Dye (decorative)

**5. Experience Orbs**
- Gain XP when collecting smelted items
- Store XP for enchanting (future)
- More XP for rarer materials

**6. Recipe Book Integration**
- Show available smelting recipes
- Visual recipe catalog in furnace UI
- Unlock tracking for new recipes

---

## 🎓 Advanced Tips

### Fuel Management

**Coal Stockpiling:**
- Keep 64+ coal in storage
- Dedicate one hotbar slot to fuel
- Mine coal ore whenever found
- Coal is always valuable

**Log Efficiency:**
- Don't craft logs into planks for fuel
- Logs are 2× more efficient than planks
- Only use planks if no logs available

**Emergency Fuel:**
- Keep sticks as last resort
- Wooden tools can be used as fuel (future)
- Furniture blocks burn too (future)

### Smelting Optimization

**Batch Processing:**
- Wait until you have 8+ Raw Iron
- Smelt full stacks (64) when possible
- Minimize fuel waste on small batches

**Output Management:**
- Collect output before it's full
- Keep output slot clear for continuous operation
- Store ingots in chests (future)

**Resource Priority:**
- Smelt iron first (tool progression)
- Coal ore can be used directly as fuel
- Save smelting for valuable materials

---

## 📖 Related Documentation

- **CRAFTING_RECIPES.md** - All crafting recipes including furnace
- **ORE_GENERATION_GUIDE.md** - Finding iron and coal ore
- **PROJECT_SUMMARY.md** - Technical implementation details
- **DEMO_GUIDE.md** - Complete gameplay guide

---

**The furnace smelting system is now fully functional with a complete 3D UI! Right-click placed furnaces to open the interface, add items by clicking slots, and watch the real-time progress bar as Raw Iron becomes Iron Ingots automatically.**
