//! Block properties - hardness, mining requirements, drops

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
        let mut registry = Self {
            properties: Vec::new(),
        };

        // Block ID 0: Air
        registry.properties.push(BlockProperties::air());

        // Block ID 1: Stone
        registry.properties.push(BlockProperties::stone());

        // Block ID 2: Dirt
        registry.properties.push(BlockProperties::dirt());

        // Block ID 3: Wood
        registry.properties.push(BlockProperties::wood());

        // Block ID 4: Sand
        registry.properties.push(BlockProperties::sand());

        // Block ID 5: Grass (same as dirt)
        registry.properties.push(BlockProperties::dirt());

        // Block ID 6: Cobblestone (same as stone)
        registry.properties.push(BlockProperties::stone());

        // Block ID 7: Planks (same as wood)
        registry.properties.push(BlockProperties::wood());

        // Block ID 8: Bricks (stone-like)
        registry.properties.push(BlockProperties::stone());

        // Block ID 9: Glass (stone-like but fragile)
        registry.properties.push(BlockProperties {
            hardness: 0.3,
            best_tool: None,
            required_tier: None,
            instant_break: false,
            is_solid: true,
        });

        // Block ID 10: Coal Ore
        registry.properties.push(BlockProperties::coal_ore());

        // Block ID 11: Iron Ore
        registry.properties.push(BlockProperties::iron_ore());

        // Block ID 12: Diamond Ore
        registry.properties.push(BlockProperties::diamond_ore());

        registry
    }

    /// Get properties for a block ID
    pub fn get(&self, block_id: u16) -> &BlockProperties {
        self.properties
            .get(block_id as usize)
            .unwrap_or(&self.properties[0]) // Default to air if invalid
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
        let wood_pick_time = stone.calculate_mining_time(
            Some((ToolType::Pickaxe, ToolMaterial::Wood)),
            false,
        );

        // Diamond pickaxe (correct tool, high tier)
        let diamond_pick_time = stone.calculate_mining_time(
            Some((ToolType::Pickaxe, ToolMaterial::Diamond)),
            false,
        );

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
}
