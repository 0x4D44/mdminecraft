//! Dropped item system with physics and lifecycle management.
//!
//! Items can be dropped from breaking blocks or defeating mobs.
//! They have physics (gravity, collision), a pickup radius, and despawn after 5 minutes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum lifetime for dropped items (5 minutes = 6000 ticks at 20 TPS).
pub const ITEM_DESPAWN_TICKS: u32 = 6000;

/// Pickup radius in blocks.
pub const PICKUP_RADIUS: f64 = 1.5;

/// Types of items that can be dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    // Block items - terrain
    Stone,
    Cobblestone,
    Dirt,
    Grass,
    Sand,
    Gravel,
    Ice,
    Snow,
    Clay,
    Bedrock,

    // Block items - trees
    OakLog,
    OakLeaves,
    BirchLog,
    BirchLeaves,
    PineLog,
    PineLeaves,

    // Block items - ores
    CoalOre,
    IronOre,
    GoldOre,
    DiamondOre,
    LapisOre,

    // Mob drops
    RawPork,
    RawBeef,
    Leather,
    Wool,
    Feather,
    Egg,
    Bone,
    RottenFlesh,
    String,
    Gunpowder,

    // Smelted/processed items
    IronIngot,
    GoldIngot,
    CookedPork,
    CookedBeef,
    Coal,

    // Crafted items (for future use)
    Stick,
    Planks,
    OakPlanks,
    BirchPlanks,
    PinePlanks,
    Furnace,

    // Special items
    Sapling,
    Apple,
    Flint,

    // Combat items
    Arrow,
    Bow,

    // Armor - Leather
    LeatherHelmet,
    LeatherChestplate,
    LeatherLeggings,
    LeatherBoots,

    // Armor - Iron
    IronHelmet,
    IronChestplate,
    IronLeggings,
    IronBoots,

    // Armor - Gold
    GoldHelmet,
    GoldChestplate,
    GoldLeggings,
    GoldBoots,

    // Armor - Diamond
    DiamondHelmet,
    DiamondChestplate,
    DiamondLeggings,
    DiamondBoots,

    // Resources
    Diamond,
    LapisLazuli,

    // Brewing items
    GlassBottle,
    WaterBottle,
    NetherWart,
    BlazePowder,

    // Potions
    PotionAwkward,
    PotionNightVision,
    PotionInvisibility,
    PotionLeaping,
    PotionFireResistance,
    PotionSwiftness,
    PotionSlowness,
    PotionWaterBreathing,
    PotionHealing,
    PotionHarming,
    PotionPoison,
    PotionRegeneration,
    PotionStrength,
    PotionWeakness,

    // Placeable utility blocks (appended to preserve stable IDs)
    CraftingTable,
    Torch,

    // Tools (appended to preserve stable IDs)
    WoodenPickaxe,
    StonePickaxe,
    IronPickaxe,
    DiamondPickaxe,
    GoldPickaxe,
    WoodenAxe,
    StoneAxe,
    IronAxe,
    DiamondAxe,
    GoldAxe,
    WoodenShovel,
    StoneShovel,
    IronShovel,
    DiamondShovel,
    GoldShovel,
    WoodenSword,
    StoneSword,
    IronSword,
    DiamondSword,
    GoldSword,
    WoodenHoe,
    StoneHoe,
    IronHoe,
    DiamondHoe,
    GoldHoe,

    // Interactive/redstone blocks (appended to preserve stable IDs)
    Ladder,
    Lever,
    StoneButton,
    OakButton,
    StonePressurePlate,
    OakPressurePlate,
    RedstoneWire,
    RedstoneTorch,

    // Additional placeable blocks (appended to preserve stable IDs)
    Glass,
    Obsidian,
    OakFence,
    OakFenceGate,
    StoneSlab,
    OakSlab,
    StoneStairs,
    OakStairs,
    GlassPane,
    Trapdoor,
    Chest,
    OakDoor,
    IronDoor,
    RedstoneLamp,
    EnchantingTable,
    BrewingStand,
    NetherWartBlock,
    SoulSand,

    // Beds (appended to preserve stable IDs)
    Bed,

    // Farming/food items (appended to preserve stable IDs)
    WheatSeeds,
    Wheat,
    Bread,
    Bookshelf,
    Carrot,
    Potato,
    BakedPotato,

    // Splash potions (appended to preserve stable IDs)
    SplashPotionAwkward,
    SplashPotionNightVision,
    SplashPotionInvisibility,
    SplashPotionLeaping,
    SplashPotionFireResistance,
    SplashPotionSwiftness,
    SplashPotionSlowness,
    SplashPotionWaterBreathing,
    SplashPotionHealing,
    SplashPotionHarming,
    SplashPotionPoison,
    SplashPotionRegeneration,
    SplashPotionStrength,
    SplashPotionWeakness,

    // Brewing ingredients (appended to preserve stable IDs)
    SpiderEye,
    GoldenCarrot,

    // Plants (appended to preserve stable IDs)
    SugarCane,

    // Materials (appended to preserve stable IDs)
    Sugar,
    Paper,
    Book,

    // Plants (appended to preserve stable IDs)
    BrownMushroom,

    // Brewing ingredients (appended to preserve stable IDs)
    FermentedSpiderEye,

    // Brewing ingredients (appended to preserve stable IDs)
    MagmaCream,

    // Brewing ingredients (appended to preserve stable IDs)
    GhastTear,

    // Brewing ingredients (appended to preserve stable IDs)
    GlisteringMelon,
    RabbitFoot,
    PhantomMembrane,

    // Potions (appended to preserve stable IDs)
    PotionSlowFalling,
    SplashPotionSlowFalling,

    // Brewing ingredients (appended to preserve stable IDs)
    RedstoneDust,
    GlowstoneDust,

    // Potion variants (appended to preserve stable IDs)
    PotionNightVisionLong,
    PotionInvisibilityLong,
    PotionLeapingLong,
    PotionLeapingStrong,
    PotionFireResistanceLong,
    PotionSwiftnessLong,
    PotionSwiftnessStrong,
    PotionSlownessLong,
    PotionWaterBreathingLong,
    PotionHealingStrong,
    PotionHarmingStrong,
    PotionPoisonLong,
    PotionPoisonStrong,
    PotionRegenerationLong,
    PotionRegenerationStrong,
    PotionStrengthLong,
    PotionStrengthStrong,
    PotionWeaknessLong,
    PotionSlowFallingLong,

    // Splash potion variants (appended to preserve stable IDs)
    SplashPotionNightVisionLong,
    SplashPotionInvisibilityLong,
    SplashPotionLeapingLong,
    SplashPotionLeapingStrong,
    SplashPotionFireResistanceLong,
    SplashPotionSwiftnessLong,
    SplashPotionSwiftnessStrong,
    SplashPotionSlownessLong,
    SplashPotionWaterBreathingLong,
    SplashPotionHealingStrong,
    SplashPotionHarmingStrong,
    SplashPotionPoisonLong,
    SplashPotionPoisonStrong,
    SplashPotionRegenerationLong,
    SplashPotionRegenerationStrong,
    SplashPotionStrengthLong,
    SplashPotionStrengthStrong,
    SplashPotionWeaknessLong,
    SplashPotionSlowFallingLong,

    // Brewing ingredients (appended to preserve stable IDs)
    Pufferfish,

    // Buckets (appended to preserve stable IDs)
    Bucket,
    WaterBucket,
    LavaBucket,

    // Walls (appended to preserve stable IDs)
    CobblestoneWall,

    // Bars (appended to preserve stable IDs)
    IronBars,

    // Stone bricks family (appended to preserve stable IDs)
    StoneBricks,
    StoneBrickSlab,
    StoneBrickStairs,
    StoneBrickWall,

    // Redstone components (appended to preserve stable IDs)
    RedstoneRepeater,
    RedstoneComparator,

    // Nether-ish items (appended to preserve stable IDs)
    NetherQuartz,

    // Redstone components (appended to preserve stable IDs)
    RedstoneObserver,

    // Pistons (appended to preserve stable IDs)
    Piston,

    // Automation blocks (appended to preserve stable IDs)
    Dispenser,
    Dropper,
    Hopper,

    // Trading currency (appended to preserve stable IDs)
    Emerald,
}

