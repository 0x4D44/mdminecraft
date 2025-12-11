//! Recipe registry for managing crafting recipes loaded from JSON config.

use mdminecraft_core::{ItemType, Recipe, ToolMaterial, ToolType};
use std::collections::HashMap;

/// Registry of crafting recipes indexed by output item name.
#[derive(Debug, Clone)]
pub struct RecipeRegistry {
    /// Map from recipe name/ID to Recipe
    recipes: HashMap<String, Recipe>,
}

impl RecipeRegistry {
    /// Create a new recipe registry from a list of named recipes.
    pub fn new(recipes: Vec<(String, Recipe)>) -> Self {
        Self {
            recipes: recipes.into_iter().collect(),
        }
    }

    /// Get a recipe by its name/ID.
    pub fn get(&self, name: &str) -> Option<&Recipe> {
        self.recipes.get(name)
    }

    /// Get all registered recipe names.
    pub fn recipe_names(&self) -> impl Iterator<Item = &String> {
        self.recipes.keys()
    }

    /// Get the number of registered recipes.
    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    /// Find all recipes that can be crafted with the given available items.
    pub fn craftable_recipes(
        &self,
        available: &[(mdminecraft_core::ItemType, u32)],
    ) -> Vec<(&String, &Recipe)> {
        self.recipes
            .iter()
            .filter(|(_, recipe)| recipe.can_craft(available))
            .collect()
    }
}

/// Parse an item string into an ItemType.
///
/// Format:
/// - "block:id" -> ItemType::Block(id)
/// - "item:id" -> ItemType::Item(id)
/// - "tool:type:material" -> ItemType::Tool(ToolType, ToolMaterial)
pub fn parse_item_type(s: &str) -> Option<ItemType> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.as_slice() {
        ["block", id_str] => id_str.parse::<u16>().ok().map(ItemType::Block),
        ["item", id_str] => id_str.parse::<u16>().ok().map(ItemType::Item),
        ["tool", tool_str, material_str] => {
            let tool = parse_tool_type(tool_str)?;
            let material = parse_tool_material(material_str)?;
            Some(ItemType::Tool(tool, material))
        }
        _ => None,
    }
}

fn parse_tool_type(s: &str) -> Option<ToolType> {
    match s {
        "pickaxe" => Some(ToolType::Pickaxe),
        "axe" => Some(ToolType::Axe),
        "shovel" => Some(ToolType::Shovel),
        "sword" => Some(ToolType::Sword),
        "hoe" => Some(ToolType::Hoe),
        _ => None,
    }
}

fn parse_tool_material(s: &str) -> Option<ToolMaterial> {
    match s {
        "wood" => Some(ToolMaterial::Wood),
        "stone" => Some(ToolMaterial::Stone),
        "iron" => Some(ToolMaterial::Iron),
        "diamond" => Some(ToolMaterial::Diamond),
        "gold" => Some(ToolMaterial::Gold),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_core::{ItemType, ToolMaterial, ToolType};

    #[test]
    fn test_recipe_registry_creation() {
        let recipe = Recipe::new(
            vec![(ItemType::Block(1), 3), (ItemType::Item(1), 2)],
            ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood),
            1,
        );

        let registry = RecipeRegistry::new(vec![("wooden_pickaxe".to_string(), recipe)]);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.get("wooden_pickaxe").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_recipe_lookup() {
        let recipe1 = Recipe::new(
            vec![(ItemType::Block(2), 3), (ItemType::Item(1), 2)],
            ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood),
            1,
        );
        let recipe2 = Recipe::new(
            vec![(ItemType::Block(3), 2), (ItemType::Item(1), 1)],
            ItemType::Tool(ToolType::Sword, ToolMaterial::Stone),
            1,
        );

        let registry = RecipeRegistry::new(vec![
            ("wooden_pickaxe".to_string(), recipe1),
            ("stone_sword".to_string(), recipe2),
        ]);

        let found = registry.get("wooden_pickaxe").unwrap();
        assert_eq!(found.inputs.len(), 2);
        assert_eq!(
            found.output,
            ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood)
        );
    }

    #[test]
    fn test_craftable_recipes() {
        let recipe1 = Recipe::new(vec![(ItemType::Block(1), 2)], ItemType::Block(2), 4);
        let recipe2 = Recipe::new(vec![(ItemType::Block(1), 5)], ItemType::Block(3), 1);

        let registry = RecipeRegistry::new(vec![
            ("planks".to_string(), recipe1),
            ("crafting_table".to_string(), recipe2),
        ]);

        // Have 3 of block 1 - can only craft planks (needs 2), not crafting table (needs 5)
        let available = vec![(ItemType::Block(1), 3)];
        let craftable = registry.craftable_recipes(&available);

        assert_eq!(craftable.len(), 1);
        assert_eq!(craftable[0].0, "planks");
    }

    #[test]
    fn test_parse_item_type() {
        // Test block parsing
        assert_eq!(parse_item_type("block:1"), Some(ItemType::Block(1)));
        assert_eq!(parse_item_type("block:100"), Some(ItemType::Block(100)));

        // Test item parsing
        assert_eq!(parse_item_type("item:5"), Some(ItemType::Item(5)));
        assert_eq!(parse_item_type("item:42"), Some(ItemType::Item(42)));

        // Test tool parsing
        assert_eq!(
            parse_item_type("tool:pickaxe:wood"),
            Some(ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood))
        );
        assert_eq!(
            parse_item_type("tool:sword:diamond"),
            Some(ItemType::Tool(ToolType::Sword, ToolMaterial::Diamond))
        );

        // Test invalid formats
        assert_eq!(parse_item_type("invalid"), None);
        assert_eq!(parse_item_type("block:abc"), None);
        assert_eq!(parse_item_type("tool:invalid:wood"), None);
    }
}
