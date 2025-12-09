//! Potion and status effect system.
//!
//! Provides status effects that can be applied to players/mobs,
//! potion types, and brewing recipes.

use serde::{Deserialize, Serialize};

/// Status effect types that can be applied to entities.
/// Each effect has a level (amplifier) that affects its strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusEffectType {
    // Positive effects
    /// Increases movement speed (20% per level)
    Speed,
    /// Increases mining speed
    Haste,
    /// Increases attack damage (+3 per level)
    Strength,
    /// Instantly restores health (4 HP per level)
    InstantHealth,
    /// Increases jump height
    JumpBoost,
    /// Restores health over time (1 HP every 2.5 seconds)
    Regeneration,
    /// Reduces damage taken (20% per level)
    Resistance,
    /// Prevents fire damage
    FireResistance,
    /// Allows breathing underwater
    WaterBreathing,
    /// Makes entity invisible
    Invisibility,
    /// Allows seeing in the dark
    NightVision,
    /// Adds extra temporary health
    Absorption,
    /// Restores hunger
    Saturation,
    /// Slows falling speed
    SlowFalling,
    /// Increases luck for loot
    Luck,

    // Negative effects
    /// Decreases movement speed (15% per level)
    Slowness,
    /// Decreases mining speed
    MiningFatigue,
    /// Instantly deals damage (3 HP per level)
    InstantDamage,
    /// Causes damage to jump
    Nausea,
    /// Reduces visibility
    Blindness,
    /// Increases hunger depletion
    Hunger,
    /// Decreases attack damage (-4 per level)
    Weakness,
    /// Deals damage over time (1 HP every 1.25 seconds)
    Poison,
    /// Deals damage over time, can kill (1 HP every 2 seconds)
    Wither,
    /// Decreases luck for loot
    BadLuck,
}

impl StatusEffectType {
    /// Get the maximum amplifier (level) for this effect.
    /// Level 1 = amplifier 0, Level 2 = amplifier 1, etc.
    pub fn max_amplifier(&self) -> u8 {
        match self {
            StatusEffectType::Speed => 2,        // Speed I-III
            StatusEffectType::Haste => 2,        // Haste I-III
            StatusEffectType::Strength => 2,     // Strength I-III
            StatusEffectType::InstantHealth => 1, // Instant Health I-II
            StatusEffectType::JumpBoost => 1,    // Jump Boost I-II
            StatusEffectType::Regeneration => 1, // Regen I-II
            StatusEffectType::Resistance => 3,   // Resistance I-IV
            StatusEffectType::FireResistance => 0, // Fire Resistance (no levels)
            StatusEffectType::WaterBreathing => 0,
            StatusEffectType::Invisibility => 0,
            StatusEffectType::NightVision => 0,
            StatusEffectType::Absorption => 3,
            StatusEffectType::Saturation => 0,
            StatusEffectType::SlowFalling => 0,
            StatusEffectType::Luck => 0,
            StatusEffectType::Slowness => 3,     // Slowness I-IV
            StatusEffectType::MiningFatigue => 2,
            StatusEffectType::InstantDamage => 1, // Instant Damage I-II
            StatusEffectType::Nausea => 0,
            StatusEffectType::Blindness => 0,
            StatusEffectType::Hunger => 2,
            StatusEffectType::Weakness => 0,
            StatusEffectType::Poison => 1,       // Poison I-II
            StatusEffectType::Wither => 1,       // Wither I-II
            StatusEffectType::BadLuck => 0,
        }
    }

    /// Check if this effect is positive (beneficial).
    pub fn is_positive(&self) -> bool {
        matches!(
            self,
            StatusEffectType::Speed
                | StatusEffectType::Haste
                | StatusEffectType::Strength
                | StatusEffectType::InstantHealth
                | StatusEffectType::JumpBoost
                | StatusEffectType::Regeneration
                | StatusEffectType::Resistance
                | StatusEffectType::FireResistance
                | StatusEffectType::WaterBreathing
                | StatusEffectType::Invisibility
                | StatusEffectType::NightVision
                | StatusEffectType::Absorption
                | StatusEffectType::Saturation
                | StatusEffectType::SlowFalling
                | StatusEffectType::Luck
        )
    }

