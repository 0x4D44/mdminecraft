use mdminecraft_core::{
    item::item_ids, DimensionId, ItemStack as CoreItemStack, ItemType as CoreItemType,
    ToolMaterial, ToolType,
};
use mdminecraft_testkit::{run_micro_worldtest, MicroWorldtestConfig};
use mdminecraft_world::{
    comparator_output_power, get_power_level, is_active, is_door_open,
    lighting::{stitch_light_seams, BlockOpacityProvider, LightType},
    mechanical_blocks, redstone_blocks, set_comparator_facing, set_comparator_output_power,
    set_comparator_subtract_mode, set_hopper_facing, set_hopper_outputs_down, set_observer_facing,
    tick_hoppers, update_container_signal, BlockEntityKey, BlockProperties, BrewingStandState,
    ChestState, Chunk, ChunkPos, DispenserState, Facing, FurnaceState, HopperState, ItemManager,
    ItemType, PotionType, RedstonePos, RedstoneSimulator, Voxel,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
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
fn micro_redstone_observer_lamp_clock_snapshot() {
    #[derive(Default)]
    struct State {
        chunks: HashMap<ChunkPos, Chunk>,
        sim: RedstoneSimulator,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        lamp_lit: bool,
        observer_active: bool,
        output_wire_power: u8,
    }

    let mut chunk = Chunk::new(ChunkPos::new(0, 0));
    chunk.set_voxel(
        6,
        64,
        5,
        Voxel {
            id: redstone_blocks::REDSTONE_LAMP_LIT,
            ..Default::default()
        },
    );

    let mut observer_state = 0;
    observer_state = set_observer_facing(observer_state, Facing::West);
    chunk.set_voxel(
        5,
        64,
        5,
        Voxel {
            id: redstone_blocks::REDSTONE_OBSERVER,
            state: observer_state,
            ..Default::default()
        },
    );

    for (x, z) in [(4, 5), (4, 6), (5, 6), (6, 6)] {
        chunk.set_voxel(
            x,
            64,
            z,
            Voxel {
                id: redstone_blocks::REDSTONE_WIRE,
                ..Default::default()
            },
        );
    }

    let mut state = State::default();
    state.chunks.insert(ChunkPos::new(0, 0), chunk);

    let observer_pos = RedstonePos::new(5, 64, 5);
    let lamp_pos = RedstonePos::new(6, 64, 5);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_redstone_observer_lamp_clock".to_string(),
            ticks: 12,
            snapshot_path: snapshot_path("micro_redstone_observer_lamp_clock.json"),
        },
        state,
        |tick, state| {
            // First initialize the observer fingerprint while the lamp is lit, then schedule the
            // lamp update to turn it off and start the feedback loop.
            if tick.0 == 0 {
                state.sim.schedule_update(observer_pos);
            }
            if tick.0 == 1 {
                state.sim.schedule_update(lamp_pos);
            }
            state.sim.tick(&mut state.chunks);
        },
        |_tick, state| {
            let chunk = state
                .chunks
                .get(&ChunkPos::new(0, 0))
                .expect("chunk exists");
            let lamp = chunk.voxel(6, 64, 5);
            let observer = chunk.voxel(5, 64, 5);
            let output_wire = chunk.voxel(4, 64, 5);

            Snap {
                lamp_lit: lamp.id == redstone_blocks::REDSTONE_LAMP_LIT,
                observer_active: is_active(observer.state),
                output_wire_power: get_power_level(output_wire.state),
            }
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_redstone_two_lever_door_snapshot() {
    #[derive(Default)]
    struct State {
        chunks: HashMap<ChunkPos, Chunk>,
        sim: RedstoneSimulator,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        left_on: bool,
        right_on: bool,
        door_open: bool,
    }

    let mut chunk = Chunk::new(ChunkPos::new(0, 0));
    chunk.set_voxel(
        8,
        64,
        5,
        Voxel {
            id: mdminecraft_world::interactive_blocks::IRON_DOOR_LOWER,
            ..Default::default()
        },
    );
    chunk.set_voxel(
        8,
        65,
        5,
        Voxel {
            id: mdminecraft_world::interactive_blocks::IRON_DOOR_UPPER,
            ..Default::default()
        },
    );

    chunk.set_voxel(
        7,
        64,
        5,
        Voxel {
            id: redstone_blocks::LEVER,
            ..Default::default()
        },
    );
    chunk.set_voxel(
        9,
        64,
        5,
        Voxel {
            id: redstone_blocks::LEVER,
            ..Default::default()
        },
    );

    let mut state = State::default();
    state.chunks.insert(ChunkPos::new(0, 0), chunk);

    let left_pos = RedstonePos::new(7, 64, 5);
    let right_pos = RedstonePos::new(9, 64, 5);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_redstone_two_lever_door".to_string(),
            ticks: 5,
            snapshot_path: snapshot_path("micro_redstone_two_lever_door.json"),
        },
        state,
        |tick, state| {
            match tick.0 {
                0 => state.sim.toggle_lever(left_pos, &mut state.chunks),
                1 => state.sim.toggle_lever(right_pos, &mut state.chunks),
                2 => state.sim.toggle_lever(left_pos, &mut state.chunks),
                3 => state.sim.toggle_lever(right_pos, &mut state.chunks),
                _ => {}
            }
            state.sim.tick(&mut state.chunks);
        },
        |_tick, state| {
            let chunk = state
                .chunks
                .get(&ChunkPos::new(0, 0))
                .expect("chunk exists");
            let left = chunk.voxel(7, 64, 5);
            let right = chunk.voxel(9, 64, 5);
            let door = chunk.voxel(8, 64, 5);

            Snap {
                left_on: is_active(left.state),
                right_on: is_active(right.state),
                door_open: is_door_open(door.state),
            }
        },
    )
    .expect("snapshot verified");
}

