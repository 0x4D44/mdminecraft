//! Item system - Tools, blocks, and other inventory items

use serde::{Deserialize, Serialize};

/// Item type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    /// A tool (pickaxe, axe, etc.)
    Tool(ToolType, ToolMaterial),
    /// A placeable block
    Block(u16), // BlockId
    /// Food item
    Food(FoodType),
    /// Generic item
    Item(u16),
}

/// Tool types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolType {
    /// Pickaxe - mines stone, ores
    Pickaxe,
    /// Axe - chops wood
    Axe,
    /// Shovel - digs dirt, sand, gravel
    Shovel,
    /// Sword - combat weapon
    Sword,
    /// Hoe - tills farmland
    Hoe,
}

/// Tool material tiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ToolMaterial {
    /// Wooden tools (tier 0)
    Wood = 0,
    /// Stone tools (tier 1)
    Stone = 1,
    /// Iron tools (tier 2)
    Iron = 2,
    /// Diamond tools (tier 3)
    Diamond = 3,
    /// Gold tools (very fast but weak mining tier - same tier as wood)
    Gold = 4,
}

/// Food types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FoodType {
    /// Apple
    Apple,
    /// Bread
    Bread,
    /// Raw meat
    RawMeat,
    /// Cooked meat
    CookedMeat,
}

impl ToolMaterial {
    /// Get the mining speed multiplier for this material
    pub fn speed_multiplier(self) -> f32 {
        match self {
            ToolMaterial::Wood => 2.0,
            ToolMaterial::Stone => 4.0,
            ToolMaterial::Iron => 6.0,
            ToolMaterial::Diamond => 8.0,
            ToolMaterial::Gold => 12.0,
        }
    }

    /// Get the maximum durability for tools of this material
    pub fn durability(self, tool_type: ToolType) -> u32 {
        let base = match self {
            ToolMaterial::Wood => 59,
            ToolMaterial::Stone => 131,
            ToolMaterial::Iron => 250,
            ToolMaterial::Diamond => 1561,
            ToolMaterial::Gold => 32,
        };

        // Swords have different durability
        if tool_type == ToolType::Sword {
            base + 1
        } else {
            base
        }
    }

    /// Check if this material can mine blocks requiring a certain tier
    pub fn can_mine_tier(self, required: ToolMaterial) -> bool {
        // Gold has same mining tier as Wood (tier 0) despite having higher enum value
        let self_tier = match self {
            ToolMaterial::Gold => 0,
            _ => self as i32,
        };
        let required_tier = match required {
            ToolMaterial::Gold => 0,
            _ => required as i32,
        };
        self_tier >= required_tier
    }

    /// Get the attack damage for this material and tool type
    /// Returns base damage (fist = 1.0, so values are added to 1.0)
    pub fn attack_damage(self, tool_type: ToolType) -> f32 {
        match tool_type {
            ToolType::Sword => match self {
                ToolMaterial::Wood => 4.0,
                ToolMaterial::Stone => 5.0,
                ToolMaterial::Iron => 6.0,
                ToolMaterial::Diamond => 7.0,
                ToolMaterial::Gold => 4.0,
            },
            ToolType::Axe => match self {
                ToolMaterial::Wood => 7.0,
                ToolMaterial::Stone => 9.0,
                ToolMaterial::Iron => 9.0,
                ToolMaterial::Diamond => 9.0,
                ToolMaterial::Gold => 7.0,
            },
            ToolType::Pickaxe => match self {
                ToolMaterial::Wood => 2.0,
                ToolMaterial::Stone => 3.0,
                ToolMaterial::Iron => 4.0,
                ToolMaterial::Diamond => 5.0,
                ToolMaterial::Gold => 2.0,
            },
            ToolType::Shovel => match self {
                ToolMaterial::Wood => 2.5,
                ToolMaterial::Stone => 3.5,
                ToolMaterial::Iron => 4.5,
                ToolMaterial::Diamond => 5.5,
                ToolMaterial::Gold => 2.5,
            },
            ToolType::Hoe => match self {
                ToolMaterial::Wood => 1.0,
                ToolMaterial::Stone => 1.0,
                ToolMaterial::Iron => 1.0,
                ToolMaterial::Diamond => 1.0,
                ToolMaterial::Gold => 1.0,
            },
        }
    }
}

impl ToolType {
    /// Get the attack speed for this tool type
    /// Returns attacks per second
    pub fn attack_speed(self) -> f32 {
        match self {
            ToolType::Sword => 1.6,
            ToolType::Axe => 0.8,
            ToolType::Pickaxe => 1.2,
            ToolType::Shovel => 1.0,
            ToolType::Hoe => 1.0,
        }
    }
}

