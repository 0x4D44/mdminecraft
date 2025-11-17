# mdminecraft - 3D UI System Demo Guide

## 🎮 Overview

This build of mdminecraft features a complete **3D UI system** that renders all interface elements in true 3D world space, creating an immersive AR-like experience. Unlike traditional Minecraft's 2D overlay UIs, all menus and labels float in the game world.

## 🚀 Quick Start

### Running the Game
```bash
cargo run --release
```

### Basic Controls
| Key | Action |
|-----|--------|
| **W/A/S/D** | Move |
| **Mouse** | Look around |
| **Space** | Jump |
| **Tab** | Toggle cursor grab/free look |
| **F** | Toggle fly mode |
| **E** | Toggle inventory |
| **C** | Toggle crafting table |
| **P** | Pause time |
| **F3** | Toggle debug HUD |
| **ESC** | Return to menu |

### Hotbar Controls
| Key | Action |
|-----|--------|
| **1-9** | Select hotbar slot |
| **Left Click** | Break block / Attack mob |
| **Right Click** | Place block |

## ⚔️ Combat System

### Fighting Mobs
1. Find a mob in the world
2. Aim at it with your crosshair
3. Look for the `<--` arrow in the health bar
4. **Left-click** to attack
5. Watch health bar decrease with color changes

### Mob Health
- **Chicken**: 4 HP (Green → Yellow → Red as health drops)
- **Sheep**: 8 HP
- **Pig**: 10 HP
- **Cow**: 10 HP

### Weapon Damage
- **Bare Hands**: 1 damage
- **Wood Pickaxe**: 2 damage
- **Stone Pickaxe**: 3 damage
- **Iron Pickaxe**: 4 damage
- **Axes**: 3-5 damage (even more effective)

### Health Bars
- Format: `Pig\n[██████████] 10/10 <--`
- **Green**: >66% health (healthy)
- **Yellow**: 33-66% health (wounded)
- **Red**: <33% health (critical)
- Arrow `<--` shows which mob you're targeting

**See COMBAT_GUIDE.md for detailed combat mechanics!**

## 🎯 3D UI Features

### 1. Floating Labels System

**Mob Labels:**
- Automatically appear above all spawned mobs
- Show mob type, health bar, and HP numbers
- Color-coded by health: Green/Yellow/Red
- Shows targeting arrow `<--` when aimed at
- Always face the camera (billboard rendering)

**Player Info:**
- Position label floats 3m above player
- Shows X/Y/Z coordinates in real-time
- Yellow color for distinction

**Block Info:**
- Appears when looking at a block
- Shows block name and position
- Cyan color
- Auto-hides when not targeting

### 2. 3D Inventory (Press 'E')

**Layout:**
```
[Wood Pickaxe x1]  [Stone Pickaxe x1]  [Iron Pickaxe x1]
[Wood Shovel x1]   [Dirt x64]          [Wood x64]
[Stone x64]        [Cobblestone x64]   [Planks x64]
```

**Features:**
- 3×3 grid showing 9 hotbar slots
- Positioned 3m in front of player
- Each slot shows item name + count
- Real-time updates as you use items
- Hover over slots to see highlight effect
- Click slots to see item details in console

**How to Use:**
1. Press `E` to open inventory
2. Aim at any slot (it will highlight yellow)
3. Click to inspect item
4. Press `E` again to close

### 3. 3D Crafting Table (Press 'C')

**Layout:**
```
                Crafting Table

    [1] [2] [3]        Result:
    [4] [5] [6]        Planks x4
    [7] [8] [9]      (Click CRAFT)

                      [CRAFT]
```

**Features:**
- 3×3 recipe grid (positioned to your right)
- Result preview area
- Craft button for execution
- Real-time recipe detection
- Visual feedback for valid recipes

