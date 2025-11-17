# mdminecraft - Hostile Mobs & Combat Guide

## 🗡️ Hostile Mob System Overview

mdminecraft now features hostile mobs that actively hunt and attack the player, creating a survival challenge and combat gameplay loop.

---

## 👹 Hostile Mob Types

### **Zombie** 🧟
- **Health**: 20 HP (tanky)
- **Damage**: 3 HP per attack
- **Speed**: 0.15 blocks/tick (slow but relentless)
- **Detection Range**: 16 blocks
- **Attack Range**: 2 blocks (melee)
- **Attack Cooldown**: 1 second (20 ticks)
- **Size**: 0.6 blocks (human-sized)

**Behavior:**
- Slow-moving melee attacker
- Very tanky - takes multiple hits to kill
- Chases player persistently within range
- Attacks every second when in melee range

**Loot Drops:**
- Rotten Flesh: 0-2 pieces
- Stick: 0-1 (rare drop)

**Where They Spawn:**
- Plains (weight: 3)
- Forest/Birch Forest (weight: 5)
- Savanna (weight: 2)

---

### **Skeleton** 🏹
- **Health**: 15 HP (medium)
- **Damage**: 2 HP per attack
- **Speed**: 0.25 blocks/tick (faster than zombies)
- **Detection Range**: 20 blocks
- **Attack Range**: 12 blocks (ranged - future)
- **Attack Cooldown**: 1 second
- **Size**: 0.5 blocks

**Behavior:**
- Faster, more agile than zombies
- Longer detection range
- Currently melee (ranged attacks planned)
- More dangerous in groups

**Loot Drops:**
- Bone: 0-2 pieces
- Arrow: 0-2 pieces

**Where They Spawn:**
- Forest/Birch Forest (weight: 4)
- Hills (weight: 3)

---

## 🧠 Combat AI System

### Detection & Aggro

**How Hostiles Find You:**
1. **Idle/Wandering State**: Mobs wander randomly
2. **Player Detection**: When player enters detection range (16-20 blocks)
3. **State Change**: Mob transitions to `Chasing` state
4. **Pursuit**: Mob moves directly toward player

**Detection Ranges:**
- Zombie: 16 blocks
- Skeleton: 20 blocks (better vision)

**Line of Sight**: Currently mobs don't require line of sight - they detect through walls

---

### Chase Behavior

**When Chasing:**
- Mob moves directly toward player position
- Updates path every tick
- Moves at full movement speed
- Will chase until:
  - Player moves out of range (1.5× detection range)
  - Mob enters attack range

**Pathfinding:**
- Direct line pathfinding (no obstacle avoidance yet)
- Calculates horizontal distance only (ignores Y-axis for movement)
- Normalizes velocity vector for consistent speed

---

### Attack Behavior

**Attack Mechanics:**
- **Trigger**: When mob is within attack range of player
- **Cooldown**: 20 ticks (1 second) between attacks
- **Damage**: Applied instantly when attack triggers
- **Player Invulnerability**: 0.5 second invuln frames prevent spam damage

**Attack Range:**
- Zombie: 2 blocks (melee)
- Skeleton: 12 blocks (ranged - future feature)

**While Attacking:**
- Mob continues moving toward player at half speed
- If player moves away > 1.2× attack range, mob returns to chasing
- Attack timer resets each successful hit

---

### De-Aggro Conditions

**Mobs Return to Wandering When:**
- Player moves beyond 1.5× detection range:
  - Zombie: 24 blocks
  - Skeleton: 30 blocks
- Mob health reaches 0 (death)

**Upon De-Aggro:**
- Velocity reset to zero
- State changes to `Wandering`
- AI timer resets
- Mob will resume normal passive behavior

---

## ⚔️ Combat Mechanics

### Damage System

**Player Damage Output:**
| Weapon | Damage | Notes |
|--------|--------|-------|
| Bare Hands | 1 HP | Ineffective against hostiles |
| Wood Pickaxe | 2 HP | 10 hits to kill Zombie |
| Stone Pickaxe | 3 HP | 7 hits to kill Zombie |
| Iron Pickaxe | 4 HP | 5 hits to kill Zombie |
| Wood Axe | 3 HP | Better for combat |
| Stone Axe | 4 HP | |
| Iron Axe | 5 HP | |
| Wood Sword | 4 HP | Best early weapon |
| Stone Sword | 5 HP | |
| Iron Sword | 6 HP | Best overall weapon |

