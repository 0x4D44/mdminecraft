use anyhow::{Context, Result};
use mdminecraft_assets::BlockRegistry;
use mdminecraft_core::{item::FoodType, ItemType};
use mdminecraft_world::{BlockId, ItemType as DroppedItemType, MobType};
use rand::Rng;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use tracing::warn;

use crate::content_packs;

const LOOT_FILE: &str = "loot.json";

#[derive(Debug, Clone, Default)]
pub struct LootTables {
    pub block: BTreeMap<BlockId, LootTable>,
    pub mob: BTreeMap<MobType, LootTable>,
}

#[derive(Debug, Clone, Default)]
pub struct LootTable {
    drops: Vec<LootDrop>,
}

#[derive(Debug, Clone)]
struct LootDrop {
    item: DroppedItemType,
    min: u32,
    max: u32,
    chance_out_of_2p32: u64,
}

impl LootTable {
    pub fn roll(&self, rng: &mut impl Rng) -> Vec<(DroppedItemType, u32)> {
        let mut drops = Vec::new();
        for entry in &self.drops {
            if entry.chance_out_of_2p32 == 0 {
                continue;
            }

            let roll = rng.gen::<u32>() as u64;
            if roll >= entry.chance_out_of_2p32 {
                continue;
            }

            let count = if entry.min == entry.max {
                entry.min
            } else {
                rng.gen_range(entry.min..=entry.max)
            };

            if count > 0 {
                drops.push((entry.item, count));
            }
        }
        drops
    }
}

#[derive(Debug, Deserialize)]
struct PackLootFile {
    #[serde(default)]
    blocks: Vec<PackBlockLootDefinition>,
    #[serde(default)]
    mobs: Vec<PackMobLootDefinition>,
}

#[derive(Debug, Deserialize)]
struct PackBlockLootDefinition {
    block: String,
    #[serde(default)]
    drops: Vec<PackLootDropDefinition>,
}

#[derive(Debug, Deserialize)]
struct PackMobLootDefinition {
    mob: String,
    #[serde(default)]
    drops: Vec<PackLootDropDefinition>,
}

#[derive(Debug, Deserialize)]
struct PackLootDropDefinition {
    item: String,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    min: Option<u32>,
    #[serde(default)]
    max: Option<u32>,
    #[serde(default)]
    chance: Option<f32>,
}

pub fn load_loot_tables_lenient(packs_root: &Path, blocks: &BlockRegistry) -> LootTables {
    let mut tables = LootTables::default();

    for pack in content_packs::discover_packs_lenient(packs_root) {
        let loot_path = pack.dir.join(LOOT_FILE);
        if !loot_path.exists() {
            continue;
        }

        let contents = match fs::read_to_string(&loot_path) {
            Ok(contents) => contents,
            Err(err) => {
                warn!(
                    "Failed to read content pack loot {}: {err:#}",
                    loot_path.display()
                );
                continue;
            }
        };

        let file: PackLootFile = match serde_json::from_str(&contents) {
            Ok(file) => file,
            Err(err) => {
                warn!(
                    "Failed to parse content pack loot {}: {err:#}",
                    loot_path.display()
                );
                continue;
            }
        };

        if let Err(err) = apply_loot_file(&mut tables, &file, blocks, &loot_path) {
            warn!(
                "Ignoring invalid loot definitions from {}: {err:#}",
                loot_path.display()
            );
        }
    }

    tables
}

fn apply_loot_file(
    tables: &mut LootTables,
    file: &PackLootFile,
    blocks: &BlockRegistry,
    source: &Path,
) -> Result<()> {
    let mut seen_blocks = BTreeSet::new();
    for def in &file.blocks {
        let block_id = parse_block_id(&def.block, blocks).with_context(|| {
            format!(
                "Invalid block loot entry '{}' in {}",
                def.block,
                source.display()
            )
        })?;

        if !seen_blocks.insert(block_id) {
            anyhow::bail!(
                "Duplicate block loot entry for '{}' in {}",
                def.block,
                source.display()
            );
        }

        let table = parse_loot_table(&def.drops, blocks).with_context(|| {
            format!(
                "Invalid drops for block '{}' in {}",
                def.block,
                source.display()
            )
        })?;
        tables.block.insert(block_id, table);
    }

    let mut seen_mobs = BTreeSet::new();
    for def in &file.mobs {
        let mob = parse_pack_mob_type(&def.mob)
            .ok_or_else(|| anyhow::anyhow!("Unknown mob '{}'", def.mob))?;

        if !seen_mobs.insert(mob) {
            anyhow::bail!(
                "Duplicate mob loot entry for '{}' in {}",
                def.mob,
                source.display()
            );
        }

        let table = parse_loot_table(&def.drops, blocks).with_context(|| {
            format!(
                "Invalid drops for mob '{}' in {}",
                def.mob,
                source.display()
            )
        })?;
        tables.mob.insert(mob, table);
    }

    Ok(())
}

