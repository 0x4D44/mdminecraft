//! Block properties - hardness, mining requirements, drops

use crate::{
    farming_blocks, interactive_blocks, redstone_blocks, BLOCK_AIR, BLOCK_BEDROCK, BLOCK_BOOKSHELF,
    BLOCK_BROWN_MUSHROOM, BLOCK_CAVE_VINES, BLOCK_CLAY, BLOCK_COAL_ORE, BLOCK_COBBLESTONE,
    BLOCK_COBBLESTONE_WALL, BLOCK_CRAFTING_TABLE, BLOCK_CRYING_OBSIDIAN, BLOCK_DIAMOND_ORE,
    BLOCK_DIRT, BLOCK_DOUBLE_OAK_SLAB, BLOCK_DOUBLE_STONE_BRICK_SLAB, BLOCK_DOUBLE_STONE_SLAB,
    BLOCK_END_PORTAL, BLOCK_END_PORTAL_FRAME, BLOCK_END_STONE, BLOCK_FURNACE, BLOCK_FURNACE_LIT,
    BLOCK_GHAST_TEAR_ORE, BLOCK_GLASS, BLOCK_GLISTERING_MELON_ORE, BLOCK_GLOWSTONE,
    BLOCK_GLOWSTONE_DUST_ORE, BLOCK_GLOW_LICHEN, BLOCK_GOLD_ORE, BLOCK_GRASS, BLOCK_GRAVEL,
    BLOCK_HANGING_ROOTS, BLOCK_ICE, BLOCK_IRON_ORE, BLOCK_LAVA, BLOCK_LAVA_FLOWING,
    BLOCK_LAVA_LEGACY, BLOCK_MAGMA_CREAM_ORE, BLOCK_MOSS_CARPET, BLOCK_NETHER_PORTAL,
    BLOCK_NETHER_QUARTZ_ORE, BLOCK_OAK_LOG, BLOCK_OAK_PLANKS, BLOCK_OBSIDIAN,
    BLOCK_PHANTOM_MEMBRANE_ORE, BLOCK_POINTED_DRIPSTONE, BLOCK_PUFFERFISH_ORE,
    BLOCK_RABBIT_FOOT_ORE, BLOCK_REDSTONE_DUST_ORE, BLOCK_RESPAWN_ANCHOR, BLOCK_SAND,
    BLOCK_SCULK_VEIN, BLOCK_SNOW, BLOCK_SPORE_BLOSSOM, BLOCK_STONE, BLOCK_STONE_BRICKS,
    BLOCK_SUGAR_CANE, BLOCK_WATER, BLOCK_WATER_FLOWING,
};
use mdminecraft_core::{ToolMaterial, ToolType};

/// Properties of a block type
#[derive(Debug, Clone)]
pub struct BlockProperties {
    /// How long it takes to mine (base time in seconds)
    pub hardness: f32,

    /// The best tool type for this block
    pub best_tool: Option<ToolType>,

    /// Minimum tool tier required to harvest this block
    pub required_tier: Option<ToolMaterial>,

    /// Whether this block can be instantly broken
    pub instant_break: bool,

    /// Whether this block is solid (affects collision)
    pub is_solid: bool,
}

impl Default for BlockProperties {
    fn default() -> Self {
        Self {
            hardness: 1.0,
            best_tool: None,
            required_tier: None,
            instant_break: false,
            is_solid: true,
        }
    }
}

impl BlockProperties {
    /// Create properties for air (instant break, not solid)
    pub fn air() -> Self {
        Self {
            hardness: 0.0,
            best_tool: None,
            required_tier: None,
            instant_break: true,
            is_solid: false,
        }
    }

