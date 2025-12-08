use std::fs;
use std::path::Path;

use crate::{AssetError, BlockDescriptor, BlockRegistry, RecipeRegistry};
use mdminecraft_core::Recipe;

/// Load a block registry from the provided JSON file path.
pub fn registry_from_file(path: &Path) -> Result<BlockRegistry, AssetError> {
    let data = fs::read_to_string(path)?;
    registry_from_str(&data)
}

/// Load a block registry from an in-memory JSON string.
pub fn registry_from_str(input: &str) -> Result<BlockRegistry, AssetError> {
    let defs = crate::load_blocks_from_str(input)?;
    Ok(BlockRegistry::new(
        defs.into_iter()
            .map(BlockDescriptor::from_definition)
            .collect(),
    ))
}

/// Load a recipe registry from the provided JSON file path.
pub fn recipe_registry_from_file(path: &Path) -> Result<RecipeRegistry, AssetError> {
    let data = fs::read_to_string(path)?;
    recipe_registry_from_str(&data)
}

/// Load a recipe registry from an in-memory JSON string.
pub fn recipe_registry_from_str(input: &str) -> Result<RecipeRegistry, AssetError> {
    let defs = crate::load_recipes_from_str(input)?;
    let recipes: Vec<(String, Recipe)> = defs
        .into_iter()
        .filter_map(|def| {
            let inputs: Vec<_> = def
                .inputs
                .iter()
                .filter_map(|inp| {
                    crate::recipe_registry::parse_item_type(&inp.item)
                        .map(|item_type| (item_type, inp.count))
                })
                .collect();

            let output = crate::recipe_registry::parse_item_type(&def.output.item)?;

            if inputs.len() == def.inputs.len() {
                Some((def.name, Recipe::new(inputs, output, def.output.count)))
            } else {
                None
            }
        })
        .collect();

    Ok(RecipeRegistry::new(recipes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recipe_registry_from_str() {
        let json = r#"[
            {
                "name": "wooden_pickaxe",
                "inputs": [
                    {"item": "block:2", "count": 3},
                    {"item": "item:1", "count": 2}
                ],
                "output": {"item": "tool:pickaxe:wood", "count": 1}
            },
            {
                "name": "stone_sword",
                "inputs": [
                    {"item": "block:3", "count": 2},
                    {"item": "item:1", "count": 1}
                ],
                "output": {"item": "tool:sword:stone"}
            }
        ]"#;

        let registry = recipe_registry_from_str(json).unwrap();
        assert_eq!(registry.len(), 2);

        let wooden_pickaxe = registry.get("wooden_pickaxe").unwrap();
        assert_eq!(wooden_pickaxe.inputs.len(), 2);
        assert_eq!(wooden_pickaxe.output_count, 1);

        let stone_sword = registry.get("stone_sword").unwrap();
        assert_eq!(stone_sword.inputs.len(), 2);
        assert_eq!(stone_sword.output_count, 1); // Default value
    }

    #[test]
    fn test_load_recipes_from_config_file() {
        // Test loading the actual config/recipes.json file
        let config_path = std::path::Path::new("../../config/recipes.json");

        if config_path.exists() {
            let registry = recipe_registry_from_file(config_path).unwrap();

            // Should have all 25 tool recipes (5 tools × 5 materials)
            assert_eq!(registry.len(), 25, "Recipe registry should contain 25 recipes");

            // Test a sample from each material tier
            let test_recipes = vec![
                ("wooden_pickaxe", 2),
                ("stone_sword", 2),
                ("iron_axe", 2),
                ("diamond_shovel", 2),
                ("golden_hoe", 2),
            ];

            for (recipe_name, expected_inputs) in test_recipes {
                let recipe = registry.get(recipe_name);
                assert!(recipe.is_some(), "Should have {} recipe", recipe_name);

                if let Some(recipe) = recipe {
                    assert_eq!(
                        recipe.inputs.len(),
                        expected_inputs,
                        "{} should have {} input types",
                        recipe_name,
                        expected_inputs
                    );
                    assert_eq!(recipe.output_count, 1, "{} should produce 1 tool", recipe_name);
                }
            }

            // Verify we have all tool types for each material
            let materials = vec!["wooden", "stone", "iron", "diamond", "golden"];
            let tools = vec!["pickaxe", "axe", "shovel", "sword", "hoe"];

            for material in &materials {
                for tool in &tools {
                    let recipe_name = format!("{}_{}", material, tool);
                    assert!(
                        registry.get(&recipe_name).is_some(),
                        "Missing recipe: {}",
                        recipe_name
                    );
                }
            }
        }
    }
}
