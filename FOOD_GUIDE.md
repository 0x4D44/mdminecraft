# mdminecraft - Food & Hunger System Guide

## 🍎 Food System Overview

mdminecraft features a comprehensive hunger and food system that adds survival challenge and encourages resource gathering, cooking, and mob farming.

---

## 🎯 Hunger Mechanics

### Hunger Bar

**Display:**
- Shows in the top-left HUD as "Hunger: X / 20"
- Maximum hunger: 20 points (10 "drumsticks")
- Hunger depletes gradually over time

**Depletion Rate:**
- 1 hunger point lost every 4 seconds (80 ticks)
- Sprinting and jumping accelerate hunger loss (future)
- Mining and combat increase hunger loss (future)

**Saturation:**
- Hidden stat that delays hunger depletion
- Saturation depletes first before hunger drops
- Different foods restore different saturation amounts
- High saturation = longer time before next hunger loss

---

### Health Regeneration

**Hunger affects health regeneration:**
- **20 Hunger (Full)**: Regenerate 1 HP every 4 seconds
- **18-19 Hunger**: Regenerate 1 HP every 6 seconds
- **15-17 Hunger**: Regenerate 1 HP every 10 seconds
- **10-14 Hunger**: No regeneration
- **Below 10 Hunger**: Health slowly drains (future)

**Strategy:**
- Keep hunger above 15 for steady healing
- Eat before combat to maximize regeneration
- Full hunger = best healing rate

---

## 🍖 Food Types

### Apple 🍎
- **Hunger Restored**: 4 points
- **Saturation Restored**: 2.4 points
- **Source**: Tree leaves (rare drop when breaking)
- **Efficiency**: Low - early game only
- **Use Case**: Emergency food when starting out

**Pros:**
- Renewable from trees
- No cooking required
- Easy to obtain

**Cons:**
- Low hunger restoration
- Low saturation
- Rare drop rate

---

### Raw Meat 🥩
- **Hunger Restored**: 3 points
- **Saturation Restored**: 1.8 points
- **Source**: Passive mobs (Pig, Cow, Chicken)
- **Efficiency**: Low - should be cooked
- **Use Case**: Emergency food if desperate

**Pros:**
- Common drop from mobs (1-3 per kill)
- Stackable (up to 64)
- Can be eaten raw

**Cons:**
- Poor stats when raw
- Much better when cooked
- Risky to rely on

**Drop Rates:**
- Pig: 1-3 Raw Meat (guaranteed)
- Cow: 1-3 Raw Meat (guaranteed)
- Chicken: 0-2 Raw Meat (not guaranteed)

---

### Cooked Meat 🍗
- **Hunger Restored**: 8 points
- **Saturation Restored**: 12.8 points
- **Source**: Cook Raw Meat in furnace
- **Efficiency**: BEST - highest stats
- **Use Case**: Primary food source mid-late game

**Cooking:**
1. Place Raw Meat in furnace input slot
2. Add fuel (Coal, Wood, Stick) to fuel slot
3. Wait 10 seconds per meat
4. Collect from output slot

**Pros:**
- Highest hunger restoration (8 points!)
- Highest saturation (12.8 points!)
- Efficient use of mob drops
- Stackable

**Cons:**
- Requires furnace and fuel
- 10 second cooking time per meat
- Needs mob farming

**Efficiency Calculation:**
- 1 Coal (80s burn time) = 8 Cooked Meat
- 1 Cooked Meat = 40% hunger bar restored
- Best food-to-hunger ratio in game

---

### Bread 🍞
- **Hunger Restored**: 5 points
- **Saturation Restored**: 6.0 points
- **Source**: Crafting (future - requires wheat)
- **Efficiency**: Medium - good mass-production food
- **Use Case**: Farming-based food source (not yet implemented)

**Crafting Recipe (Future):**
```
[Wheat] [Wheat] [Wheat]
```

**Pros:**
- Renewable from farming
- No cooking required
- Easy to mass-produce with wheat farm

**Cons:**
- Requires wheat farming (not yet implemented)
- Medium stats (worse than Cooked Meat)

---

## 🍽️ Eating Food

### How to Eat

**Controls:**
1. Select food item in hotbar (number keys 1-9)
2. Press **R** key to consume
3. Food disappears from hotbar
4. Hunger and saturation restored instantly

**Visual Feedback:**
- Hunger bar updates immediately in HUD
- Console log shows: "Ate [Food Name]: +X hunger, +Y saturation"

**Restrictions:**
- Cannot eat at full hunger (20/20)
- Food must be in selected hotbar slot
- Eating is instant (no animation yet)

---

## 🔥 Cooking System

