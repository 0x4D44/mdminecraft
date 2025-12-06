//! Furnace and smelting system.
//!
//! Provides furnace block functionality with smelting recipes,
//! fuel management, and progress tracking.

use crate::drop_item::ItemType;
use serde::{Deserialize, Serialize};

/// Smelting time per item in seconds.
pub const SMELT_TIME_SECONDS: f32 = 10.0;

/// A smelting recipe: input item -> output item.
#[derive(Debug, Clone, Copy)]
pub struct SmeltRecipe {
    pub input: ItemType,
    pub output: ItemType,
}

/// All available smelting recipes.
pub const SMELT_RECIPES: &[SmeltRecipe] = &[
    SmeltRecipe {
        input: ItemType::IronOre,
        output: ItemType::IronIngot,
    },
    SmeltRecipe {
        input: ItemType::GoldOre,
        output: ItemType::GoldIngot,
    },
    SmeltRecipe {
        input: ItemType::RawPork,
        output: ItemType::CookedPork,
    },
    SmeltRecipe {
        input: ItemType::RawBeef,
        output: ItemType::CookedBeef,
    },
];

/// Get the smelting output for an input item.
pub fn get_smelt_output(input: ItemType) -> Option<ItemType> {
    SMELT_RECIPES
        .iter()
        .find(|r| r.input == input)
        .map(|r| r.output)
}

/// Fuel burn times (in items that can be smelted).
#[derive(Debug, Clone, Copy)]
pub struct FuelValue {
    pub item: ItemType,
    /// Number of items this fuel can smelt (can be fractional).
    pub burn_value: f32,
}

/// All valid fuel items and their burn values.
pub const FUEL_VALUES: &[FuelValue] = &[
    FuelValue {
        item: ItemType::Coal,
        burn_value: 8.0,
    },
    FuelValue {
        item: ItemType::OakLog,
        burn_value: 1.5,
    },
    FuelValue {
        item: ItemType::OakPlanks,
        burn_value: 0.5,
    },
    FuelValue {
        item: ItemType::Stick,
        burn_value: 0.25,
    },
];

/// Get the burn value for a fuel item (0.0 if not valid fuel).
pub fn get_fuel_value(item: ItemType) -> f32 {
    FUEL_VALUES
        .iter()
        .find(|f| f.item == item)
        .map(|f| f.burn_value)
        .unwrap_or(0.0)
}

/// Check if an item is valid fuel.
pub fn is_fuel(item: ItemType) -> bool {
    get_fuel_value(item) > 0.0
}

/// State of a furnace in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FurnaceState {
    /// Item in the input slot (type and count).
    pub input: Option<(ItemType, u32)>,
    /// Item in the fuel slot (type and count).
    pub fuel: Option<(ItemType, u32)>,
    /// Item in the output slot (type and count).
    pub output: Option<(ItemType, u32)>,
    /// Current smelting progress (0.0 to 1.0).
    pub smelt_progress: f32,
    /// Remaining fuel burn time (in items, decrements as smelting progresses).
    pub fuel_remaining: f32,
    /// Whether the furnace is currently active (lit).
    pub is_lit: bool,
}

impl Default for FurnaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl FurnaceState {
    /// Create a new empty furnace.
    pub fn new() -> Self {
        Self {
            input: None,
            fuel: None,
            output: None,
            smelt_progress: 0.0,
            fuel_remaining: 0.0,
            is_lit: false,
        }
    }

    /// Update the furnace state (call once per tick/frame).
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds.
    ///
    /// # Returns
    /// `true` if the furnace state changed (lit/unlit transition).
    pub fn update(&mut self, dt: f32) -> bool {
        let was_lit = self.is_lit;

        // Check if we can smelt
        let can_smelt = self.can_smelt();

        if can_smelt {
            // Try to consume fuel if needed
            if self.fuel_remaining <= 0.0 {
                if let Some((fuel_type, fuel_count)) = &mut self.fuel {
                    let burn_value = get_fuel_value(*fuel_type);
                    if burn_value > 0.0 && *fuel_count > 0 {
                        self.fuel_remaining = burn_value;
                        *fuel_count -= 1;
                        if *fuel_count == 0 {
                            self.fuel = None;
                        }
                    }
                }
            }

            // If we have fuel, progress smelting
            if self.fuel_remaining > 0.0 {
                self.is_lit = true;

                // Progress smelting
                let progress_per_second = 1.0 / SMELT_TIME_SECONDS;
                self.smelt_progress += progress_per_second * dt;

                // Consume fuel proportionally
                let fuel_per_second = 1.0 / SMELT_TIME_SECONDS;
                self.fuel_remaining -= fuel_per_second * dt;
                self.fuel_remaining = self.fuel_remaining.max(0.0);

                // Check if smelting is complete
                if self.smelt_progress >= 1.0 {
                    self.complete_smelt();
                    self.smelt_progress = 0.0;
                }
            } else {
                self.is_lit = false;
            }
        } else {
            // Can't smelt, reset progress
            self.smelt_progress = 0.0;
            self.is_lit = self.fuel_remaining > 0.0;
        }

        // Return true if lit state changed
        was_lit != self.is_lit
    }

