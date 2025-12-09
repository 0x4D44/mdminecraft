//! Item system - Tools, blocks, and other inventory items

use serde::{Deserialize, Serialize};
use crate::enchantment::{Enchantment, EnchantmentType};

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

    /// Get the harvest tier of this material (0=Wood, 1=Stone, 2=Iron, 3=Diamond).
    /// Gold has the same harvest tier as Wood (0) despite being valuable.
    /// This matches the HarvestLevel enum values in the assets crate.
    pub fn harvest_tier(self) -> u8 {
        match self {
            ToolMaterial::Wood | ToolMaterial::Gold => 0,
            ToolMaterial::Stone => 1,
            ToolMaterial::Iron => 2,
            ToolMaterial::Diamond => 3,
        }
    }

    /// Check if this material can mine blocks requiring a certain tier
    pub fn can_mine_tier(self, required: ToolMaterial) -> bool {
        self.harvest_tier() >= required.harvest_tier()
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

    /// Get the mining speed multiplier when this tool is used on its preferred block types.
    /// Returns 1.0 for tools used on non-preferred blocks (no bonus or penalty).
    /// The actual mining speed also depends on the tool material's speed_multiplier().
    pub fn effectiveness_multiplier(self) -> f32 {
        match self {
            // Tools get a 1.5x effectiveness bonus on their preferred blocks
            // This is in addition to the material's base speed multiplier
            ToolType::Pickaxe => 1.5,  // Effective on stone, ores
            ToolType::Axe => 1.5,      // Effective on wood
            ToolType::Shovel => 1.5,   // Effective on dirt, sand, gravel
            ToolType::Sword => 1.0,    // No mining bonus (combat tool)
            ToolType::Hoe => 1.0,      // No mining bonus (farming tool)
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
    /// Enchantments applied to this item (None for non-enchantable items)
    pub enchantments: Option<Vec<Enchantment>>,
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
            enchantments: None,
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

    /// Get the harvest tier for this tool (0-3).
    /// Returns None if this item is not a tool.
    /// The harvest tier determines which blocks this tool can successfully mine.
    pub fn harvest_tier(&self) -> Option<u8> {
        match self.item_type {
            ItemType::Tool(_, material) => Some(material.harvest_tier()),
            _ => None,
        }
    }

    /// Check if this tool can harvest blocks requiring a specific tier.
    /// Returns false if this item is not a tool.
    pub fn can_harvest_tier(&self, required_tier: u8) -> bool {
        self.harvest_tier()
            .map(|tier| tier >= required_tier)
            .unwrap_or(false)
    }

    /// Get the mining speed multiplier for this tool.
    /// Returns the material speed multiplier * tool effectiveness multiplier.
    /// Returns 1.0 for non-tools (hand mining speed).
    pub fn mining_speed_multiplier(&self) -> f32 {
        match self.item_type {
            ItemType::Tool(tool_type, material) => {
                material.speed_multiplier() * tool_type.effectiveness_multiplier()
            }
            _ => 1.0, // Hand mining
        }
    }

    /// Check if this item can be enchanted
    pub fn is_enchantable(&self) -> bool {
        matches!(self.item_type, ItemType::Tool(_, _))
    }

    /// Add an enchantment to this item
    /// Returns true if the enchantment was added successfully
    pub fn add_enchantment(&mut self, enchantment: Enchantment) -> bool {
        // Only tools can be enchanted for now
        if !self.is_enchantable() {
            return false;
        }

        // Initialize enchantments vec if needed
        if self.enchantments.is_none() {
            self.enchantments = Some(Vec::new());
        }

        if let Some(ref mut enchants) = self.enchantments {
            // Check compatibility with existing enchantments
            for existing in enchants.iter() {
                if !existing.enchantment_type.is_compatible_with(&enchantment.enchantment_type) {
                    return false; // Incompatible enchantment
                }
            }

            // Check if we already have this enchantment type (upgrade level if so)
            for existing in enchants.iter_mut() {
                if existing.enchantment_type == enchantment.enchantment_type {
                    // Upgrade to higher level
                    if enchantment.level > existing.level {
                        existing.level = enchantment.level;
                    }
                    return true;
                }
            }

            // Add new enchantment
            enchants.push(enchantment);
            true
        } else {
            false
        }
    }

    /// Get all enchantments on this item
    pub fn get_enchantments(&self) -> &[Enchantment] {
        self.enchantments.as_deref().unwrap_or(&[])
    }

    /// Check if this item has a specific enchantment type
    pub fn has_enchantment(&self, enchant_type: EnchantmentType) -> bool {
        self.get_enchantments()
            .iter()
            .any(|e| e.enchantment_type == enchant_type)
    }

    /// Get the level of a specific enchantment, or 0 if not present
    pub fn enchantment_level(&self, enchant_type: EnchantmentType) -> u8 {
        self.get_enchantments()
            .iter()
            .find(|e| e.enchantment_type == enchant_type)
            .map(|e| e.level)
            .unwrap_or(0)
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

    #[test]
    fn test_harvest_tier() {
        // Verify tier values match HarvestLevel enum
        assert_eq!(ToolMaterial::Wood.harvest_tier(), 0);
        assert_eq!(ToolMaterial::Stone.harvest_tier(), 1);
        assert_eq!(ToolMaterial::Iron.harvest_tier(), 2);
        assert_eq!(ToolMaterial::Diamond.harvest_tier(), 3);

        // Gold has same harvest tier as Wood
        assert_eq!(ToolMaterial::Gold.harvest_tier(), 0);
        assert_eq!(ToolMaterial::Gold.harvest_tier(), ToolMaterial::Wood.harvest_tier());
    }

    #[test]
    fn test_can_mine_tier_with_harvest_tier() {
        // Wood tools can mine wood-tier blocks
        assert!(ToolMaterial::Wood.can_mine_tier(ToolMaterial::Wood));

        // Iron tools can mine stone-tier blocks
        assert!(ToolMaterial::Iron.can_mine_tier(ToolMaterial::Stone));
        assert!(ToolMaterial::Iron.can_mine_tier(ToolMaterial::Wood));

        // Wood tools cannot mine iron-tier blocks
        assert!(!ToolMaterial::Wood.can_mine_tier(ToolMaterial::Iron));
        assert!(!ToolMaterial::Stone.can_mine_tier(ToolMaterial::Diamond));

        // Gold has same mining capability as wood
        assert!(ToolMaterial::Gold.can_mine_tier(ToolMaterial::Wood));
        assert!(!ToolMaterial::Gold.can_mine_tier(ToolMaterial::Stone));
    }

    #[test]
    fn test_item_stack_harvest_tier() {
        // Tool items return their harvest tier
        let wood_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood), 1);
        assert_eq!(wood_pick.harvest_tier(), Some(0));

        let stone_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Stone), 1);
        assert_eq!(stone_pick.harvest_tier(), Some(1));

        let iron_axe = ItemStack::new(ItemType::Tool(ToolType::Axe, ToolMaterial::Iron), 1);
        assert_eq!(iron_axe.harvest_tier(), Some(2));

        let diamond_sword = ItemStack::new(ItemType::Tool(ToolType::Sword, ToolMaterial::Diamond), 1);
        assert_eq!(diamond_sword.harvest_tier(), Some(3));

        // Gold has same tier as wood
        let gold_shovel = ItemStack::new(ItemType::Tool(ToolType::Shovel, ToolMaterial::Gold), 1);
        assert_eq!(gold_shovel.harvest_tier(), Some(0));

        // Non-tool items return None
        let block_stack = ItemStack::new(ItemType::Block(1), 64);
        assert_eq!(block_stack.harvest_tier(), None);

        let food_stack = ItemStack::new(ItemType::Food(FoodType::Apple), 16);
        assert_eq!(food_stack.harvest_tier(), None);
    }

    #[test]
    fn test_item_stack_can_harvest_tier() {
        // Wooden pickaxe can harvest wood-tier (0) blocks
        let wood_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood), 1);
        assert!(wood_pick.can_harvest_tier(0)); // Wood tier
        assert!(!wood_pick.can_harvest_tier(1)); // Stone tier
        assert!(!wood_pick.can_harvest_tier(2)); // Iron tier
        assert!(!wood_pick.can_harvest_tier(3)); // Diamond tier

        // Stone pickaxe can harvest wood and stone tiers
        let stone_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Stone), 1);
        assert!(stone_pick.can_harvest_tier(0));
        assert!(stone_pick.can_harvest_tier(1));
        assert!(!stone_pick.can_harvest_tier(2));
        assert!(!stone_pick.can_harvest_tier(3));

        // Iron pickaxe can harvest up to iron tier
        let iron_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron), 1);
        assert!(iron_pick.can_harvest_tier(0));
        assert!(iron_pick.can_harvest_tier(1));
        assert!(iron_pick.can_harvest_tier(2));
        assert!(!iron_pick.can_harvest_tier(3));

        // Diamond pickaxe can harvest all tiers
        let diamond_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Diamond), 1);
        assert!(diamond_pick.can_harvest_tier(0));
        assert!(diamond_pick.can_harvest_tier(1));
        assert!(diamond_pick.can_harvest_tier(2));
        assert!(diamond_pick.can_harvest_tier(3));

        // Gold has same capability as wood
        let gold_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Gold), 1);
        assert!(gold_pick.can_harvest_tier(0));
        assert!(!gold_pick.can_harvest_tier(1));

        // Non-tool items cannot harvest any tier
        let block_stack = ItemStack::new(ItemType::Block(1), 64);
        assert!(!block_stack.can_harvest_tier(0));
        assert!(!block_stack.can_harvest_tier(1));
        assert!(!block_stack.can_harvest_tier(2));
        assert!(!block_stack.can_harvest_tier(3));
    }

    #[test]
    fn test_tool_effectiveness_multiplier() {
        // Mining tools have 1.5x effectiveness on their preferred blocks
        assert_eq!(ToolType::Pickaxe.effectiveness_multiplier(), 1.5);
        assert_eq!(ToolType::Axe.effectiveness_multiplier(), 1.5);
        assert_eq!(ToolType::Shovel.effectiveness_multiplier(), 1.5);

        // Non-mining tools have no effectiveness bonus
        assert_eq!(ToolType::Sword.effectiveness_multiplier(), 1.0);
        assert_eq!(ToolType::Hoe.effectiveness_multiplier(), 1.0);
    }

    #[test]
    fn test_mining_speed_multiplier() {
        // Test pickaxe mining speeds (material * effectiveness)
        let wood_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood), 1);
        assert_eq!(wood_pick.mining_speed_multiplier(), 2.0 * 1.5); // 3.0

        let stone_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Stone), 1);
        assert_eq!(stone_pick.mining_speed_multiplier(), 4.0 * 1.5); // 6.0

        let iron_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron), 1);
        assert_eq!(iron_pick.mining_speed_multiplier(), 6.0 * 1.5); // 9.0

        let diamond_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Diamond), 1);
        assert_eq!(diamond_pick.mining_speed_multiplier(), 8.0 * 1.5); // 12.0

        // Gold is fastest but weakest tier
        let gold_pick = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Gold), 1);
        assert_eq!(gold_pick.mining_speed_multiplier(), 12.0 * 1.5); // 18.0
        assert!(gold_pick.mining_speed_multiplier() > diamond_pick.mining_speed_multiplier());

        // Test sword (no effectiveness bonus)
        let diamond_sword = ItemStack::new(ItemType::Tool(ToolType::Sword, ToolMaterial::Diamond), 1);
        assert_eq!(diamond_sword.mining_speed_multiplier(), 8.0 * 1.0); // 8.0

        // Non-tool items use hand speed
        let block_stack = ItemStack::new(ItemType::Block(1), 64);
        assert_eq!(block_stack.mining_speed_multiplier(), 1.0);
    }

    #[test]
    fn test_enchantment_application() {
        let mut pickaxe = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Diamond), 1);

        // Initially no enchantments
        assert!(pickaxe.get_enchantments().is_empty());
        assert!(!pickaxe.has_enchantment(EnchantmentType::Efficiency));

        // Add Efficiency enchantment
        let efficiency = Enchantment::new(EnchantmentType::Efficiency, 3);
        assert!(pickaxe.add_enchantment(efficiency));

        // Verify enchantment was added
        assert_eq!(pickaxe.get_enchantments().len(), 1);
        assert!(pickaxe.has_enchantment(EnchantmentType::Efficiency));
        assert_eq!(pickaxe.enchantment_level(EnchantmentType::Efficiency), 3);

        // Add another compatible enchantment
        let unbreaking = Enchantment::new(EnchantmentType::Unbreaking, 2);
        assert!(pickaxe.add_enchantment(unbreaking));
        assert_eq!(pickaxe.get_enchantments().len(), 2);
    }

    #[test]
    fn test_enchantment_upgrade() {
        let mut sword = ItemStack::new(ItemType::Tool(ToolType::Sword, ToolMaterial::Iron), 1);

        // Add Sharpness I
        let sharpness1 = Enchantment::new(EnchantmentType::Sharpness, 1);
        assert!(sword.add_enchantment(sharpness1));
        assert_eq!(sword.enchantment_level(EnchantmentType::Sharpness), 1);

        // Upgrade to Sharpness III
        let sharpness3 = Enchantment::new(EnchantmentType::Sharpness, 3);
        assert!(sword.add_enchantment(sharpness3));
        assert_eq!(sword.enchantment_level(EnchantmentType::Sharpness), 3);

        // Still only one enchantment (upgraded, not duplicated)
        assert_eq!(sword.get_enchantments().len(), 1);
    }

    #[test]
    fn test_enchantment_incompatibility() {
        let mut pickaxe = ItemStack::new(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Diamond), 1);

        // Add Silk Touch
        let silk_touch = Enchantment::new(EnchantmentType::SilkTouch, 1);
        assert!(pickaxe.add_enchantment(silk_touch));

        // Try to add Fortune (incompatible with Silk Touch)
        let fortune = Enchantment::new(EnchantmentType::Fortune, 3);
        assert!(!pickaxe.add_enchantment(fortune)); // Should fail

        // Still only Silk Touch
        assert_eq!(pickaxe.get_enchantments().len(), 1);
        assert!(pickaxe.has_enchantment(EnchantmentType::SilkTouch));
        assert!(!pickaxe.has_enchantment(EnchantmentType::Fortune));
    }

    #[test]
    fn test_non_tool_not_enchantable() {
        let mut block = ItemStack::new(ItemType::Block(1), 64);

        // Blocks cannot be enchanted
        assert!(!block.is_enchantable());

        let efficiency = Enchantment::new(EnchantmentType::Efficiency, 1);
        assert!(!block.add_enchantment(efficiency));
    }
}