/// An item stack in inventory
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    /// Type of item
    pub item_type: ItemType,
    /// Quantity in stack
    pub count: u32,
    /// Durability for tools (None for non-tools)
    pub durability: Option<u32>,
}

impl ItemStack {
    /// Create a new item stack
    pub fn new(item_type: ItemType, count: u32) -> Self {
        let durability = match item_type {
            ItemType::Tool(tool_type, material) => Some(material.durability(tool_type)),
            _ => None,
        };

        Self {
            item_type,
            count,
            durability,
        }
    }

    /// Maximum stack size for this item type
    pub fn max_stack_size(&self) -> u32 {
        match self.item_type {
            ItemType::Tool(_, _) => 1, // Tools don't stack
            ItemType::Block(_) => 64,
            ItemType::Food(_) => 64,
            ItemType::Item(_) => 64,
        }
    }

    /// Check if this stack can accept more items
    pub fn can_add(&self, count: u32) -> bool {
        self.count + count <= self.max_stack_size()
    }

    /// Damage the tool (reduce durability)
    pub fn damage_tool(&mut self, amount: u32) -> bool {
        if let Some(ref mut durability) = self.durability {
            if *durability > amount {
                *durability -= amount;
                false // Tool still usable
            } else {
                true // Tool broken
            }
        } else {
            false
        }
    }

    /// Damage durability by a given amount (for tools)
    pub fn damage_durability(&mut self, amount: u32) {
        if let Some(ref mut durability) = self.durability {
            *durability = durability.saturating_sub(amount);
        }
    }

    /// Check if tool is broken (durability = 0)
    pub fn is_broken(&self) -> bool {
        self.durability.map(|d| d == 0).unwrap_or(false)
    }

    /// Get maximum durability for this item
    pub fn max_durability(&self) -> Option<u32> {
        match self.item_type {
            ItemType::Tool(tool_type, material) => Some(material.durability(tool_type)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_material_ordering() {
        assert!(ToolMaterial::Diamond > ToolMaterial::Iron);
        assert!(ToolMaterial::Iron > ToolMaterial::Stone);
        assert!(ToolMaterial::Stone > ToolMaterial::Wood);
    }

    #[test]
    fn test_tool_durability() {
        let wood_pick = ToolMaterial::Wood.durability(ToolType::Pickaxe);
        let diamond_pick = ToolMaterial::Diamond.durability(ToolType::Pickaxe);
        assert!(diamond_pick > wood_pick);
    }

    #[test]
    fn test_item_stack() {
        let mut stack = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron), 1);

        assert_eq!(stack.count, 1);
        assert!(stack.durability.is_some());
        assert_eq!(stack.max_stack_size(), 1);

        // Damage the tool
        let broken = stack.damage_tool(100);
        assert!(!broken);
        assert_eq!(stack.durability, Some(150));

        // Break the tool
        let broken = stack.damage_tool(200);
        assert!(broken);
    }

    #[test]
    fn test_block_stack() {
        let stack = ItemStack::new(ItemType::Block(1), 64);
        assert_eq!(stack.count, 64);
        assert_eq!(stack.max_stack_size(), 64);
        assert!(stack.durability.is_none());
    }

    #[test]
    fn test_attack_damage() {
        // Test sword damage progression
        assert_eq!(ToolMaterial::Wood.attack_damage(ToolType::Sword), 4.0);
        assert_eq!(ToolMaterial::Stone.attack_damage(ToolType::Sword), 5.0);
        assert_eq!(ToolMaterial::Iron.attack_damage(ToolType::Sword), 6.0);
        assert_eq!(ToolMaterial::Diamond.attack_damage(ToolType::Sword), 7.0);
        assert_eq!(ToolMaterial::Gold.attack_damage(ToolType::Sword), 4.0);

        // Test axe damage (higher than swords)
        assert_eq!(ToolMaterial::Diamond.attack_damage(ToolType::Axe), 9.0);
        assert!(ToolMaterial::Diamond.attack_damage(ToolType::Axe) > ToolMaterial::Diamond.attack_damage(ToolType::Sword));

        // Test hoe damage (minimal)
        assert_eq!(ToolMaterial::Diamond.attack_damage(ToolType::Hoe), 1.0);
        assert_eq!(ToolMaterial::Wood.attack_damage(ToolType::Hoe), 1.0);
    }

    #[test]
    fn test_attack_speed() {
        // Swords are fastest
        assert_eq!(ToolType::Sword.attack_speed(), 1.6);

        // Axes are slowest
        assert_eq!(ToolType::Axe.attack_speed(), 0.8);

        // Others in between
        assert_eq!(ToolType::Pickaxe.attack_speed(), 1.2);
        assert_eq!(ToolType::Shovel.attack_speed(), 1.0);
        assert_eq!(ToolType::Hoe.attack_speed(), 1.0);
    }
}