    /// Check if the furnace can smelt (has valid input and room for output).
    fn can_smelt(&self) -> bool {
        if let Some((input_type, _)) = &self.input {
            if let Some(output_type) = get_smelt_output(*input_type) {
                // Check if output slot is empty or has the same item with room
                match &self.output {
                    None => true,
                    Some((out_type, out_count)) => {
                        *out_type == output_type && *out_count < output_type.max_stack_size()
                    }
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Complete a smelting operation.
    fn complete_smelt(&mut self) {
        if let Some((input_type, input_count)) = &mut self.input {
            if let Some(output_type) = get_smelt_output(*input_type) {
                // Consume input
                *input_count -= 1;
                if *input_count == 0 {
                    self.input = None;
                }

                // Add to output
                match &mut self.output {
                    None => {
                        self.output = Some((output_type, 1));
                    }
                    Some((_, out_count)) => {
                        *out_count += 1;
                    }
                }
            }
        }
    }

    /// Add an item to the input slot.
    ///
    /// # Returns
    /// Number of items that couldn't be added (0 if all added).
    pub fn add_input(&mut self, item_type: ItemType, count: u32) -> u32 {
        // Check if this item is smeltable
        if get_smelt_output(item_type).is_none() {
            return count; // Can't smelt this item
        }

        match &mut self.input {
            None => {
                let max = item_type.max_stack_size();
                let add = count.min(max);
                self.input = Some((item_type, add));
                count - add
            }
            Some((existing_type, existing_count)) => {
                if *existing_type == item_type {
                    let max = item_type.max_stack_size();
                    let space = max.saturating_sub(*existing_count);
                    let add = count.min(space);
                    *existing_count += add;
                    count - add
                } else {
                    count // Slot occupied with different item
                }
            }
        }
    }

    /// Add an item to the fuel slot.
    ///
    /// # Returns
    /// Number of items that couldn't be added (0 if all added).
    pub fn add_fuel(&mut self, item_type: ItemType, count: u32) -> u32 {
        // Check if this item is valid fuel
        if !is_fuel(item_type) {
            return count; // Not valid fuel
        }

        match &mut self.fuel {
            None => {
                let max = item_type.max_stack_size();
                let add = count.min(max);
                self.fuel = Some((item_type, add));
                count - add
            }
            Some((existing_type, existing_count)) => {
                if *existing_type == item_type {
                    let max = item_type.max_stack_size();
                    let space = max.saturating_sub(*existing_count);
                    let add = count.min(space);
                    *existing_count += add;
                    count - add
                } else {
                    count // Slot occupied with different item
                }
            }
        }
    }

    /// Take all items from the output slot.
    ///
    /// # Returns
    /// The items taken, or None if slot was empty.
    pub fn take_output(&mut self) -> Option<(ItemType, u32)> {
        self.output.take()
    }

    /// Take all items from the input slot.
    pub fn take_input(&mut self) -> Option<(ItemType, u32)> {
        self.input.take()
    }

    /// Take all items from the fuel slot.
    pub fn take_fuel(&mut self) -> Option<(ItemType, u32)> {
        self.fuel.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smelt_recipes() {
        assert_eq!(
            get_smelt_output(ItemType::IronOre),
            Some(ItemType::IronIngot)
        );
        assert_eq!(
            get_smelt_output(ItemType::GoldOre),
            Some(ItemType::GoldIngot)
        );
        assert_eq!(
            get_smelt_output(ItemType::RawPork),
            Some(ItemType::CookedPork)
        );
        assert_eq!(
            get_smelt_output(ItemType::RawBeef),
            Some(ItemType::CookedBeef)
        );
        assert_eq!(get_smelt_output(ItemType::Stone), None);
    }

    #[test]
    fn test_fuel_values() {
        assert_eq!(get_fuel_value(ItemType::Coal), 8.0);
        assert_eq!(get_fuel_value(ItemType::OakLog), 1.5);
        assert_eq!(get_fuel_value(ItemType::OakPlanks), 0.5);
        assert_eq!(get_fuel_value(ItemType::Stone), 0.0);

        assert!(is_fuel(ItemType::Coal));
        assert!(!is_fuel(ItemType::Stone));
    }

    #[test]
    fn test_furnace_state() {
        let mut furnace = FurnaceState::new();

        // Add input and fuel
        assert_eq!(furnace.add_input(ItemType::IronOre, 1), 0);
        assert_eq!(furnace.add_fuel(ItemType::Coal, 1), 0);

        // Should have items
        assert!(furnace.input.is_some());
        assert!(furnace.fuel.is_some());

        // Simulate smelting for 11 seconds (extra time to handle float precision)
        for _ in 0..220 {
            furnace.update(0.05);
        }

        // Should have output
        assert!(furnace.output.is_some());
        let (output_type, output_count) = furnace.output.unwrap();
        assert_eq!(output_type, ItemType::IronIngot);
        assert_eq!(output_count, 1);
    }

    #[test]
    fn test_invalid_input() {
        let mut furnace = FurnaceState::new();

        // Stone can't be smelted
        assert_eq!(furnace.add_input(ItemType::Stone, 1), 1);
        assert!(furnace.input.is_none());
    }

    #[test]
    fn test_invalid_fuel() {
        let mut furnace = FurnaceState::new();

        // Stone can't be fuel
        assert_eq!(furnace.add_fuel(ItemType::Stone, 1), 1);
        assert!(furnace.fuel.is_none());
    }
}