### Furnace Cooking

**Supported Recipes:**
| Input | Output | Time | Fuel Cost |
|-------|--------|------|-----------|
| Raw Meat | Cooked Meat | 10s | 1/8 Coal |

**How to Cook:**
1. Right-click placed Furnace block to open 3D UI
2. Click **Input Slot** (top-left)
3. Transfer Raw Meat from hotbar (1 at a time)
4. Click **Fuel Slot** (bottom-left)
5. Add Coal, Wood, or Stick as fuel
6. Watch progress bar (center) fill to 100%
7. Click **Output Slot** (right) to collect Cooked Meat

**Progress Indicators:**
- Progress bar shows percentage (0-100%)
- Fuel slot shows 🔥 icon when burning
- Timer shows remaining fuel time (e.g., "🔥 42.5s")

**Automatic Operation:**
- Furnace continues smelting as long as input and fuel exist
- Multiple items process sequentially
- Can walk away and return later

---

### Fuel Types

**Coal (Best):**
- Burn time: 80 seconds
- Cooks: 8 items
- Source: Coal ore (smelting or mining)

**Wood Planks:**
- Burn time: 15 seconds
- Cooks: 1.5 items
- Source: Crafting from logs

**Sticks:**
- Burn time: 5 seconds
- Cooks: 0.5 items
- Source: Crafting from planks

**Efficiency Ranking:**
1. Coal (best)
2. Wood Planks (early game)
3. Sticks (emergency only)

---

## 🐄 Food Sources

### Passive Mob Farming

**Best Mobs for Food:**

**Pig 🐷**
- Drop: 1-3 Raw Meat (guaranteed)
- Health: 10 HP
- Spawns: Plains, Forest
- Best target: Common and good drops

**Cow 🐄**
- Drop: 1-3 Raw Meat (guaranteed)
- Health: 10 HP
- Spawns: Plains, Forest, Hills, Savanna
- Best target: Very common, reliable

**Chicken 🐔**
- Drop: 0-2 Raw Meat (not guaranteed)
- Health: 4 HP
- Spawns: Plains, Forest, Savanna
- Worst target: Unreliable drops

**Sheep 🐑**
- Drop: Wool (1-2 blocks), no food
- Health: 8 HP
- Spawns: Plains, Hills
- Not a food source

---

### Tree Farming (Apples)

**Apple Drops:**
- Source: Breaking leaf blocks
- Drop rate: ~2-5% per leaf block
- Renewable: Replant trees from saplings
- Efficiency: Very low - not recommended

**How to Farm:**
1. Find oak or birch trees
2. Break all leaf blocks
3. Collect rare apple drops
4. Replant saplings for future trees

**Reality Check:**
- Breaking 100 leaves = 2-5 apples
- Each apple = 4 hunger (poor)
- Better to farm mobs

---

## 📊 Food Comparison

### Stats Table

| Food | Hunger | Saturation | Efficiency | Availability | Overall Rating |
|------|--------|------------|------------|--------------|----------------|
| Cooked Meat | 8 | 12.8 | ⭐⭐⭐⭐⭐ | Medium | **Best** |
| Bread | 5 | 6.0 | ⭐⭐⭐⭐ | Low (future) | Good |
| Apple | 4 | 2.4 | ⭐⭐ | Low | Poor |
| Raw Meat | 3 | 1.8 | ⭐ | High | Emergency Only |

### Hunger per Kill

**Pig or Cow Kill:**
- Average drop: 2 Raw Meat
- Cooked: 2 Cooked Meat
- Total hunger: 16 points (80% of bar!)
- Total saturation: 25.6 points

**Efficiency:**
- 1 mob kill + cooking = almost full hunger bar
- Best food source in game
- Sustainable with mob spawning

---

## 🎮 Survival Strategies

### Early Game (First 10 Minutes)

**Food Priority:**
1. Break leaves for apples (emergency food)
2. Hunt chickens for easy Raw Meat (4 HP mobs)
3. Eat Raw Meat if hunger drops below 10
4. Build crafting table and furnace ASAP

**Goal:** Survive long enough to set up cooking

---

### Mid Game (After Furnace)

**Food Strategy:**
1. Hunt pigs and cows (10 HP, guaranteed meat drops)
2. Stockpile Raw Meat in hotbar
3. Cook 8+ meats per coal in furnace
4. Keep 5+ Cooked Meat in hotbar at all times
5. Eat when hunger drops to 15 (maintain regen)

**Goal:** Maintain healthy food stockpile

---

### Late Game (Established Base)

**Food Optimization:**
1. Dedicated mob farm area near base
2. Furnace constantly cooking meat
3. Stockpile 64 Cooked Meat
4. Never worry about hunger again
5. Focus on exploration and combat