    /// Check if this effect is instant (applied once, not over time).
    pub fn is_instant(&self) -> bool {
        matches!(
            self,
            StatusEffectType::InstantHealth
                | StatusEffectType::InstantDamage
                | StatusEffectType::Saturation
        )
    }
}

/// An active status effect with duration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatusEffect {
    /// The type of effect.
    pub effect_type: StatusEffectType,
    /// The amplifier (level - 1). 0 = Level I, 1 = Level II, etc.
    pub amplifier: u8,
    /// Duration in game ticks (20 ticks = 1 second). 0 for instant effects.
    pub duration_ticks: u32,
    /// Whether particles should be shown.
    pub show_particles: bool,
}

impl StatusEffect {
    /// Create a new status effect.
    pub fn new(effect_type: StatusEffectType, amplifier: u8, duration_ticks: u32) -> Self {
        let max_amp = effect_type.max_amplifier();
        Self {
            effect_type,
            amplifier: amplifier.min(max_amp),
            duration_ticks,
            show_particles: true,
        }
    }

    /// Create an instant effect (like Instant Health).
    pub fn instant(effect_type: StatusEffectType, amplifier: u8) -> Self {
        Self::new(effect_type, amplifier, 0)
    }

    /// Get the effect level (1-based).
    pub fn level(&self) -> u8 {
        self.amplifier + 1
    }

    /// Update the effect, reducing duration. Returns true if effect expired.
    pub fn tick(&mut self) -> bool {
        if self.effect_type.is_instant() {
            return true; // Instant effects expire immediately after application
        }
        if self.duration_ticks > 0 {
            self.duration_ticks -= 1;
        }
        self.duration_ticks == 0
    }

    /// Get duration in seconds.
    pub fn duration_seconds(&self) -> f32 {
        self.duration_ticks as f32 / 20.0
    }
}

/// Collection of active status effects on an entity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusEffects {
    /// Active effects (only one of each type allowed).
    effects: Vec<StatusEffect>,
}

impl StatusEffects {
    /// Create empty status effects.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or upgrade a status effect.
    /// If an effect of the same type exists:
    /// - If new effect has higher amplifier, replace it
    /// - If same amplifier, extend duration
    pub fn add(&mut self, effect: StatusEffect) {
        if let Some(existing) = self
            .effects
            .iter_mut()
            .find(|e| e.effect_type == effect.effect_type)
        {
            if effect.amplifier > existing.amplifier {
                // Higher level replaces
                *existing = effect;
            } else if effect.amplifier == existing.amplifier
                && effect.duration_ticks > existing.duration_ticks
            {
                // Same level, longer duration extends
                existing.duration_ticks = effect.duration_ticks;
            }
            // Lower level or shorter duration: ignore
        } else {
            self.effects.push(effect);
        }
    }

    /// Remove an effect by type.
    pub fn remove(&mut self, effect_type: StatusEffectType) {
        self.effects.retain(|e| e.effect_type != effect_type);
    }

    /// Clear all effects.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Check if an effect is active.
    pub fn has(&self, effect_type: StatusEffectType) -> bool {
        self.effects.iter().any(|e| e.effect_type == effect_type)
    }

    /// Get the amplifier of an effect (None if not active).
    pub fn amplifier(&self, effect_type: StatusEffectType) -> Option<u8> {
        self.effects
            .iter()
            .find(|e| e.effect_type == effect_type)
            .map(|e| e.amplifier)
    }

    /// Get an effect by type.
    pub fn get(&self, effect_type: StatusEffectType) -> Option<&StatusEffect> {
        self.effects.iter().find(|e| e.effect_type == effect_type)
    }

    /// Update all effects, removing expired ones.
    /// Returns a list of effects that expired this tick.
    pub fn tick(&mut self) -> Vec<StatusEffectType> {
        let mut expired = Vec::new();
        self.effects.retain_mut(|effect| {
            if effect.tick() {
                expired.push(effect.effect_type);
                false
            } else {
                true
            }
        });
        expired
    }

    /// Get iterator over active effects.
    pub fn iter(&self) -> impl Iterator<Item = &StatusEffect> {
        self.effects.iter()
    }