const ALL_ITEM_TYPES: &[ItemType] = &[
    ItemType::Stone,
    ItemType::Cobblestone,
    ItemType::Dirt,
    ItemType::Grass,
    ItemType::Sand,
    ItemType::Gravel,
    ItemType::Ice,
    ItemType::Snow,
    ItemType::Clay,
    ItemType::Bedrock,
    ItemType::OakLog,
    ItemType::OakLeaves,
    ItemType::BirchLog,
    ItemType::BirchLeaves,
    ItemType::PineLog,
    ItemType::PineLeaves,
    ItemType::CoalOre,
    ItemType::IronOre,
    ItemType::GoldOre,
    ItemType::DiamondOre,
    ItemType::LapisOre,
    ItemType::RawPork,
    ItemType::RawBeef,
    ItemType::Leather,
    ItemType::Wool,
    ItemType::Feather,
    ItemType::Egg,
    ItemType::Bone,
    ItemType::RottenFlesh,
    ItemType::String,
    ItemType::Gunpowder,
    ItemType::IronIngot,
    ItemType::GoldIngot,
    ItemType::CookedPork,
    ItemType::CookedBeef,
    ItemType::Coal,
    ItemType::Stick,
    ItemType::Planks,
    ItemType::OakPlanks,
    ItemType::BirchPlanks,
    ItemType::PinePlanks,
    ItemType::Furnace,
    ItemType::Sapling,
    ItemType::Apple,
    ItemType::Flint,
    ItemType::Arrow,
    ItemType::Bow,
    ItemType::LeatherHelmet,
    ItemType::LeatherChestplate,
    ItemType::LeatherLeggings,
    ItemType::LeatherBoots,
    ItemType::IronHelmet,
    ItemType::IronChestplate,
    ItemType::IronLeggings,
    ItemType::IronBoots,
    ItemType::GoldHelmet,
    ItemType::GoldChestplate,
    ItemType::GoldLeggings,
    ItemType::GoldBoots,
    ItemType::DiamondHelmet,
    ItemType::DiamondChestplate,
    ItemType::DiamondLeggings,
    ItemType::DiamondBoots,
    ItemType::Diamond,
    ItemType::LapisLazuli,
    ItemType::GlassBottle,
    ItemType::WaterBottle,
    ItemType::NetherWart,
    ItemType::BlazePowder,
    ItemType::PotionAwkward,
    ItemType::PotionNightVision,
    ItemType::PotionInvisibility,
    ItemType::PotionLeaping,
    ItemType::PotionFireResistance,
    ItemType::PotionSwiftness,
    ItemType::PotionSlowness,
    ItemType::PotionWaterBreathing,
    ItemType::PotionHealing,
    ItemType::PotionHarming,
    ItemType::PotionPoison,
    ItemType::PotionRegeneration,
    ItemType::PotionStrength,
    ItemType::PotionWeakness,
    ItemType::CraftingTable,
    ItemType::Torch,
    ItemType::WoodenPickaxe,
    ItemType::StonePickaxe,
    ItemType::IronPickaxe,
    ItemType::DiamondPickaxe,
    ItemType::GoldPickaxe,
    ItemType::WoodenAxe,
    ItemType::StoneAxe,
    ItemType::IronAxe,
    ItemType::DiamondAxe,
    ItemType::GoldAxe,
    ItemType::WoodenShovel,
    ItemType::StoneShovel,
    ItemType::IronShovel,
    ItemType::DiamondShovel,
    ItemType::GoldShovel,
    ItemType::WoodenSword,
    ItemType::StoneSword,
    ItemType::IronSword,
    ItemType::DiamondSword,
    ItemType::GoldSword,
    ItemType::WoodenHoe,
    ItemType::StoneHoe,
    ItemType::IronHoe,
    ItemType::DiamondHoe,
    ItemType::GoldHoe,
    ItemType::Ladder,
    ItemType::Lever,
    ItemType::StoneButton,
    ItemType::OakButton,
    ItemType::StonePressurePlate,
    ItemType::OakPressurePlate,
    ItemType::RedstoneWire,
    ItemType::RedstoneTorch,
    ItemType::Glass,
    ItemType::Obsidian,
    ItemType::OakFence,
    ItemType::OakFenceGate,
    ItemType::StoneSlab,
    ItemType::OakSlab,
    ItemType::StoneStairs,
    ItemType::OakStairs,
    ItemType::GlassPane,
    ItemType::Trapdoor,
    ItemType::Chest,
    ItemType::OakDoor,
    ItemType::IronDoor,
    ItemType::RedstoneLamp,
    ItemType::EnchantingTable,
    ItemType::BrewingStand,
    ItemType::NetherWartBlock,
    ItemType::SoulSand,
    ItemType::Bed,
    ItemType::WheatSeeds,
    ItemType::Wheat,
    ItemType::Bread,
    ItemType::Bookshelf,
    ItemType::Carrot,
    ItemType::Potato,
    ItemType::BakedPotato,
    ItemType::SplashPotionAwkward,
    ItemType::SplashPotionNightVision,
    ItemType::SplashPotionInvisibility,
    ItemType::SplashPotionLeaping,
    ItemType::SplashPotionFireResistance,
    ItemType::SplashPotionSwiftness,
    ItemType::SplashPotionSlowness,
    ItemType::SplashPotionWaterBreathing,
    ItemType::SplashPotionHealing,
    ItemType::SplashPotionHarming,
    ItemType::SplashPotionPoison,
    ItemType::SplashPotionRegeneration,
    ItemType::SplashPotionStrength,
    ItemType::SplashPotionWeakness,
    ItemType::SpiderEye,
    ItemType::GoldenCarrot,
    ItemType::SugarCane,
    ItemType::Sugar,
    ItemType::Paper,
    ItemType::Book,
    ItemType::BrownMushroom,
    ItemType::FermentedSpiderEye,
    ItemType::MagmaCream,
    ItemType::GhastTear,
    ItemType::GlisteringMelon,
    ItemType::RabbitFoot,
    ItemType::PhantomMembrane,
    ItemType::PotionSlowFalling,
    ItemType::SplashPotionSlowFalling,
    ItemType::RedstoneDust,
    ItemType::GlowstoneDust,
    ItemType::PotionNightVisionLong,
    ItemType::PotionInvisibilityLong,
    ItemType::PotionLeapingLong,
    ItemType::PotionLeapingStrong,
    ItemType::PotionFireResistanceLong,
    ItemType::PotionSwiftnessLong,
    ItemType::PotionSwiftnessStrong,
    ItemType::PotionSlownessLong,
    ItemType::PotionWaterBreathingLong,
    ItemType::PotionHealingStrong,
    ItemType::PotionHarmingStrong,
    ItemType::PotionPoisonLong,
    ItemType::PotionPoisonStrong,
    ItemType::PotionRegenerationLong,
    ItemType::PotionRegenerationStrong,
    ItemType::PotionStrengthLong,
    ItemType::PotionStrengthStrong,
    ItemType::PotionWeaknessLong,
    ItemType::PotionSlowFallingLong,
    ItemType::SplashPotionNightVisionLong,
    ItemType::SplashPotionInvisibilityLong,
    ItemType::SplashPotionLeapingLong,
    ItemType::SplashPotionLeapingStrong,
    ItemType::SplashPotionFireResistanceLong,
    ItemType::SplashPotionSwiftnessLong,
    ItemType::SplashPotionSwiftnessStrong,
    ItemType::SplashPotionSlownessLong,
    ItemType::SplashPotionWaterBreathingLong,
    ItemType::SplashPotionHealingStrong,
    ItemType::SplashPotionHarmingStrong,
    ItemType::SplashPotionPoisonLong,
    ItemType::SplashPotionPoisonStrong,
    ItemType::SplashPotionRegenerationLong,
    ItemType::SplashPotionRegenerationStrong,
    ItemType::SplashPotionStrengthLong,
    ItemType::SplashPotionStrengthStrong,
    ItemType::SplashPotionWeaknessLong,
    ItemType::SplashPotionSlowFallingLong,
    ItemType::Pufferfish,
    ItemType::Bucket,
    ItemType::WaterBucket,
    ItemType::LavaBucket,
    ItemType::CobblestoneWall,
    ItemType::IronBars,
    ItemType::StoneBricks,
    ItemType::StoneBrickSlab,
    ItemType::StoneBrickStairs,
    ItemType::StoneBrickWall,
    ItemType::RedstoneRepeater,
    ItemType::RedstoneComparator,
    ItemType::NetherQuartz,
    ItemType::RedstoneObserver,
    ItemType::Piston,
    ItemType::Dispenser,
    ItemType::Dropper,
    ItemType::Hopper,
    ItemType::Emerald,
];

impl ItemType {
    /// Get the numeric ID for this item type (used in crafting recipes).
    pub fn id(&self) -> u16 {
        *self as u16
    }

    /// Convert a numeric item ID back into an [`ItemType`].
    pub fn from_id(id: u16) -> Option<Self> {
        ALL_ITEM_TYPES.get(id as usize).copied()
    }

    /// Get the maximum stack size for this item type.
    pub fn max_stack_size(&self) -> u32 {
        match self {
            // Most block items stack to 64
            ItemType::Stone
            | ItemType::Cobblestone
            | ItemType::Dirt
            | ItemType::Grass
            | ItemType::Sand
            | ItemType::Gravel
            | ItemType::Ice
            | ItemType::Snow
            | ItemType::Clay
            | ItemType::Bedrock
            | ItemType::OakLog
            | ItemType::OakLeaves
            | ItemType::BirchLog
            | ItemType::BirchLeaves
            | ItemType::PineLog
            | ItemType::PineLeaves
            | ItemType::CoalOre
            | ItemType::IronOre
            | ItemType::GoldOre
            | ItemType::DiamondOre
            | ItemType::LapisOre
            | ItemType::Wool
            | ItemType::Diamond
            | ItemType::Feather
            | ItemType::Bone
            | ItemType::RottenFlesh
            | ItemType::String
            | ItemType::Gunpowder
            | ItemType::SpiderEye
            | ItemType::IronIngot
            | ItemType::GoldIngot
            | ItemType::Coal
            | ItemType::Stick
            | ItemType::Planks
            | ItemType::OakPlanks
            | ItemType::BirchPlanks
            | ItemType::PinePlanks
            | ItemType::Furnace
            | ItemType::Sapling
            | ItemType::Flint
            | ItemType::Arrow
            | ItemType::LapisLazuli
            | ItemType::CraftingTable
            | ItemType::Torch
            | ItemType::Ladder
            | ItemType::Lever
            | ItemType::StoneButton
            | ItemType::OakButton
            | ItemType::StonePressurePlate
            | ItemType::OakPressurePlate
            | ItemType::RedstoneWire
            | ItemType::RedstoneTorch
            | ItemType::RedstoneRepeater
            | ItemType::RedstoneComparator
            | ItemType::RedstoneObserver
            | ItemType::Piston
            | ItemType::Dispenser
            | ItemType::Dropper
            | ItemType::Hopper
            | ItemType::Glass
            | ItemType::Obsidian
            | ItemType::OakFence
            | ItemType::OakFenceGate
            | ItemType::CobblestoneWall
            | ItemType::StoneSlab
            | ItemType::OakSlab
            | ItemType::StoneStairs
            | ItemType::OakStairs
            | ItemType::StoneBricks
            | ItemType::StoneBrickSlab
            | ItemType::StoneBrickStairs
            | ItemType::StoneBrickWall
            | ItemType::GlassPane
            | ItemType::IronBars
            | ItemType::Trapdoor
            | ItemType::Chest
            | ItemType::OakDoor
            | ItemType::IronDoor
            | ItemType::RedstoneLamp
            | ItemType::EnchantingTable
            | ItemType::BrewingStand
            | ItemType::NetherWartBlock
            | ItemType::SoulSand
            | ItemType::Bed
            | ItemType::WheatSeeds
            | ItemType::Wheat
            | ItemType::Bread
            | ItemType::Bookshelf
            | ItemType::Carrot
            | ItemType::Potato
            | ItemType::BakedPotato
            | ItemType::GoldenCarrot
            | ItemType::SugarCane
            | ItemType::Sugar
            | ItemType::Paper
            | ItemType::Book
            | ItemType::BrownMushroom
            | ItemType::FermentedSpiderEye => 64,
            ItemType::MagmaCream
            | ItemType::GhastTear
            | ItemType::GlisteringMelon
            | ItemType::RabbitFoot
            | ItemType::PhantomMembrane
            | ItemType::RedstoneDust
            | ItemType::GlowstoneDust
            | ItemType::Pufferfish
            | ItemType::NetherQuartz
            | ItemType::Emerald => 64,

            // Food and resources stack to 16
            ItemType::RawPork
            | ItemType::RawBeef
            | ItemType::CookedPork
            | ItemType::CookedBeef
            | ItemType::Leather
            | ItemType::Egg
            | ItemType::Apple => 16,

            // Non-stackable items (weapons, armor, potions)
            ItemType::Bow
            | ItemType::LeatherHelmet
            | ItemType::LeatherChestplate
            | ItemType::LeatherLeggings
            | ItemType::LeatherBoots
            | ItemType::IronHelmet
            | ItemType::IronChestplate
            | ItemType::IronLeggings
            | ItemType::IronBoots
            | ItemType::GoldHelmet
            | ItemType::GoldChestplate
            | ItemType::GoldLeggings
            | ItemType::GoldBoots
            | ItemType::DiamondHelmet
            | ItemType::DiamondChestplate
            | ItemType::DiamondLeggings
            | ItemType::DiamondBoots
            | ItemType::PotionAwkward
            | ItemType::PotionNightVision
            | ItemType::PotionInvisibility
            | ItemType::PotionLeaping
            | ItemType::PotionFireResistance
            | ItemType::PotionSwiftness
            | ItemType::PotionSlowness
            | ItemType::PotionWaterBreathing
            | ItemType::PotionHealing
            | ItemType::PotionHarming
            | ItemType::PotionPoison
            | ItemType::PotionRegeneration
            | ItemType::PotionStrength
            | ItemType::PotionWeakness
            | ItemType::PotionSlowFalling
            | ItemType::SplashPotionAwkward
            | ItemType::SplashPotionNightVision
            | ItemType::SplashPotionInvisibility
            | ItemType::SplashPotionLeaping
            | ItemType::SplashPotionFireResistance
            | ItemType::SplashPotionSwiftness
            | ItemType::SplashPotionSlowness
            | ItemType::SplashPotionWaterBreathing
            | ItemType::SplashPotionHealing
            | ItemType::SplashPotionHarming
            | ItemType::SplashPotionPoison
            | ItemType::SplashPotionRegeneration
            | ItemType::SplashPotionStrength
            | ItemType::SplashPotionWeakness
            | ItemType::SplashPotionSlowFalling
            | ItemType::PotionNightVisionLong
            | ItemType::PotionInvisibilityLong
            | ItemType::PotionLeapingLong
            | ItemType::PotionLeapingStrong
            | ItemType::PotionFireResistanceLong
            | ItemType::PotionSwiftnessLong
            | ItemType::PotionSwiftnessStrong
            | ItemType::PotionSlownessLong
            | ItemType::PotionWaterBreathingLong
            | ItemType::PotionHealingStrong
            | ItemType::PotionHarmingStrong
            | ItemType::PotionPoisonLong
            | ItemType::PotionPoisonStrong
            | ItemType::PotionRegenerationLong
            | ItemType::PotionRegenerationStrong
            | ItemType::PotionStrengthLong
            | ItemType::PotionStrengthStrong
            | ItemType::PotionWeaknessLong
            | ItemType::PotionSlowFallingLong
            | ItemType::SplashPotionNightVisionLong
            | ItemType::SplashPotionInvisibilityLong
            | ItemType::SplashPotionLeapingLong
            | ItemType::SplashPotionLeapingStrong
            | ItemType::SplashPotionFireResistanceLong
            | ItemType::SplashPotionSwiftnessLong
            | ItemType::SplashPotionSwiftnessStrong
            | ItemType::SplashPotionSlownessLong
            | ItemType::SplashPotionWaterBreathingLong
            | ItemType::SplashPotionHealingStrong
            | ItemType::SplashPotionHarmingStrong
            | ItemType::SplashPotionPoisonLong
            | ItemType::SplashPotionPoisonStrong
            | ItemType::SplashPotionRegenerationLong
            | ItemType::SplashPotionRegenerationStrong
            | ItemType::SplashPotionStrengthLong
            | ItemType::SplashPotionStrengthStrong
            | ItemType::SplashPotionWeaknessLong
            | ItemType::SplashPotionSlowFallingLong
            | ItemType::WoodenPickaxe
            | ItemType::StonePickaxe
            | ItemType::IronPickaxe
            | ItemType::DiamondPickaxe
            | ItemType::GoldPickaxe
            | ItemType::WoodenAxe
            | ItemType::StoneAxe
            | ItemType::IronAxe
            | ItemType::DiamondAxe
            | ItemType::GoldAxe
            | ItemType::WoodenShovel
            | ItemType::StoneShovel
            | ItemType::IronShovel
            | ItemType::DiamondShovel
            | ItemType::GoldShovel
            | ItemType::WoodenSword
            | ItemType::StoneSword
            | ItemType::IronSword
            | ItemType::DiamondSword
            | ItemType::GoldSword
            | ItemType::WoodenHoe
            | ItemType::StoneHoe
            | ItemType::IronHoe
            | ItemType::DiamondHoe
            | ItemType::GoldHoe
            | ItemType::Bucket
            | ItemType::WaterBucket
            | ItemType::LavaBucket => 1,

            // Brewing ingredients stack to 64
            ItemType::GlassBottle
            | ItemType::WaterBottle
            | ItemType::NetherWart
            | ItemType::BlazePowder => 64,
        }
    }

