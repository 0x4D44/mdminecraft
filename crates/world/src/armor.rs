//! Armor system with defense calculation and durability.
//!
//! Provides armor slots, defense values, and damage reduction calculation.

use crate::drop_item::ItemType;
use mdminecraft_core::{Enchantment, EnchantmentType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageKind {
    Generic,
    Fire,
    Blast,
    Projectile,
    Fall,
}

/// Armor slot types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmorSlot {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

/// Armor material types (determines defense and durability)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmorMaterial {
    Leather,
    Iron,
    Gold,
    Diamond,
}

/// A piece of armor with durability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmorPiece {
    pub item_type: ItemType,
    pub slot: ArmorSlot,
    pub material: ArmorMaterial,
    pub durability: u32,
    pub max_durability: u32,
    /// Enchantments on this armor piece
    pub enchantments: Vec<Enchantment>,
}

impl ArmorPiece {
    /// Create a new armor piece from an item type (no enchantments)
    pub fn from_item(item_type: ItemType) -> Option<Self> {
        Self::from_item_with_enchantments(item_type, Vec::new())
    }

    /// Create a new armor piece from an item type with enchantments
    pub fn from_item_with_enchantments(
        item_type: ItemType,
        enchantments: Vec<Enchantment>,
    ) -> Option<Self> {
        let (slot, material) = match item_type {
            // Leather
            ItemType::LeatherHelmet => (ArmorSlot::Helmet, ArmorMaterial::Leather),
            ItemType::LeatherChestplate => (ArmorSlot::Chestplate, ArmorMaterial::Leather),
            ItemType::LeatherLeggings => (ArmorSlot::Leggings, ArmorMaterial::Leather),
            ItemType::LeatherBoots => (ArmorSlot::Boots, ArmorMaterial::Leather),
            // Iron
            ItemType::IronHelmet => (ArmorSlot::Helmet, ArmorMaterial::Iron),
            ItemType::IronChestplate => (ArmorSlot::Chestplate, ArmorMaterial::Iron),
            ItemType::IronLeggings => (ArmorSlot::Leggings, ArmorMaterial::Iron),
            ItemType::IronBoots => (ArmorSlot::Boots, ArmorMaterial::Iron),
            // Gold
            ItemType::GoldHelmet => (ArmorSlot::Helmet, ArmorMaterial::Gold),
            ItemType::GoldChestplate => (ArmorSlot::Chestplate, ArmorMaterial::Gold),
            ItemType::GoldLeggings => (ArmorSlot::Leggings, ArmorMaterial::Gold),
            ItemType::GoldBoots => (ArmorSlot::Boots, ArmorMaterial::Gold),
            // Diamond
            ItemType::DiamondHelmet => (ArmorSlot::Helmet, ArmorMaterial::Diamond),
            ItemType::DiamondChestplate => (ArmorSlot::Chestplate, ArmorMaterial::Diamond),
            ItemType::DiamondLeggings => (ArmorSlot::Leggings, ArmorMaterial::Diamond),
            ItemType::DiamondBoots => (ArmorSlot::Boots, ArmorMaterial::Diamond),
            _ => return None,
        };

        let max_durability = get_max_durability(slot, material);
        Some(Self {
            item_type,
            slot,
            material,
            durability: max_durability,
            max_durability,
            enchantments,
        })
    }

    /// Get the Protection enchantment level if present
    pub fn protection_level(&self) -> u8 {
        self.enchantment_level(EnchantmentType::Protection)
    }

    pub fn enchantment_level(&self, enchantment_type: EnchantmentType) -> u8 {
        self.enchantments
            .iter()
            .filter(|e| e.enchantment_type == enchantment_type)
            .map(|e| e.level)
            .max()
            .unwrap_or(0)
    }

    /// Get the defense points provided by this armor piece
    pub fn defense(&self) -> u32 {
        get_defense_points(self.slot, self.material)
    }

    /// Damage the armor piece, returning true if it breaks
    pub fn damage(&mut self, amount: u32) -> bool {
        if amount >= self.durability {
            self.durability = 0;
            true
        } else {
            self.durability -= amount;
            false
        }
    }

