//! Crafting system with JSON-based recipe loading.
//!
//! Provides data-driven crafting with recipe validation and
//! crafting station management.

use crate::inventory::{Inventory, ItemId, ItemStack};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Crafting recipe input requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeInput {
    /// Item required for crafting.
    pub item_id: ItemId,
    /// Amount of this item required.
    pub count: u8,
}

/// Crafting recipe definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    /// Unique recipe identifier (e.g., "wooden_planks").
    pub id: String,
    /// Items required to craft this recipe.
    pub inputs: Vec<RecipeInput>,
    /// Item produced by this recipe.
    pub output_item: ItemId,
    /// Amount of output item produced.
    pub output_count: u8,
}

impl Recipe {
    /// Check if the given inventory contains all required inputs.
    pub fn can_craft(&self, inventory: &Inventory) -> bool {
        self.inputs
            .iter()
            .all(|input| inventory.has_item(input.item_id, input.count))
    }

    /// Try to craft this recipe using items from the inventory.
    /// Returns the output item stack if successful, None if inputs are missing.
    ///
    /// This function is atomic - if crafting fails partway through,
    /// all removed items are restored to the inventory.
    pub fn craft(&self, inventory: &mut Inventory) -> Option<ItemStack> {
        // Check if we have all inputs.
        if !self.can_craft(inventory) {
            return None;
        }

        // Track removed items for potential rollback
        let mut removed_items: Vec<(ItemId, u8)> = Vec::with_capacity(self.inputs.len());

        // Remove inputs from inventory.
        for input in &self.inputs {
            let removed = inventory.remove_item(input.item_id, input.count);
            if removed < input.count {
                // This shouldn't happen if can_craft returned true, but handle gracefully.
                // Rollback all previously removed items
                for (item_id, count) in removed_items {
                    inventory.add_item(ItemStack::new(item_id, count));
                }
                // Also return the partially removed items from this iteration
                if removed > 0 {
                    inventory.add_item(ItemStack::new(input.item_id, removed));
                }
                return None;
            }
            removed_items.push((input.item_id, removed));
        }

        // All inputs successfully removed - return output
        Some(ItemStack::new(self.output_item, self.output_count))
    }
}

/// Recipe registry managing all loaded recipes.
#[derive(Debug, Clone, Default)]
pub struct RecipeRegistry {
    recipes: HashMap<String, Recipe>,
}

impl RecipeRegistry {
    /// Create a new empty recipe registry.
    pub fn new() -> Self {
        Self {
            recipes: HashMap::new(),
        }
    }

    /// Load recipes from a JSON file.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path).context("Failed to read recipe file")?;
        Self::load_from_str(&content)
    }

    /// Load recipes from a JSON string.
    pub fn load_from_str(content: &str) -> Result<Self> {
        let recipes: Vec<Recipe> =
            serde_json::from_str(content).context("Failed to parse recipe JSON")?;

        let mut registry = Self::new();
        for recipe in recipes {
            registry.add_recipe(recipe);
        }

        Ok(registry)
    }

    /// Add a recipe to the registry.
    pub fn add_recipe(&mut self, recipe: Recipe) {
        self.recipes.insert(recipe.id.clone(), recipe);
    }

    /// Get a recipe by ID.
    pub fn get_recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipes.get(id)
    }

    /// Get all recipes.
    pub fn all_recipes(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }

    /// Find all recipes that can be crafted with the given inventory.
    pub fn craftable_recipes<'a>(
        &'a self,
        inventory: &'a Inventory,
    ) -> impl Iterator<Item = &'a Recipe> + 'a {
        self.recipes
            .values()
            .filter(move |recipe| recipe.can_craft(inventory))
    }

    /// Count total number of recipes.
    pub fn recipe_count(&self) -> usize {
        self.recipes.len()
    }
}

/// 3x3 crafting grid for crafting table.
#[derive(Debug, Clone)]
pub struct CraftingGrid {
    slots: [Option<ItemStack>; 9],
}

impl CraftingGrid {
    /// Create a new empty crafting grid.
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }

    /// Get an item from a grid slot (0-8).
    pub fn get(&self, slot: usize) -> Option<&ItemStack> {
        if slot >= 9 {
            return None;
        }
        self.slots[slot].as_ref()
    }

    /// Set an item in a grid slot.
    pub fn set(&mut self, slot: usize, stack: Option<ItemStack>) -> bool {
        if slot >= 9 {
            return false;
        }
        self.slots[slot] = stack;
        true
    }

    /// Clear the crafting grid.
    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            *slot = None;
        }
    }

    /// Check if the grid is empty.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| slot.is_none())
    }
}

impl Default for CraftingGrid {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_can_craft_check() {
        let recipe = Recipe {
            id: "test".into(),
            inputs: vec![
                RecipeInput {
                    item_id: 1,
                    count: 2,
                },
                RecipeInput {
                    item_id: 2,
                    count: 1,
                },
            ],
            output_item: 3,
            output_count: 4,
        };

        let mut inv = Inventory::new();
        assert!(!recipe.can_craft(&inv));

        inv.add_item(ItemStack::new(1, 2));
        assert!(!recipe.can_craft(&inv));

        inv.add_item(ItemStack::new(2, 1));
        assert!(recipe.can_craft(&inv));
    }

