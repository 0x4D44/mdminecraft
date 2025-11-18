# mdminecraft - Survival Guide: Hunger & Food System

## 🍖 Hunger System Overview

mdminecraft features a complete hunger system that drives survival gameplay. Managing hunger is essential for health regeneration and avoiding starvation damage.

---

## 📊 Hunger Mechanics

### Hunger Bar

**Stats:**
- **Maximum Hunger**: 20 points (like health)
- **Starting Hunger**: 20 (full)
- **Drain Rate**: 0.1 points/second
- **Time to Empty**: 2 minutes from full

**Hunger Display:**
- Current implementation: Console logs
- Future: Visual hunger bar UI

---

### Saturation System

**What is Saturation?**
- Hidden "food buffer" that drains before hunger
- Protects your hunger bar from decreasing immediately
- Provided by eating food alongside hunger restoration

**Mechanics:**
- **Starting Saturation**: 5.0
- **Maximum Saturation**: 20.0
- **Drain Rate**: 0.1 points/second (same as hunger)
- **Behavior**: Saturation drains first, then hunger drains

**Why It Matters:**
- Good food provides high saturation (longer lasting)
- Poor food provides low saturation (need to eat often)
- Saturation = "how long until hungry again"

---

## 🍗 Food Types & Values

### Food Comparison Table

| Food | Hunger Restored | Saturation | Total Benefit | Source |
|------|----------------|------------|---------------|--------|
| **Cooked Meat** | 8.0 | 12.8 | 20.8 | Cooking Raw Meat (future) |
| **Bread** | 5.0 | 6.0 | 11.0 | Crafting (future) |
| **Apple** | 4.0 | 2.4 | 6.4 | Trees (future) |
| **Raw Meat** | 3.0 | 1.8 | 4.8 | Killing animals |

**Best Food Rankings:**
1. Cooked Meat - Best overall (needs cooking)
2. Bread - Good balance (needs wheat)
3. Apple - Decent quick food
4. Raw Meat - Emergency food only

---

### Obtaining Food

**Current Sources:**

**From Passive Mobs:**
- Pig: 1-3 Raw Meat
- Cow: 1-3 Raw Meat
- Chicken: 0-2 Raw Meat
- Sheep: No food drops (wool only)

**From Hostile Mobs:**
- Zombie: 0-2 Rotten Flesh (displays as Raw Meat currently)
- Skeleton: No food drops

**Future Sources:**
- Farming: Wheat → Bread
- Fishing: Fish
- Hunting: Better drop rates
- Cooking: Raw → Cooked (+5 hunger, +11 saturation!)

---

## 🍽️ Eating Food

### How to Eat

**Steps:**
1. Select food item in hotbar (keys 1-9)
2. Press **`R`** key to consume
3. One food item is consumed from stack
4. Hunger and saturation are restored
5. Console shows: `"Ate [FoodType]! +X hunger, +Y saturation"`

**Controls:**
- **R** - Eat selected item
- **1-9** - Select hotbar slot
- **E** - Open inventory (check food supplies)

**Requirements:**
- Must have food selected in hotbar
- Food must be ItemType::Food variant
- Cannot eat non-food items

**Cooldown:**
- No cooldown - can eat spam if you have food
- But why waste food? Only eat when hungry!

---

## ❤️ Health Regeneration

### Regeneration Rates

**Full Hunger (>18 points):**
- **Regen Rate**: 1.0 HP/second
- **Time to Full Health**: 20 seconds (0 → 20 HP)
- **Status**: FAST regeneration
- **Strategy**: Stay above 18 hunger for combat healing

**Decent Hunger (7-18 points):**
- **Regen Rate**: 0.3 HP/second
- **Time to Full Health**: 66 seconds
- **Status**: SLOW regeneration
- **Strategy**: Acceptable for exploration

**Hungry (0-6 points):**
- **Regen Rate**: 0 HP/second
- **Time to Full Health**: Never
- **Status**: NO regeneration
- **Strategy**: EAT IMMEDIATELY!

---

### Regeneration Conditions

**Requirements:**
1. **Health Not Full**: Current HP < 20
2. **Not Recently Damaged**: 3+ seconds since last hit
3. **Sufficient Hunger**: >6 hunger points

**What Blocks Regeneration:**
- Taking damage (resets 3-second timer)
- Low hunger (≤6 points)
- Already at full health

**Invulnerability Frames:**
- 0.5 second invuln after taking damage
- Prevents multiple simultaneous hits
- Doesn't affect regeneration timer

---

## ⚠️ Starvation

### Starvation Damage