    /// Create properties for dirt/grass
    pub fn dirt() -> Self {
        Self {
            hardness: 0.5,
            best_tool: Some(ToolType::Shovel),
            required_tier: None,
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for stone
    pub fn stone() -> Self {
        Self {
            hardness: 1.5,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: Some(ToolMaterial::Wood),
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for wood
    pub fn wood() -> Self {
        Self {
            hardness: 2.0,
            best_tool: Some(ToolType::Axe),
            required_tier: None,
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for sand
    pub fn sand() -> Self {
        Self {
            hardness: 0.5,
            best_tool: Some(ToolType::Shovel),
            required_tier: None,
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for iron ore
    pub fn iron_ore() -> Self {
        Self {
            hardness: 3.0,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: Some(ToolMaterial::Stone),
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for diamond ore
    pub fn diamond_ore() -> Self {
        Self {
            hardness: 3.0,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: Some(ToolMaterial::Iron),
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for coal ore
    pub fn coal_ore() -> Self {
        Self {
            hardness: 3.0,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: Some(ToolMaterial::Wood),
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for gold ore.
    pub fn gold_ore() -> Self {
        Self {
            hardness: 3.0,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: Some(ToolMaterial::Iron),
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for glass (solid but non-opaque in the registry).
    pub fn glass() -> Self {
        Self {
            hardness: 0.3,
            best_tool: None,
            required_tier: None,
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for obsidian (very slow, diamond pickaxe required).
    pub fn obsidian() -> Self {
        Self {
            hardness: 50.0,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: Some(ToolMaterial::Diamond),
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for bedrock (effectively unbreakable).
    pub fn bedrock() -> Self {
        Self {
            hardness: 9_999.0,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: Some(ToolMaterial::Diamond),
            instant_break: false,
            is_solid: true,
        }
    }

    /// Create properties for fluids (not solid).
    pub fn fluid() -> Self {
        Self {
            hardness: 0.0,
            best_tool: None,
            required_tier: None,
            instant_break: true,
            is_solid: false,
        }
    }

    /// Calculate mining time for this block
    pub fn calculate_mining_time(
        &self,
        tool: Option<(ToolType, ToolMaterial)>,
        in_water: bool,
    ) -> f32 {
        if self.instant_break {
            return 0.0;
        }

        let base_time = self.hardness * 1.5; // Base time multiplier

        // Calculate tool effectiveness
        let tool_multiplier = if let Some((tool_type, material)) = tool {
            if Some(tool_type) == self.best_tool {
                // Using correct tool
                material.speed_multiplier()
            } else {
                // Wrong tool is slower
                1.0
            }
        } else {
            // No tool (hand mining)
            1.0
        };

        // Check if tool tier is sufficient
        let can_harvest = if let Some(required) = self.required_tier {
            if let Some((_tool_type, material)) = tool {
                material.can_mine_tier(required)
            } else {
                false
            }
        } else {
            true
        };

        // If can't harvest, mining takes 5x longer
        let harvest_penalty = if can_harvest { 1.0 } else { 5.0 };

        // Water slows mining by 5x
        let water_penalty = if in_water { 5.0 } else { 1.0 };

        base_time / tool_multiplier * harvest_penalty * water_penalty
    }

    /// Check if this block can be harvested with the given tool
    pub fn can_harvest(&self, tool: Option<(ToolType, ToolMaterial)>) -> bool {
        if let Some(required) = self.required_tier {
            if let Some((_tool_type, material)) = tool {
                material.can_mine_tier(required)
            } else {
                false
            }
        } else {
            true
        }
    }
}

/// Block properties registry
pub struct BlockPropertiesRegistry {
    properties: Vec<BlockProperties>,
}

impl BlockPropertiesRegistry {
    /// Create a new registry with default properties
    pub fn new() -> Self {
        let mut properties = vec![BlockProperties::default(); 256];

        properties[BLOCK_AIR as usize] = BlockProperties::air();
        properties[BLOCK_STONE as usize] = BlockProperties::stone();
        properties[BLOCK_DIRT as usize] = BlockProperties::dirt();
        properties[BLOCK_GRASS as usize] = BlockProperties::dirt();
        properties[BLOCK_SAND as usize] = BlockProperties::sand();
        properties[BLOCK_GRAVEL as usize] = BlockProperties::sand();
        properties[BLOCK_WATER as usize] = BlockProperties::fluid();
        properties[BLOCK_WATER_FLOWING as usize] = BlockProperties::fluid();
        properties[BLOCK_LAVA as usize] = BlockProperties::fluid();
        properties[BLOCK_LAVA_LEGACY as usize] = BlockProperties::fluid();
        properties[BLOCK_LAVA_FLOWING as usize] = BlockProperties::fluid();

        properties[BLOCK_ICE as usize] = BlockProperties {
            hardness: 0.5,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: None,
            instant_break: false,
            is_solid: true,
        };
        properties[BLOCK_SNOW as usize] = BlockProperties {
            hardness: 0.2,
            best_tool: Some(ToolType::Shovel),
            required_tier: None,
            instant_break: false,
            is_solid: true,
        };
        properties[BLOCK_CLAY as usize] = BlockProperties {
            hardness: 0.6,
            best_tool: Some(ToolType::Shovel),
            required_tier: None,
            instant_break: false,
            is_solid: true,
        };
        properties[BLOCK_BEDROCK as usize] = BlockProperties::bedrock();
        properties[BLOCK_END_PORTAL_FRAME as usize] = BlockProperties::bedrock();

        properties[BLOCK_OAK_LOG as usize] = BlockProperties::wood();
        properties[BLOCK_OAK_PLANKS as usize] = BlockProperties::wood();
        properties[BLOCK_CRAFTING_TABLE as usize] = BlockProperties::wood();
        properties[BLOCK_BOOKSHELF as usize] = BlockProperties::wood();

        properties[BLOCK_COAL_ORE as usize] = BlockProperties::coal_ore();
        properties[BLOCK_IRON_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_GOLD_ORE as usize] = BlockProperties::gold_ore();
        properties[BLOCK_DIAMOND_ORE as usize] = BlockProperties::diamond_ore();
        properties[BLOCK_MAGMA_CREAM_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_GHAST_TEAR_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_GLISTERING_MELON_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_RABBIT_FOOT_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_PHANTOM_MEMBRANE_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_REDSTONE_DUST_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_GLOWSTONE_DUST_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_PUFFERFISH_ORE as usize] = BlockProperties::iron_ore();
        properties[BLOCK_NETHER_QUARTZ_ORE as usize] = BlockProperties::iron_ore();

        properties[BLOCK_COBBLESTONE as usize] = BlockProperties::stone();
        properties[BLOCK_COBBLESTONE_WALL as usize] = BlockProperties::stone();
        properties[BLOCK_STONE_BRICKS as usize] = BlockProperties::stone();
        properties[BLOCK_DOUBLE_STONE_SLAB as usize] = BlockProperties::stone();
        properties[BLOCK_DOUBLE_STONE_BRICK_SLAB as usize] = BlockProperties::stone();
        properties[BLOCK_DOUBLE_OAK_SLAB as usize] = BlockProperties::wood();
        properties[interactive_blocks::STONE_BRICK_SLAB as usize] = BlockProperties::stone();
        properties[interactive_blocks::STONE_BRICK_STAIRS as usize] = BlockProperties::stone();
        properties[interactive_blocks::STONE_BRICK_WALL as usize] = BlockProperties::stone();
        properties[BLOCK_FURNACE as usize] = BlockProperties::stone();
        properties[BLOCK_FURNACE_LIT as usize] = BlockProperties::stone();
        properties[BLOCK_OBSIDIAN as usize] = BlockProperties::obsidian();
        properties[BLOCK_CRYING_OBSIDIAN as usize] = BlockProperties::obsidian();
        properties[BLOCK_RESPAWN_ANCHOR as usize] = BlockProperties::obsidian();
        properties[BLOCK_GLASS as usize] = BlockProperties::glass();
        properties[BLOCK_END_STONE as usize] = BlockProperties::stone();
        properties[BLOCK_GLOWSTONE as usize] = BlockProperties {
            hardness: 0.3,
            best_tool: Some(ToolType::Pickaxe),
            required_tier: None,
            instant_break: false,
            is_solid: true,
        };

        // Non-solid interaction blocks (collision should ignore these; shapes handled elsewhere).
        properties[interactive_blocks::TORCH as usize] = BlockProperties::air();
        properties[interactive_blocks::LADDER as usize] = BlockProperties::air();
        properties[BLOCK_NETHER_PORTAL as usize] = BlockProperties::air();
        properties[BLOCK_END_PORTAL as usize] = BlockProperties::air();

        // Redstone components that should not block movement.
        properties[redstone_blocks::LEVER as usize] = BlockProperties::air();
        properties[redstone_blocks::STONE_BUTTON as usize] = BlockProperties::air();
        properties[redstone_blocks::OAK_BUTTON as usize] = BlockProperties::air();
        properties[redstone_blocks::STONE_PRESSURE_PLATE as usize] = BlockProperties::air();
        properties[redstone_blocks::OAK_PRESSURE_PLATE as usize] = BlockProperties::air();
        properties[redstone_blocks::REDSTONE_WIRE as usize] = BlockProperties::air();
        properties[redstone_blocks::REDSTONE_TORCH as usize] = BlockProperties::air();
        properties[redstone_blocks::REDSTONE_REPEATER as usize] = BlockProperties::air();
        properties[redstone_blocks::REDSTONE_COMPARATOR as usize] = BlockProperties::air();

        // Crops are non-solid and instantly break.
        for crop_id in farming_blocks::WHEAT_0..=farming_blocks::WHEAT_7 {
            properties[crop_id as usize] = BlockProperties::air();
        }
        for crop_id in farming_blocks::CARROTS_0..=farming_blocks::CARROTS_3 {
            properties[crop_id as usize] = BlockProperties::air();
        }
        for crop_id in farming_blocks::POTATOES_0..=farming_blocks::POTATOES_3 {
            properties[crop_id as usize] = BlockProperties::air();
        }

        // Sugar cane is non-solid and instantly breaks.
        properties[BLOCK_SUGAR_CANE as usize] = BlockProperties::air();

        // Mushrooms are non-solid and instantly break.
        properties[BLOCK_BROWN_MUSHROOM as usize] = BlockProperties::air();

        // Cave decorations that should not block movement.
        properties[BLOCK_GLOW_LICHEN as usize] = BlockProperties::air();
        properties[BLOCK_POINTED_DRIPSTONE as usize] = BlockProperties::air();
        properties[BLOCK_CAVE_VINES as usize] = BlockProperties::air();
        properties[BLOCK_MOSS_CARPET as usize] = BlockProperties::air();
        properties[BLOCK_SPORE_BLOSSOM as usize] = BlockProperties::air();
        properties[BLOCK_HANGING_ROOTS as usize] = BlockProperties::air();
        properties[BLOCK_SCULK_VEIN as usize] = BlockProperties::air();

        Self { properties }
    }

    /// Get properties for a block ID
    pub fn get(&self, block_id: u16) -> &BlockProperties {
        self.properties
            .get(block_id as usize)
            .unwrap_or(&self.properties[BLOCK_AIR as usize]) // Default to air if invalid
    }
}

impl Default for BlockPropertiesRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mining_time() {
        let stone = BlockProperties::stone();

        // Hand mining stone (slow)
        let hand_time = stone.calculate_mining_time(None, false);

        // Wooden pickaxe (correct tool, minimum tier)
        let wood_pick_time =
            stone.calculate_mining_time(Some((ToolType::Pickaxe, ToolMaterial::Wood)), false);

        // Diamond pickaxe (correct tool, high tier)
        let diamond_pick_time =
            stone.calculate_mining_time(Some((ToolType::Pickaxe, ToolMaterial::Diamond)), false);

        assert!(wood_pick_time < hand_time);
        assert!(diamond_pick_time < wood_pick_time);
    }

    #[test]
    fn test_harvest_requirements() {
        let diamond_ore = BlockProperties::diamond_ore();

        // Can't harvest with wood pickaxe
        assert!(!diamond_ore.can_harvest(Some((ToolType::Pickaxe, ToolMaterial::Wood))));

        // Can't harvest with stone pickaxe
        assert!(!diamond_ore.can_harvest(Some((ToolType::Pickaxe, ToolMaterial::Stone))));

        // Can harvest with iron pickaxe
        assert!(diamond_ore.can_harvest(Some((ToolType::Pickaxe, ToolMaterial::Iron))));

        // Can harvest with diamond pickaxe
        assert!(diamond_ore.can_harvest(Some((ToolType::Pickaxe, ToolMaterial::Diamond))));
    }

    #[test]
    fn test_registry() {
        let registry = BlockPropertiesRegistry::new();

        // Air should be instant break
        assert!(registry.get(0).instant_break);

        // Stone should require pickaxe
        assert_eq!(registry.get(1).best_tool, Some(ToolType::Pickaxe));

        // Dirt should use shovel
        assert_eq!(registry.get(2).best_tool, Some(ToolType::Shovel));
    }

    #[test]
    fn test_registry_uses_blocks_json_ids() {
        let registry = BlockPropertiesRegistry::new();

        let coal_ore = registry.get(BLOCK_COAL_ORE);
        assert_eq!(coal_ore.best_tool, Some(ToolType::Pickaxe));
        assert_eq!(coal_ore.required_tier, Some(ToolMaterial::Wood));
        assert!(!coal_ore.instant_break);
        assert!(coal_ore.hardness > 0.0);

        let iron_ore = registry.get(BLOCK_IRON_ORE);
        assert_eq!(iron_ore.required_tier, Some(ToolMaterial::Stone));

        let oak_log = registry.get(BLOCK_OAK_LOG);
        assert_eq!(oak_log.best_tool, Some(ToolType::Axe));

        let water = registry.get(BLOCK_WATER);
        assert!(!water.is_solid);
        assert!(water.instant_break);

        let invalid = registry.get(999);
        assert!(invalid.instant_break);
        assert!(!invalid.is_solid);

        let torch = registry.get(interactive_blocks::TORCH);
        assert!(torch.instant_break);
        assert!(!torch.is_solid);

        let redstone_wire = registry.get(redstone_blocks::REDSTONE_WIRE);
        assert!(redstone_wire.instant_break);
        assert!(!redstone_wire.is_solid);

        let wheat = registry.get(farming_blocks::WHEAT_0);
        assert!(wheat.instant_break);
        assert!(!wheat.is_solid);
    }
}