fn parse_block_id(token: &str, blocks: &BlockRegistry) -> Result<BlockId> {
    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("Block key cannot be empty");
    }

    if let Ok(id) = token.parse::<u16>() {
        return Ok(id);
    }

    let token = token.strip_prefix("block:").unwrap_or(token);
    if let Ok(id) = token.parse::<u16>() {
        return Ok(id);
    }

    blocks
        .id_by_name(token)
        .or_else(|| {
            token
                .strip_prefix("minecraft:")
                .filter(|rest| !rest.contains(':'))
                .and_then(|rest| blocks.id_by_name(rest))
        })
        .ok_or_else(|| anyhow::anyhow!("Unknown block '{}'", token))
}

fn parse_pack_mob_type(token: &str) -> Option<MobType> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    MobType::parse(token).or_else(|| {
        token
            .strip_prefix("minecraft:")
            .or_else(|| token.strip_prefix("mdm:"))
            .and_then(MobType::parse)
    })
}

fn parse_loot_table(defs: &[PackLootDropDefinition], blocks: &BlockRegistry) -> Result<LootTable> {
    let mut drops = Vec::with_capacity(defs.len());
    for def in defs {
        drops.push(parse_loot_drop(def, blocks)?);
    }
    Ok(LootTable { drops })
}

fn parse_loot_drop(def: &PackLootDropDefinition, blocks: &BlockRegistry) -> Result<LootDrop> {
    let token = def.item.trim();
    if token.is_empty() {
        anyhow::bail!("Drop item cannot be empty");
    }

    let core_item = parse_core_item_type(token, blocks)
        .ok_or_else(|| anyhow::anyhow!("Unknown item token '{}'", token))?;
    let item =
        crate::game::GameWorld::convert_core_item_type_to_dropped(core_item).ok_or_else(|| {
            anyhow::anyhow!(
                "Item token '{}' cannot be represented as a dropped item",
                token
            )
        })?;

    if def.count.is_some() && (def.min.is_some() || def.max.is_some()) {
        anyhow::bail!("Use either 'count' or 'min'/'max' for item '{}'", token);
    }

    let (min, max) = match (def.count, def.min, def.max) {
        (Some(count), None, None) => (count, count),
        (None, min, max) => {
            let min = min.unwrap_or(1);
            let max = max.unwrap_or(min);
            if max < min {
                anyhow::bail!("Drop item '{}' has max {} < min {}", token, max, min);
            }
            (min, max)
        }
        _ => unreachable!("validated above"),
    };

    let chance = def.chance.unwrap_or(1.0);
    if !chance.is_finite() {
        anyhow::bail!("Drop item '{}' chance must be finite", token);
    }
    if !(0.0..=1.0).contains(&chance) {
        anyhow::bail!("Drop item '{}' chance must be in [0, 1]", token);
    }

    let chance_out_of_2p32 = (chance as f64 * (u32::MAX as f64 + 1.0)) as u64;

    Ok(LootDrop {
        item,
        min,
        max,
        chance_out_of_2p32,
    })
}

fn parse_core_item_type(token: &str, blocks: &BlockRegistry) -> Option<ItemType> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    // Convenience: bare token is a block key if known.
    if !token.contains(':') {
        if let Some(block_id) = blocks.id_by_name(token) {
            return Some(ItemType::Block(block_id));
        }
    }

    if let Some(item) = mdminecraft_assets::parse_item_type_with_blocks(token, Some(blocks)) {
        return Some(item);
    }

    // Extra syntaxes for loot definitions.
    if let Some(rest) = token.strip_prefix("food:") {
        return parse_food(rest);
    }
    if let Some(rest) = token.strip_prefix("potion:") {
        let id = rest.trim().parse::<u16>().ok()?;
        return Some(ItemType::Potion(id));
    }
    if let Some(rest) = token.strip_prefix("splash_potion:") {
        let id = rest.trim().parse::<u16>().ok()?;
        return Some(ItemType::SplashPotion(id));
    }

    None
}

fn parse_food(token: &str) -> Option<ItemType> {
    let food = match token.trim().to_ascii_lowercase().as_str() {
        "apple" => FoodType::Apple,
        "bread" => FoodType::Bread,
        "raw_meat" | "rawmeat" => FoodType::RawMeat,
        "cooked_meat" | "cookedmeat" => FoodType::CookedMeat,
        "carrot" => FoodType::Carrot,
        "potato" => FoodType::Potato,
        "baked_potato" | "bakedpotato" => FoodType::BakedPotato,
        "golden_carrot" | "goldencarrot" => FoodType::GoldenCarrot,
        _ => return None,
    };
    Some(ItemType::Food(food))
}