    /// Damage the armor piece with Unbreaking consideration.
    ///
    /// Deterministic approximation of vanilla: durability loss is negated with probability
    /// `unbreaking_level/(unbreaking_level+1)`. We avoid RNG by using a modulus check
    /// against the current durability value.
    pub fn damage_with_unbreaking(&mut self, amount: u32) -> bool {
        let unbreaking_level = self.enchantment_level(EnchantmentType::Unbreaking);
        if unbreaking_level > 0 {
            let denominator = u32::from(unbreaking_level) + 1;
            if denominator > 1 && !self.durability.is_multiple_of(denominator) {
                return false;
            }
        }

        self.damage(amount)
    }

    /// Check if the armor is broken
    pub fn is_broken(&self) -> bool {
        self.durability == 0
    }

    /// Get durability as a ratio (0.0 to 1.0)
    pub fn durability_ratio(&self) -> f32 {
        self.durability as f32 / self.max_durability as f32
    }
}

/// Get defense points for armor piece
/// Full set totals: Leather=4, Iron=8, Diamond=10 (as per spec)
pub fn get_defense_points(slot: ArmorSlot, material: ArmorMaterial) -> u32 {
    match (slot, material) {
        // Leather: total 4 (1+1+1+1)
        (ArmorSlot::Helmet, ArmorMaterial::Leather) => 1,
        (ArmorSlot::Chestplate, ArmorMaterial::Leather) => 1,
        (ArmorSlot::Leggings, ArmorMaterial::Leather) => 1,
        (ArmorSlot::Boots, ArmorMaterial::Leather) => 1,
        // Iron: total 8 (2+2+2+2)
        (ArmorSlot::Helmet, ArmorMaterial::Iron) => 2,
        (ArmorSlot::Chestplate, ArmorMaterial::Iron) => 2,
        (ArmorSlot::Leggings, ArmorMaterial::Iron) => 2,
        (ArmorSlot::Boots, ArmorMaterial::Iron) => 2,
        // Gold: total 6 (1+2+2+1)
        (ArmorSlot::Helmet, ArmorMaterial::Gold) => 1,
        (ArmorSlot::Chestplate, ArmorMaterial::Gold) => 2,
        (ArmorSlot::Leggings, ArmorMaterial::Gold) => 2,
        (ArmorSlot::Boots, ArmorMaterial::Gold) => 1,
        // Diamond: total 10 (2+3+3+2)
        (ArmorSlot::Helmet, ArmorMaterial::Diamond) => 2,
        (ArmorSlot::Chestplate, ArmorMaterial::Diamond) => 3,
        (ArmorSlot::Leggings, ArmorMaterial::Diamond) => 3,
        (ArmorSlot::Boots, ArmorMaterial::Diamond) => 2,
    }
}

/// Get max durability for armor piece
pub fn get_max_durability(slot: ArmorSlot, material: ArmorMaterial) -> u32 {
    // Base durability per slot, multiplied by material factor
    let base = match slot {
        ArmorSlot::Helmet => 11,
        ArmorSlot::Chestplate => 16,
        ArmorSlot::Leggings => 15,
        ArmorSlot::Boots => 13,
    };

    let multiplier = match material {
        ArmorMaterial::Leather => 5,
        ArmorMaterial::Iron => 15,
        ArmorMaterial::Gold => 7,
        ArmorMaterial::Diamond => 33,
    };

    base * multiplier
}

/// Player's equipped armor
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerArmor {
    pub helmet: Option<ArmorPiece>,
    pub chestplate: Option<ArmorPiece>,
    pub leggings: Option<ArmorPiece>,
    pub boots: Option<ArmorPiece>,
}

impl PlayerArmor {
    /// Create empty armor slots
    pub fn new() -> Self {
        Self::default()
    }

    /// Get total defense points from all armor
    pub fn total_defense(&self) -> u32 {
        let mut total = 0;
        if let Some(piece) = &self.helmet {
            total += piece.defense();
        }
        if let Some(piece) = &self.chestplate {
            total += piece.defense();
        }
        if let Some(piece) = &self.leggings {
            total += piece.defense();
        }
        if let Some(piece) = &self.boots {
            total += piece.defense();
        }
        total
    }

    /// Get total Protection enchantment levels from all armor pieces
    pub fn total_protection(&self) -> u8 {
        self.total_enchantment_level(EnchantmentType::Protection)
    }

