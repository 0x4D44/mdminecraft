//! Armor system with defense calculation and durability.
//!
//! Provides armor slots, defense values, and damage reduction calculation.

use crate::drop_item::ItemType;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ArmorPiece {
    pub item_type: ItemType,
    pub slot: ArmorSlot,
    pub material: ArmorMaterial,
    pub durability: u32,
    pub max_durability: u32,
}

impl ArmorPiece {
    /// Create a new armor piece from an item type
    pub fn from_item(item_type: ItemType) -> Option<Self> {
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
        })
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

    /// Calculate damage reduction multiplier (0.0 to 1.0)
    /// Returns the fraction of damage that gets through armor
    /// Formula: damage * (1 - defense/25) as per spec
    pub fn damage_multiplier(&self) -> f32 {
        let defense = self.total_defense();
        // Formula: 1 - defense/25
        // Max defense is 10 (full diamond = 2+3+3+2 = 10), which reduces by 40%
        let reduction = (defense as f32 / 25.0).min(0.8);
        1.0 - reduction
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
    pub fn take_damage(&mut self, raw_damage: f32) -> f32 {
        let multiplier = self.damage_multiplier();
        let actual_damage = raw_damage * multiplier;

        // Damage each equipped piece (1 durability per hit)
        if let Some(piece) = &mut self.helmet {
            if piece.damage(1) {
                self.helmet = None;
            }
        }
        if let Some(piece) = &mut self.chestplate {
            if piece.damage(1) {
                self.chestplate = None;
            }
        }
        if let Some(piece) = &mut self.leggings {
            if piece.damage(1) {
                self.leggings = None;
            }
        }
        if let Some(piece) = &mut self.boots {
            if piece.damage(1) {
                self.boots = None;
            }
        }

        actual_damage
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
        let actual = armor.take_damage(raw);
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
}