#[test]
fn micro_item_sorter_lite_snapshot() {
    struct State {
        chunks: HashMap<ChunkPos, Chunk>,
        redstone: RedstoneSimulator,
        chests: BTreeMap<BlockEntityKey, ChestState>,
        hoppers: BTreeMap<BlockEntityKey, HopperState>,
        dispensers: BTreeMap<BlockEntityKey, DispenserState>,
        droppers: BTreeMap<BlockEntityKey, DispenserState>,
        dropped_items: ItemManager,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        control_signal: u8,
        comparator_output: u8,
        hopper_locked: bool,
        hopper_items: u32,
        output_chest_items: u32,
    }

    fn count_core_slots(slots: &[Option<CoreItemStack>]) -> u32 {
        slots
            .iter()
            .filter_map(|slot| slot.as_ref().map(|s| s.count))
            .sum()
    }

    let control_pos = RedstonePos::new(8, 64, 5);
    let comparator_pos = RedstonePos::new(9, 64, 5);
    let hopper_pos = RedstonePos::new(10, 64, 5);
    let output_chest_pos = RedstonePos::new(10, 64, 6);

    let control_key = BlockEntityKey {
        dimension: DimensionId::Overworld,
        x: control_pos.x,
        y: control_pos.y,
        z: control_pos.z,
    };
    let hopper_key = BlockEntityKey {
        dimension: DimensionId::Overworld,
        x: hopper_pos.x,
        y: hopper_pos.y,
        z: hopper_pos.z,
    };
    let output_chest_key = BlockEntityKey {
        dimension: DimensionId::Overworld,
        x: output_chest_pos.x,
        y: output_chest_pos.y,
        z: output_chest_pos.z,
    };

    let mut chunk = Chunk::new(ChunkPos::new(0, 0));
    chunk.set_voxel(
        control_pos.x as usize,
        control_pos.y as usize,
        control_pos.z as usize,
        Voxel {
            id: mdminecraft_world::interactive_blocks::CHEST,
            ..Default::default()
        },
    );

    let mut comparator_state = 0;
    comparator_state = set_comparator_facing(comparator_state, Facing::East);
    comparator_state = set_comparator_subtract_mode(comparator_state, false);
    comparator_state = set_comparator_output_power(comparator_state, 1);
    chunk.set_voxel(
        comparator_pos.x as usize,
        comparator_pos.y as usize,
        comparator_pos.z as usize,
        Voxel {
            id: redstone_blocks::REDSTONE_COMPARATOR,
            state: comparator_state,
            ..Default::default()
        },
    );

    let mut hopper_state = 0;
    hopper_state = set_hopper_facing(hopper_state, Facing::South);
    hopper_state = set_hopper_outputs_down(hopper_state, false);
    chunk.set_voxel(
        hopper_pos.x as usize,
        hopper_pos.y as usize,
        hopper_pos.z as usize,
        Voxel {
            id: mechanical_blocks::HOPPER,
            state: hopper_state,
            ..Default::default()
        },
    );

    chunk.set_voxel(
        output_chest_pos.x as usize,
        output_chest_pos.y as usize,
        output_chest_pos.z as usize,
        Voxel {
            id: mdminecraft_world::interactive_blocks::CHEST,
            ..Default::default()
        },
    );

    let mut chests = BTreeMap::new();
    let mut control_chest = ChestState::default();
    control_chest.slots[0] = Some(CoreItemStack::new(
        CoreItemType::Block(mdminecraft_world::BLOCK_COBBLESTONE),
        1,
    ));
    chests.insert(control_key, control_chest);
    chests.insert(output_chest_key, ChestState::default());

    let mut hoppers = BTreeMap::new();
    let mut hopper = HopperState::default();
    hopper.slots[0] = Some(CoreItemStack::new(
        CoreItemType::Block(mdminecraft_world::BLOCK_COBBLESTONE),
        1,
    ));
    hoppers.insert(hopper_key, hopper);

    let mut state = State {
        chunks: HashMap::new(),
        redstone: RedstoneSimulator::new(),
        chests,
        hoppers,
        dispensers: BTreeMap::new(),
        droppers: BTreeMap::new(),
        dropped_items: ItemManager::new(),
    };
    state.chunks.insert(ChunkPos::new(0, 0), chunk);

    {
        let slots = &state
            .chests
            .get(&control_key)
            .expect("control chest exists")
            .slots;
        update_container_signal(&mut state.chunks, &mut state.redstone, control_pos, slots);
    }
    state.redstone.schedule_update(hopper_pos);

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_item_sorter_lite".to_string(),
            ticks: 20,
            snapshot_path: snapshot_path("micro_item_sorter_lite.json"),
        },
        state,
        |tick, state| {
            if tick.0 == 5 {
                let chest = state.chests.get_mut(&control_key).expect("control exists");
                chest.slots = std::array::from_fn(|_| None);
                update_container_signal(
                    &mut state.chunks,
                    &mut state.redstone,
                    control_pos,
                    &chest.slots,
                );
            }

            state.redstone.tick(&mut state.chunks);
            tick_hoppers(
                mdminecraft_world::HopperTickContext {
                    chunks: &mut state.chunks,
                    redstone_sim: &mut state.redstone,
                    item_manager: &mut state.dropped_items,
                    chests: &mut state.chests,
                    hoppers: &mut state.hoppers,
                    dispensers: &mut state.dispensers,
                    droppers: &mut state.droppers,
                },
                |_| None,
            );
        },
        |_tick, state| {
            let chunk = state
                .chunks
                .get(&ChunkPos::new(0, 0))
                .expect("chunk exists");
            let control_voxel = chunk.voxel(
                control_pos.x as usize,
                control_pos.y as usize,
                control_pos.z as usize,
            );
            let comparator_voxel = chunk.voxel(
                comparator_pos.x as usize,
                comparator_pos.y as usize,
                comparator_pos.z as usize,
            );
            let hopper_voxel = chunk.voxel(
                hopper_pos.x as usize,
                hopper_pos.y as usize,
                hopper_pos.z as usize,
            );

            let hopper_items = state
                .hoppers
                .get(&hopper_key)
                .map(|hopper| count_core_slots(&hopper.slots))
                .unwrap_or(0);
            let output_chest_items = state
                .chests
                .get(&output_chest_key)
                .map(|chest| count_core_slots(&chest.slots))
                .unwrap_or(0);

            Snap {
                control_signal: get_power_level(control_voxel.state),
                comparator_output: comparator_output_power(comparator_voxel.state),
                hopper_locked: is_active(hopper_voxel.state),
                hopper_items,
                output_chest_items,
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
