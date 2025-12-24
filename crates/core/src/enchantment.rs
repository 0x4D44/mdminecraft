use serde::{Deserialize, Serialize};

/// Types of enchantments that can be applied to items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnchantmentType {
    // Tool enchantments
    /// Increases mining speed
    Efficiency,
    /// Allows silk touch harvesting of blocks
    SilkTouch,
    /// Increases block drop amounts
    Fortune,

    // Weapon enchantments
    /// Increases attack damage
    Sharpness,
    /// Increases knockback
    Knockback,
    /// Sets targets on fire
    FireAspect,
    /// Increases bow/arrow damage
    Power,
    /// Increases bow/arrow knockback
    Punch,
    /// Bow/arrow sets targets on fire
    Flame,
    /// Allows firing without consuming arrows (requires at least one arrow in inventory).
    Infinity,

    // Armor enchantments
    /// Reduces damage from all sources
    Protection,
    /// Reduces fire damage
    FireProtection,
    /// Reduces explosion damage
    BlastProtection,
    /// Reduces projectile damage
    ProjectileProtection,
    /// Reduces fall damage (boots)
    FeatherFalling,
    /// Extends underwater breathing (helmet)
    Respiration,
    /// Removes underwater mining speed penalty (helmet).
    AquaAffinity,
    /// Increases underwater movement speed (boots).
    DepthStrider,

    // Universal enchantments
    /// Reduces durability loss
    Unbreaking,
    /// Repairs item using XP
    Mending,
}

impl EnchantmentType {
    /// Get the maximum level for this enchantment
    pub fn max_level(&self) -> u8 {
        match self {
            EnchantmentType::Efficiency => 5,
            EnchantmentType::SilkTouch => 1,
            EnchantmentType::Fortune => 3,
            EnchantmentType::Sharpness => 5,
            EnchantmentType::Knockback => 2,
            EnchantmentType::FireAspect => 2,
            EnchantmentType::Power => 5,
            EnchantmentType::Punch => 2,
            EnchantmentType::Flame => 1,
            EnchantmentType::Infinity => 1,
            EnchantmentType::Protection => 4,
            EnchantmentType::FireProtection => 4,
            EnchantmentType::BlastProtection => 4,
            EnchantmentType::ProjectileProtection => 4,
            EnchantmentType::FeatherFalling => 4,
            EnchantmentType::Respiration => 3,
            EnchantmentType::AquaAffinity => 1,
            EnchantmentType::DepthStrider => 3,
            EnchantmentType::Unbreaking => 3,
            EnchantmentType::Mending => 1,
        }
    }

    /// Check if this enchantment is compatible with another
    /// (some enchantments are mutually exclusive)
    pub fn is_compatible_with(&self, other: &EnchantmentType) -> bool {
        // Silk Touch and Fortune are incompatible
        if matches!(self, EnchantmentType::SilkTouch) && matches!(other, EnchantmentType::Fortune) {
            return false;
        }
        if matches!(self, EnchantmentType::Fortune) && matches!(other, EnchantmentType::SilkTouch) {
            return false;
        }

        // Protection enchantments are incompatible with each other
        let protection_types = [
            EnchantmentType::Protection,
            EnchantmentType::FireProtection,
            EnchantmentType::BlastProtection,
            EnchantmentType::ProjectileProtection,
        ];

        if protection_types.contains(self) && protection_types.contains(other) && self != other {
            return false;
        }

        // Infinity and Mending are incompatible (vanilla).
        if matches!(self, EnchantmentType::Infinity) && matches!(other, EnchantmentType::Mending) {
            return false;
        }
        if matches!(self, EnchantmentType::Mending) && matches!(other, EnchantmentType::Infinity) {
            return false;
        }

        // All other combinations are compatible
        true
    }
}

/// An enchantment with a specific level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Enchantment {
    /// The type of enchantment
    pub enchantment_type: EnchantmentType,
    /// The level of the enchantment (1 to max_level)
    pub level: u8,
}

impl Enchantment {
    /// Create a new enchantment
    pub fn new(enchantment_type: EnchantmentType, level: u8) -> Self {
        let max_level = enchantment_type.max_level();
        let level = level.min(max_level); // Clamp to max level
        Self {
            enchantment_type,
            level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_level() {
        assert_eq!(EnchantmentType::Efficiency.max_level(), 5);
        assert_eq!(EnchantmentType::SilkTouch.max_level(), 1);
        assert_eq!(EnchantmentType::Unbreaking.max_level(), 3);
        assert_eq!(EnchantmentType::AquaAffinity.max_level(), 1);
        assert_eq!(EnchantmentType::DepthStrider.max_level(), 3);
    }

    #[test]
    fn test_incompatible_enchantments() {
        assert!(!EnchantmentType::SilkTouch.is_compatible_with(&EnchantmentType::Fortune));
        assert!(!EnchantmentType::Fortune.is_compatible_with(&EnchantmentType::SilkTouch));
        assert!(!EnchantmentType::Protection.is_compatible_with(&EnchantmentType::FireProtection));
        assert!(!EnchantmentType::Infinity.is_compatible_with(&EnchantmentType::Mending));
    }

    #[test]
    fn test_compatible_enchantments() {
        assert!(EnchantmentType::Efficiency.is_compatible_with(&EnchantmentType::Unbreaking));
        assert!(EnchantmentType::Sharpness.is_compatible_with(&EnchantmentType::Knockback));
        assert!(EnchantmentType::Protection.is_compatible_with(&EnchantmentType::Unbreaking));
    }

    #[test]
    fn test_enchantment_level_clamping() {
        let ench = Enchantment::new(EnchantmentType::SilkTouch, 10); // Try level 10
        assert_eq!(ench.level, 1); // Should be clamped to 1

        let ench = Enchantment::new(EnchantmentType::Efficiency, 3);
        assert_eq!(ench.level, 3); // Should stay at 3
    }
}