**Mob Damage Output:**
| Mob | Damage | Hits to Kill Player (20 HP) |
|-----|--------|------------------------------|
| Zombie | 3 HP | 7 hits |
| Skeleton | 2 HP | 10 hits |

**Player Invulnerability:**
- 0.5 second (10 ticks) invuln after taking damage
- Prevents multiple simultaneous hits
- Visual feedback in console logs

---

### Combat Strategies

**Fighting Zombies:**
1. **Kite & Hit**: Back away after each hit to avoid damage
2. **Use Reach**: Attack from just outside melee range, then retreat
3. **High Ground**: Zombies can't jump - use terrain
4. **Weapon Choice**: Stone+ pickaxe minimum, swords preferred
5. **Group Management**: Fight one at a time, use chokepoints

**Fighting Skeletons:**
1. **Close the Gap**: Move in quickly before they can "shoot" (future)
2. **Zigzag Movement**: Avoid predictable straight-line approach
3. **Cover**: Use trees and terrain to block line of sight
4. **Speed**: Faster than zombies, so retreat if needed
5. **Kill Priority**: High - they're dangerous at range

**Multi-Mob Combat:**
1. **Separate**: Pull mobs apart to fight 1v1
2. **Retreat**: Fall back to safe area if overwhelmed
3. **Hotbar Ready**: Keep weapon selected at all times
4. **Health Check**: Monitor health display regularly

---

## 🎁 Loot Drop System

### How Loot Works

**Drop Mechanics:**
1. **Trigger**: When mob health reaches 0
2. **Generation**: Loot table rolled for each mob type
3. **Deterministic Random**: Based on game tick (consistent seed-based)
4. **Auto-Collect**: Drops directly into player hotbar
5. **Stack Merging**: Combines with existing stacks if possible

**Drop Timing:**
- Instant upon mob death
- No physical drop entities (direct to inventory)
- Console log shows what was looted

**Inventory Full:**
- Warning message in console
- Loot is lost if no space available
- Clear hotbar slots before combat for safety

---

### Loot Tables

#### Passive Mobs

**Pig**
- Raw Meat: 1-3 pieces (guaranteed)

**Cow**
- Raw Meat: 1-3 pieces (guaranteed)

**Sheep**
- Wool: 1-2 blocks (Item ID 1001)

**Chicken**
- Raw Meat: 0-2 pieces (not guaranteed)

#### Hostile Mobs

**Zombie**
- Rotten Flesh: 0-2 pieces (Food - currently displayed as Raw Meat)
- Stick: 0-1 (rare drop, 50% chance)

**Skeleton**
- Bone: 0-2 pieces (Item ID 1002)
- Arrow: 0-2 pieces (Item ID 1003)

---

### Loot Uses

**Food Items (Raw Meat):**
- Future: Restore health/hunger
- Currently: Collectible resource
- Stackable: Yes (up to 64)

**Crafting Materials:**
- Bones: Future bone meal crafting
- Arrows: Future bow ammunition
- Wool: Future decorative blocks

**Trading/Currency:**
- Future: Villager trading system
- Future: Shop purchases

---

## 🎯 Visual Indicators

### Hostile Mob Labels

**What You See:**
```
Zombie [HOSTILE]
[██████████] 20/20 <--
```

**Label Components:**
1. **Mob Name**: "Zombie" or "Skeleton"
2. **[HOSTILE] Tag**: Red label identifying dangerous mobs
3. **Health Bar**: █ characters showing current health
4. **HP Numbers**: "20/20" current/max health
5. **Arrow**: "<--" when mob is targeted

**Color Coding:**
- **Hostile Mobs**: Bright red (1.0, 0.2, 0.2) - always red regardless of health
- **Passive Mobs**: Health-based coloring
  - Green (>66% HP)
  - Yellow (33-66% HP)
  - Red (<33% HP)

