//! Crafting system - Recipes for creating items from materials

use crate::{ItemType, ToolMaterial, ToolType};
use serde::{Deserialize, Serialize};

/// A crafting recipe that transforms input items into output items
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    /// Items required as input (item type and count)
    pub inputs: Vec<(ItemType, u32)>,
    /// Item produced as output
    pub output: ItemType,
    /// Number of output items produced
    pub output_count: u32,
}

impl Recipe {
    /// Create a new recipe
    pub fn new(inputs: Vec<(ItemType, u32)>, output: ItemType, output_count: u32) -> Self {
        Self {
            inputs,
            output,
            output_count,
        }
    }

    /// Check if the recipe can be crafted with the given available items
    pub fn can_craft(&self, available: &[(ItemType, u32)]) -> bool {
        self.inputs.iter().all(|(required_type, required_count)| {
            available
                .iter()
                .find(|(avail_type, _)| avail_type == required_type)
                .map(|(_, avail_count)| avail_count >= required_count)
                .unwrap_or(false)
        })
    }
}

/// Standard tool recipes following Minecraft patterns
pub struct ToolRecipes;

impl ToolRecipes {
    /// Get the recipe for a tool given its type and material
    ///
    /// Recipes follow standard Minecraft patterns:
    /// - Pickaxe: 3 material + 2 sticks
    /// - Axe: 3 material + 2 sticks
    /// - Shovel: 1 material + 2 sticks
    /// - Sword: 2 material + 1 stick
    /// - Hoe: 2 material + 2 sticks
    pub fn get_recipe(tool_type: ToolType, material: ToolMaterial) -> Recipe {
        let material_item = Self::material_to_item(material);
        let stick = ItemType::Item(1); // Stick (placeholder block ID)

        let (material_count, stick_count) = match tool_type {
            ToolType::Pickaxe => (3, 2),
            ToolType::Axe => (3, 2),
            ToolType::Shovel => (1, 2),
            ToolType::Sword => (2, 1),
            ToolType::Hoe => (2, 2),
        };

        Recipe::new(
            vec![(material_item, material_count), (stick, stick_count)],
            ItemType::Tool(tool_type, material),
            1,
        )
    }

    /// Convert tool material to the corresponding crafting material item
    fn material_to_item(material: ToolMaterial) -> ItemType {
        match material {
            ToolMaterial::Wood => ItemType::Block(2), // Placeholder: Planks
            ToolMaterial::Stone => ItemType::Block(3), // Placeholder: Cobblestone
            ToolMaterial::Iron => ItemType::Item(10), // Placeholder: Iron Ingot
            ToolMaterial::Diamond => ItemType::Item(11), // Placeholder: Diamond
            ToolMaterial::Gold => ItemType::Item(12), // Placeholder: Gold Ingot
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recipe_can_craft() {
        let recipe = Recipe::new(
            vec![(ItemType::Block(1), 3), (ItemType::Item(1), 2)],
            ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood),
            1,
        );

        // Has enough materials
        let available = vec![(ItemType::Block(1), 5), (ItemType::Item(1), 3)];
        assert!(recipe.can_craft(&available));

        // Has exact materials
        let available = vec![(ItemType::Block(1), 3), (ItemType::Item(1), 2)];
        assert!(recipe.can_craft(&available));

        // Missing one material type
        let available = vec![(ItemType::Block(1), 5)];
        assert!(!recipe.can_craft(&available));

        // Not enough of one material
        let available = vec![(ItemType::Block(1), 2), (ItemType::Item(1), 3)];
        assert!(!recipe.can_craft(&available));
    }

    #[test]
    fn test_tool_recipes() {
        // Test pickaxe recipe (3 material + 2 sticks)
        let recipe = ToolRecipes::get_recipe(ToolType::Pickaxe, ToolMaterial::Iron);
        assert_eq!(
            recipe.output,
            ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron)
        );
        assert_eq!(recipe.output_count, 1);
        assert_eq!(recipe.inputs.len(), 2);
        assert_eq!(recipe.inputs[0].1, 3); // 3 iron ingots
        assert_eq!(recipe.inputs[1].1, 2); // 2 sticks

        // Test sword recipe (2 material + 1 stick)
        let recipe = ToolRecipes::get_recipe(ToolType::Sword, ToolMaterial::Diamond);
        assert_eq!(recipe.inputs[0].1, 2); // 2 diamonds
        assert_eq!(recipe.inputs[1].1, 1); // 1 stick

        // Test shovel recipe (1 material + 2 sticks)
        let recipe = ToolRecipes::get_recipe(ToolType::Shovel, ToolMaterial::Stone);
        assert_eq!(recipe.inputs[0].1, 1); // 1 cobblestone
        assert_eq!(recipe.inputs[1].1, 2); // 2 sticks
    }

    #[test]
    fn test_all_tool_material_combinations() {
        let tool_types = [
            ToolType::Pickaxe,
            ToolType::Axe,
            ToolType::Shovel,
            ToolType::Sword,
            ToolType::Hoe,
        ];
        let materials = [
            ToolMaterial::Wood,
            ToolMaterial::Stone,
            ToolMaterial::Iron,
            ToolMaterial::Diamond,
            ToolMaterial::Gold,
        ];

        // Every combination should have a valid recipe
        for tool_type in &tool_types {
            for material in &materials {
                let recipe = ToolRecipes::get_recipe(*tool_type, *material);
                assert_eq!(recipe.output, ItemType::Tool(*tool_type, *material));
                assert_eq!(recipe.output_count, 1);
                assert_eq!(recipe.inputs.len(), 2); // Material + sticks
            }
        }
    }
}