    /// Get the item that drops when a block is broken.
    ///
    /// Returns Some((item_type, count)) or None if nothing drops.
    ///
    /// Block IDs reference (from blocks.json):
    /// - 0: Air (no drop)
    /// - 1: Stone
    /// - 2: Dirt
    /// - 3: Grass (drops dirt, not grass block)
    /// - 4: Sand
    /// - 5: Gravel
    /// - 6: Water (no drop)
    /// - 7: Ice
    /// - 8: Snow
    /// - 9: Clay
    /// - 10: Bedrock (no drop in survival)
    /// - 11: Oak Log
    /// - 12: Oak Planks
    /// - 13: Crafting Table
    /// - 14: Coal Ore
    /// - 15: Iron Ore
    /// - 16: Gold Ore
    /// - 17: Diamond Ore
    /// - 24: Cobblestone
    pub fn from_block(block_id: u16) -> Option<(ItemType, u32)> {
        match block_id {
            // Terrain blocks
            1 => Some((ItemType::Cobblestone, 1)), // Stone drops cobblestone (like Minecraft)
            24 => Some((ItemType::Cobblestone, 1)),
            2 => Some((ItemType::Dirt, 1)),
            3 => Some((ItemType::Dirt, 1)), // Grass drops dirt (like Minecraft)
            4 => Some((ItemType::Sand, 1)),
            5 => Some((ItemType::Gravel, 1)),
            8 => Some((ItemType::Snow, 1)),
            9 => Some((ItemType::Clay, 1)),

            // Tree blocks
            11 => Some((ItemType::OakLog, 1)),
            71 => Some((ItemType::BirchLog, 1)),
            73 => Some((ItemType::PineLog, 1)),
            12 => Some((ItemType::OakPlanks, 1)),
            13 => Some((ItemType::CraftingTable, 1)),

            // Ore blocks - coal and diamond drop resources; others drop ore blocks
            14 => Some((ItemType::Coal, 1)),
            15 => Some((ItemType::IronOre, 1)),
            16 => Some((ItemType::GoldOre, 1)),
            17 => Some((ItemType::Diamond, 1)),

            // Obsidian
            23 => Some((ItemType::Obsidian, 1)),

            // Glass and panes (glass requires silk touch; panes drop themselves)
            37 => Some((ItemType::GlassPane, 1)),

            // Doors (either half drops the door item)
            26 | 27 => Some((ItemType::OakDoor, 1)),
            28 | 29 => Some((ItemType::IronDoor, 1)),

            // Beds (either half drops the bed item)
            65 | 66 => Some((ItemType::Bed, 1)),

            // Building blocks
            31 => Some((ItemType::OakFence, 1)),
            32 => Some((ItemType::OakFenceGate, 1)),
            114 => Some((ItemType::CobblestoneWall, 1)),
            115 => Some((ItemType::IronBars, 1)),
            116 => Some((ItemType::StoneBricks, 1)),
            117 => Some((ItemType::StoneBrickSlab, 1)),
            118 => Some((ItemType::StoneBrickStairs, 1)),
            119 => Some((ItemType::StoneBrickWall, 1)),
            120 => Some((ItemType::StoneSlab, 2)),
            121 => Some((ItemType::OakSlab, 2)),
            122 => Some((ItemType::StoneBrickSlab, 2)),
            33 => Some((ItemType::StoneSlab, 1)),
            34 => Some((ItemType::OakSlab, 1)),
            35 => Some((ItemType::StoneStairs, 1)),
            36 => Some((ItemType::OakStairs, 1)),
            67 => Some((ItemType::Chest, 1)),
            68 => Some((ItemType::Trapdoor, 1)),

            // Furnace
            18 => Some((ItemType::Furnace, 1)),
            19 => Some((ItemType::Furnace, 1)), // Lit furnace also drops furnace

            // Redstone lamp (both lit and unlit)
            45 | 46 => Some((ItemType::RedstoneLamp, 1)),

            // Farmland drops dirt (like Minecraft)
            47 | 48 => Some((ItemType::Dirt, 1)),

            // Wheat crops (stage 7 drops wheat; earlier stages drop seeds).
            49..=55 => Some((ItemType::WheatSeeds, 1)), // wheat_0..wheat_6
            56 => Some((ItemType::Wheat, 1)),           // wheat_7

            // Carrot/potato crops: always drop the produce item (used for replanting).
            57..=60 => Some((ItemType::Carrot, 1)), // carrots_0..carrots_3
            61..=64 => Some((ItemType::Potato, 1)), // potatoes_0..potatoes_3

            // Lapis ore (drops 4-9 lapis, using 6 as average for now)
            98 => Some((ItemType::LapisLazuli, 6)),

            // Torch
            69 => Some((ItemType::Torch, 1)),

            // Utility blocks
            99 => Some((ItemType::EnchantingTable, 1)),
            100 => Some((ItemType::BrewingStand, 1)),
            101 => Some((ItemType::NetherWartBlock, 1)),
            102 => Some((ItemType::SoulSand, 1)),
            103 => Some((ItemType::Bookshelf, 1)),
            104 => Some((ItemType::SugarCane, 1)),
            105 => Some((ItemType::BrownMushroom, 1)),
            106 => Some((ItemType::MagmaCream, 1)),
            107 => Some((ItemType::GhastTear, 1)),
            108 => Some((ItemType::GlisteringMelon, 1)),
            109 => Some((ItemType::RabbitFoot, 1)),
            110 => Some((ItemType::PhantomMembrane, 1)),
            111 => Some((ItemType::RedstoneDust, 1)),
            112 => Some((ItemType::GlowstoneDust, 1)),
            113 => Some((ItemType::Pufferfish, 1)),

            // Magma block: drops blaze powder (Overworld proxy for brewing fuel).
            80 => Some((ItemType::BlazePowder, 1)),

            // Interactive/redstone components
            30 => Some((ItemType::Ladder, 1)),
            38 => Some((ItemType::Lever, 1)),
            39 => Some((ItemType::StoneButton, 1)),
            40 => Some((ItemType::OakButton, 1)),
            41 => Some((ItemType::StonePressurePlate, 1)),
            42 => Some((ItemType::OakPressurePlate, 1)),
            // Vanilla-ish: breaking redstone wire drops redstone dust.
            43 => Some((ItemType::RedstoneDust, 1)),
            44 => Some((ItemType::RedstoneTorch, 1)),
            123 => Some((ItemType::RedstoneRepeater, 1)),
            124 => Some((ItemType::RedstoneComparator, 1)),
            125 => Some((ItemType::NetherQuartz, 1)),
            126 => Some((ItemType::RedstoneObserver, 1)),
            127 | 128 => Some((ItemType::Piston, 1)),
            129 => Some((ItemType::Dispenser, 1)),
            130 => Some((ItemType::Dropper, 1)),
            131 => Some((ItemType::Hopper, 1)),

            // No drops: Air (0), Water (6), Ice (7; needs Silk Touch), Bedrock (10), Glass (25; needs Silk Touch)
            _ => None,
        }
    }

