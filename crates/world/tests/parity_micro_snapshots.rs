use mdminecraft_core::{item::item_ids, ToolMaterial, ToolType};
use mdminecraft_testkit::{run_micro_worldtest, MicroWorldtestConfig};
use mdminecraft_world::{
    get_power_level, is_active,
    lighting::{stitch_light_seams, BlockOpacityProvider, LightType},
    redstone_blocks, BlockProperties, BrewingStandState, Chunk, ChunkPos, FurnaceState, ItemType,
    PotionType, RedstonePos, RedstoneSimulator, Voxel,
};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name)
}

#[test]
fn micro_redstone_wire_propagation_snapshot() {
    #[derive(Default)]
    struct State {
        chunks: HashMap<ChunkPos, Chunk>,
        sim: RedstoneSimulator,
        lever_toggled: bool,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        lever_active: bool,
        lever_power: u8,
        wire1_power: u8,
        wire2_power: u8,
        lamp_lit: bool,
    }

    let mut chunk = Chunk::new(ChunkPos::new(0, 0));
    chunk.set_voxel(
        5,
        64,
        5,
        Voxel {
            id: redstone_blocks::LEVER,
            ..Default::default()
        },
    );
    chunk.set_voxel(
        6,
        64,
        5,
        Voxel {
            id: redstone_blocks::REDSTONE_WIRE,
            ..Default::default()
        },
    );
    chunk.set_voxel(
        7,
        64,
        5,
        Voxel {
            id: redstone_blocks::REDSTONE_WIRE,
            ..Default::default()
        },
    );
    chunk.set_voxel(
        8,
        64,
        5,
        Voxel {
            id: redstone_blocks::REDSTONE_LAMP,
            ..Default::default()
        },
    );

    let mut state = State::default();
    state.chunks.insert(ChunkPos::new(0, 0), chunk);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_redstone_wire_propagation".to_string(),
            ticks: 3,
            snapshot_path: snapshot_path("micro_redstone_wire_propagation.json"),
        },
        state,
        |tick, state| {
            if tick.0 == 0 && !state.lever_toggled {
                state
                    .sim
                    .toggle_lever(RedstonePos::new(5, 64, 5), &mut state.chunks);
                state.lever_toggled = true;
            }
            state.sim.tick(&mut state.chunks);
        },
        |_tick, state| {
            let chunk = state
                .chunks
                .get(&ChunkPos::new(0, 0))
                .expect("chunk exists");
            let lever = chunk.voxel(5, 64, 5);
            let wire1 = chunk.voxel(6, 64, 5);
            let wire2 = chunk.voxel(7, 64, 5);
            let lamp = chunk.voxel(8, 64, 5);

            Snap {
                lever_active: is_active(lever.state),
                lever_power: get_power_level(lever.state),
                wire1_power: get_power_level(wire1.state),
                wire2_power: get_power_level(wire2.state),
                lamp_lit: lamp.id == redstone_blocks::REDSTONE_LAMP_LIT,
            }
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_furnace_single_smelt_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        furnace: FurnaceState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        is_lit: bool,
        input: Option<(ItemType, u32)>,
        fuel: Option<(ItemType, u32)>,
        output: Option<(ItemType, u32)>,
        smelt_progress_milli: i32,
        fuel_remaining_milli: i32,
    }

    let mut furnace = FurnaceState::new();
    assert_eq!(furnace.add_input(ItemType::IronOre, 1), 0);
    assert_eq!(furnace.add_fuel(ItemType::Coal, 1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_furnace_single_smelt".to_string(),
            ticks: 220,
            snapshot_path: snapshot_path("micro_furnace_single_smelt.json"),
        },
        State { furnace },
        |_tick, state| {
            state.furnace.update(0.05);
        },
        |_tick, state| {
            let smelt_progress_milli = (state.furnace.smelt_progress * 1000.0).round() as i32;
            let fuel_remaining_milli = (state.furnace.fuel_remaining * 1000.0).round() as i32;
            Snap {
                is_lit: state.furnace.is_lit,
                input: state.furnace.input,
                fuel: state.furnace.fuel,
                output: state.furnace.output,
                smelt_progress_milli,
                fuel_remaining_milli,
            }
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_water_to_awkward_to_strength_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
        staged_strength: bool,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Water));
    }
    assert_eq!(stand.add_ingredient(item_ids::NETHER_WART, 1), 0);
    assert_eq!(stand.add_fuel(2), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_water_to_awkward_to_strength".to_string(),
            ticks: 90,
            snapshot_path: snapshot_path("micro_brewing_water_to_awkward_to_strength.json"),
        },
        State {
            stand,
            staged_strength: false,
        },
        |_tick, state| {
            let completed = state.stand.update(0.5);
            if completed
                && !state.staged_strength
                && state
                    .stand
                    .bottles
                    .iter()
                    .all(|b| *b == Some(PotionType::Awkward))
            {
                // Stage the second brew: Awkward + Blaze Powder -> Strength.
                let remainder = state.stand.add_ingredient(item_ids::BLAZE_POWDER, 1);
                assert_eq!(remainder, 0);
                state.staged_strength = true;
            }
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_gunpowder_to_splash_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Swiftness));
    }
    assert_eq!(stand.add_ingredient(item_ids::GUNPOWDER, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_gunpowder_to_splash".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_gunpowder_to_splash.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_spider_eye_to_poison_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::SPIDER_EYE, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_spider_eye_to_poison".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_awkward_spider_eye_to_poison.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_sugar_to_swiftness_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::SUGAR, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_sugar_to_swiftness".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_awkward_sugar_to_swiftness.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_swiftness_fermented_spider_eye_to_slowness_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Swiftness));
    }
    assert_eq!(stand.add_ingredient(item_ids::FERMENTED_SPIDER_EYE, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_swiftness_fermented_spider_eye_to_slowness".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path(
                "micro_brewing_swiftness_fermented_spider_eye_to_slowness.json",
            ),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_magma_cream_to_fire_resistance_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::MAGMA_CREAM, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_magma_cream_to_fire_resistance".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path(
                "micro_brewing_awkward_magma_cream_to_fire_resistance.json",
            ),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_ghast_tear_to_regeneration_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::GHAST_TEAR, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_ghast_tear_to_regeneration".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_awkward_ghast_tear_to_regeneration.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_glistering_melon_to_healing_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::GLISTERING_MELON, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_glistering_melon_to_healing".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_awkward_glistering_melon_to_healing.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_rabbit_foot_to_leaping_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::RABBIT_FOOT, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_rabbit_foot_to_leaping".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_awkward_rabbit_foot_to_leaping.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_phantom_membrane_to_slow_falling_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::PHANTOM_MEMBRANE, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_phantom_membrane_to_slow_falling".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path(
                "micro_brewing_awkward_phantom_membrane_to_slow_falling.json",
            ),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_swiftness_redstone_to_long_swiftness_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        bottle_is_extended: [bool; 3],
        bottle_amplifier: [u8; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Swiftness));
    }
    assert_eq!(stand.add_ingredient(item_ids::REDSTONE_DUST, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_swiftness_redstone_to_long_swiftness".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_swiftness_redstone_to_long_swiftness.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            bottle_is_extended: state.stand.bottle_is_extended,
            bottle_amplifier: state.stand.bottle_amplifier,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_healing_glowstone_to_strong_healing_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        bottle_is_extended: [bool; 3],
        bottle_amplifier: [u8; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Healing));
    }
    assert_eq!(stand.add_ingredient(item_ids::GLOWSTONE_DUST, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_healing_glowstone_to_strong_healing".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path("micro_brewing_healing_glowstone_to_strong_healing.json"),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            bottle_is_extended: state.stand.bottle_is_extended,
            bottle_amplifier: state.stand.bottle_amplifier,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_brewing_awkward_pufferfish_to_water_breathing_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        stand: BrewingStandState,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        bottles: [Option<PotionType>; 3],
        bottle_is_splash: [bool; 3],
        bottle_is_extended: [bool; 3],
        bottle_amplifier: [u8; 3],
        ingredient: Option<(u16, u32)>,
        fuel: u32,
        is_brewing: bool,
        brew_progress_milli: i32,
    }

    let mut stand = BrewingStandState::new();
    for slot in 0..3 {
        assert!(stand.add_bottle(slot, PotionType::Awkward));
    }
    assert_eq!(stand.add_ingredient(item_ids::PUFFERFISH, 1), 0);
    assert_eq!(stand.add_fuel(1), 0);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_brewing_awkward_pufferfish_to_water_breathing".to_string(),
            ticks: 45,
            snapshot_path: snapshot_path(
                "micro_brewing_awkward_pufferfish_to_water_breathing.json",
            ),
        },
        State { stand },
        |_tick, state| {
            state.stand.update(0.5);
        },
        |_tick, state| Snap {
            bottles: state.stand.bottles,
            bottle_is_splash: state.stand.bottle_is_splash,
            bottle_is_extended: state.stand.bottle_is_extended,
            bottle_amplifier: state.stand.bottle_amplifier,
            ingredient: state.stand.ingredient,
            fuel: state.stand.fuel,
            is_brewing: state.stand.is_brewing,
            brew_progress_milli: (state.stand.brew_progress * 1000.0).round() as i32,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_mining_stone_drops_snapshot() {
    #[derive(Debug, Clone)]
    struct State {
        time_mining: f32,
        time_required: f32,
        mined: bool,
        drop: Option<(ItemType, u32)>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        mined: bool,
        time_mining_milli: i32,
        time_required_milli: i32,
        drop: Option<(ItemType, u32)>,
    }

    let block_props = BlockProperties::stone();
    let tool = Some((ToolType::Pickaxe, ToolMaterial::Wood));
    let time_required = block_props.calculate_mining_time(tool, false);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_mining_stone_drops".to_string(),
            ticks: 30,
            snapshot_path: snapshot_path("micro_mining_stone_drops.json"),
        },
        State {
            time_mining: 0.0,
            time_required,
            mined: false,
            drop: None,
        },
        |_tick, state| {
            if state.mined {
                return;
            }
            state.time_mining += 0.05;
            if state.time_mining >= state.time_required {
                state.mined = true;
                state.drop = ItemType::from_block(1);
            }
        },
        |_tick, state| Snap {
            mined: state.mined,
            time_mining_milli: (state.time_mining * 1000.0).round() as i32,
            time_required_milli: (state.time_required * 1000.0).round() as i32,
            drop: state.drop,
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_lighting_seam_stitch_snapshot() {
    #[derive(Debug, Default, Clone)]
    struct Registry;

    impl BlockOpacityProvider for Registry {
        fn is_opaque(&self, block_id: u16) -> bool {
            block_id != 0
        }
    }

    #[derive(Default)]
    struct State {
        chunks: HashMap<ChunkPos, Chunk>,
        registry: Registry,
        last_processed: usize,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        processed_nodes: usize,
        source_light: u8,
        seam_light: u8,
        interior_light_1: u8,
        interior_light_2: u8,
    }

    let pos_a = ChunkPos::new(0, 0);
    let pos_b = ChunkPos::new(1, 0);
    let mut chunk_a = Chunk::new(pos_a);
    let chunk_b = Chunk::new(pos_b);

    // Seed block light on the east edge of chunk A (air voxel with blocklight).
    let y = 8usize;
    let z = 8usize;
    chunk_a.set_voxel(
        15,
        y,
        z,
        Voxel {
            id: 0,
            state: 0,
            light_sky: 0,
            light_block: 15,
        },
    );

    let mut state = State::default();
    state.chunks.insert(pos_a, chunk_a);
    state.chunks.insert(pos_b, chunk_b);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_lighting_seam_stitch".to_string(),
            ticks: 2,
            snapshot_path: snapshot_path("micro_lighting_seam_stitch.json"),
        },
        state,
        |_tick, state| {
            state.last_processed = stitch_light_seams(
                &mut state.chunks,
                &state.registry,
                pos_a,
                LightType::BlockLight,
            );
        },
        |_tick, state| {
            let chunk_a = state.chunks.get(&pos_a).expect("chunk A exists");
            let chunk_b = state.chunks.get(&pos_b).expect("chunk B exists");
            Snap {
                processed_nodes: state.last_processed,
                source_light: chunk_a.voxel(15, y, z).light_block,
                seam_light: chunk_b.voxel(0, y, z).light_block,
                interior_light_1: chunk_b.voxel(1, y, z).light_block,
                interior_light_2: chunk_b.voxel(2, y, z).light_block,
            }
        },
    )
    .expect("snapshot verified");
}