    /// Check if any effects are active.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get speed multiplier from Speed/Slowness effects.
    pub fn speed_multiplier(&self) -> f32 {
        let mut multiplier = 1.0;

        if let Some(amp) = self.amplifier(StatusEffectType::Speed) {
            // Speed: +20% per level
            multiplier *= 1.0 + 0.2 * (amp + 1) as f32;
        }

        if let Some(amp) = self.amplifier(StatusEffectType::Slowness) {
            // Slowness: -15% per level
            multiplier *= 1.0 - 0.15 * (amp + 1) as f32;
        }

        multiplier.max(0.0)
    }

    /// Get attack damage modifier from Strength/Weakness effects.
    pub fn attack_damage_modifier(&self) -> f32 {
        let mut modifier = 0.0;

        if let Some(amp) = self.amplifier(StatusEffectType::Strength) {
            // Strength: +3 damage per level
            modifier += 3.0 * (amp + 1) as f32;
        }

        if let Some(amp) = self.amplifier(StatusEffectType::Weakness) {
            // Weakness: -4 damage per level
            modifier -= 4.0 * (amp + 1) as f32;
        }

        modifier
    }

    /// Get damage reduction multiplier from Resistance effect.
    pub fn damage_reduction(&self) -> f32 {
        if let Some(amp) = self.amplifier(StatusEffectType::Resistance) {
            // Resistance: 20% reduction per level
            let reduction = 0.2 * (amp + 1) as f32;
            return 1.0 - reduction.min(0.8); // Cap at 80% reduction
        }
        1.0
    }
}

/// Types of potions that can be brewed and consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PotionType {
    // Base potions (no effect)
    /// Empty glass bottle
    Water,
    /// Brewed with Nether Wart, base for most potions
    Awkward,
    /// Brewed with Redstone, base for Mundane Extended
    Mundane,
    /// Brewed with Glowstone
    Thick,

    // Effect potions
    NightVision,
    Invisibility,
    Leaping,
    FireResistance,
    Swiftness,
    Slowness,
    WaterBreathing,
    Healing,
    Harming,
    Poison,
    Regeneration,
    Strength,
    Weakness,
    Luck,
    SlowFalling,
}

impl PotionType {
    /// Get the status effect this potion applies (if any).
    pub fn effect(&self) -> Option<StatusEffectType> {
        match self {
            PotionType::Water | PotionType::Awkward | PotionType::Mundane | PotionType::Thick => {
                None
            }
            PotionType::NightVision => Some(StatusEffectType::NightVision),
            PotionType::Invisibility => Some(StatusEffectType::Invisibility),
            PotionType::Leaping => Some(StatusEffectType::JumpBoost),
            PotionType::FireResistance => Some(StatusEffectType::FireResistance),
            PotionType::Swiftness => Some(StatusEffectType::Speed),
            PotionType::Slowness => Some(StatusEffectType::Slowness),
            PotionType::WaterBreathing => Some(StatusEffectType::WaterBreathing),
            PotionType::Healing => Some(StatusEffectType::InstantHealth),
            PotionType::Harming => Some(StatusEffectType::InstantDamage),
            PotionType::Poison => Some(StatusEffectType::Poison),
            PotionType::Regeneration => Some(StatusEffectType::Regeneration),
            PotionType::Strength => Some(StatusEffectType::Strength),
            PotionType::Weakness => Some(StatusEffectType::Weakness),
            PotionType::Luck => Some(StatusEffectType::Luck),
            PotionType::SlowFalling => Some(StatusEffectType::SlowFalling),
        }
    }

    /// Get the base duration in ticks (at level I, without extension).
    pub fn base_duration_ticks(&self) -> u32 {
        match self {
            // Base potions: no duration
            PotionType::Water | PotionType::Awkward | PotionType::Mundane | PotionType::Thick => 0,
            // Instant effects
            PotionType::Healing | PotionType::Harming => 0,
            // 3 minute effects (3600 ticks)
            PotionType::NightVision | PotionType::Invisibility | PotionType::FireResistance => {
                3600
            }
            // 3 minute effects
            PotionType::WaterBreathing => 3600,
            // 3 minute Swiftness
            PotionType::Swiftness => 3600,
            // 1:30 Slowness
            PotionType::Slowness => 1800,
            // 3 minute Leaping
            PotionType::Leaping => 3600,
            // 45 second Poison
            PotionType::Poison => 900,
            // 45 second Regeneration
            PotionType::Regeneration => 900,
            // 3 minute Strength
            PotionType::Strength => 3600,
            // 1:30 Weakness
            PotionType::Weakness => 1800,
            // 5 minute Luck
            PotionType::Luck => 6000,
            // 1:30 Slow Falling
            PotionType::SlowFalling => 1800,
        }
    }