    #[test]
    fn recipe_craft_consumes_inputs() {
        let recipe = Recipe {
            id: "test".into(),
            inputs: vec![RecipeInput {
                item_id: 1,
                count: 3,
            }],
            output_item: 2,
            output_count: 1,
        };

        let mut inv = Inventory::new();
        inv.add_item(ItemStack::new(1, 5));

        let output = recipe.craft(&mut inv).unwrap();
        assert_eq!(output.item_id, 2);
        assert_eq!(output.count, 1);

        // Should have 2 of item 1 remaining.
        assert_eq!(inv.count_item(1), 2);
    }

    #[test]
    fn recipe_craft_fails_without_inputs() {
        let recipe = Recipe {
            id: "test".into(),
            inputs: vec![RecipeInput {
                item_id: 1,
                count: 10,
            }],
            output_item: 2,
            output_count: 1,
        };

        let mut inv = Inventory::new();
        inv.add_item(ItemStack::new(1, 5));

        assert!(recipe.craft(&mut inv).is_none());
        assert_eq!(inv.count_item(1), 5); // Items not consumed
    }

    #[test]
    fn recipe_registry_load_from_str() {
        let json = r#"[
            {
                "id": "wooden_planks",
                "inputs": [{"item_id": 1, "count": 1}],
                "output_item": 2,
                "output_count": 4
            },
            {
                "id": "sticks",
                "inputs": [{"item_id": 2, "count": 2}],
                "output_item": 3,
                "output_count": 4
            }
        ]"#;

        let registry = RecipeRegistry::load_from_str(json).unwrap();
        assert_eq!(registry.recipe_count(), 2);

        let planks = registry.get_recipe("wooden_planks").unwrap();
        assert_eq!(planks.output_item, 2);
        assert_eq!(planks.output_count, 4);
    }

    #[test]
    fn recipe_registry_craftable_filter() {
        let json = r#"[
            {
                "id": "planks",
                "inputs": [{"item_id": 1, "count": 1}],
                "output_item": 2,
                "output_count": 4
            },
            {
                "id": "sticks",
                "inputs": [{"item_id": 2, "count": 2}],
                "output_item": 3,
                "output_count": 4
            }
        ]"#;

        let registry = RecipeRegistry::load_from_str(json).unwrap();
        let mut inv = Inventory::new();
        inv.add_item(ItemStack::new(1, 10));

        let craftable: Vec<_> = registry.craftable_recipes(&inv).collect();
        assert_eq!(craftable.len(), 1);
        assert_eq!(craftable[0].id, "planks");
    }

    #[test]
    fn crafting_grid_operations() {
        let mut grid = CraftingGrid::new();
        assert!(grid.is_empty());

        grid.set(0, Some(ItemStack::new(1, 1)));
        grid.set(4, Some(ItemStack::new(2, 1)));

        assert!(!grid.is_empty());
        assert_eq!(grid.get(0).unwrap().item_id, 1);
        assert_eq!(grid.get(4).unwrap().item_id, 2);
        assert!(grid.get(1).is_none());

        grid.clear();
        assert!(grid.is_empty());
    }

    #[test]
    fn craft_rollback_on_partial_failure() {
        // Create a recipe requiring multiple different items
        let recipe = Recipe {
            id: "complex".into(),
            inputs: vec![
                RecipeInput {
                    item_id: 1,
                    count: 2,
                },
                RecipeInput {
                    item_id: 2,
                    count: 3,
                },
                RecipeInput {
                    item_id: 3,
                    count: 5,
                },
            ],
            output_item: 10,
            output_count: 1,
        };

        // Create inventory with enough of first two items but not third
        let mut inv = Inventory::new();
        inv.add_item(ItemStack::new(1, 5)); // 5 of item 1
        inv.add_item(ItemStack::new(2, 4)); // 4 of item 2
        inv.add_item(ItemStack::new(3, 2)); // Only 2 of item 3 (need 5)

        // Record original counts
        let orig_1 = inv.count_item(1);
        let orig_2 = inv.count_item(2);
        let orig_3 = inv.count_item(3);

        // Crafting should fail because we don't have enough of item 3
        assert!(recipe.craft(&mut inv).is_none());

        // Verify all items were restored (rollback worked)
        assert_eq!(inv.count_item(1), orig_1, "Item 1 should be restored");
        assert_eq!(inv.count_item(2), orig_2, "Item 2 should be restored");
        assert_eq!(inv.count_item(3), orig_3, "Item 3 should be restored");
    }

    #[test]
    fn craft_success_consumes_all_inputs() {
        let recipe = Recipe {
            id: "multi".into(),
            inputs: vec![
                RecipeInput {
                    item_id: 1,
                    count: 2,
                },
                RecipeInput {
                    item_id: 2,
                    count: 1,
                },
            ],
            output_item: 10,
            output_count: 3,
        };

        let mut inv = Inventory::new();
        inv.add_item(ItemStack::new(1, 5));
        inv.add_item(ItemStack::new(2, 3));

        let output = recipe.craft(&mut inv).unwrap();
        assert_eq!(output.item_id, 10);
        assert_eq!(output.count, 3);

        // Verify inputs were consumed
        assert_eq!(inv.count_item(1), 3); // 5 - 2 = 3
        assert_eq!(inv.count_item(2), 2); // 3 - 1 = 2
    }
}