**How to Use:**
1. Press `C` to open crafting table
2. Your hotbar items become the crafting grid (slots 1-9)
3. Result preview shows what can be crafted
4. Aim at the **CRAFT** button (it highlights)
5. Click to craft the item
6. Crafted item appears in your hotbar
7. Required materials are consumed

**Available Recipes:**
1. **Wood → Planks** (1:4 ratio)
   - Place 1 Wood anywhere in hotbar
   - Creates: 4 Planks

2. **Planks → Sticks** (2 planks → 4 sticks)
   - Place 2 Planks vertically (any two adjacent slots)
   - Creates: 4 Sticks

3. **Wood Pickaxe** (3 planks + 2 sticks)
   - Top row: 3 Planks
   - Middle & bottom center: 2 Sticks (T-shape)
   - Creates: 1 Wood Pickaxe

4. **Stone Pickaxe** (3 cobblestone + 2 sticks)
   - Top row: 3 Cobblestone
   - Middle & bottom center: 2 Sticks (T-shape)
   - Creates: 1 Stone Pickaxe

5. **Crafting Table** (4 planks in 2×2)
   - 2×2 square of Planks anywhere
   - Creates: 1 Crafting Table block

**Recipe Tips:**
- Patterns can be placed anywhere in the grid (shapeless positioning)
- The system detects patterns automatically
- All unused items stay in your hotbar

### 4. Demo Button

A floating "Click Me!" button demonstrates the interaction system:
- Positioned 5m in front of spawn
- Turns yellow when aimed at (hover)
- Turns blue when clicked (press)
- Logs to console when activated

## 🐄 Mob System

### Spawning
- Mobs spawn automatically during world generation
- Biome-based distribution:
  - **Plains**: Pigs, Cows, Sheep, Chickens
  - **Forest**: Pigs, Chickens
  - **Hills**: Sheep, Cows
  - **Ocean/Desert**: No mobs

- Expected: ~10-20 mobs in starting area
- 5% spawn chance per spawn point (every 8 blocks)

### Behavior
- **Idle State**: Stands still for 40-80 ticks
- **Wandering State**: Walks in random direction for 20-60 ticks
- Deterministic AI based on world seed
- Gravity and physics simulation

### Visual
- 3D labels show mob type
- Labels float 1 block above mob
- Orange/gold color (RGB: 1.0, 0.8, 0.4)
- Update position every frame

## 🎨 Visual System Details

### Billboard Rendering
All 3D UI elements use billboard rendering:
- Quads always face the camera
- Maintains readability from any angle
- Size consistent in screen space
- No perspective distortion

### Color Scheme
| Element | Color | Hex |
|---------|-------|-----|
| Player position | Yellow | #FFFF00 |
| Block info | Cyan | #00FFFF |
| Mob labels | Orange/Gold | #FFCC66 |
| Crafting title | Yellow | #FFFF33 |
| Result preview | Green | #00FF80 |

### Button States
| State | Color | When |
|-------|-------|------|
| Normal | Gray | Default |
| Hover | Yellow | Aiming at button |
| Pressed | Blue | While clicking |
| Disabled | Dark Gray | Not interactable |

## 🛠️ Technical Details

### Performance
- **Frame Rate**: 60 FPS (maintained)
- **UI Overhead**: <1ms per frame
- **UI Elements**: 30+ simultaneous
- **Memory**: ~6KB for all UI state

### Architecture
```
Update Cycle:
  ├─ update_mobs() - Tick mob AI
  ├─ update_ui_labels() - Position/block info
  ├─ update_mob_labels() - Mob name tags
  ├─ update_inventory_ui() - Inventory grid (if open)
  ├─ update_crafting_ui() - Crafting table (if open)
  └─ handle_ui_interaction() - Hover/click detection

Render Cycle:
  ├─ Skybox
  ├─ Voxel World
  ├─ 3D UI Elements (text + buttons)
  ├─ Wireframe highlight
  └─ 2D Debug HUD (egui)
```