    /// Create a status effect from this potion type.
    pub fn create_effect(&self, amplifier: u8, extended: bool) -> Option<StatusEffect> {
        let effect_type = self.effect()?;
        let mut duration = self.base_duration_ticks();

        // Extended potions last 8/3 times as long
        if extended && duration > 0 {
            duration = duration * 8 / 3;
        }

        Some(StatusEffect::new(effect_type, amplifier, duration))
    }
}

/// Brewing time per operation in seconds.
pub const BREW_TIME_SECONDS: f32 = 20.0;

/// A brewing recipe: base potion + ingredient -> result potion.
#[derive(Debug, Clone, Copy)]
pub struct BrewRecipe {
    /// The base potion (water bottle, awkward potion, etc.)
    pub base: PotionType,
    /// The ingredient item ID (from item_ids)
    pub ingredient: u16,
    /// The resulting potion type
    pub result: PotionType,
}

/// All available brewing recipes.
/// Organized by brewing stage: Water -> Awkward -> Effect Potions
pub const BREW_RECIPES: &[BrewRecipe] = &[
    // Stage 1: Water bottles -> base potions
    BrewRecipe {
        base: PotionType::Water,
        ingredient: 102, // NETHER_WART
        result: PotionType::Awkward,
    },
    // Stage 2: Awkward potions -> effect potions
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 109, // SUGAR
        result: PotionType::Swiftness,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 112, // RABBIT_FOOT
        result: PotionType::Leaping,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 110, // GLISTERING_MELON
        result: PotionType::Healing,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 108, // SPIDER_EYE
        result: PotionType::Poison,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 105, // GHAST_TEAR
        result: PotionType::Regeneration,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 103, // BLAZE_POWDER
        result: PotionType::Strength,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 111, // GOLDEN_CARROT
        result: PotionType::NightVision,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 106, // MAGMA_CREAM
        result: PotionType::FireResistance,
    },
    BrewRecipe {
        base: PotionType::Awkward,
        ingredient: 113, // PHANTOM_MEMBRANE
        result: PotionType::SlowFalling,
    },
    // Corruption recipes: effect potion + fermented spider eye -> negative version
    BrewRecipe {
        base: PotionType::Swiftness,
        ingredient: 107, // FERMENTED_SPIDER_EYE
        result: PotionType::Slowness,
    },
    BrewRecipe {
        base: PotionType::NightVision,
        ingredient: 107, // FERMENTED_SPIDER_EYE
        result: PotionType::Invisibility,
    },
    BrewRecipe {
        base: PotionType::Healing,
        ingredient: 107, // FERMENTED_SPIDER_EYE
        result: PotionType::Harming,
    },
    BrewRecipe {
        base: PotionType::Poison,
        ingredient: 107, // FERMENTED_SPIDER_EYE
        result: PotionType::Harming,
    },
    // Water breathing from pufferfish (not in item_ids yet, use placeholder)
    // Weakness from water bottle + fermented spider eye
    BrewRecipe {
        base: PotionType::Water,
        ingredient: 107, // FERMENTED_SPIDER_EYE
        result: PotionType::Weakness,
    },
];

/// Get the brewing result for a base potion and ingredient.
pub fn get_brew_result(base: PotionType, ingredient: u16) -> Option<PotionType> {
    BREW_RECIPES
        .iter()
        .find(|r| r.base == base && r.ingredient == ingredient)
        .map(|r| r.result)
}

/// State of a brewing stand in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrewingStandState {
    /// Potion slots (3 bottles). Each slot stores (potion_type, is_extended, amplifier).
    /// None = empty slot, Some = bottle with potion
    pub bottles: [Option<PotionType>; 3],
    /// Ingredient slot (item_id, count).
    pub ingredient: Option<(u16, u32)>,
    /// Fuel slot (blaze powder count).
    pub fuel: u32,
    /// Current brewing progress (0.0 to 1.0).
    pub brew_progress: f32,
    /// Whether the brewing stand is currently active.
    pub is_brewing: bool,
}