#[cfg(test)]
pub fn load_loot_tables_strict_from_root(
    packs_root: &Path,
    blocks: &BlockRegistry,
) -> Result<LootTables> {
    let mut tables = LootTables::default();

    for pack in content_packs::discover_packs_strict(packs_root)? {
        let loot_path = pack.dir.join(LOOT_FILE);
        if !loot_path.exists() {
            continue;
        }

        let contents = fs::read_to_string(&loot_path)
            .with_context(|| format!("Failed to read {}", loot_path.display()))?;
        let file: PackLootFile = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", loot_path.display()))?;
        apply_loot_file(&mut tables, &file, blocks, &loot_path)?;
    }

    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_assets::BlockDescriptor;
    use rand::{rngs::StdRng, SeedableRng};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mdminecraft_pack_loot_{timestamp}"))
    }

    fn test_block_registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor::simple("air", false),
            BlockDescriptor::simple("stone", true),
            BlockDescriptor::simple("dirt", true),
            BlockDescriptor::simple("grass", true),
        ])
    }

    #[test]
    fn loot_overrides_apply_in_manifest_order_and_skip_disabled_packs() {
        let packs_root = unique_temp_root();
        fs::create_dir_all(&packs_root).expect("packs root create");
        let blocks = test_block_registry();

        let low_pack = packs_root.join("low_pack");
        fs::create_dir_all(&low_pack).expect("pack create");
        fs::write(low_pack.join("pack.json"), r#"{"priority":0}"#).expect("write manifest");
        fs::write(
            low_pack.join("loot.json"),
            r#"{"blocks":[{"block":"stone","drops":[{"item":"item:7","count":1}]}],"mobs":[{"mob":"zombie","drops":[{"item":"item:7","count":1}]}]}"#,
        )
        .expect("write loot");

        let disabled_pack = packs_root.join("disabled_pack");
        fs::create_dir_all(&disabled_pack).expect("pack create");
        fs::write(
            disabled_pack.join("pack.json"),
            r#"{"enabled":false,"priority":5}"#,
        )
        .expect("write manifest");
        fs::write(
            disabled_pack.join("loot.json"),
            r#"{"blocks":[{"block":"stone","drops":[{"item":"item:8","count":1}]}]}"#,
        )
        .expect("write loot");

        let high_pack = packs_root.join("high_pack");
        fs::create_dir_all(&high_pack).expect("pack create");
        fs::write(high_pack.join("pack.json"), r#"{"priority":10}"#).expect("write manifest");
        fs::write(
            high_pack.join("loot.json"),
            r#"{"blocks":[{"block":"stone","drops":[{"item":"item:9","count":1}]}]}"#,
        )
        .expect("write loot");

        let tables =
            load_loot_tables_strict_from_root(&packs_root, &blocks).expect("loot should load");

        let stone_id = blocks.id_by_name("stone").expect("stone id");
        let block_table = tables.block.get(&stone_id).expect("stone loot table");
        let mut rng = StdRng::seed_from_u64(123);
        assert_eq!(
            block_table.roll(&mut rng),
            vec![(DroppedItemType::GoldIngot, 1)],
            "high priority pack should override block loot"
        );

        let mob_table = tables.mob.get(&MobType::Zombie).expect("zombie loot table");
        let mut rng = StdRng::seed_from_u64(123);
        assert_eq!(
            mob_table.roll(&mut rng),
            vec![(DroppedItemType::IronIngot, 1)],
            "disabled pack should be ignored for mob overrides"
        );

        let _ = fs::remove_dir_all(&packs_root);
    }

    #[test]
    fn loot_drop_validation_catches_unknown_items() {
        let packs_root = unique_temp_root();
        fs::create_dir_all(&packs_root).expect("packs root create");
        let blocks = test_block_registry();

        let pack = packs_root.join("bad_pack");
        fs::create_dir_all(&pack).expect("pack create");
        fs::write(
            pack.join("loot.json"),
            r#"{"blocks":[{"block":"stone","drops":[{"item":"nope","count":1}]}]}"#,
        )
        .expect("write loot");

        let err = load_loot_tables_strict_from_root(&packs_root, &blocks)
            .expect_err("invalid loot should error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Unknown item token"),
            "expected unknown item error, got: {msg}"
        );

        let _ = fs::remove_dir_all(&packs_root);
    }

    #[test]
    fn loot_parses_minecraft_namespaced_block_and_mob_keys() {
        let blocks = test_block_registry();
        let stone_id = blocks.id_by_name("stone").expect("stone id");

        assert_eq!(
            super::parse_block_id("minecraft:stone", &blocks).expect("minecraft block key"),
            stone_id
        );
        assert_eq!(
            super::parse_block_id("block:minecraft:stone", &blocks).expect("minecraft block: key"),
            stone_id
        );
        assert_eq!(
            super::parse_pack_mob_type("minecraft:zombie"),
            Some(MobType::Zombie)
        );
    }
}