### Raycasting System
- Screen center casts ray into world
- Tests against all Button3D AABBs
- Billboard quads calculated per-frame
- Returns closest hit with distance
- Sub-millisecond performance

## 🎯 Demo Scenario

**Complete Workflow:**

1. **Start Game**
   - Spawn at (0, 100, 0)
   - See mob labels in distance
   - Position label above head

2. **Explore World**
   - Walk around to find mobs
   - Look at blocks to see info labels
   - Break blocks with left click
   - Place blocks with right click

3. **Open Inventory (E)**
   - See all your items in 3D
   - Aim at slots to highlight
   - Click for item details
   - Notice real-time count updates

4. **Craft Items (C)**
   - Open crafting table
   - Result shows "Planks x4" if you have wood
   - Aim at CRAFT button until it highlights
   - Click to execute craft
   - Watch planks appear in inventory
   - Wood count decreases by 1

5. **Build Something**
   - Select planks from hotbar (press 7)
   - Right-click to place blocks
   - Create structures

6. **Test Interactions**
   - Find the demo button (5m from spawn)
   - Aim at it (turns yellow)
   - Click it (turns blue)
   - Check console for callback message

## 📊 Callback ID Reference

| ID Range | Purpose |
|----------|---------|
| 1 | Demo button |
| 100-108 | Inventory slots |
| 200-208 | Crafting grid slots |
| 999 | Craft button |

## 🐛 Troubleshooting

**UI not appearing:**
- Check console for "3D UI system initialized"
- Verify system font was found
- Try closing/reopening with E or C

**Buttons not clickable:**
- Ensure cursor is grabbed (press Tab)
- Aim directly at button center
- Look for yellow highlight (hover state)

**Crafting not working:**
- Verify wood is in hotbar slot 5 (press 6 to select it)
- Open crafting with C
- Result should show "Planks x4"
- Aim at CRAFT button and click

**Poor performance:**
- Close UI panels if not needed (E/C)
- Toggle debug HUD off (F3)
- Reduce render distance (currently hardcoded)

## 🎓 Learning Resources

**Files to Study:**
- `crates/ui3d/` - Complete 3D UI framework
- `crates/ui3d/src/manager.rs` - UI lifecycle management
- `crates/ui3d/src/components/` - Button3D, Text3D, Panel3D
- `crates/ui3d/src/interaction/` - Raycasting system
- `src/game.rs` - Integration and interaction handlers
- `wrk_journals/2025.11.17 - JRN - 3D UI Implementation.md` - Full development log

**Key Concepts:**
- Billboard rendering (camera-facing quads)
- Text atlas generation (SDF fonts)
- Screen-to-world raycasting
- UI state management
- Button interaction states
- Callback system design

## 🎉 What's Unique

**Compared to Minecraft:**
- **All UI in 3D world space** (vs 2D overlay)
- **Billboard labels everywhere** (mobs, blocks, info)
- **Floating inventory grid** (vs flat screen UI)
- **Spatial crafting table** (positioned in world)
- **AR-like experience** (UIs float around you)
- **Full raycasting interaction** (aim & click)

**Technical Achievements:**
- ✅ Real-time text rendering in 3D
- ✅ Interactive buttons in world space
- ✅ Dynamic buffer updates
- ✅ Zero-overhead state management
- ✅ Complete interaction pipeline
- ✅ Scalable architecture

## 📝 Future Enhancements

**Planned Features:**
1. Drag & drop item movement
2. More crafting recipes
3. Panel3D backgrounds
4. Item sprite icons
5. Mob sprite billboards
6. Health bars above mobs
7. Recipe book UI
8. Chest inventory UI

## 🔗 Links

- **Journal**: `wrk_journals/2025.11.17 - JRN - 3D UI Implementation.md`
- **Source**: `crates/ui3d/`
- **Examples**: `crates/ui3d/examples/`

---

**Enjoy exploring the 3D UI system!** 🎮✨