    pub fn total_enchantment_level(&self, enchantment_type: EnchantmentType) -> u8 {
        let mut total: u8 = 0;
        if let Some(piece) = &self.helmet {
            total = total.saturating_add(piece.enchantment_level(enchantment_type));
        }
        if let Some(piece) = &self.chestplate {
            total = total.saturating_add(piece.enchantment_level(enchantment_type));
        }
        if let Some(piece) = &self.leggings {
            total = total.saturating_add(piece.enchantment_level(enchantment_type));
        }
        if let Some(piece) = &self.boots {
            total = total.saturating_add(piece.enchantment_level(enchantment_type));
        }
        total
    }

    /// Calculate damage reduction multiplier (0.0 to 1.0)
    /// Returns the fraction of damage that gets through armor
    /// Formula: damage * (1 - defense/25) * (1 - protection*0.04)
    pub fn damage_multiplier(&self) -> f32 {
        self.damage_multiplier_for(DamageKind::Generic)
    }

    pub fn damage_multiplier_for(&self, kind: DamageKind) -> f32 {
        let armor_multiplier = match kind {
            DamageKind::Fall => 1.0,
            _ => {
                let defense = self.total_defense();
                // Base armor reduction: 1 - defense/25
                // Max defense is 10 (full diamond = 2+3+3+2 = 10), which reduces by 40%
                let armor_reduction = (defense as f32 / 25.0).min(0.8);
                1.0 - armor_reduction
            }
        };

        // Protection enchantment: 4% reduction per level, max 16 levels (64% max)
        // In vanilla MC: EPF = sum of protection levels, damage_mult = 1 - EPF*0.04
        let mut protection = self.total_enchantment_level(EnchantmentType::Protection);
        protection = match kind {
            DamageKind::Generic | DamageKind::Fall => protection,
            DamageKind::Fire => protection
                .saturating_add(self.total_enchantment_level(EnchantmentType::FireProtection)),
            DamageKind::Blast => protection
                .saturating_add(self.total_enchantment_level(EnchantmentType::BlastProtection)),
            DamageKind::Projectile => protection.saturating_add(
                self.total_enchantment_level(EnchantmentType::ProjectileProtection),
            ),
        };
        protection = protection.min(16);
        let protection_reduction = (protection as f32 * 0.04).min(0.64);
        let protection_multiplier = 1.0 - protection_reduction;

        let extra_multiplier = match kind {
            DamageKind::Fall => {
                // Vanilla-ish: Feather Falling reduces fall damage (boots-only in vanilla).
                // Approx: 12% per level, up to 48% at level IV.
                let feather_level = self
                    .total_enchantment_level(EnchantmentType::FeatherFalling)
                    .min(4);
                let reduction = (feather_level as f32 * 0.12).min(0.48);
                1.0 - reduction
            }
            _ => 1.0,
        };

        // Combine reductions multiplicatively
        armor_multiplier * protection_multiplier * extra_multiplier
    }

    /// Equip armor piece, returning the previously equipped piece if any
    pub fn equip(&mut self, piece: ArmorPiece) -> Option<ArmorPiece> {
        let slot = match piece.slot {
            ArmorSlot::Helmet => &mut self.helmet,
            ArmorSlot::Chestplate => &mut self.chestplate,
            ArmorSlot::Leggings => &mut self.leggings,
            ArmorSlot::Boots => &mut self.boots,
        };
        slot.replace(piece)
    }

    /// Unequip armor from a slot
    pub fn unequip(&mut self, slot: ArmorSlot) -> Option<ArmorPiece> {
        let slot_ref = match slot {
            ArmorSlot::Helmet => &mut self.helmet,
            ArmorSlot::Chestplate => &mut self.chestplate,
            ArmorSlot::Leggings => &mut self.leggings,
            ArmorSlot::Boots => &mut self.boots,
        };
        slot_ref.take()
    }

    /// Get armor in a slot
    pub fn get(&self, slot: ArmorSlot) -> Option<&ArmorPiece> {
        match slot {
            ArmorSlot::Helmet => self.helmet.as_ref(),
            ArmorSlot::Chestplate => self.chestplate.as_ref(),
            ArmorSlot::Leggings => self.leggings.as_ref(),
            ArmorSlot::Boots => self.boots.as_ref(),
        }
    }

    /// Damage all armor pieces when taking damage
    /// Returns the actual damage after armor reduction
    pub fn reduce_damage(&self, raw_damage: f32, kind: DamageKind) -> f32 {
        raw_damage * self.damage_multiplier_for(kind)
    }