**When Hunger Reaches 0:**
- **Damage**: 0.5 HP per second
- **Damage Interval**: Every 1 second after last damage
- **Can Kill**: YES - starvation is lethal
- **Warning**: Hunger bar will show 0/20

**Mechanics:**
- Overrides invulnerability frames
- Damage triggers every second
- Stops when you eat food or die
- Cannot regenerate while starving

**How to Survive:**
1. Eat food IMMEDIATELY when hunger drops below 6
2. Always carry emergency food
3. Hunt animals before running out of food
4. Don't let hunger reach 0!

---

## 🎮 Survival Strategies

### Early Game (Starting)

**Goals:**
- Kill passive mobs (pigs, cows, chickens)
- Collect 20+ Raw Meat
- Eat when hunger drops below 15

**Food Priority:**
1. Hunt cows/pigs (guaranteed 1-3 meat)
2. Avoid chickens (unreliable 0-2 drops)
3. Don't eat unless hungry (<15 hunger)

**Tip:** You start with full hunger - hunt first, eat later!

---

### Mid Game (Combat)

**Goals:**
- Maintain >18 hunger for fast regeneration
- Keep 10+ food in inventory
- Eat after taking damage to heal quickly

**Combat Strategy:**
1. Eat before big fights (get to 20 hunger)
2. Fight mobs to get food drops
3. Retreat if hunger drops below 10
4. Eat between encounters

**Tip:** Fast regen at >18 hunger means you can tank more hits!

---

### Late Game (Exploration)

**Goals:**
- Always carry 20+ food
- Never drop below 15 hunger
- Cook meat for better saturation (future)

**Efficiency:**
- Raw Meat: 3 hunger, eat every 30 seconds if active
- Cooked Meat: 8 hunger, eat every 80 seconds
- Bread: 5 hunger, eat every 50 seconds

**Tip:** High saturation food = less frequent eating = more time exploring!

---

## 📈 Hunger Management Tips

### Conservation

**Don't Waste Food:**
- Only eat when hunger < 15 (no benefit if already high)
- Eating at 19/20 hunger wastes 1 hunger point
- Save cooked meat for emergencies (high restoration)

**Optimal Eating:**
- Raw Meat: Eat at 14-17 hunger (maximizes 3 points)
- Cooked Meat: Eat at 10-12 hunger (maximizes 8 points)
- Bread: Eat at 13-15 hunger (maximizes 5 points)

---

### Food Stockpiling

**Minimum Reserves:**
- Early Game: 10 food items
- Mid Game: 20 food items
- Late Game: 40 food items

**Where to Store:**
- Hotbar slots for easy access
- Use chest storage (future feature)
- Drop low-priority items to make room

**Hunting Efficiency:**
- 1 Cow/Pig = 1-3 meat (average 2)
- 10 Cows = ~20 meat
- 20 meat = 6 minutes of survival

---

### Emergency Procedures

**When Hunger < 6:**
1. **Stop Fighting**: Retreat immediately
2. **Eat Food**: Consume all available food
3. **Hunt Passive Mobs**: Safest food source
4. **Avoid Hostile Mobs**: Can't regenerate anyway

**When Starving (0 hunger):**
1. **EAT NOW**: Even raw meat
2. **Run to Safety**: Avoid all combat
3. **Health Check**: Monitor HP closely
4. **Find More Food**: Hunt immediately after eating

---

## 🔄 Hunger-Health Cycle

### The Survival Loop

```
Start: Full Health (20 HP) + Full Hunger (20)
   ↓
1. Explore & Fight
   - Hunger drains (0.1/sec)
   - Take damage from mobs
   ↓
2. Hunger Drops Below 15
   - Eat food (press R)
   - Hunger restored (+3 to +8)
   ↓
3. Health Regenerates
   - Wait 3 seconds after damage
   - Regenerate at 0.3-1.0 HP/sec
   ↓
4. Back to Full Health & Hunger
   - Ready for more exploration
   - Repeat cycle
```

---

### Synergy with Combat

**Before Combat:**
- Eat to get hunger > 18
- Enables fast regen (1 HP/sec)
- Can heal 5 HP in 5 seconds after hit

**During Combat:**
- Don't eat (need weapon equipped)
- Focus on dealing damage
- Use kiting to avoid hits

**After Combat:**
- Wait 3 seconds
- Auto-regen kicks in
- Full health in 20 seconds

---

## 🎯 Advanced Hunger Tactics

### Regen Tanking

**Strategy:**
- Maintain >18 hunger always
- Take hits knowing you'll regen quickly
- Aggressive playstyle viable

