use anyhow::Result;
use mdminecraft_world::{BiomeId, MobType};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};
use tracing::warn;

use crate::content_packs;

const SPAWNS_FILE: &str = "spawns.json";

#[derive(Debug, Deserialize)]
struct PackSpawnOverrideDefinition {
    biome: String,
    mob: String,
    weight: f32,
}

pub fn load_mob_spawn_table_lenient(packs_root: &Path) -> BTreeMap<BiomeId, Vec<(MobType, f32)>> {
    let mut table = mdminecraft_world::default_spawn_table();

    for pack in content_packs::discover_packs_lenient(packs_root) {
        let spawns_path = pack.dir.join(SPAWNS_FILE);
        if !spawns_path.exists() {
            continue;
        }

        let contents = match fs::read_to_string(&spawns_path) {
            Ok(contents) => contents,
            Err(err) => {
                warn!(
                    "Failed to read content pack spawns {}: {err:#}",
                    spawns_path.display()
                );
                continue;
            }
        };

        let defs: Vec<PackSpawnOverrideDefinition> = match serde_json::from_str(&contents) {
            Ok(defs) => defs,
            Err(err) => {
                warn!(
                    "Failed to parse content pack spawns {}: {err:#}",
                    spawns_path.display()
                );
                continue;
            }
        };

        for def in defs {
            if let Err(err) = apply_spawn_override(&mut table, &def) {
                warn!(
                    "Ignoring invalid spawn override from {}: {err:#}",
                    spawns_path.display()
                );
            }
        }
    }

    table
}

fn apply_spawn_override(
    table: &mut BTreeMap<BiomeId, Vec<(MobType, f32)>>,
    def: &PackSpawnOverrideDefinition,
) -> Result<()> {
    let biome = BiomeId::parse(&def.biome)
        .ok_or_else(|| anyhow::anyhow!("Unknown biome '{}'", def.biome))?;
    let mob =
        MobType::parse(&def.mob).ok_or_else(|| anyhow::anyhow!("Unknown mob '{}'", def.mob))?;

    if !def.weight.is_finite() {
        anyhow::bail!(
            "Spawn weight for biome '{}' mob '{}' must be finite",
            biome.as_str(),
            mob.as_str()
        );
    }
    if def.weight < 0.0 {
        anyhow::bail!(
            "Spawn weight for biome '{}' mob '{}' must be >= 0",
            biome.as_str(),
            mob.as_str()
        );
    }

    let mut should_remove_biome = false;
    if def.weight == 0.0 {
        if let Some(list) = table.get_mut(&biome) {
            list.retain(|(t, _)| *t != mob);
            should_remove_biome = list.is_empty();
        }
    } else {
        let list = table.entry(biome).or_default();
        match list.iter_mut().find(|(t, _)| *t == mob) {
            Some((_, weight)) => *weight = def.weight,
            None => list.push((mob, def.weight)),
        }
    }

    if should_remove_biome {
        table.remove(&biome);
    }

    Ok(())
}

#[cfg(test)]
pub fn load_mob_spawn_table_strict_from_root(
    packs_root: &Path,
) -> Result<BTreeMap<BiomeId, Vec<(MobType, f32)>>> {
    use anyhow::Context;

    let mut table = mdminecraft_world::default_spawn_table();

    for pack in content_packs::discover_packs_strict(packs_root)? {
        let spawns_path = pack.dir.join(SPAWNS_FILE);
        if !spawns_path.exists() {
            continue;
        }

        let contents = fs::read_to_string(&spawns_path)
            .with_context(|| format!("Failed to read {}", spawns_path.display()))?;
        let defs: Vec<PackSpawnOverrideDefinition> = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", spawns_path.display()))?;

        for def in defs {
            apply_spawn_override(&mut table, &def)
                .with_context(|| format!("Invalid spawn override in {}", spawns_path.display()))?;
        }
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mdminecraft_pack_spawns_{timestamp}"))
    }

    #[test]
    fn pack_spawn_overrides_apply_in_manifest_order_and_skip_disabled_packs() {
        let packs_root = unique_temp_root();
        fs::create_dir_all(&packs_root).expect("packs root create");

        let low_pack = packs_root.join("low_pack");
        fs::create_dir_all(&low_pack).expect("pack create");
        fs::write(low_pack.join("pack.json"), r#"{"priority":0}"#).expect("write manifest");
        fs::write(
            low_pack.join("spawns.json"),
            r#"[{"biome":"ocean","mob":"chicken","weight":5.0},{"biome":"plains","mob":"villager","weight":100.0}]"#,
        )
        .expect("write spawns");

        let disabled_pack = packs_root.join("disabled_pack");
        fs::create_dir_all(&disabled_pack).expect("pack create");
        fs::write(
            disabled_pack.join("pack.json"),
            r#"{"enabled":false,"priority":5}"#,
        )
        .expect("write manifest");
        fs::write(
            disabled_pack.join("spawns.json"),
            r#"[{"biome":"plains","mob":"cow","weight":0.0}]"#,
        )
        .expect("write spawns");

        let high_pack = packs_root.join("high_pack");
        fs::create_dir_all(&high_pack).expect("pack create");
        fs::write(high_pack.join("pack.json"), r#"{"priority":10}"#).expect("write manifest");
        fs::write(
            high_pack.join("spawns.json"),
            r#"[{"biome":"plains","mob":"villager","weight":0.0}]"#,
        )
        .expect("write spawns");

        let table =
            load_mob_spawn_table_strict_from_root(&packs_root).expect("spawn table should load");

        let ocean = table
            .get(&BiomeId::Ocean)
            .expect("ocean biome should exist");
        assert_eq!(ocean, &vec![(MobType::Chicken, 5.0)]);

        let plains = table
            .get(&BiomeId::Plains)
            .expect("plains biome should exist");
        assert!(
            !plains.iter().any(|(t, _)| *t == MobType::Villager),
            "high priority pack should remove villager spawns"
        );
        assert!(
            plains
                .iter()
                .any(|(t, w)| *t == MobType::Cow && (*w - 8.0).abs() < f32::EPSILON),
            "disabled pack should not remove base cow spawns"
        );
        assert_eq!(
            plains.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
            vec![MobType::Pig, MobType::Cow, MobType::Sheep, MobType::Chicken]
        );

        let _ = fs::remove_dir_all(&packs_root);
    }
}
