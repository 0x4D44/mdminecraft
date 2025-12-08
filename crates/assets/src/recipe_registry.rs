//! Recipe registry for managing crafting recipes loaded from JSON config.

use mdminecraft_core::Recipe;
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
    pub fn craftable_recipes(&self, available: &[(mdminecraft_core::ItemType, u32)]) -> Vec<(&String, &Recipe)> {
        self.recipes
            .iter()
            .filter(|(_, recipe)| recipe.can_craft(available))
            .collect()
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
        assert_eq!(found.output, ItemType::Tool(ToolType::Pickaxe, ToolMaterial::Wood));
    }

    #[test]
    fn test_craftable_recipes() {
        let recipe1 = Recipe::new(
            vec![(ItemType::Block(1), 2)],
            ItemType::Block(2),
            4,
        );
        let recipe2 = Recipe::new(
            vec![(ItemType::Block(1), 5)],
            ItemType::Block(3),
            1,
        );

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
}