impl Default for BrewingStandState {
    fn default() -> Self {
        Self::new()
    }
}

impl BrewingStandState {
    /// Create a new empty brewing stand.
    pub fn new() -> Self {
        Self {
            bottles: [None, None, None],
            ingredient: None,
            fuel: 0,
            brew_progress: 0.0,
            is_brewing: false,
        }
    }

    /// Update the brewing stand state (call once per tick/frame).
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds.
    ///
    /// # Returns
    /// `true` if brewing completed this update.
    pub fn update(&mut self, dt: f32) -> bool {
        // If already brewing, continue until complete
        if self.is_brewing {
            // Progress brewing
            let progress_per_second = 1.0 / BREW_TIME_SECONDS;
            self.brew_progress += progress_per_second * dt;

            // Check if brewing is complete
            if self.brew_progress >= 1.0 {
                self.complete_brew();
                self.brew_progress = 0.0;
                self.is_brewing = false;
                return true;
            }
            return false;
        }

        // Not currently brewing - check if we can start
        if self.can_brew() && self.fuel > 0 {
            // Start brewing - consume fuel
            self.fuel -= 1;
            self.is_brewing = true;
        } else {
            // Can't brew
            self.brew_progress = 0.0;
        }

        false
    }

    /// Check if the brewing stand can brew (has valid ingredient and at least one bottle).
    fn can_brew(&self) -> bool {
        if let Some((ingredient_id, _)) = &self.ingredient {
            // Check if any bottle can be brewed with this ingredient
            for bottle in &self.bottles {
                if let Some(potion_type) = bottle {
                    if get_brew_result(*potion_type, *ingredient_id).is_some() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Complete a brewing operation.
    fn complete_brew(&mut self) {
        if let Some((ingredient_id, ingredient_count)) = &mut self.ingredient {
            let ing_id = *ingredient_id;

            // Transform each bottle that has a valid recipe
            for bottle in &mut self.bottles {
                if let Some(potion_type) = bottle {
                    if let Some(result) = get_brew_result(*potion_type, ing_id) {
                        *bottle = Some(result);
                    }
                }
            }

            // Consume one ingredient
            *ingredient_count -= 1;
            if *ingredient_count == 0 {
                self.ingredient = None;
            }
        }
    }

    /// Add fuel (blaze powder) to the brewing stand.
    ///
    /// # Returns
    /// Number of items that couldn't be added (0 if all added).
    pub fn add_fuel(&mut self, count: u32) -> u32 {
        // Max 64 blaze powder
        let space = 64_u32.saturating_sub(self.fuel);
        let add = count.min(space);
        self.fuel += add;
        count - add
    }

    /// Add an ingredient to the brewing stand.
    ///
    /// # Returns
    /// Number of items that couldn't be added (0 if all added).
    pub fn add_ingredient(&mut self, item_id: u16, count: u32) -> u32 {
        match &mut self.ingredient {
            None => {
                let add = count.min(64);
                self.ingredient = Some((item_id, add));
                count - add
            }
            Some((existing_id, existing_count)) => {
                if *existing_id == item_id {
                    let space = 64_u32.saturating_sub(*existing_count);
                    let add = count.min(space);
                    *existing_count += add;
                    count - add
                } else {
                    count // Slot occupied with different item
                }
            }
        }
    }

    /// Add a potion bottle to a specific slot (0, 1, or 2).
    ///
    /// # Returns
    /// `true` if the bottle was added, `false` if slot was occupied.
    pub fn add_bottle(&mut self, slot: usize, potion_type: PotionType) -> bool {
        if slot >= 3 {
            return false;
        }
        if self.bottles[slot].is_some() {
            return false;
        }
        self.bottles[slot] = Some(potion_type);
        true
    }

    /// Take a potion bottle from a specific slot.
    ///
    /// # Returns
    /// The potion type, or None if slot was empty.
    pub fn take_bottle(&mut self, slot: usize) -> Option<PotionType> {
        if slot >= 3 {
            return None;
        }
        self.bottles[slot].take()
    }

    /// Take the ingredient from the brewing stand.
    pub fn take_ingredient(&mut self) -> Option<(u16, u32)> {
        self.ingredient.take()
    }

    /// Get the number of filled bottle slots.
    pub fn bottle_count(&self) -> usize {
        self.bottles.iter().filter(|b| b.is_some()).count()
    }

    /// Check if the brewing stand has any bottles.
    pub fn has_bottles(&self) -> bool {
        self.bottles.iter().any(|b| b.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_effect_creation() {
        let effect = StatusEffect::new(StatusEffectType::Speed, 1, 600);
        assert_eq!(effect.level(), 2); // amplifier 1 = Level II
        assert_eq!(effect.duration_seconds(), 30.0); // 600 ticks = 30 seconds
    }

    #[test]
    fn test_status_effect_tick() {
        let mut effect = StatusEffect::new(StatusEffectType::Speed, 0, 3);
        assert!(!effect.tick()); // 2 remaining
        assert!(!effect.tick()); // 1 remaining
        assert!(effect.tick()); // 0 remaining, expired
    }

    #[test]
    fn test_instant_effect() {
        let mut effect = StatusEffect::instant(StatusEffectType::InstantHealth, 0);
        assert!(effect.effect_type.is_instant());
        assert!(effect.tick()); // Instant effects expire immediately
    }

    #[test]
    fn test_status_effects_add() {
        let mut effects = StatusEffects::new();

        // Add Speed I for 60 seconds
        effects.add(StatusEffect::new(StatusEffectType::Speed, 0, 1200));
        assert!(effects.has(StatusEffectType::Speed));
        assert_eq!(effects.amplifier(StatusEffectType::Speed), Some(0));

        // Try to add Speed I with shorter duration - should not override
        effects.add(StatusEffect::new(StatusEffectType::Speed, 0, 600));
        assert_eq!(effects.get(StatusEffectType::Speed).unwrap().duration_ticks, 1200);

        // Add Speed II - should override
        effects.add(StatusEffect::new(StatusEffectType::Speed, 1, 400));
        assert_eq!(effects.amplifier(StatusEffectType::Speed), Some(1));
    }

    #[test]
    fn test_speed_multiplier() {
        let mut effects = StatusEffects::new();
        assert_eq!(effects.speed_multiplier(), 1.0);

        // Speed I: +20%
        effects.add(StatusEffect::new(StatusEffectType::Speed, 0, 100));
        assert!((effects.speed_multiplier() - 1.2).abs() < 0.01);

        // Speed II: +40%
        effects.add(StatusEffect::new(StatusEffectType::Speed, 1, 100));
        assert!((effects.speed_multiplier() - 1.4).abs() < 0.01);
    }

    #[test]
    fn test_attack_modifier() {
        let mut effects = StatusEffects::new();
        assert_eq!(effects.attack_damage_modifier(), 0.0);

        // Strength I: +3
        effects.add(StatusEffect::new(StatusEffectType::Strength, 0, 100));
        assert!((effects.attack_damage_modifier() - 3.0).abs() < 0.01);

        // Strength II: +6
        effects.add(StatusEffect::new(StatusEffectType::Strength, 1, 100));
        assert!((effects.attack_damage_modifier() - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_potion_type_effect() {
        assert!(PotionType::Water.effect().is_none());
        assert!(PotionType::Awkward.effect().is_none());
        assert_eq!(PotionType::Swiftness.effect(), Some(StatusEffectType::Speed));
        assert_eq!(PotionType::Healing.effect(), Some(StatusEffectType::InstantHealth));
    }

    #[test]
    fn test_potion_create_effect() {
        let effect = PotionType::Swiftness.create_effect(0, false).unwrap();
        assert_eq!(effect.effect_type, StatusEffectType::Speed);
        assert_eq!(effect.amplifier, 0);
        assert_eq!(effect.duration_ticks, 3600); // 3 minutes

        // Extended
        let extended = PotionType::Swiftness.create_effect(0, true).unwrap();
        assert_eq!(extended.duration_ticks, 3600 * 8 / 3); // 8 minutes
    }

    #[test]
    fn test_effect_types() {
        assert!(StatusEffectType::Speed.is_positive());
        assert!(!StatusEffectType::Slowness.is_positive());
        assert!(StatusEffectType::InstantHealth.is_instant());
        assert!(!StatusEffectType::Speed.is_instant());
    }

    #[test]
    fn test_brew_recipes() {
        // Water + Nether Wart = Awkward
        assert_eq!(
            get_brew_result(PotionType::Water, 102),
            Some(PotionType::Awkward)
        );
        // Awkward + Sugar = Swiftness
        assert_eq!(
            get_brew_result(PotionType::Awkward, 109),
            Some(PotionType::Swiftness)
        );
        // Swiftness + Fermented Spider Eye = Slowness
        assert_eq!(
            get_brew_result(PotionType::Swiftness, 107),
            Some(PotionType::Slowness)
        );
        // Invalid recipe
        assert_eq!(get_brew_result(PotionType::Thick, 109), None);
    }

    #[test]
    fn test_brewing_stand_new() {
        let stand = BrewingStandState::new();
        assert_eq!(stand.bottles, [None, None, None]);
        assert!(stand.ingredient.is_none());
        assert_eq!(stand.fuel, 0);
        assert_eq!(stand.brew_progress, 0.0);
        assert!(!stand.is_brewing);
    }

    #[test]
    fn test_brewing_stand_add_fuel() {
        let mut stand = BrewingStandState::new();
        assert_eq!(stand.add_fuel(10), 0);
        assert_eq!(stand.fuel, 10);

        // Add more fuel
        assert_eq!(stand.add_fuel(60), 6); // Only 54 more fit
        assert_eq!(stand.fuel, 64);
    }

    #[test]
    fn test_brewing_stand_add_bottles() {
        let mut stand = BrewingStandState::new();

        assert!(stand.add_bottle(0, PotionType::Water));
        assert!(stand.add_bottle(1, PotionType::Water));
        assert!(stand.add_bottle(2, PotionType::Water));

        // Can't add to occupied slot
        assert!(!stand.add_bottle(0, PotionType::Awkward));

        // Invalid slot
        assert!(!stand.add_bottle(3, PotionType::Water));

        assert_eq!(stand.bottle_count(), 3);
    }

    #[test]
    fn test_brewing_stand_take_bottle() {
        let mut stand = BrewingStandState::new();
        stand.add_bottle(0, PotionType::Swiftness);

        assert_eq!(stand.take_bottle(0), Some(PotionType::Swiftness));
        assert_eq!(stand.take_bottle(0), None); // Already taken
        assert_eq!(stand.take_bottle(3), None); // Invalid slot
    }

    #[test]
    fn test_brewing_stand_brewing() {
        let mut stand = BrewingStandState::new();

        // Add water bottles
        stand.add_bottle(0, PotionType::Water);
        stand.add_bottle(1, PotionType::Water);

        // Add nether wart ingredient
        assert_eq!(stand.add_ingredient(102, 1), 0);

        // Add fuel
        stand.add_fuel(1);

        // Simulate brewing for 21 seconds
        let mut completed = false;
        for _ in 0..420 {
            if stand.update(0.05) {
                completed = true;
            }
        }

        assert!(completed);

        // Bottles should be awkward potions
        assert_eq!(stand.bottles[0], Some(PotionType::Awkward));
        assert_eq!(stand.bottles[1], Some(PotionType::Awkward));

        // Ingredient consumed
        assert!(stand.ingredient.is_none());

        // Fuel consumed
        assert_eq!(stand.fuel, 0);
    }

    #[test]
    fn test_brewing_stand_no_fuel() {
        let mut stand = BrewingStandState::new();
        stand.add_bottle(0, PotionType::Water);
        stand.add_ingredient(102, 1);

        // No fuel - can't brew
        stand.update(1.0);
        assert!(!stand.is_brewing);
        assert_eq!(stand.brew_progress, 0.0);
        assert_eq!(stand.bottles[0], Some(PotionType::Water)); // Unchanged
    }

    #[test]
    fn test_brewing_stand_no_valid_recipe() {
        let mut stand = BrewingStandState::new();
        stand.add_bottle(0, PotionType::Thick); // No recipes for Thick
        stand.add_ingredient(109, 1); // Sugar
        stand.add_fuel(1);

        stand.update(1.0);
        assert!(!stand.is_brewing);
        assert_eq!(stand.fuel, 1); // Fuel not consumed
    }
}