    /// Get the item that drops from leaves with random chance.
    ///
    /// Leaves have a chance to drop saplings (1/16) and oak leaves
    /// have a chance to drop apples (1/200).
    ///
    /// # Arguments
    /// * `block_id` - The block ID of the leaves
    /// * `random_value` - A random value from 0.0 to 1.0
    ///
    /// # Returns
    /// Some((item_type, count)) if a special drop occurs, None otherwise.
    pub fn from_leaves_random(block_id: u16, random_value: f64) -> Option<(ItemType, u32)> {
        match block_id {
            70 => {
                // Oak leaves: 1/200 apple, 1/16 sapling
                if random_value < 0.005 {
                    Some((ItemType::Apple, 1))
                } else if random_value < 0.005 + 0.0625 {
                    Some((ItemType::Sapling, 1))
                } else {
                    None
                }
            }
            72 | 74 => {
                // Birch/Spruce leaves: 1/16 sapling
                if random_value < 0.0625 {
                    Some((ItemType::Sapling, 1))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get the silk touch drop for a block (drops the block itself).
    ///
    /// When using Silk Touch, blocks drop themselves instead of their
    /// normal drops (e.g., stone drops stone instead of cobblestone).
    ///
    /// # Arguments
    /// * `block_id` - The block ID being broken
    ///
    /// # Returns
    /// Some((item_type, count)) for the block itself, None if not applicable.
    pub fn silk_touch_drop(block_id: u16) -> Option<(ItemType, u32)> {
        match block_id {
            // Blocks that normally drop something else
            1 => Some((ItemType::Stone, 1)), // Stone drops stone instead of cobblestone
            3 => Some((ItemType::Grass, 1)), // Grass drops grass block instead of dirt
            14 => Some((ItemType::CoalOre, 1)), // Coal ore drops ore block instead of coal
            98 => Some((ItemType::LapisOre, 1)), // Lapis ore drops ore block instead of lapis
            17 => Some((ItemType::DiamondOre, 1)), // Diamond ore drops ore block
            7 => Some((ItemType::Ice, 1)),
            25 => Some((ItemType::Glass, 1)),
            70 => Some((ItemType::OakLeaves, 1)),
            72 => Some((ItemType::BirchLeaves, 1)),
            74 => Some((ItemType::PineLeaves, 1)),
            // Blocks that already drop themselves can use normal drops
            _ => ItemType::from_block(block_id),
        }
    }

    /// Get the fortune-modified drop for a block.
    ///
    /// Fortune increases drop counts for certain blocks like ores.
    /// Each level gives a chance for bonus drops.
    ///
    /// # Arguments
    /// * `block_id` - The block ID being broken
    /// * `fortune_level` - The Fortune enchantment level (1-3)
    /// * `random_value` - A random value from 0.0 to 1.0
    ///
    /// # Returns
    /// Some((item_type, count)) with potentially increased count.
    pub fn fortune_drop(
        block_id: u16,
        fortune_level: u8,
        random_value: f64,
    ) -> Option<(ItemType, u32)> {
        let base_drop = ItemType::from_block(block_id)?;
        let (item_type, base_count) = base_drop;

        // Fortune affects coal, diamond, and lapis ores
        let affected_blocks = [14, 17, 98]; // coal, diamond, lapis
        if !affected_blocks.contains(&block_id) {
            return Some((item_type, base_count));
        }

        // Fortune formula: base_count + random(0, fortune_level + 1) bonus items
        // Using random_value to determine bonus (simplified formula)
        // Fortune I: 0-2 bonus, Fortune II: 0-3 bonus, Fortune III: 0-4 bonus
        let max_bonus = fortune_level as u32 + 1;
        let bonus = (random_value * max_bonus as f64).floor() as u32;
        let final_count = base_count + bonus;

        Some((item_type, final_count))
    }

    /// Get the block ID that this item places (if applicable).
    ///
    /// # Returns
    /// Some(block_id) if this item can be placed as a block, None otherwise.
    pub fn to_block(&self) -> Option<u16> {
        match self {
            ItemType::Stone => Some(1),
            ItemType::Cobblestone => Some(24),
            ItemType::Dirt => Some(2),
            ItemType::Grass => Some(3),
            ItemType::Sand => Some(4),
            ItemType::Gravel => Some(5),
            ItemType::Ice => Some(7),
            ItemType::Snow => Some(8),
            ItemType::Clay => Some(9),
            ItemType::OakLog => Some(11),
            ItemType::OakLeaves => Some(70),
            ItemType::BirchLog => Some(71),
            ItemType::BirchLeaves => Some(72),
            ItemType::PineLog => Some(73),
            ItemType::PineLeaves => Some(74),
            ItemType::OakPlanks => Some(12),
            ItemType::CraftingTable => Some(13),
            ItemType::CoalOre => Some(14),
            ItemType::IronOre => Some(15),
            ItemType::GoldOre => Some(16),
            ItemType::DiamondOre => Some(17),
            ItemType::Furnace => Some(18),
            ItemType::Torch => Some(69),
            ItemType::LapisOre => Some(98),
            ItemType::Ladder => Some(30),
            ItemType::Lever => Some(38),
            ItemType::StoneButton => Some(39),
            ItemType::OakButton => Some(40),
            ItemType::StonePressurePlate => Some(41),
            ItemType::OakPressurePlate => Some(42),
            ItemType::RedstoneWire => Some(43),
            ItemType::RedstoneTorch => Some(44),
            ItemType::RedstoneRepeater => Some(123),
            ItemType::RedstoneComparator => Some(124),
            ItemType::RedstoneObserver => Some(126),
            ItemType::Piston => Some(127),
            ItemType::Dispenser => Some(129),
            ItemType::Dropper => Some(130),
            ItemType::Hopper => Some(131),
            ItemType::Glass => Some(25),
            ItemType::Obsidian => Some(23),
            ItemType::OakFence => Some(31),
            ItemType::OakFenceGate => Some(32),
            ItemType::CobblestoneWall => Some(114),
            ItemType::StoneSlab => Some(33),
            ItemType::OakSlab => Some(34),
            ItemType::StoneStairs => Some(35),
            ItemType::OakStairs => Some(36),
            ItemType::StoneBricks => Some(116),
            ItemType::StoneBrickSlab => Some(117),
            ItemType::StoneBrickStairs => Some(118),
            ItemType::StoneBrickWall => Some(119),
            ItemType::GlassPane => Some(37),
            ItemType::IronBars => Some(115),
            ItemType::Trapdoor => Some(68),
            ItemType::Chest => Some(67),
            ItemType::OakDoor => Some(26),
            ItemType::IronDoor => Some(28),
            ItemType::RedstoneLamp => Some(45),
            ItemType::EnchantingTable => Some(99),
            ItemType::BrewingStand => Some(100),
            ItemType::NetherWartBlock => Some(101),
            ItemType::SoulSand => Some(102),
            ItemType::Bookshelf => Some(103),
            ItemType::SugarCane => Some(104),
            ItemType::BrownMushroom => Some(105),
            ItemType::Bed => Some(66),
            // Non-placeable items (mob drops, food, crafted items)
            _ => None,
        }
    }

    /// Convert a placeable block ID into the corresponding item type (if any).
    ///
    /// This is the inverse of [`ItemType::to_block`] for block items.
    pub fn from_placeable_block(block_id: u16) -> Option<ItemType> {
        match block_id {
            1 => Some(ItemType::Stone),
            24 => Some(ItemType::Cobblestone),
            2 => Some(ItemType::Dirt),
            3 => Some(ItemType::Grass),
            4 => Some(ItemType::Sand),
            5 => Some(ItemType::Gravel),
            7 => Some(ItemType::Ice),
            8 => Some(ItemType::Snow),
            9 => Some(ItemType::Clay),
            11 => Some(ItemType::OakLog),
            70 => Some(ItemType::OakLeaves),
            71 => Some(ItemType::BirchLog),
            72 => Some(ItemType::BirchLeaves),
            73 => Some(ItemType::PineLog),
            74 => Some(ItemType::PineLeaves),
            12 => Some(ItemType::OakPlanks),
            13 => Some(ItemType::CraftingTable),
            14 => Some(ItemType::CoalOre),
            15 => Some(ItemType::IronOre),
            16 => Some(ItemType::GoldOre),
            17 => Some(ItemType::DiamondOre),
            18 | 19 => Some(ItemType::Furnace),
            69 => Some(ItemType::Torch),
            98 => Some(ItemType::LapisOre),
            30 => Some(ItemType::Ladder),
            38 => Some(ItemType::Lever),
            39 => Some(ItemType::StoneButton),
            40 => Some(ItemType::OakButton),
            41 => Some(ItemType::StonePressurePlate),
            42 => Some(ItemType::OakPressurePlate),
            43 => Some(ItemType::RedstoneWire),
            44 => Some(ItemType::RedstoneTorch),
            123 => Some(ItemType::RedstoneRepeater),
            124 => Some(ItemType::RedstoneComparator),
            126 => Some(ItemType::RedstoneObserver),
            127 => Some(ItemType::Piston),
            129 => Some(ItemType::Dispenser),
            130 => Some(ItemType::Dropper),
            131 => Some(ItemType::Hopper),
            23 => Some(ItemType::Obsidian),
            25 => Some(ItemType::Glass),
            31 => Some(ItemType::OakFence),
            32 => Some(ItemType::OakFenceGate),
            114 => Some(ItemType::CobblestoneWall),
            33 => Some(ItemType::StoneSlab),
            34 => Some(ItemType::OakSlab),
            35 => Some(ItemType::StoneStairs),
            36 => Some(ItemType::OakStairs),
            37 => Some(ItemType::GlassPane),
            115 => Some(ItemType::IronBars),
            116 => Some(ItemType::StoneBricks),
            117 => Some(ItemType::StoneBrickSlab),
            118 => Some(ItemType::StoneBrickStairs),
            119 => Some(ItemType::StoneBrickWall),
            26 | 27 => Some(ItemType::OakDoor),
            28 | 29 => Some(ItemType::IronDoor),
            45 | 46 => Some(ItemType::RedstoneLamp),
            67 => Some(ItemType::Chest),
            68 => Some(ItemType::Trapdoor),
            99 => Some(ItemType::EnchantingTable),
            100 => Some(ItemType::BrewingStand),
            101 => Some(ItemType::NetherWartBlock),
            102 => Some(ItemType::SoulSand),
            103 => Some(ItemType::Bookshelf),
            104 => Some(ItemType::SugarCane),
            105 => Some(ItemType::BrownMushroom),
            65 | 66 => Some(ItemType::Bed),
            _ => None,
        }
    }

    /// Check if this item is a drinkable potion.
    pub fn is_potion(&self) -> bool {
        matches!(
            self,
            ItemType::PotionAwkward
                | ItemType::PotionNightVision
                | ItemType::PotionInvisibility
                | ItemType::PotionLeaping
                | ItemType::PotionFireResistance
                | ItemType::PotionSwiftness
                | ItemType::PotionSlowness
                | ItemType::PotionWaterBreathing
                | ItemType::PotionHealing
                | ItemType::PotionHarming
                | ItemType::PotionPoison
                | ItemType::PotionRegeneration
                | ItemType::PotionStrength
                | ItemType::PotionWeakness
                | ItemType::PotionNightVisionLong
                | ItemType::PotionInvisibilityLong
                | ItemType::PotionLeapingLong
                | ItemType::PotionLeapingStrong
                | ItemType::PotionFireResistanceLong
                | ItemType::PotionSwiftnessLong
                | ItemType::PotionSwiftnessStrong
                | ItemType::PotionSlownessLong
                | ItemType::PotionWaterBreathingLong
                | ItemType::PotionHealingStrong
                | ItemType::PotionHarmingStrong
                | ItemType::PotionPoisonLong
                | ItemType::PotionPoisonStrong
                | ItemType::PotionRegenerationLong
                | ItemType::PotionRegenerationStrong
                | ItemType::PotionStrengthLong
                | ItemType::PotionStrengthStrong
                | ItemType::PotionWeaknessLong
                | ItemType::PotionSlowFalling
                | ItemType::PotionSlowFallingLong
        )
    }

    /// Convert a potion item type to its PotionType.
    /// Returns None if not a potion item.
    pub fn to_potion_type(&self) -> Option<crate::PotionType> {
        match self {
            ItemType::PotionAwkward => Some(crate::PotionType::Awkward),
            ItemType::PotionNightVision => Some(crate::PotionType::NightVision),
            ItemType::PotionInvisibility => Some(crate::PotionType::Invisibility),
            ItemType::PotionLeaping => Some(crate::PotionType::Leaping),
            ItemType::PotionFireResistance => Some(crate::PotionType::FireResistance),
            ItemType::PotionSwiftness => Some(crate::PotionType::Swiftness),
            ItemType::PotionSlowness => Some(crate::PotionType::Slowness),
            ItemType::PotionWaterBreathing => Some(crate::PotionType::WaterBreathing),
            ItemType::PotionHealing => Some(crate::PotionType::Healing),
            ItemType::PotionHarming => Some(crate::PotionType::Harming),
            ItemType::PotionPoison => Some(crate::PotionType::Poison),
            ItemType::PotionRegeneration => Some(crate::PotionType::Regeneration),
            ItemType::PotionStrength => Some(crate::PotionType::Strength),
            ItemType::PotionWeakness => Some(crate::PotionType::Weakness),
            ItemType::PotionSlowFalling => Some(crate::PotionType::SlowFalling),
            ItemType::PotionNightVisionLong => Some(crate::PotionType::NightVision),
            ItemType::PotionInvisibilityLong => Some(crate::PotionType::Invisibility),
            ItemType::PotionLeapingLong | ItemType::PotionLeapingStrong => {
                Some(crate::PotionType::Leaping)
            }
            ItemType::PotionFireResistanceLong => Some(crate::PotionType::FireResistance),
            ItemType::PotionSwiftnessLong | ItemType::PotionSwiftnessStrong => {
                Some(crate::PotionType::Swiftness)
            }
            ItemType::PotionSlownessLong => Some(crate::PotionType::Slowness),
            ItemType::PotionWaterBreathingLong => Some(crate::PotionType::WaterBreathing),
            ItemType::PotionHealingStrong => Some(crate::PotionType::Healing),
            ItemType::PotionHarmingStrong => Some(crate::PotionType::Harming),
            ItemType::PotionPoisonLong | ItemType::PotionPoisonStrong => {
                Some(crate::PotionType::Poison)
            }
            ItemType::PotionRegenerationLong | ItemType::PotionRegenerationStrong => {
                Some(crate::PotionType::Regeneration)
            }
            ItemType::PotionStrengthLong | ItemType::PotionStrengthStrong => {
                Some(crate::PotionType::Strength)
            }
            ItemType::PotionWeaknessLong => Some(crate::PotionType::Weakness),
            ItemType::PotionSlowFallingLong => Some(crate::PotionType::SlowFalling),
            _ => None,
        }
    }

    /// Create a potion item type from a PotionType.
    pub fn from_potion_type(potion: crate::PotionType) -> Option<ItemType> {
        match potion {
            crate::PotionType::Awkward => Some(ItemType::PotionAwkward),
            crate::PotionType::NightVision => Some(ItemType::PotionNightVision),
            crate::PotionType::Invisibility => Some(ItemType::PotionInvisibility),
            crate::PotionType::Leaping => Some(ItemType::PotionLeaping),
            crate::PotionType::FireResistance => Some(ItemType::PotionFireResistance),
            crate::PotionType::Swiftness => Some(ItemType::PotionSwiftness),
            crate::PotionType::Slowness => Some(ItemType::PotionSlowness),
            crate::PotionType::WaterBreathing => Some(ItemType::PotionWaterBreathing),
            crate::PotionType::Healing => Some(ItemType::PotionHealing),
            crate::PotionType::Harming => Some(ItemType::PotionHarming),
            crate::PotionType::Poison => Some(ItemType::PotionPoison),
            crate::PotionType::Regeneration => Some(ItemType::PotionRegeneration),
            crate::PotionType::Strength => Some(ItemType::PotionStrength),
            crate::PotionType::Weakness => Some(ItemType::PotionWeakness),
            crate::PotionType::SlowFalling => Some(ItemType::PotionSlowFalling),
            // Base potions without effects - no item representation
            crate::PotionType::Water | crate::PotionType::Mundane | crate::PotionType::Thick => {
                None
            }
            // Potions not yet implemented as items
            crate::PotionType::Luck => None,
        }
    }
}

/// A dropped item entity in the world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroppedItem {
    /// Unique ID for this dropped item.
    pub id: u64,
    /// World X position.
    pub x: f64,
    /// World Y position.
    pub y: f64,
    /// World Z position.
    pub z: f64,
    /// Velocity in X direction.
    pub vel_x: f64,
    /// Velocity in Y direction.
    pub vel_y: f64,
    /// Velocity in Z direction.
    pub vel_z: f64,
    /// Type of item.
    pub item_type: ItemType,
    /// Count/stack size.
    pub count: u32,
    /// Ticks remaining before despawn.
    pub lifetime_ticks: u32,
    /// Whether the item is on the ground (no longer falling).
    pub on_ground: bool,
}

impl DroppedItem {
    /// Create a new dropped item at the given position.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this item
    /// * `x, y, z` - World position
    /// * `item_type` - Type of item
    /// * `count` - Stack size
    ///
    /// Items spawn with small random velocity for visual scatter.
    pub fn new(id: u64, x: f64, y: f64, z: f64, item_type: ItemType, count: u32) -> Self {
        // Simple pseudo-random velocity based on ID
        let vel_x = ((id % 100) as f64 - 50.0) / 200.0; // -0.25 to 0.25
        let vel_z = (((id / 100) % 100) as f64 - 50.0) / 200.0;
        let vel_y = 0.2; // Small upward velocity

        Self {
            id,
            x,
            y,
            z,
            vel_x,
            vel_y,
            vel_z,
            item_type,
            count,
            lifetime_ticks: ITEM_DESPAWN_TICKS,
            on_ground: false,
        }
    }

    /// Update the item's physics and lifetime.
    ///
    /// # Arguments
    /// * `ground_height` - The Y coordinate of the ground at this position
    ///
    /// # Returns
    /// `true` if the item should be removed (despawned), `false` otherwise.
    pub fn update(&mut self, ground_height: f64) -> bool {
        // Decrement lifetime
        if self.lifetime_ticks > 0 {
            self.lifetime_ticks -= 1;
        } else {
            return true; // Despawn
        }

        // Apply physics if not on ground
        if !self.on_ground {
            // Gravity
            self.vel_y -= 0.04; // Gravity acceleration (slightly less than mobs)

            // Air resistance
            self.vel_x *= 0.98;
            self.vel_y *= 0.98;
            self.vel_z *= 0.98;

            // Update position
            self.x += self.vel_x;
            self.y += self.vel_y;
            self.z += self.vel_z;

            // Ground collision (items float slightly above ground)
            let item_ground_level = ground_height + 0.25;
            if self.y <= item_ground_level {
                self.y = item_ground_level;
                self.vel_y = 0.0;
                self.vel_x *= 0.5; // Friction
                self.vel_z *= 0.5;

                // Mark as on ground if velocity is low
                if self.vel_x.abs() < 0.01 && self.vel_z.abs() < 0.01 {
                    self.on_ground = true;
                }
            }
        }

        false // Don't despawn yet
    }

    /// Check if this item can be picked up by a player/mob at the given position.
    ///
    /// # Arguments
    /// * `px, py, pz` - Position of the player/mob
    ///
    /// # Returns
    /// `true` if within pickup radius.
    pub fn can_pickup(&self, px: f64, py: f64, pz: f64) -> bool {
        let dx = self.x - px;
        let dy = self.y - py;
        let dz = self.z - pz;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= PICKUP_RADIUS * PICKUP_RADIUS
    }

    /// Merge another item stack into this one if possible.
    ///
    /// # Arguments
    /// * `other` - Another dropped item to merge
    ///
    /// # Returns
    /// Number of items successfully merged (may be less than other.count if stack limit reached).
    pub fn try_merge(&mut self, other: &DroppedItem) -> u32 {
        if self.item_type != other.item_type {
            return 0; // Can't merge different item types
        }

        let max_stack = self.item_type.max_stack_size();
        let available_space = max_stack.saturating_sub(self.count);
        let merge_amount = available_space.min(other.count);

        self.count += merge_amount;
        merge_amount
    }
}

/// Manages all dropped items in the world.
/// Uses BTreeMap for deterministic iteration order (critical for multiplayer sync).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemManager {
    items: BTreeMap<u64, DroppedItem>,
    next_id: u64,
}

impl ItemManager {
    /// Create a new empty item manager.
    pub fn new() -> Self {
        Self {
            items: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Spawn a new dropped item.
    ///
    /// # Arguments
    /// * `x, y, z` - World position
    /// * `item_type` - Type of item
    /// * `count` - Stack size
    ///
    /// # Returns
    /// The ID of the newly spawned item.
    pub fn spawn_item(&mut self, x: f64, y: f64, z: f64, item_type: ItemType, count: u32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let item = DroppedItem::new(id, x, y, z, item_type, count);
        self.items.insert(id, item);
        id
    }

    /// Update all items (physics and lifetime).
    ///
    /// # Arguments
    /// * `get_ground_height` - Function to get ground height at (x, z) position
    ///
    /// # Returns
    /// Number of items that despawned this tick.
    pub fn update<F>(&mut self, get_ground_height: F) -> usize
    where
        F: Fn(f64, f64) -> f64,
    {
        let mut to_remove = Vec::new();

        for (id, item) in self.items.iter_mut() {
            let ground_height = get_ground_height(item.x, item.z);
            if item.update(ground_height) {
                to_remove.push(*id);
            }
        }

        let despawn_count = to_remove.len();
        for id in to_remove {
            self.items.remove(&id);
        }

        despawn_count
    }

    /// Attempt to pick up items near a given position.
    ///
    /// # Arguments
    /// * `x, y, z` - Position of the player/mob
    ///
    /// # Returns
    /// List of (item_type, count) tuples that were picked up.
    pub fn pickup_items(&mut self, x: f64, y: f64, z: f64) -> Vec<(ItemType, u32)> {
        let mut picked_up = Vec::new();
        let mut to_remove = Vec::new();

        for (id, item) in self.items.iter() {
            if item.can_pickup(x, y, z) {
                picked_up.push((item.item_type, item.count));
                to_remove.push(*id);
            }
        }

        for id in to_remove {
            self.items.remove(&id);
        }

        picked_up
    }

    /// Take (and remove) a single item from the first dropped stack within `radius` of (x, y, z).
    ///
    /// This is intended for deterministic systems like hoppers: it always selects the lowest-ID
    /// dropped item that matches the radius check.
    pub fn take_one_near(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Option<(ItemType, u32)> {
        self.take_one_near_if(x, y, z, radius, |_| true)
    }

    /// Take (and remove) a single item from the first dropped stack within `radius` of (x, y, z)
    /// that passes the provided predicate.
    ///
    /// This is intended for deterministic systems (e.g., hoppers) that must avoid removing items
    /// they cannot accept: selection is still deterministic (lowest-ID first).
    pub fn take_one_near_if<F>(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        predicate: F,
    ) -> Option<(ItemType, u32)>
    where
        F: Fn(ItemType) -> bool,
    {
        let radius = radius.max(0.0);
        let radius_sq = radius * radius;

        let mut picked_id: Option<u64> = None;
        for (id, item) in self.items.iter() {
            let dx = item.x - x;
            let dy = item.y - y;
            let dz = item.z - z;
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq <= radius_sq && predicate(item.item_type) {
                picked_id = Some(*id);
                break;
            }
        }

        let id = picked_id?;
        let mut remove = false;
        let item_type = {
            let item = self.items.get_mut(&id)?;
            if item.count > 1 {
                item.count -= 1;
            } else {
                remove = true;
            }
            item.item_type
        };

        if remove {
            self.items.remove(&id);
        }

        Some((item_type, 1))
    }

    /// Get the number of active dropped items.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Get a reference to a specific item by ID.
    pub fn get(&self, id: u64) -> Option<&DroppedItem> {
        self.items.get(&id)
    }

    /// Get a mutable reference to a specific item by ID.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut DroppedItem> {
        self.items.get_mut(&id)
    }

    /// Get all items as a slice.
    pub fn items(&self) -> Vec<&DroppedItem> {
        self.items.values().collect()
    }

    /// Merge nearby items of the same type.
    ///
    /// Items within 1 block of each other will be merged if they're the same type.
    ///
    /// # Returns
    /// Number of items merged (removed).
    pub fn merge_nearby_items(&mut self) -> usize {
        const MERGE_RADIUS: f64 = 1.0;
        let mut merged_count = 0;
        let mut to_remove = Vec::new();

        // Get all item IDs
        let ids: Vec<u64> = self.items.keys().copied().collect();

        for i in 0..ids.len() {
            if to_remove.contains(&ids[i]) {
                continue;
            }

            for j in (i + 1)..ids.len() {
                if to_remove.contains(&ids[j]) {
                    continue;
                }

                let (id_a, id_b) = (ids[i], ids[j]);

                // Check distance
                let (item_a, item_b) = match (self.items.get(&id_a), self.items.get(&id_b)) {
                    (Some(a), Some(b)) => (a.clone(), b.clone()),
                    _ => continue,
                };

                let dx = item_a.x - item_b.x;
                let dy = item_a.y - item_b.y;
                let dz = item_a.z - item_b.z;
                let dist_sq = dx * dx + dy * dy + dz * dz;

                if dist_sq <= MERGE_RADIUS * MERGE_RADIUS {
                    // Try to merge item_b into item_a
                    if let Some(item_a_mut) = self.items.get_mut(&id_a) {
                        let merged = item_a_mut.try_merge(&item_b);
                        if merged == item_b.count {
                            // Fully merged, remove item_b
                            to_remove.push(id_b);
                            merged_count += 1;
                        }
                    }
                }
            }
        }

        for id in to_remove {
            self.items.remove(&id);
        }

        merged_count
    }
}

impl Default for ItemManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_type_from_id_roundtrips() {
        assert_eq!(ALL_ITEM_TYPES.len(), ItemType::Emerald as usize + 1);

        for (idx, item_type) in ALL_ITEM_TYPES.iter().copied().enumerate() {
            assert_eq!(item_type.id(), idx as u16);
            assert_eq!(ItemType::from_id(idx as u16), Some(item_type));
        }

        assert_eq!(ItemType::from_id(u16::MAX), None);
    }

    #[test]
    fn test_item_type_max_stack() {
        // Block items stack to 64
        assert_eq!(ItemType::Stone.max_stack_size(), 64);
        assert_eq!(ItemType::OakLog.max_stack_size(), 64);
        assert_eq!(ItemType::Ice.max_stack_size(), 64);
        assert_eq!(ItemType::Feather.max_stack_size(), 64);
        assert_eq!(ItemType::Bed.max_stack_size(), 64);
        assert_eq!(ItemType::Diamond.max_stack_size(), 64);
        assert_eq!(ItemType::WheatSeeds.max_stack_size(), 64);
        assert_eq!(ItemType::Wheat.max_stack_size(), 64);
        assert_eq!(ItemType::Bread.max_stack_size(), 64);
        assert_eq!(ItemType::Bookshelf.max_stack_size(), 64);
        assert_eq!(ItemType::CobblestoneWall.max_stack_size(), 64);
        assert_eq!(ItemType::IronBars.max_stack_size(), 64);
        assert_eq!(ItemType::StoneBricks.max_stack_size(), 64);
        assert_eq!(ItemType::StoneBrickSlab.max_stack_size(), 64);
        assert_eq!(ItemType::StoneBrickStairs.max_stack_size(), 64);
        assert_eq!(ItemType::StoneBrickWall.max_stack_size(), 64);
        assert_eq!(ItemType::RedstoneRepeater.max_stack_size(), 64);
        assert_eq!(ItemType::RedstoneComparator.max_stack_size(), 64);
        assert_eq!(ItemType::NetherQuartz.max_stack_size(), 64);
        assert_eq!(ItemType::RedstoneObserver.max_stack_size(), 64);
        assert_eq!(ItemType::Piston.max_stack_size(), 64);
        assert_eq!(ItemType::Dispenser.max_stack_size(), 64);
        assert_eq!(ItemType::Dropper.max_stack_size(), 64);
        assert_eq!(ItemType::Hopper.max_stack_size(), 64);
        assert_eq!(ItemType::Carrot.max_stack_size(), 64);
        assert_eq!(ItemType::Potato.max_stack_size(), 64);
        assert_eq!(ItemType::BakedPotato.max_stack_size(), 64);

        // Food/resources stack to 16
        assert_eq!(ItemType::RawPork.max_stack_size(), 16);
        assert_eq!(ItemType::Apple.max_stack_size(), 16);

        // Tools don't stack
        assert_eq!(ItemType::WoodenPickaxe.max_stack_size(), 1);
        assert_eq!(ItemType::DiamondSword.max_stack_size(), 1);
        assert_eq!(ItemType::Bucket.max_stack_size(), 1);
        assert_eq!(ItemType::WaterBucket.max_stack_size(), 1);
        assert_eq!(ItemType::LavaBucket.max_stack_size(), 1);
    }

    #[test]
    fn test_item_type_from_block() {
        // Terrain blocks - stone drops cobblestone (like Minecraft)
        assert_eq!(ItemType::from_block(1), Some((ItemType::Cobblestone, 1)));
        assert_eq!(ItemType::from_block(2), Some((ItemType::Dirt, 1)));
        assert_eq!(ItemType::from_block(3), Some((ItemType::Dirt, 1))); // Grass drops dirt
        assert_eq!(ItemType::from_block(4), Some((ItemType::Sand, 1)));
        assert_eq!(ItemType::from_block(5), Some((ItemType::Gravel, 1)));
        assert_eq!(ItemType::from_block(7), None); // Ice needs Silk Touch
        assert_eq!(ItemType::from_block(8), Some((ItemType::Snow, 1)));
        assert_eq!(ItemType::from_block(9), Some((ItemType::Clay, 1)));

        // Tree/building blocks
        assert_eq!(ItemType::from_block(11), Some((ItemType::OakLog, 1)));
        assert_eq!(ItemType::from_block(71), Some((ItemType::BirchLog, 1)));
        assert_eq!(ItemType::from_block(73), Some((ItemType::PineLog, 1)));
        assert_eq!(ItemType::from_block(12), Some((ItemType::OakPlanks, 1)));

        // Ore blocks - coal ore drops coal directly (like Minecraft)
        assert_eq!(ItemType::from_block(14), Some((ItemType::Coal, 1)));
        assert_eq!(ItemType::from_block(15), Some((ItemType::IronOre, 1)));
        assert_eq!(ItemType::from_block(16), Some((ItemType::GoldOre, 1)));
        assert_eq!(ItemType::from_block(17), Some((ItemType::Diamond, 1)));

        // Obsidian
        assert_eq!(ItemType::from_block(23), Some((ItemType::Obsidian, 1)));

        // Magma blocks drop blaze powder (Overworld proxy).
        assert_eq!(ItemType::from_block(80), Some((ItemType::BlazePowder, 1)));

        // Glass needs Silk Touch (panes drop themselves)
        assert_eq!(ItemType::from_block(25), None);
        assert_eq!(ItemType::from_block(37), Some((ItemType::GlassPane, 1)));

        // Doors: either half drops the door item.
        assert_eq!(ItemType::from_block(26), Some((ItemType::OakDoor, 1)));
        assert_eq!(ItemType::from_block(27), Some((ItemType::OakDoor, 1)));
        assert_eq!(ItemType::from_block(28), Some((ItemType::IronDoor, 1)));
        assert_eq!(ItemType::from_block(29), Some((ItemType::IronDoor, 1)));

        // Beds: either half drops the bed item.
        assert_eq!(ItemType::from_block(65), Some((ItemType::Bed, 1)));
        assert_eq!(ItemType::from_block(66), Some((ItemType::Bed, 1)));

        // Basic building blocks
        assert_eq!(ItemType::from_block(31), Some((ItemType::OakFence, 1)));
        assert_eq!(ItemType::from_block(32), Some((ItemType::OakFenceGate, 1)));
        assert_eq!(ItemType::from_block(33), Some((ItemType::StoneSlab, 1)));
        assert_eq!(ItemType::from_block(34), Some((ItemType::OakSlab, 1)));
        assert_eq!(ItemType::from_block(35), Some((ItemType::StoneStairs, 1)));
        assert_eq!(ItemType::from_block(36), Some((ItemType::OakStairs, 1)));
        assert_eq!(ItemType::from_block(116), Some((ItemType::StoneBricks, 1)));
        assert_eq!(
            ItemType::from_block(117),
            Some((ItemType::StoneBrickSlab, 1))
        );
        assert_eq!(
            ItemType::from_block(118),
            Some((ItemType::StoneBrickStairs, 1))
        );
        assert_eq!(
            ItemType::from_block(119),
            Some((ItemType::StoneBrickWall, 1))
        );
        assert_eq!(ItemType::from_block(120), Some((ItemType::StoneSlab, 2)));
        assert_eq!(ItemType::from_block(121), Some((ItemType::OakSlab, 2)));
        assert_eq!(
            ItemType::from_block(122),
            Some((ItemType::StoneBrickSlab, 2))
        );
        assert_eq!(ItemType::from_block(67), Some((ItemType::Chest, 1)));
        assert_eq!(ItemType::from_block(68), Some((ItemType::Trapdoor, 1)));

        // No drops
        assert_eq!(ItemType::from_block(0), None); // Air
        assert_eq!(ItemType::from_block(6), None); // Water
        assert_eq!(ItemType::from_block(10), None); // Bedrock
        assert_eq!(ItemType::from_block(13), Some((ItemType::CraftingTable, 1))); // Crafting table
        assert_eq!(ItemType::from_block(69), Some((ItemType::Torch, 1))); // Torch

        // Wheat crops
        assert_eq!(ItemType::from_block(49), Some((ItemType::WheatSeeds, 1)));
        assert_eq!(ItemType::from_block(55), Some((ItemType::WheatSeeds, 1)));
        assert_eq!(ItemType::from_block(56), Some((ItemType::Wheat, 1)));
        assert_eq!(ItemType::from_block(57), Some((ItemType::Carrot, 1)));
        assert_eq!(ItemType::from_block(60), Some((ItemType::Carrot, 1)));
        assert_eq!(ItemType::from_block(61), Some((ItemType::Potato, 1)));
        assert_eq!(ItemType::from_block(64), Some((ItemType::Potato, 1)));
        assert_eq!(ItemType::from_block(103), Some((ItemType::Bookshelf, 1)));

        // Redstone wire drops redstone dust (vanilla-ish).
        assert_eq!(ItemType::from_block(43), Some((ItemType::RedstoneDust, 1)));

        // Redstone repeater drops itself.
        assert_eq!(
            ItemType::from_block(123),
            Some((ItemType::RedstoneRepeater, 1))
        );

        // Redstone comparator drops itself.
        assert_eq!(
            ItemType::from_block(124),
            Some((ItemType::RedstoneComparator, 1))
        );

        // Nether quartz ore drops nether quartz.
        assert_eq!(ItemType::from_block(125), Some((ItemType::NetherQuartz, 1)));

        // Observer drops itself.
        assert_eq!(
            ItemType::from_block(126),
            Some((ItemType::RedstoneObserver, 1))
        );

        // Pistons drop themselves (either base or head).
        assert_eq!(ItemType::from_block(127), Some((ItemType::Piston, 1)));
        assert_eq!(ItemType::from_block(128), Some((ItemType::Piston, 1)));

        assert_eq!(ItemType::from_block(129), Some((ItemType::Dispenser, 1)));
        assert_eq!(ItemType::from_block(130), Some((ItemType::Dropper, 1)));
        assert_eq!(ItemType::from_block(131), Some((ItemType::Hopper, 1)));
    }

    #[test]
    fn test_item_type_to_block() {
        assert_eq!(ItemType::Stone.to_block(), Some(1));
        assert_eq!(ItemType::Dirt.to_block(), Some(2));
        assert_eq!(ItemType::OakLog.to_block(), Some(11));
        assert_eq!(ItemType::OakPlanks.to_block(), Some(12));
        assert_eq!(ItemType::CoalOre.to_block(), Some(14));
        assert_eq!(ItemType::IronOre.to_block(), Some(15));
        assert_eq!(ItemType::GoldOre.to_block(), Some(16));
        assert_eq!(ItemType::DiamondOre.to_block(), Some(17));
        assert_eq!(ItemType::Bed.to_block(), Some(66));
        assert_eq!(ItemType::Bookshelf.to_block(), Some(103));
        assert_eq!(ItemType::CobblestoneWall.to_block(), Some(114));
        assert_eq!(ItemType::IronBars.to_block(), Some(115));
        assert_eq!(ItemType::StoneBricks.to_block(), Some(116));
        assert_eq!(ItemType::StoneBrickSlab.to_block(), Some(117));
        assert_eq!(ItemType::StoneBrickStairs.to_block(), Some(118));
        assert_eq!(ItemType::StoneBrickWall.to_block(), Some(119));
        assert_eq!(ItemType::RedstoneRepeater.to_block(), Some(123));
        assert_eq!(ItemType::RedstoneComparator.to_block(), Some(124));
        assert_eq!(ItemType::RedstoneObserver.to_block(), Some(126));
        assert_eq!(ItemType::Piston.to_block(), Some(127));
        assert_eq!(ItemType::Dispenser.to_block(), Some(129));
        assert_eq!(ItemType::Dropper.to_block(), Some(130));
        assert_eq!(ItemType::Hopper.to_block(), Some(131));

        // Non-placeable items
        assert_eq!(ItemType::RawPork.to_block(), None);
        assert_eq!(ItemType::Apple.to_block(), None);
        assert_eq!(ItemType::Stick.to_block(), None);
    }

    #[test]
    fn test_item_type_from_placeable_block() {
        assert_eq!(ItemType::from_placeable_block(66), Some(ItemType::Bed));
        assert_eq!(ItemType::from_placeable_block(65), Some(ItemType::Bed));
        assert_eq!(
            ItemType::from_placeable_block(103),
            Some(ItemType::Bookshelf)
        );
        assert_eq!(
            ItemType::from_placeable_block(114),
            Some(ItemType::CobblestoneWall)
        );
        assert_eq!(
            ItemType::from_placeable_block(115),
            Some(ItemType::IronBars)
        );
        assert_eq!(
            ItemType::from_placeable_block(116),
            Some(ItemType::StoneBricks)
        );
        assert_eq!(
            ItemType::from_placeable_block(117),
            Some(ItemType::StoneBrickSlab)
        );
        assert_eq!(
            ItemType::from_placeable_block(118),
            Some(ItemType::StoneBrickStairs)
        );
        assert_eq!(
            ItemType::from_placeable_block(119),
            Some(ItemType::StoneBrickWall)
        );
        assert_eq!(
            ItemType::from_placeable_block(123),
            Some(ItemType::RedstoneRepeater)
        );
        assert_eq!(
            ItemType::from_placeable_block(124),
            Some(ItemType::RedstoneComparator)
        );
        assert_eq!(
            ItemType::from_placeable_block(126),
            Some(ItemType::RedstoneObserver)
        );
        assert_eq!(ItemType::from_placeable_block(127), Some(ItemType::Piston));
        assert_eq!(
            ItemType::from_placeable_block(129),
            Some(ItemType::Dispenser)
        );
        assert_eq!(ItemType::from_placeable_block(130), Some(ItemType::Dropper));
        assert_eq!(ItemType::from_placeable_block(131), Some(ItemType::Hopper));
    }

    #[test]
    fn test_leaves_random_drops() {
        // Oak leaves - apple drop (< 0.005)
        assert_eq!(
            ItemType::from_leaves_random(70, 0.001),
            Some((ItemType::Apple, 1))
        );

        // Oak leaves - sapling drop (0.005 to 0.0675)
        assert_eq!(
            ItemType::from_leaves_random(70, 0.01),
            Some((ItemType::Sapling, 1))
        );

        // Oak leaves - no drop (> 0.0675)
        assert_eq!(ItemType::from_leaves_random(70, 0.1), None);

        // Birch leaves - sapling drop
        assert_eq!(
            ItemType::from_leaves_random(72, 0.03),
            Some((ItemType::Sapling, 1))
        );

        // Spruce leaves - sapling drop
        assert_eq!(
            ItemType::from_leaves_random(74, 0.05),
            Some((ItemType::Sapling, 1))
        );

        // Non-leaf block
        assert_eq!(ItemType::from_leaves_random(1, 0.001), None);
    }

    #[test]
    fn test_dropped_item_creation() {
        let item = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 5);

        assert_eq!(item.id, 1);
        assert_eq!(item.x, 10.0);
        assert_eq!(item.y, 64.0);
        assert_eq!(item.z, 20.0);
        assert_eq!(item.item_type, ItemType::Stone);
        assert_eq!(item.count, 5);
        assert_eq!(item.lifetime_ticks, ITEM_DESPAWN_TICKS);
        assert!(!item.on_ground);
    }

    #[test]
    fn test_dropped_item_physics() {
        let mut item = DroppedItem::new(1, 10.0, 70.0, 20.0, ItemType::Stone, 1);
        let ground_height = 64.0;

        // Simulate falling
        for _ in 0..100 {
            if item.update(ground_height) {
                break;
            }
        }

        // Should have landed on ground
        assert!(item.on_ground);
        assert!((item.y - (ground_height + 0.25)).abs() < 0.1);
    }

    #[test]
    fn test_dropped_item_lifetime() {
        let mut item = DroppedItem::new(1, 10.0, 64.25, 20.0, ItemType::Stone, 1);
        item.on_ground = true;
        item.lifetime_ticks = 2;

        assert!(!item.update(64.0)); // Tick 1
        assert!(!item.update(64.0)); // Tick 2
        assert!(item.update(64.0)); // Tick 3 - should despawn
    }

    #[test]
    fn test_item_pickup_radius() {
        let item = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 1);

        // Within range
        assert!(item.can_pickup(10.0, 64.0, 20.0));
        assert!(item.can_pickup(10.5, 64.0, 20.0));
        assert!(item.can_pickup(10.0, 64.5, 20.0));

        // Out of range
        assert!(!item.can_pickup(12.0, 64.0, 20.0));
        assert!(!item.can_pickup(10.0, 70.0, 20.0));
    }

    #[test]
    fn test_item_merge() {
        let mut item1 = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 10);
        let item2 = DroppedItem::new(2, 10.5, 64.0, 20.0, ItemType::Stone, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 5);
        assert_eq!(item1.count, 15);
    }

