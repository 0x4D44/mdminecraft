use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct BlockDef {
    name: String,
}

fn blocks_json_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/blocks.json")
}

fn load_block_name_to_id() -> HashMap<String, u16> {
    let raw = std::fs::read_to_string(blocks_json_path()).expect("read config/blocks.json");
    let defs: Vec<BlockDef> = serde_json::from_str(&raw).expect("parse config/blocks.json");
    defs.into_iter()
        .enumerate()
        .map(|(idx, def)| (def.name, idx as u16))
        .collect()
}

fn id_of(map: &HashMap<String, u16>, name: &str) -> u16 {
    *map.get(name)
        .unwrap_or_else(|| panic!("missing block name in blocks.json: {name}"))
}

#[test]
fn block_ids_match_blocks_json() {
    let map = load_block_name_to_id();

    // Core terrain / stations.
    assert_eq!(mdminecraft_world::BLOCK_AIR, id_of(&map, "air"));
    assert_eq!(mdminecraft_world::BLOCK_STONE, id_of(&map, "stone"));
    assert_eq!(mdminecraft_world::BLOCK_OAK_LOG, id_of(&map, "oak_log"));
    assert_eq!(
        mdminecraft_world::BLOCK_OAK_PLANKS,
        id_of(&map, "oak_planks")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_CRAFTING_TABLE,
        id_of(&map, "crafting_table")
    );
    assert_eq!(mdminecraft_world::BLOCK_FURNACE, id_of(&map, "furnace"));
    assert_eq!(
        mdminecraft_world::BLOCK_FURNACE_LIT,
        id_of(&map, "furnace_lit")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_COBBLESTONE,
        id_of(&map, "cobblestone")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::GLASS,
        id_of(&map, "glass")
    );

    // Interaction blocks.
    assert_eq!(
        mdminecraft_world::interactive_blocks::OAK_DOOR_LOWER,
        id_of(&map, "oak_door_lower")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::OAK_DOOR_UPPER,
        id_of(&map, "oak_door_upper")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::IRON_DOOR_LOWER,
        id_of(&map, "iron_door_lower")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::IRON_DOOR_UPPER,
        id_of(&map, "iron_door_upper")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::LADDER,
        id_of(&map, "ladder")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::OAK_FENCE,
        id_of(&map, "oak_fence")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::OAK_FENCE_GATE,
        id_of(&map, "oak_fence_gate")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::STONE_SLAB,
        id_of(&map, "stone_slab")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::OAK_SLAB,
        id_of(&map, "oak_slab")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::STONE_STAIRS,
        id_of(&map, "stone_stairs")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::OAK_STAIRS,
        id_of(&map, "oak_stairs")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::GLASS_PANE,
        id_of(&map, "glass_pane")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::BED_HEAD,
        id_of(&map, "bed_head")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::BED_FOOT,
        id_of(&map, "bed_foot")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::CHEST,
        id_of(&map, "chest")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::TRAPDOOR,
        id_of(&map, "trapdoor")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::TORCH,
        id_of(&map, "torch")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::COBBLESTONE_WALL,
        id_of(&map, "cobblestone_wall")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::IRON_BARS,
        id_of(&map, "iron_bars")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_STONE_BRICKS,
        id_of(&map, "stone_bricks")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::STONE_BRICK_SLAB,
        id_of(&map, "stone_brick_slab")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::STONE_BRICK_STAIRS,
        id_of(&map, "stone_brick_stairs")
    );
    assert_eq!(
        mdminecraft_world::interactive_blocks::STONE_BRICK_WALL,
        id_of(&map, "stone_brick_wall")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_DOUBLE_STONE_SLAB,
        id_of(&map, "double_stone_slab")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_DOUBLE_OAK_SLAB,
        id_of(&map, "double_oak_slab")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_DOUBLE_STONE_BRICK_SLAB,
        id_of(&map, "double_stone_brick_slab")
    );

    // Redstone blocks.
    assert_eq!(
        mdminecraft_world::redstone_blocks::LEVER,
        id_of(&map, "lever")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::STONE_BUTTON,
        id_of(&map, "stone_button")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::OAK_BUTTON,
        id_of(&map, "oak_button")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE,
        id_of(&map, "stone_pressure_plate")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE,
        id_of(&map, "oak_pressure_plate")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::REDSTONE_WIRE,
        id_of(&map, "redstone_wire")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::REDSTONE_TORCH,
        id_of(&map, "redstone_torch")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::REDSTONE_LAMP,
        id_of(&map, "redstone_lamp")
    );
    assert_eq!(
        mdminecraft_world::redstone_blocks::REDSTONE_LAMP_LIT,
        id_of(&map, "redstone_lamp_lit")
    );

    // Farming blocks.
    assert_eq!(
        mdminecraft_world::farming_blocks::FARMLAND,
        id_of(&map, "farmland")
    );
    assert_eq!(
        mdminecraft_world::farming_blocks::FARMLAND_WET,
        id_of(&map, "farmland_wet")
    );
    assert_eq!(mdminecraft_world::BLOCK_BOOKSHELF, id_of(&map, "bookshelf"));
    assert_eq!(
        mdminecraft_world::BLOCK_SUGAR_CANE,
        id_of(&map, "sugar_cane")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_BROWN_MUSHROOM,
        id_of(&map, "brown_mushroom")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_MAGMA_CREAM_ORE,
        id_of(&map, "magma_cream_ore")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_GHAST_TEAR_ORE,
        id_of(&map, "ghast_tear_ore")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_GLISTERING_MELON_ORE,
        id_of(&map, "glistering_melon_ore")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_RABBIT_FOOT_ORE,
        id_of(&map, "rabbit_foot_ore")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_PHANTOM_MEMBRANE_ORE,
        id_of(&map, "phantom_membrane_ore")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_REDSTONE_DUST_ORE,
        id_of(&map, "redstone_dust_ore")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_GLOWSTONE_DUST_ORE,
        id_of(&map, "glowstone_dust_ore")
    );
    assert_eq!(
        mdminecraft_world::BLOCK_PUFFERFISH_ORE,
        id_of(&map, "pufferfish_ore")
    );

    let wheat = [
        mdminecraft_world::farming_blocks::WHEAT_0,
        mdminecraft_world::farming_blocks::WHEAT_1,
        mdminecraft_world::farming_blocks::WHEAT_2,
        mdminecraft_world::farming_blocks::WHEAT_3,
        mdminecraft_world::farming_blocks::WHEAT_4,
        mdminecraft_world::farming_blocks::WHEAT_5,
        mdminecraft_world::farming_blocks::WHEAT_6,
        mdminecraft_world::farming_blocks::WHEAT_7,
    ];
    for (stage, actual) in wheat.into_iter().enumerate() {
        assert_eq!(actual, id_of(&map, &format!("wheat_{stage}")));
    }

    let carrots = [
        mdminecraft_world::farming_blocks::CARROTS_0,
        mdminecraft_world::farming_blocks::CARROTS_1,
        mdminecraft_world::farming_blocks::CARROTS_2,
        mdminecraft_world::farming_blocks::CARROTS_3,
    ];
    for (stage, actual) in carrots.into_iter().enumerate() {
        assert_eq!(actual, id_of(&map, &format!("carrots_{stage}")));
    }

    let potatoes = [
        mdminecraft_world::farming_blocks::POTATOES_0,
        mdminecraft_world::farming_blocks::POTATOES_1,
        mdminecraft_world::farming_blocks::POTATOES_2,
        mdminecraft_world::farming_blocks::POTATOES_3,
    ];
    for (stage, actual) in potatoes.into_iter().enumerate() {
        assert_eq!(actual, id_of(&map, &format!("potatoes_{stage}")));
    }
}