**Health Bar Symbols:**
- █ = Filled health
- ░ = Missing health
- Length: 10 characters total

---

## 🌍 Spawn System

### Biome Distribution

**Plains:**
- Pig: 10
- Cow: 8
- Sheep: 12
- Chicken: 10
- **Zombie: 3** ⚔️

**Forest / Birch Forest:**
- Pig: 8
- Cow: 4
- Chicken: 10
- **Zombie: 5** ⚔️
- **Skeleton: 4** ⚔️

**Hills:**
- Sheep: 15
- Cow: 5
- **Skeleton: 3** ⚔️

**Savanna:**
- Cow: 6
- Chicken: 8
- **Zombie: 2** ⚔️

**No Hostile Spawns:**
- Desert
- Tundra
- Ocean
- Extreme Hills
- Swamp

---

### Spawn Mechanics

**When Mobs Spawn:**
- During chunk generation
- Mobs are placed on surface blocks
- Height determined by terrain elevation
- Persistent across game session

**Spawn Density:**
- ~2-5 hostile mobs per chunk in populated biomes
- Forest biomes have highest hostile density
- Passive mobs spawn alongside hostiles

**Spawn Locations:**
- Surface level only (top non-air block)
- Biome-appropriate positions
- Deterministic based on world seed

---

## 🎮 Gameplay Integration

### Combat Loop

```
1. Explore World
   ↓
2. Encounter Hostile Mob (detection range)
   ↓
3. Mob Chases Player
   ↓
4. Choose: Fight or Flight
   ↓
5a. Fight:
    - Attack with weapon
    - Take damage (invuln frames)
    - Kill mob → Loot drops
    - Loot auto-collected

5b. Flight:
    - Run beyond de-aggro range
    - Mob returns to wandering
    - Regroup and prepare
   ↓
6. Use Loot for Crafting
   ↓
7. Craft Better Weapons
   ↓
8. Return to Combat (stronger)
```

---

### Progression Path

**Early Game (Starting Equipment):**
- Wood Pickaxe (2 damage)
- Strategy: Avoid zombies, run from groups
- Goal: Kill passive mobs for food

**Mid Game (Stone Tools):**
- Stone Pickaxe (3 damage)
- Strategy: Fight zombies 1v1, kite skeletons
- Goal: Collect loot, craft better tools

**Late Game (Iron Tools):**
- Iron Sword (6 damage)
- Strategy: Aggressive combat, clear areas
- Goal: Farm hostile mobs for resources

---

## 📊 Combat Statistics

### Time to Kill (TTK)

**Zombie (20 HP):**
| Weapon | Hits Required | Time (seconds) |
|--------|---------------|----------------|
| Bare Hands (1) | 20 | 40s+ |
| Wood Pick (2) | 10 | 20s+ |
| Stone Pick (3) | 7 | 14s+ |
| Iron Pick (4) | 5 | 10s+ |
| Stone Axe (4) | 5 | 10s+ |
| Iron Sword (6) | 4 | 8s+ |

**Skeleton (15 HP):**
| Weapon | Hits Required | Time (seconds) |
|--------|---------------|----------------|
| Wood Pick (2) | 8 | 16s+ |
| Stone Pick (3) | 5 | 10s+ |
| Iron Sword (6) | 3 | 6s+ |

*Note: Times assume no missed hits and optimal attack timing*

---

### Danger Ratings

**Solo Encounters:**
- 1 Zombie: ⚠️ Medium Threat (manageable with stone tools)
- 1 Skeleton: ⚠️⚠️ High Threat (faster, ranged capability)
- 2+ Zombies: ⚠️⚠️⚠️ Extreme Threat (overwhelming damage)

**Group Encounters:**
- 3+ Mobs: 🔴 FLEE (guaranteed death without iron+ gear)
- Mixed Groups: 🔴 FLEE (skeletons + zombies = deadly combo)

---

## 💡 Pro Tips

### Combat Tips