    pub fn take_damage(&mut self, raw_damage: f32, kind: DamageKind) -> f32 {
        let multiplier = self.damage_multiplier_for(kind);
        let actual_damage = raw_damage * multiplier;

        // Damage each equipped piece (1 durability per hit)
        if let Some(piece) = &mut self.helmet {
            if piece.damage_with_unbreaking(1) {
                self.helmet = None;
            }
        }
        if let Some(piece) = &mut self.chestplate {
            if piece.damage_with_unbreaking(1) {
                self.chestplate = None;
            }
        }
        if let Some(piece) = &mut self.leggings {
            if piece.damage_with_unbreaking(1) {
                self.leggings = None;
            }
        }
        if let Some(piece) = &mut self.boots {
            if piece.damage_with_unbreaking(1) {
                self.boots = None;
            }
        }

        actual_damage
    }

    /// Apply Mending: use XP to repair equipped armor pieces with Mending.
    ///
    /// Returns the amount of XP that remains after repairs.
    /// Vanilla-ish: 1 XP restores 2 durability.
    pub fn repair_with_mending(&mut self, xp_amount: u32) -> u32 {
        let mut remaining_xp = xp_amount;

        fn repair_piece(piece: &mut Option<ArmorPiece>, remaining_xp: &mut u32) {
            if *remaining_xp == 0 {
                return;
            }
            let Some(piece) = piece.as_mut() else {
                return;
            };
            if piece.is_broken() {
                return;
            }
            if piece.enchantment_level(EnchantmentType::Mending) == 0 {
                return;
            }
            if piece.durability >= piece.max_durability {
                return;
            }

            let missing = piece.max_durability - piece.durability;
            let xp_needed = missing.div_ceil(2);
            let xp_to_use = (*remaining_xp).min(xp_needed);
            if xp_to_use == 0 {
                return;
            }

            piece.durability = (piece.durability + xp_to_use * 2).min(piece.max_durability);
            *remaining_xp -= xp_to_use;
        }

        // Deterministic repair order: helmet -> chestplate -> leggings -> boots.
        repair_piece(&mut self.helmet, &mut remaining_xp);
        repair_piece(&mut self.chestplate, &mut remaining_xp);
        repair_piece(&mut self.leggings, &mut remaining_xp);
        repair_piece(&mut self.boots, &mut remaining_xp);

        remaining_xp
    }
}