**Goal:** Food security achieved

---

## 💡 Pro Tips

### Hunger Management

1. **Don't Overfill**: Eating at 18/20 hunger wastes food
2. **Optimal Timing**: Eat at 12-15 hunger for efficiency
3. **Pre-Combat**: Always enter combat at 20/20 hunger for regen
4. **Saturation Matters**: High saturation foods last longer

### Cooking Efficiency

1. **Batch Cooking**: Fill furnace input with 8+ meats
2. **Coal Only**: Don't waste coal on single items
3. **AFK Cooking**: Furnace works while you explore nearby
4. **Stock Fuel**: Keep coal stockpile near furnace

### Mob Hunting

1. **Weapon Matters**: Stone+ pickaxe for faster kills
2. **Target Pigs/Cows**: Best food drops
3. **Ignore Chickens**: Unreliable drops
4. **Avoid Sheep**: No food drops (wool only)
5. **Night Hunting**: More mobs spawn (future)

### Food Storage

1. **Hotbar Slots**: Dedicate slot 9 to food
2. **Cooked > Raw**: Always cook before storing
3. **Stack Management**: Keep stacks together
4. **Emergency Raw**: Keep 2-3 Raw Meat for emergencies

---

## 🔧 Technical Details

### Hunger Depletion Code

```rust
// Depletion rate: 1 hunger per 4 seconds (80 ticks)
if self.current_tick % 80 == 0 {
    self.player_hunger.hunger -= 1.0;
    if self.player_hunger.hunger < 0.0 {
        self.player_hunger.hunger = 0.0;
    }
}
```

### Food Consumption Code

```rust
// Food stats lookup (src/game.rs:1330)
let (hunger_restored, saturation_restored) = match food_type {
    FoodType::Apple => (4.0, 2.4),
    FoodType::Bread => (5.0, 6.0),
    FoodType::RawMeat => (3.0, 1.8),
    FoodType::CookedMeat => (8.0, 12.8),
};

// Apply to player
self.player_hunger.eat(hunger_restored, saturation_restored);
```

### Cooking Recipe Code

```rust
// Smelting recipes (src/game.rs:830)
fn get_smelt_result(item_type: &ItemType) -> Option<ItemType> {
    match item_type {
        ItemType::Food(FoodType::RawMeat) => Some(ItemType::Food(FoodType::CookedMeat)),
        // ... other recipes
    }
}
```

---

## 🐛 Known Limitations

### Current Restrictions

- Wheat farming not yet implemented (no Bread crafting)
- Potatoes not yet implemented (no Baked Potato)
- Fishing not yet implemented (no fish)
- No hunger damage when starving (future)
- No eating animation (instant consumption)
- No sound effects for eating
- Can't eat while moving (must stand still)

### Planned Improvements

- Wheat crops and farming system
- Potato crops (raw and baked)
- Fishing for alternative food source
- Starvation damage below 0 hunger
- Eating animations and sounds
- Eating while moving
- Golden apples with special effects
- Mushroom stew and soups

---

## 🎯 Future Food Types

### Short-Term (Next Updates)

**Baked Potato:**
- Cook Potato in furnace
- 5 hunger, 6.0 saturation
- Requires potato crop farming

**Bread (Crafting):**
- 3 Wheat → 1 Bread
- Already coded, needs wheat farming

**Rotten Flesh:**
- Drop from Zombies (already dropping)
- 4 hunger, 0.8 saturation
- Chance of hunger effect (future)

### Mid-Term

**Cooked Fish:**
- From fishing system
- 5 hunger, 6.0 saturation
- Alternative to meat

**Mushroom Stew:**
- Crafting from mushrooms
- 6 hunger, 7.2 saturation
- Bowl required

**Golden Apple:**
- Crafting with gold
- 4 hunger + regeneration effect
- Rare and powerful

### Long-Term

**Cake:**
- Multi-bite placeable food
- Decorative and functional

**Honey:**
- From bee hives
- Removes poison effects

**Suspicious Stew:**
- Random potion effects
- High risk, high reward

---

## 📚 Related Guides

For more information, see:
- `FURNACE_SMELTING_GUIDE.md` - Detailed furnace mechanics and recipes
- `HOSTILE_MOBS_GUIDE.md` - Combat and mob drops (Rotten Flesh)
- `CRAFTING_RECIPES.md` - All crafting recipes
- `DEMO_GUIDE.md` - Complete gameplay overview

---

**Stay fed. Stay alive.** 🍖

*Last updated: 2025-11-18 (Cooking system implementation)*