    #[test]
    fn test_item_merge_different_types() {
        let mut item1 = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 10);
        let item2 = DroppedItem::new(2, 10.5, 64.0, 20.0, ItemType::Dirt, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 0);
        assert_eq!(item1.count, 10);
    }

    #[test]
    fn test_item_merge_stack_limit() {
        let mut item1 = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 62);
        let item2 = DroppedItem::new(2, 10.5, 64.0, 20.0, ItemType::Stone, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 2); // Can only add 2 more (64 - 62)
        assert_eq!(item1.count, 64);
    }

    #[test]
    fn test_item_manager_spawn() {
        let mut manager = ItemManager::new();

        let id1 = manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        let id2 = manager.spawn_item(15.0, 64.0, 25.0, ItemType::Dirt, 3);

        assert_eq!(manager.count(), 2);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_item_manager_update() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 70.0, 20.0, ItemType::Stone, 1);

        let ground_height = |_x: f64, _z: f64| 64.0;

        // Simulate some ticks
        for _ in 0..50 {
            manager.update(ground_height);
        }

        assert_eq!(manager.count(), 1);

        // Item should be on ground now
        let item = manager.get(1).unwrap();
        assert!(item.on_ground);
    }

    #[test]
    fn test_item_manager_despawn() {
        let mut manager = ItemManager::new();
        let id = manager.spawn_item(10.0, 64.25, 20.0, ItemType::Stone, 1);

        if let Some(item) = manager.items.get_mut(&id) {
            item.on_ground = true;
            item.lifetime_ticks = 1;
        }

        let ground_height = |_x: f64, _z: f64| 64.0;

        manager.update(ground_height);
        assert_eq!(manager.count(), 1);

        let despawned = manager.update(ground_height);
        assert_eq!(despawned, 1);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_item_manager_pickup() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        manager.spawn_item(15.0, 64.0, 25.0, ItemType::Dirt, 3);

        // Pickup near first item
        let picked_up = manager.pickup_items(10.0, 64.0, 20.0);
        assert_eq!(picked_up.len(), 1);
        assert_eq!(picked_up[0], (ItemType::Stone, 5));
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_item_manager_take_one_near_picks_lowest_id() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 2);
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Dirt, 2);

        let first = manager.take_one_near(10.0, 64.0, 20.0, 0.01);
        assert_eq!(first, Some((ItemType::Stone, 1)));
        assert_eq!(manager.get(1).unwrap().count, 1);

        let second = manager.take_one_near(10.0, 64.0, 20.0, 0.01);
        assert_eq!(second, Some((ItemType::Stone, 1)));
        assert!(manager.get(1).is_none());

        let third = manager.take_one_near(10.0, 64.0, 20.0, 0.01);
        assert_eq!(third, Some((ItemType::Dirt, 1)));
        assert_eq!(manager.get(2).unwrap().count, 1);
    }

    #[test]
    fn test_item_manager_take_one_near_respects_radius() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 1);
        manager.spawn_item(12.0, 64.0, 20.0, ItemType::Dirt, 1);

        assert_eq!(
            manager.take_one_near(10.0, 64.0, 20.0, 0.5),
            Some((ItemType::Stone, 1))
        );
        assert_eq!(manager.take_one_near(10.0, 64.0, 20.0, 0.5), None);
        assert_eq!(
            manager.take_one_near(12.0, 64.0, 20.0, 0.5),
            Some((ItemType::Dirt, 1))
        );
    }

    #[test]
    fn test_item_manager_merge() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        manager.spawn_item(10.5, 64.0, 20.0, ItemType::Stone, 3);

        let merged = manager.merge_nearby_items();
        assert_eq!(merged, 1);
        assert_eq!(manager.count(), 1);

        // Get the remaining item (should be the first one spawned)
        let items = manager.items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].count, 8);
        assert_eq!(items[0].item_type, ItemType::Stone);
    }

    #[test]
    fn test_item_manager_deterministic_iteration() {
        // BTreeMap provides deterministic iteration order for multiplayer sync
        let mut manager = ItemManager::new();

        // Spawn items (IDs will be 1, 2, 3, 4, 5 in order)
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 1);
        manager.spawn_item(20.0, 64.0, 30.0, ItemType::Dirt, 2);
        manager.spawn_item(30.0, 64.0, 40.0, ItemType::Sand, 3);
        manager.spawn_item(15.0, 64.0, 25.0, ItemType::Ice, 4);
        manager.spawn_item(5.0, 64.0, 10.0, ItemType::Gravel, 5);

        // Collect items multiple times - should always be in same (ID) order
        let order1: Vec<u64> = manager.items().iter().map(|i| i.id).collect();
        let order2: Vec<u64> = manager.items().iter().map(|i| i.id).collect();

        assert_eq!(order1, order2);
        // BTreeMap iterates in key order (ID order)
        assert_eq!(order1, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_silk_touch_drops() {
        // Silk touch should drop the block itself instead of the normal drop
        assert_eq!(ItemType::silk_touch_drop(1), Some((ItemType::Stone, 1))); // Stone instead of cobblestone
        assert_eq!(ItemType::silk_touch_drop(3), Some((ItemType::Grass, 1))); // Grass instead of dirt
        assert_eq!(ItemType::silk_touch_drop(14), Some((ItemType::CoalOre, 1)));
        // Coal ore instead of coal
        assert_eq!(ItemType::silk_touch_drop(7), Some((ItemType::Ice, 1))); // Ice is Silk Touch-only
        assert_eq!(ItemType::silk_touch_drop(25), Some((ItemType::Glass, 1))); // Glass is Silk Touch-only
        assert_eq!(
            ItemType::silk_touch_drop(70),
            Some((ItemType::OakLeaves, 1))
        );
    }

    #[test]
    fn test_fortune_drops() {
        // Fortune should increase drop counts for affected ores
        // Fortune III (level 3) with random value 0.5 should give bonus
        let result = ItemType::fortune_drop(14, 3, 0.5); // Coal ore
        assert!(result.is_some());
        let (item_type, count) = result.unwrap();
        assert_eq!(item_type, ItemType::Coal);
        assert!(count >= 1); // Should have at least base count

        // Non-affected blocks shouldn't get bonus
        let result = ItemType::fortune_drop(2, 3, 0.5); // Dirt
        assert_eq!(result, Some((ItemType::Dirt, 1))); // No bonus
    }

    #[test]
    fn test_potion_item_types() {
        // Test potion-related functions
        assert!(ItemType::PotionHealing.is_potion());
        assert!(ItemType::PotionHealingStrong.is_potion());
        assert!(ItemType::PotionStrength.is_potion());
        assert!(!ItemType::Apple.is_potion());
        assert!(!ItemType::Stone.is_potion());
    }

    #[test]
    fn test_all_potions_is_potion() {
        // All potion types should return true for is_potion()
        assert!(ItemType::PotionAwkward.is_potion());
        assert!(ItemType::PotionNightVision.is_potion());
        assert!(ItemType::PotionInvisibility.is_potion());
        assert!(ItemType::PotionLeaping.is_potion());
        assert!(ItemType::PotionFireResistance.is_potion());
        assert!(ItemType::PotionSwiftness.is_potion());
        assert!(ItemType::PotionSlowness.is_potion());
        assert!(ItemType::PotionWaterBreathing.is_potion());
        assert!(ItemType::PotionHealing.is_potion());
        assert!(ItemType::PotionHarming.is_potion());
        assert!(ItemType::PotionPoison.is_potion());
        assert!(ItemType::PotionRegeneration.is_potion());
        assert!(ItemType::PotionStrength.is_potion());
        assert!(ItemType::PotionWeakness.is_potion());
        assert!(ItemType::PotionSlowFalling.is_potion());
        assert!(ItemType::PotionNightVisionLong.is_potion());
        assert!(ItemType::PotionInvisibilityLong.is_potion());
        assert!(ItemType::PotionLeapingLong.is_potion());
        assert!(ItemType::PotionLeapingStrong.is_potion());
        assert!(ItemType::PotionFireResistanceLong.is_potion());
        assert!(ItemType::PotionSwiftnessLong.is_potion());
        assert!(ItemType::PotionSwiftnessStrong.is_potion());
        assert!(ItemType::PotionSlownessLong.is_potion());
        assert!(ItemType::PotionWaterBreathingLong.is_potion());
        assert!(ItemType::PotionHealingStrong.is_potion());
        assert!(ItemType::PotionHarmingStrong.is_potion());
        assert!(ItemType::PotionPoisonLong.is_potion());
        assert!(ItemType::PotionPoisonStrong.is_potion());
        assert!(ItemType::PotionRegenerationLong.is_potion());
        assert!(ItemType::PotionRegenerationStrong.is_potion());
        assert!(ItemType::PotionStrengthLong.is_potion());
        assert!(ItemType::PotionStrengthStrong.is_potion());
        assert!(ItemType::PotionWeaknessLong.is_potion());
        assert!(ItemType::PotionSlowFallingLong.is_potion());
    }

    #[test]
    fn test_to_potion_type() {
        use crate::PotionType;

        assert_eq!(
            ItemType::PotionAwkward.to_potion_type(),
            Some(PotionType::Awkward)
        );
        assert_eq!(
            ItemType::PotionNightVision.to_potion_type(),
            Some(PotionType::NightVision)
        );
        assert_eq!(
            ItemType::PotionInvisibility.to_potion_type(),
            Some(PotionType::Invisibility)
        );
        assert_eq!(
            ItemType::PotionLeaping.to_potion_type(),
            Some(PotionType::Leaping)
        );
        assert_eq!(
            ItemType::PotionFireResistance.to_potion_type(),
            Some(PotionType::FireResistance)
        );
        assert_eq!(
            ItemType::PotionSwiftness.to_potion_type(),
            Some(PotionType::Swiftness)
        );
        assert_eq!(
            ItemType::PotionSlowness.to_potion_type(),
            Some(PotionType::Slowness)
        );
        assert_eq!(
            ItemType::PotionWaterBreathing.to_potion_type(),
            Some(PotionType::WaterBreathing)
        );
        assert_eq!(
            ItemType::PotionHealing.to_potion_type(),
            Some(PotionType::Healing)
        );
        assert_eq!(
            ItemType::PotionHarming.to_potion_type(),
            Some(PotionType::Harming)
        );
        assert_eq!(
            ItemType::PotionPoison.to_potion_type(),
            Some(PotionType::Poison)
        );
        assert_eq!(
            ItemType::PotionRegeneration.to_potion_type(),
            Some(PotionType::Regeneration)
        );
        assert_eq!(
            ItemType::PotionStrength.to_potion_type(),
            Some(PotionType::Strength)
        );
        assert_eq!(
            ItemType::PotionWeakness.to_potion_type(),
            Some(PotionType::Weakness)
        );
        assert_eq!(
            ItemType::PotionSlowFalling.to_potion_type(),
            Some(PotionType::SlowFalling)
        );
        assert_eq!(
            ItemType::PotionSwiftnessLong.to_potion_type(),
            Some(PotionType::Swiftness)
        );
        assert_eq!(
            ItemType::PotionHealingStrong.to_potion_type(),
            Some(PotionType::Healing)
        );
        assert_eq!(
            ItemType::PotionSlowFallingLong.to_potion_type(),
            Some(PotionType::SlowFalling)
        );

        // Non-potion items should return None
        assert_eq!(ItemType::Apple.to_potion_type(), None);
        assert_eq!(ItemType::Stone.to_potion_type(), None);
    }

    #[test]
    fn test_from_potion_type() {
        use crate::PotionType;

        assert_eq!(
            ItemType::from_potion_type(PotionType::Awkward),
            Some(ItemType::PotionAwkward)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::NightVision),
            Some(ItemType::PotionNightVision)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Invisibility),
            Some(ItemType::PotionInvisibility)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Leaping),
            Some(ItemType::PotionLeaping)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::FireResistance),
            Some(ItemType::PotionFireResistance)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Swiftness),
            Some(ItemType::PotionSwiftness)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Slowness),
            Some(ItemType::PotionSlowness)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::WaterBreathing),
            Some(ItemType::PotionWaterBreathing)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Healing),
            Some(ItemType::PotionHealing)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Harming),
            Some(ItemType::PotionHarming)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Poison),
            Some(ItemType::PotionPoison)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Regeneration),
            Some(ItemType::PotionRegeneration)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Strength),
            Some(ItemType::PotionStrength)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::Weakness),
            Some(ItemType::PotionWeakness)
        );
        assert_eq!(
            ItemType::from_potion_type(PotionType::SlowFalling),
            Some(ItemType::PotionSlowFalling)
        );

        // Base potions have no item type
        assert_eq!(ItemType::from_potion_type(PotionType::Water), None);
        assert_eq!(ItemType::from_potion_type(PotionType::Mundane), None);
        assert_eq!(ItemType::from_potion_type(PotionType::Thick), None);

        // Unimplemented potions
        assert_eq!(ItemType::from_potion_type(PotionType::Luck), None);
    }

    #[test]
    fn test_item_type_id() {
        // Test that ID conversion works
        assert!(ItemType::Stone.id() < u16::MAX);
        assert_ne!(ItemType::Stone.id(), ItemType::Dirt.id());
    }

    #[test]
    fn test_non_stackable_items() {
        // Weapons and armor should stack to 1
        assert_eq!(ItemType::Bow.max_stack_size(), 1);
        assert_eq!(ItemType::LeatherHelmet.max_stack_size(), 1);
        assert_eq!(ItemType::IronChestplate.max_stack_size(), 1);
        assert_eq!(ItemType::GoldLeggings.max_stack_size(), 1);
        assert_eq!(ItemType::DiamondBoots.max_stack_size(), 1);

        // Potions should stack to 1
        assert_eq!(ItemType::PotionHealing.max_stack_size(), 1);
        assert_eq!(ItemType::PotionStrength.max_stack_size(), 1);
        assert_eq!(ItemType::PotionSlowFalling.max_stack_size(), 1);
        assert_eq!(ItemType::SplashPotionSlowFalling.max_stack_size(), 1);
    }

    #[test]
    fn test_item_manager_get() {
        let mut manager = ItemManager::new();
        let id = manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);

        assert!(manager.get(id).is_some());
        assert_eq!(manager.get(id).unwrap().item_type, ItemType::Stone);

        // Non-existent ID
        assert!(manager.get(9999).is_none());
    }

    #[test]
    fn test_dropped_item_serialization() {
        let item = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Diamond, 5);

        let serialized = serde_json::to_string(&item).unwrap();
        let deserialized: DroppedItem = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.item_type, ItemType::Diamond);
        assert_eq!(deserialized.count, 5);
    }

    #[test]
    fn test_item_type_serialization() {
        let item_type = ItemType::DiamondOre;

        let serialized = serde_json::to_string(&item_type).unwrap();
        let deserialized: ItemType = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized, ItemType::DiamondOre);
    }

    #[test]
    fn test_dropped_item_velocity_based_on_id() {
        // Different IDs should have different velocities
        let item1 = DroppedItem::new(1, 0.0, 0.0, 0.0, ItemType::Stone, 1);
        let item2 = DroppedItem::new(100, 0.0, 0.0, 0.0, ItemType::Stone, 1);

        // Velocities should differ (based on ID modulo)
        assert!(
            (item1.vel_x - item2.vel_x).abs() > 0.001 || (item1.vel_z - item2.vel_z).abs() > 0.001
        );
    }

    #[test]
    fn test_item_manager_pickup_removes_items() {
        let mut manager = ItemManager::new();
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        manager.spawn_item(20.0, 64.0, 30.0, ItemType::Dirt, 3);

        assert_eq!(manager.count(), 2);

        // Pickup first item
        let picked = manager.pickup_items(10.0, 64.0, 20.0);
        assert_eq!(picked.len(), 1);
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_item_pickup_out_of_range() {
        let item = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 1);

        // Test various out-of-range positions
        assert!(!item.can_pickup(20.0, 64.0, 20.0)); // Far X
        assert!(!item.can_pickup(10.0, 74.0, 20.0)); // Far Y
        assert!(!item.can_pickup(10.0, 64.0, 30.0)); // Far Z
        assert!(!item.can_pickup(12.0, 66.0, 22.0)); // Combined far
    }

    #[test]
    fn test_merge_nearby_items_distance() {
        // Test that merge_nearby_items respects distance
        let mut manager = ItemManager::new();

        // Spawn two items far apart - they should NOT merge
        manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
        manager.spawn_item(200.0, 64.0, 200.0, ItemType::Stone, 3);

        let merged = manager.merge_nearby_items();
        assert_eq!(merged, 0); // Too far apart to merge
        assert_eq!(manager.count(), 2);
    }

    #[test]
    fn test_to_block_placeable_items() {
        // Test all placeable item types
        assert!(ItemType::Stone.to_block().is_some());
        assert!(ItemType::Dirt.to_block().is_some());
        assert!(ItemType::Grass.to_block().is_some());
        assert!(ItemType::Sand.to_block().is_some());
        assert!(ItemType::Gravel.to_block().is_some());
        assert!(ItemType::Ice.to_block().is_some());
        assert!(ItemType::Snow.to_block().is_some());
        assert!(ItemType::Clay.to_block().is_some());
        assert!(ItemType::OakLog.to_block().is_some());
        assert!(ItemType::OakPlanks.to_block().is_some());
        assert!(ItemType::CraftingTable.to_block().is_some());
        assert!(ItemType::Furnace.to_block().is_some());
        assert!(ItemType::Torch.to_block().is_some());
        assert!(ItemType::Ladder.to_block().is_some());
        assert!(ItemType::Lever.to_block().is_some());
        assert!(ItemType::StoneButton.to_block().is_some());
        assert!(ItemType::OakButton.to_block().is_some());
        assert!(ItemType::StonePressurePlate.to_block().is_some());
        assert!(ItemType::OakPressurePlate.to_block().is_some());
        assert!(ItemType::RedstoneWire.to_block().is_some());
        assert!(ItemType::RedstoneTorch.to_block().is_some());
        assert!(ItemType::RedstoneRepeater.to_block().is_some());
        assert!(ItemType::RedstoneComparator.to_block().is_some());
        assert!(ItemType::RedstoneObserver.to_block().is_some());
    }

    #[test]
    fn test_from_block_ores() {
        // Test ore drops - lapis drops 4-8 items
        let result = ItemType::from_block(98);
        assert!(result.is_some());
        let (item_type, count) = result.unwrap();
        assert_eq!(item_type, ItemType::LapisLazuli);
        assert!((4..=8).contains(&count));
    }
}