/// Check if an item type is armor
pub fn is_armor(item_type: ItemType) -> bool {
    matches!(
        item_type,
        ItemType::LeatherHelmet
            | ItemType::LeatherChestplate
            | ItemType::LeatherLeggings
            | ItemType::LeatherBoots
            | ItemType::IronHelmet
            | ItemType::IronChestplate
            | ItemType::IronLeggings
            | ItemType::IronBoots
            | ItemType::GoldHelmet
            | ItemType::GoldChestplate
            | ItemType::GoldLeggings
            | ItemType::GoldBoots
            | ItemType::DiamondHelmet
            | ItemType::DiamondChestplate
            | ItemType::DiamondLeggings
            | ItemType::DiamondBoots
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_armor_piece_creation() {
        let piece = ArmorPiece::from_item(ItemType::IronChestplate).unwrap();
        assert_eq!(piece.slot, ArmorSlot::Chestplate);
        assert_eq!(piece.material, ArmorMaterial::Iron);
        assert_eq!(piece.defense(), 2); // Iron chestplate = 2 defense
        assert_eq!(piece.max_durability, 16 * 15); // 240
    }

    #[test]
    fn test_armor_defense() {
        let mut armor = PlayerArmor::new();
        assert_eq!(armor.total_defense(), 0);
        assert_eq!(armor.damage_multiplier(), 1.0);

        // Equip full iron armor
        armor.equip(ArmorPiece::from_item(ItemType::IronHelmet).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::IronChestplate).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::IronLeggings).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::IronBoots).unwrap());

        // Iron: 2+2+2+2 = 8 defense (per spec)
        assert_eq!(armor.total_defense(), 8);
        // 8/25 = 0.32 reduction, so multiplier is 0.68
        assert!((armor.damage_multiplier() - 0.68).abs() < 0.01);
    }

    #[test]
    fn test_armor_durability() {
        let mut piece = ArmorPiece::from_item(ItemType::LeatherHelmet).unwrap();
        let initial = piece.durability;
        assert!(!piece.is_broken());

        // Damage it
        piece.damage(10);
        assert_eq!(piece.durability, initial - 10);

        // Break it
        piece.durability = 5;
        assert!(piece.damage(10));
        assert!(piece.is_broken());
    }

    #[test]
    fn test_armor_take_damage() {
        let mut armor = PlayerArmor::new();
        armor.equip(ArmorPiece::from_item(ItemType::DiamondChestplate).unwrap());

        // Diamond chestplate gives 3 defense = 3/25 = 12% reduction
        let raw = 10.0;
        let actual = armor.take_damage(raw, DamageKind::Generic);
        // multiplier = 1 - 3/25 = 0.88, so 10 * 0.88 = 8.8
        assert!((actual - 8.8).abs() < 0.01);

        // Check durability was reduced
        assert_eq!(armor.chestplate.as_ref().unwrap().durability, 16 * 33 - 1);
    }

    #[test]
    fn test_is_armor() {
        assert!(is_armor(ItemType::IronHelmet));
        assert!(is_armor(ItemType::DiamondBoots));
        assert!(!is_armor(ItemType::Coal));
        assert!(!is_armor(ItemType::Bow));
    }

    #[test]
    fn test_protection_enchantment() {
        let mut armor = PlayerArmor::new();

        // Create armor with Protection IV enchantment
        let protection_enchant = Enchantment::new(EnchantmentType::Protection, 4);
        let chestplate = ArmorPiece::from_item_with_enchantments(
            ItemType::DiamondChestplate,
            vec![protection_enchant],
        )
        .unwrap();

        armor.equip(chestplate);

        // Diamond chestplate: 3 defense = 3/25 = 12% armor reduction
        // Protection IV: 4 * 4% = 16% protection reduction
        // Total: (1 - 0.12) * (1 - 0.16) = 0.88 * 0.84 = 0.7392
        assert_eq!(armor.total_protection(), 4);
        let multiplier = armor.damage_multiplier();
        assert!((multiplier - 0.7392).abs() < 0.01);
    }

    #[test]
    fn test_blast_protection_only_applies_to_blast_damage() {
        let mut armor = PlayerArmor::new();
        let blast = Enchantment::new(EnchantmentType::BlastProtection, 4);
        armor.equip(
            ArmorPiece::from_item_with_enchantments(ItemType::DiamondChestplate, vec![blast])
                .unwrap(),
        );

        // No generic Protection enchant: generic damage is only reduced by armor.
        assert!((armor.damage_multiplier_for(DamageKind::Generic) - 0.88).abs() < 0.01);

        // Blast protection adds 16% more reduction.
        let blast_multiplier = armor.damage_multiplier_for(DamageKind::Blast);
        assert!((blast_multiplier - 0.7392).abs() < 0.01);
    }

    #[test]
    fn test_feather_falling_applies_only_to_fall_damage() {
        let mut armor = PlayerArmor::new();
        let feather = Enchantment::new(EnchantmentType::FeatherFalling, 4);
        armor.equip(
            ArmorPiece::from_item_with_enchantments(ItemType::DiamondBoots, vec![feather]).unwrap(),
        );

        // No base armor reduction for fall damage; Feather Falling IV reduces by 48%.
        let fall_multiplier = armor.damage_multiplier_for(DamageKind::Fall);
        assert!((fall_multiplier - 0.52).abs() < 0.01);

        // Generic damage is reduced only by base armor points (diamond boots = 2 defense).
        let generic_multiplier = armor.damage_multiplier_for(DamageKind::Generic);
        assert!((generic_multiplier - 0.92).abs() < 0.01);
    }

    #[test]
    fn test_armor_piece_damage_with_unbreaking_is_deterministic() {
        let unbreaking = Enchantment::new(EnchantmentType::Unbreaking, 3);
        let mut piece =
            ArmorPiece::from_item_with_enchantments(ItemType::DiamondBoots, vec![unbreaking])
                .unwrap();

        // When durability is not divisible by (level+1), damage is ignored.
        piece.durability = 5;
        assert!(!piece.damage_with_unbreaking(1));
        assert_eq!(piece.durability, 5);

        // When divisible, damage applies.
        piece.durability = 4;
        assert!(!piece.damage_with_unbreaking(1));
        assert_eq!(piece.durability, 3);
    }

    #[test]
    fn test_mending_repairs_armor_and_consumes_xp() {
        let mut armor = PlayerArmor::new();
        let mending = Enchantment::new(EnchantmentType::Mending, 1);
        let mut boots =
            ArmorPiece::from_item_with_enchantments(ItemType::DiamondBoots, vec![mending]).unwrap();
        boots.durability = boots.max_durability.saturating_sub(10);
        armor.equip(boots);

        let remaining = armor.repair_with_mending(3);
        assert_eq!(remaining, 0);
        let boots = armor.boots.as_ref().unwrap();
        assert_eq!(boots.durability, boots.max_durability.saturating_sub(4));
    }

    #[test]
    fn test_full_protection_armor() {
        let mut armor = PlayerArmor::new();

        // Equip full diamond armor with Protection IV on each piece
        let protection_enchant = Enchantment::new(EnchantmentType::Protection, 4);
        armor.equip(
            ArmorPiece::from_item_with_enchantments(
                ItemType::DiamondHelmet,
                vec![protection_enchant],
            )
            .unwrap(),
        );
        armor.equip(
            ArmorPiece::from_item_with_enchantments(
                ItemType::DiamondChestplate,
                vec![protection_enchant],
            )
            .unwrap(),
        );
        armor.equip(
            ArmorPiece::from_item_with_enchantments(
                ItemType::DiamondLeggings,
                vec![protection_enchant],
            )
            .unwrap(),
        );
        armor.equip(
            ArmorPiece::from_item_with_enchantments(
                ItemType::DiamondBoots,
                vec![protection_enchant],
            )
            .unwrap(),
        );

        // Total protection: 4 * 4 = 16 levels (capped at 64% reduction)
        assert_eq!(armor.total_protection(), 16);
        // Diamond armor: 10 defense = 10/25 = 40% armor reduction
        // Protection: 16 * 4% = 64% (max)
        // Total: (1 - 0.40) * (1 - 0.64) = 0.60 * 0.36 = 0.216
        let multiplier = armor.damage_multiplier();
        assert!((multiplier - 0.216).abs() < 0.01);
    }

    #[test]
    fn test_armor_get_slot() {
        let mut armor = PlayerArmor::new();
        armor.equip(ArmorPiece::from_item(ItemType::IronHelmet).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::IronBoots).unwrap());

        // Test get() for each slot
        assert!(armor.get(ArmorSlot::Helmet).is_some());
        assert!(armor.get(ArmorSlot::Chestplate).is_none());
        assert!(armor.get(ArmorSlot::Leggings).is_none());
        assert!(armor.get(ArmorSlot::Boots).is_some());

        // Verify the retrieved pieces are correct
        assert_eq!(
            armor.get(ArmorSlot::Helmet).unwrap().item_type,
            ItemType::IronHelmet
        );
        assert_eq!(
            armor.get(ArmorSlot::Boots).unwrap().item_type,
            ItemType::IronBoots
        );
    }

    #[test]
    fn test_armor_unequip() {
        let mut armor = PlayerArmor::new();
        armor.equip(ArmorPiece::from_item(ItemType::DiamondChestplate).unwrap());

        // Unequip the chestplate
        let piece = armor.unequip(ArmorSlot::Chestplate);
        assert!(piece.is_some());
        assert_eq!(piece.unwrap().item_type, ItemType::DiamondChestplate);

        // Slot should now be empty
        assert!(armor.get(ArmorSlot::Chestplate).is_none());

        // Unequip again - should return None
        assert!(armor.unequip(ArmorSlot::Chestplate).is_none());
    }

    #[test]
    fn test_armor_unequip_all_slots() {
        let mut armor = PlayerArmor::new();
        armor.equip(ArmorPiece::from_item(ItemType::LeatherHelmet).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::LeatherChestplate).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::LeatherLeggings).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::LeatherBoots).unwrap());

        // Unequip all
        assert!(armor.unequip(ArmorSlot::Helmet).is_some());
        assert!(armor.unequip(ArmorSlot::Chestplate).is_some());
        assert!(armor.unequip(ArmorSlot::Leggings).is_some());
        assert!(armor.unequip(ArmorSlot::Boots).is_some());

        // All should now be empty
        assert_eq!(armor.total_defense(), 0);
    }

    #[test]
    fn test_armor_piece_durability_ratio() {
        let mut piece = ArmorPiece::from_item(ItemType::IronChestplate).unwrap();

        // Full durability
        assert!((piece.durability_ratio() - 1.0).abs() < 0.001);

        // Half durability
        piece.durability = piece.max_durability / 2;
        assert!((piece.durability_ratio() - 0.5).abs() < 0.01);

        // Broken
        piece.durability = 0;
        assert_eq!(piece.durability_ratio(), 0.0);
    }

    #[test]
    fn test_all_leather_armor() {
        let helmet = ArmorPiece::from_item(ItemType::LeatherHelmet).unwrap();
        let chestplate = ArmorPiece::from_item(ItemType::LeatherChestplate).unwrap();
        let leggings = ArmorPiece::from_item(ItemType::LeatherLeggings).unwrap();
        let boots = ArmorPiece::from_item(ItemType::LeatherBoots).unwrap();

        // Leather total: 1+1+1+1 = 4
        assert_eq!(helmet.defense(), 1);
        assert_eq!(chestplate.defense(), 1);
        assert_eq!(leggings.defense(), 1);
        assert_eq!(boots.defense(), 1);

        // Verify material
        assert_eq!(helmet.material, ArmorMaterial::Leather);
    }

    #[test]
    fn test_all_gold_armor() {
        let helmet = ArmorPiece::from_item(ItemType::GoldHelmet).unwrap();
        let chestplate = ArmorPiece::from_item(ItemType::GoldChestplate).unwrap();
        let leggings = ArmorPiece::from_item(ItemType::GoldLeggings).unwrap();
        let boots = ArmorPiece::from_item(ItemType::GoldBoots).unwrap();

        // Gold total: 1+2+2+1 = 6
        assert_eq!(helmet.defense(), 1);
        assert_eq!(chestplate.defense(), 2);
        assert_eq!(leggings.defense(), 2);
        assert_eq!(boots.defense(), 1);

        // Verify material
        assert_eq!(chestplate.material, ArmorMaterial::Gold);
    }

    #[test]
    fn test_all_diamond_armor() {
        let helmet = ArmorPiece::from_item(ItemType::DiamondHelmet).unwrap();
        let chestplate = ArmorPiece::from_item(ItemType::DiamondChestplate).unwrap();
        let leggings = ArmorPiece::from_item(ItemType::DiamondLeggings).unwrap();
        let boots = ArmorPiece::from_item(ItemType::DiamondBoots).unwrap();

        // Diamond total: 2+3+3+2 = 10
        assert_eq!(helmet.defense(), 2);
        assert_eq!(chestplate.defense(), 3);
        assert_eq!(leggings.defense(), 3);
        assert_eq!(boots.defense(), 2);

        // Verify max durability
        assert_eq!(helmet.max_durability, 11 * 33); // 363
        assert_eq!(chestplate.max_durability, 16 * 33); // 528
    }

    #[test]
    fn test_armor_from_non_armor_item() {
        // Non-armor items should return None
        assert!(ArmorPiece::from_item(ItemType::Coal).is_none());
        assert!(ArmorPiece::from_item(ItemType::Bow).is_none());
        assert!(ArmorPiece::from_item(ItemType::IronIngot).is_none());
    }

    #[test]
    fn test_armor_damage_breaks() {
        let mut piece = ArmorPiece::from_item(ItemType::LeatherBoots).unwrap();

        // Damage more than durability - should break
        piece.durability = 5;
        let broke = piece.damage(10);
        assert!(broke);
        assert!(piece.is_broken());
        assert_eq!(piece.durability, 0);
    }

    #[test]
    fn test_armor_damage_exact_break() {
        let mut piece = ArmorPiece::from_item(ItemType::GoldLeggings).unwrap();

        // Damage exactly equals durability
        piece.durability = 10;
        let broke = piece.damage(10);
        assert!(broke);
        assert!(piece.is_broken());
    }

    #[test]
    fn test_equip_replaces_existing() {
        let mut armor = PlayerArmor::new();

        // Equip leather helmet
        armor.equip(ArmorPiece::from_item(ItemType::LeatherHelmet).unwrap());
        assert_eq!(
            armor.get(ArmorSlot::Helmet).unwrap().material,
            ArmorMaterial::Leather
        );

        // Replace with iron helmet
        let old = armor.equip(ArmorPiece::from_item(ItemType::IronHelmet).unwrap());
        assert!(old.is_some());
        assert_eq!(old.unwrap().material, ArmorMaterial::Leather);
        assert_eq!(
            armor.get(ArmorSlot::Helmet).unwrap().material,
            ArmorMaterial::Iron
        );
    }

    #[test]
    fn test_armor_take_damage_breaks_armor() {
        let mut armor = PlayerArmor::new();

        // Equip armor with low durability
        let mut piece = ArmorPiece::from_item(ItemType::LeatherHelmet).unwrap();
        piece.durability = 1;
        armor.helmet = Some(piece);

        // Take damage - should break the helmet
        armor.take_damage(10.0, DamageKind::Generic);
        assert!(armor.helmet.is_none());
    }

    #[test]
    fn test_player_armor_serialization() {
        let mut armor = PlayerArmor::new();
        armor.equip(ArmorPiece::from_item(ItemType::IronHelmet).unwrap());
        armor.equip(ArmorPiece::from_item(ItemType::IronChestplate).unwrap());

        let serialized = serde_json::to_string(&armor).unwrap();
        let deserialized: PlayerArmor = serde_json::from_str(&serialized).unwrap();

        assert!(deserialized.helmet.is_some());
        assert!(deserialized.chestplate.is_some());
        assert!(deserialized.leggings.is_none());
        assert!(deserialized.boots.is_none());
    }

    #[test]
    fn test_armor_piece_serialization() {
        let piece = ArmorPiece::from_item(ItemType::DiamondBoots).unwrap();

        let serialized = serde_json::to_string(&piece).unwrap();
        let deserialized: ArmorPiece = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.item_type, ItemType::DiamondBoots);
        assert_eq!(deserialized.slot, ArmorSlot::Boots);
        assert_eq!(deserialized.material, ArmorMaterial::Diamond);
    }

    #[test]
    fn test_max_durability_all_materials() {
        // Verify durability formula for all slots and materials
        for slot in [
            ArmorSlot::Helmet,
            ArmorSlot::Chestplate,
            ArmorSlot::Leggings,
            ArmorSlot::Boots,
        ] {
            for material in [
                ArmorMaterial::Leather,
                ArmorMaterial::Iron,
                ArmorMaterial::Gold,
                ArmorMaterial::Diamond,
            ] {
                let durability = get_max_durability(slot, material);
                assert!(durability > 0);

                // Diamond should have highest durability
                if material == ArmorMaterial::Diamond {
                    assert!(durability > get_max_durability(slot, ArmorMaterial::Leather));
                    assert!(durability > get_max_durability(slot, ArmorMaterial::Iron));
                    assert!(durability > get_max_durability(slot, ArmorMaterial::Gold));
                }
            }
        }
    }

    #[test]
    fn test_defense_points_all_combinations() {
        // Test all slot/material combinations
        for slot in [
            ArmorSlot::Helmet,
            ArmorSlot::Chestplate,
            ArmorSlot::Leggings,
            ArmorSlot::Boots,
        ] {
            for material in [
                ArmorMaterial::Leather,
                ArmorMaterial::Iron,
                ArmorMaterial::Gold,
                ArmorMaterial::Diamond,
            ] {
                let defense = get_defense_points(slot, material);
                assert!((1..=3).contains(&defense));
            }
        }
    }

    #[test]
    fn test_is_armor_comprehensive() {
        // All armor items should return true
        let armor_items = [
            ItemType::LeatherHelmet,
            ItemType::LeatherChestplate,
            ItemType::LeatherLeggings,
            ItemType::LeatherBoots,
            ItemType::IronHelmet,
            ItemType::IronChestplate,
            ItemType::IronLeggings,
            ItemType::IronBoots,
            ItemType::GoldHelmet,
            ItemType::GoldChestplate,
            ItemType::GoldLeggings,
            ItemType::GoldBoots,
            ItemType::DiamondHelmet,
            ItemType::DiamondChestplate,
            ItemType::DiamondLeggings,
            ItemType::DiamondBoots,
        ];

        for item in armor_items {
            assert!(is_armor(item), "{:?} should be armor", item);
        }
    }
}
