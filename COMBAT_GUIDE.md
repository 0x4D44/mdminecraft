# mdminecraft - Combat System Guide

## 🗡️ Combat Overview

You can now fight the passive mobs! Each mob has health and can be defeated with weapons or bare hands.

## 🎯 How to Fight Mobs

### Finding Mobs
1. Walk around the world
2. Look for floating labels showing mob names
3. Mobs spawn in biome-appropriate locations:
   - **Plains**: Pigs, Cows, Sheep, Chickens
   - **Forest**: Pigs, Chickens
   - **Hills**: Sheep, Cows

### Targeting System
1. Aim your crosshair at a mob
2. When targeted, the label shows an arrow: `<--`
3. Example: `Pig\n[██████░░░░] 6/10 <--`
4. Maximum range: 8 blocks

### Attacking
1. Look directly at a mob (until you see the `<--` indicator)
2. **Left-click** to attack
3. Damage appears in console
4. Health bar updates in real-time

## 💪 Weapon Damage

### Bare Hands
- **Damage**: 1 per hit
- **Total Hits to Kill**:
  - Chicken: 4 hits
  - Sheep: 8 hits
  - Pig/Cow: 10 hits

### Pickaxes (Mining Tools)
- **Wood Pickaxe**: 2 damage per hit
- **Stone Pickaxe**: 3 damage per hit
- **Iron Pickaxe**: 4 damage per hit

### Axes (Chopping Tools)
- **Wood Axe**: 3 damage per hit
- **Stone Axe**: 4 damage per hit
- **Iron Axe**: 5 damage per hit

### Swords (Combat Weapons - if added)
- **Wood Sword**: 4 damage per hit
- **Stone Sword**: 5 damage per hit
- **Iron Sword**: 6 damage per hit

## 🏥 Mob Health

Each mob type has different health:

| Mob | Health | Hits (Bare Hands) | Hits (Iron Pickaxe) |
|-----|--------|-------------------|---------------------|
| **Chicken** | 4 HP | 4 | 1 |
| **Sheep** | 8 HP | 8 | 2 |
| **Pig** | 10 HP | 10 | 3 |
| **Cow** | 10 HP | 10 | 3 |

## 📊 Health Display

### Health Bar Colors
- **Green** [████████░░]: >66% health (healthy)
- **Yellow** [████░░░░░░]: 33-66% health (wounded)
- **Red** [██░░░░░░░░]: <33% health (critical)

### Health Bar Format
```
Pig
[██████████] 10/10 <--
```
- First line: Mob name
- Second line: Health bar + numbers + targeting arrow

### Example Combat Sequence
```
1. Initial:  Pig [██████████] 10/10 <--
2. Hit once: Pig [████████░░] 8/10 <--  (2 damage with wood pickaxe)
3. Hit twice: Pig [██████░░░░] 6/10 <--
4. Hit 3x:   Pig [████░░░░░░] 4/10 <--  (Yellow color)
5. Hit 4x:   Pig [██░░░░░░░░] 2/10 <--  (Red color)
6. Hit 5x:   Pig [░░░░░░░░░░] 0/10 <--
7. Death:    "Pig died!" - mob and label removed
```

## 🎮 Combat Tips

### Effective Combat
1. **Use the right tool**: Iron pickaxe does 4x damage of bare hands
2. **Get close**: 8-block range means you need to be fairly near
3. **Aim center mass**: Targeting works best when aimed at mob center
4. **Watch the health**: The `<--` arrow confirms you're locked on

### Console Feedback
Every attack shows in the console:
```
INFO: Hit Pig for 2.0 damage! Health: 8.0/10.0
INFO: Hit Pig for 2.0 damage! Health: 6.0/10.0
INFO: Hit Pig for 2.0 damage! Health: 4.0/10.0
INFO: Hit Pig for 2.0 damage! Health: 2.0/10.0
INFO: Hit Pig for 2.0 damage! Health: 0.0/10.0
INFO: Pig died!
INFO: Removing dead Pig
```

### Strategy
- **Chickens**: Easy to kill (4 HP), good for testing
- **Sheep**: Medium difficulty (8 HP)
- **Pigs/Cows**: Tougher (10 HP), use better weapons

## 🔧 Technical Details

### Raycasting
- Uses sphere collision detection
- Mob hitbox = mob size (0.3-0.7 blocks radius)
- Finds closest mob within 8 blocks
- Sub-millisecond performance

### Damage Calculation
```rust
let damage = match (tool_type, material) {
    (Pickaxe, Wood) => 2.0,
    (Pickaxe, Stone) => 3.0,
    (Pickaxe, Iron) => 4.0,
    (Axe, Wood) => 3.0,
    // ... etc
    _ => 1.0  // Bare hands
};
```

### Death Handling
1. Check `mob.health <= 0.0`
2. Log death message
3. Remove mob from Vec
4. Remove UI label
5. Update targeted_mob index
6. Clean memory

## 🐛 Troubleshooting

**Not hitting the mob:**
- Check for `<--` arrow in label
- Get closer (within 8 blocks)
- Aim at center of mob
- Check console for "Hit" messages

**No damage showing:**
- Verify mob health in label
- Check weapon in hotbar
- Console should show damage numbers

**Mob not dying:**
- Check health reaches 0.0
- Should see "died!" message
- Mob and label should disappear

## 🎯 Challenge Ideas

### Time Trials
- Kill 5 chickens as fast as possible
- Kill a cow with bare hands only
- Kill 10 mobs without missing

### Weapon Tests
- Compare damage of different tools
- Find most efficient weapon
- Test attack speed limits

### Survival
- See how long you can last against mobs
- (Note: passive mobs don't attack back... yet!)

## 🚀 Future Enhancements

**Planned Features:**
- Hostile mobs (zombies, skeletons)
- Mob knockback on hit
- Mob drops (meat, wool, etc.)
- Mob retaliation/aggro
- Critical hits
- Enchantments
- Mob armor/resistance

---

**Happy Hunting!** 🏹