1. **Always Keep Weapon Selected**: Set weapon to hotbar slot 1
2. **Know Your Escape Route**: Identify retreat path before engaging
3. **Use the Terrain**: High ground, chokepoints, water
4. **Count Your Hits**: Track damage dealt to know when mob will die
5. **Monitor Health**: Check HUD frequently during combat
6. **Clear Hotbar Space**: Make room for loot before fights
7. **Attack Between Cooldowns**: Time your hits for maximum DPS
8. **Retreat When Low**: Health doesn't regenerate (yet)

### Survival Tips

1. **Night is Dangerous**: More hostiles spawn (future feature)
2. **Stay Mobile**: Standing still = getting surrounded
3. **Build Shelters**: Safe zones to retreat to
4. **Craft Extras**: Keep backup weapons in hotbar
5. **Loot Management**: Drop useless items to make room
6. **Sound Awareness**: Future - mob sounds indicate danger
7. **Light Sources**: Future - torches prevent spawns

### Farming Tips

1. **Hunt Zombies for Sticks**: Rare drop but useful for crafting
2. **Skeleton Bones**: Future bone meal for crops
3. **Meat from Passives**: Safer food source than fighting zombies
4. **Group Spawn Areas**: Farm high-density biomes (forests)
5. **Kite Into Traps**: Future - environmental damage

---

## 🔧 Technical Details

### AI State Machine

```
State: Idle
├─> Wander randomly (40-80 ticks)
├─> Detect player? → Chasing
└─> Continue idle

State: Wandering
├─> Move in direction (20-60 ticks)
├─> Detect player? → Chasing
└─> Return to Idle

State: Chasing
├─> Move toward player
├─> Out of range? → Wandering
├─> In attack range? → Attacking
└─> Continue chasing

State: Attacking
├─> Attack every 20 ticks
├─> Move toward player (half speed)
├─> Player escapes? → Chasing
└─> Continue attacking
```

### Damage Calculation

```rust
// Player attacks mob
let damage = weapon_damage_table[weapon_type][material];
mob.damage(damage);
if mob.is_dead() {
    generate_loot(mob.type);
    remove_mob(mob);
}

// Mob attacks player
if ai_timer >= 20 {
    let damage = mob_type.attack_damage();
    player.damage(damage);
    ai_timer = 0; // Reset cooldown
}
```

### Loot Generation

```rust
// Deterministic pseudo-random
let seed = current_tick;
for (item, min, max) in loot_table {
    let count = (seed * 48271) % (max - min + 1) + min;
    if count > 0 {
        try_add_to_hotbar(ItemStack::new(item, count));
    }
}
```

---

## 🐛 Known Limitations

### Current State
- Skeletons use melee attacks (ranged arrows not implemented yet)
- No spawn caps (mobs accumulate over time)
- Mobs can't jump or pathfind around obstacles
- No mob despawning system
- Loot lost if hotbar full (no ground drops)
- No visual attack animations
- No sound effects

### Planned Improvements
- Arrow projectiles for skeletons
- Mob spawning limits per chunk/biome
- Basic pathfinding around walls
- Despawn distant mobs to prevent lag
- Ground item drops with pickup mechanic
- Attack swing animations
- Combat sound effects
- Day/night spawn rate modifiers

---

## 🎯 Future Features

### Short-Term (Next Updates)
- Skeleton ranged attacks with arrow projectiles
- Mob spawn caps to prevent overcrowding
- Ground loot drops (visual entities)
- Attack animations for both player and mobs
- Combat sound effects
- Mob death animations

### Mid-Term
- New hostile types: Spider, Creeper
- Boss mobs with unique mechanics
- Mob equipment (zombies with armor)
- Rare drops and treasure
- Experience/leveling system
- Mob AI improvements (pathfinding)

### Long-Term
- Mob villages and structures
- Mob factions and diplomacy
- Tameable hostile mobs
- Mob breeding system
- Dynamic difficulty scaling
- Multiplayer mob synchronization

---

**Survive. Fight. Conquer.** ⚔️

For more guides, see:
- `COMBAT_GUIDE.md` - Combat mechanics for all mobs
- `DEMO_GUIDE.md` - Complete game overview
- `CRAFTING_RECIPES.md` - Weapon crafting guide