**Math:**
- Zombie hits for 3 HP
- You regen 1 HP/sec
- Net loss: 2 HP per hit if you can space hits 1 second apart
- Can tank 10 zombie hits before needing to retreat

---

### Hunger Rushing

**Strategy:**
- Let hunger drop to 10-12
- Eat just before combat
- Saves food for when you need regen

**Math:**
- Save ~50% of food
- Still have slow regen (0.3 HP/sec) for exploration
- Fast regen available on-demand

---

### Food Rationing

**Strategy:**
- Different food for different situations
- Raw Meat: Top off hunger (12-17)
- Cooked Meat: Emergency healing (below 12)
- Bread: General use (10-15)

**Efficiency:**
- Maximizes hunger restored per food
- Minimizes waste
- Extends food supply

---

## 📊 Hunger Statistics

### Consumption Rates

**Activity-Based Drain:**
- Current: Constant 0.1/second
- Future: Variable based on activity
  - Sprinting: 0.3/second
  - Fighting: 0.2/second
  - Walking: 0.1/second
  - Standing: 0.05/second

**Time to Empty:**
- From Full (20): 200 seconds (3m 20s)
- From Good (15): 150 seconds (2m 30s)
- From Decent (10): 100 seconds (1m 40s)

---

### Food Efficiency

**Hunger Points per Food:**
| Food | Hunger/Item | Items Needed for Full | Cost-Effectiveness |
|------|-------------|----------------------|-------------------|
| Cooked Meat | 8 | 2.5 | ⭐⭐⭐⭐⭐ Best |
| Bread | 5 | 4 | ⭐⭐⭐⭐ Good |
| Apple | 4 | 5 | ⭐⭐⭐ Okay |
| Raw Meat | 3 | 6.7 | ⭐⭐ Poor |

**Saturation Points per Food:**
| Food | Saturation/Item | Duration | Efficiency |
|------|----------------|----------|------------|
| Cooked Meat | 12.8 | 128 seconds | ⭐⭐⭐⭐⭐ Best |
| Bread | 6.0 | 60 seconds | ⭐⭐⭐⭐ Good |
| Apple | 2.4 | 24 seconds | ⭐⭐ Poor |
| Raw Meat | 1.8 | 18 seconds | ⭐ Worst |

---

## 🐛 Known Mechanics & Limitations

### Current State
- No visual hunger bar (console only)
- All activity drains hunger at same rate
- Can't cook food yet (planned feature)
- No hunger loss from healing (may be added)

### Planned Improvements
- Visual hunger bar UI
- Activity-based drain rates
- Cooking system for raw → cooked food
- Farming for bread ingredients
- Food quality/poisoning mechanics
- Hunger affects mining/movement speed

---

## 🎮 Controls Reference

| Key | Action |
|-----|--------|
| **R** | Eat selected food item |
| **1-9** | Select hotbar slot (food must be selected) |
| **E** | Open inventory (check food supplies) |

**Tips:**
- Keep food in easy-to-reach slots (1-3)
- Use middle slots (4-6) for tools/weapons
- Use far slots (7-9) for blocks/building materials

---

## 💡 Pro Survival Tips

1. **Always Carry Food**: Minimum 10 items at all times
2. **Eat Before Combat**: Get to >18 hunger for fast regen
3. **Hunt Proactively**: Don't wait until starving
4. **Watch Hunger**: Check console logs frequently
5. **Emergency Slots**: Dedicate hotbar slot 1 to food
6. **Hoard Food**: More is always better
7. **Cook Later**: Save raw meat for future cooking system
8. **Regen Smart**: Wait for fast regen before re-engaging
9. **Retreat When Low**: <6 hunger = no regen = death
10. **Practice Hunting**: Learn mob patterns for safe food farming

---

## 🔮 Future Features

### Coming Soon
- **Visual Hunger Bar**: On-screen display next to health
- **Cooking System**: Furnace to cook raw → cooked
- **Farming**: Grow wheat for bread
- **Food Quality**: Different tiers beyond raw/cooked
- **Hunger Effects**: Speed/mining affected by hunger
- **Food Poisoning**: Rotten food damages instead of heals

### Long-Term
- **Nutrition System**: Balanced diet bonuses
- **Food Buffs**: Temporary stat boosts
- **Brewing**: Potions for hunger/health
- **Fishing**: New food source
- **Trading**: Buy/sell food with villagers

---

**Stay Fed. Stay Alive.** 🍖

For more guides, see:
- `HOSTILE_MOBS_GUIDE.md` - Combat and mob drops
- `CRAFTING_RECIPES.md` - All crafting recipes
- `DEMO_GUIDE.md` - Complete game overview
