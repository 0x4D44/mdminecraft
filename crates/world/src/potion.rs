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
}
