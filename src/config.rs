use anyhow::Result;
use mdminecraft_assets::{BlockDescriptor, BlockRegistry};
use mdminecraft_core::RegistryKey;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::Path,
};
use tracing::warn;

use crate::content_packs;

const DEFAULT_CONTROLS_PATH: &str = "config/controls.toml";
const DEFAULT_BLOCKS_PATH: &str = "config/blocks.json";
const DEFAULT_CONTENT_PACKS_DIR: &str = content_packs::CONTENT_PACKS_DIR;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ControlsConfig {
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    /// Field of view in degrees.
    pub fov_degrees: f32,
    /// Chunk radius used for loading/unloading the world around the player.
    pub render_distance: i32,
    /// Master volume (0.0 to 1.0).
    pub master_volume: f32,
    /// Music volume (0.0 to 1.0).
    pub music_volume: f32,
    /// Sound effects volume (0.0 to 1.0).
    pub sfx_volume: f32,
    /// Ambient sounds volume (0.0 to 1.0).
    pub ambient_volume: f32,
    /// Whether audio is muted.
    pub audio_muted: bool,
    pub bindings: BindingOverrides,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct BindingOverrides {
    pub base: HashMap<String, Vec<String>>,
    pub gameplay: HashMap<String, Vec<String>>,
    pub ui: HashMap<String, Vec<String>>,
}

impl Default for ControlsConfig {
    fn default() -> Self {
        Self {
            // Sensitivity of 0.006 means ~0.34° per pixel of mouse movement
            // Range: 0.001 (very slow) to 0.01 (fast)
            mouse_sensitivity: 0.006,
            invert_y: false,
            fov_degrees: 70.0,
            render_distance: 8,
            master_volume: 1.0,
            music_volume: 0.5,
            sfx_volume: 1.0,
            ambient_volume: 0.7,
            audio_muted: false,
            bindings: BindingOverrides::default(),
        }
    }
}

impl ControlsConfig {
    /// Load controls configuration from the default path.
    pub fn load() -> Self {
        Self::load_from_path(Path::new(DEFAULT_CONTROLS_PATH))
    }

    /// Load configuration from an explicit path, falling back to defaults on errors.
    pub fn load_from_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(contents) => match toml::from_str::<ControlsConfig>(&contents) {
                Ok(cfg) => cfg,
                Err(err) => {
                    warn!("Failed to parse {}: {err}. Using defaults", path.display());
                    ControlsConfig::default()
                }
            },
            Err(err) => {
                if path != Path::new(DEFAULT_CONTROLS_PATH) {
                    warn!("Failed to read {}: {err}. Using defaults", path.display());
                } else if err.kind() != std::io::ErrorKind::NotFound {
                    warn!("Failed to read {}: {err}. Using defaults", path.display());
                } else {
                    warn!(
                        "Controls config not found at {}. Using defaults",
                        path.display()
                    );
                }
                ControlsConfig::default()
            }
        }
    }

    /// Save controls configuration to the default path.
    pub fn save(&self) -> Result<()> {
        self.save_to_path(Path::new(DEFAULT_CONTROLS_PATH))
    }

    /// Save controls configuration to an explicit path.
    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        let toml = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml)?;
        Ok(())
    }
}

/// Load block registry from JSON definition, falling back to defaults.
pub fn load_block_registry() -> BlockRegistry {
    load_block_registry_lenient(
        Path::new(DEFAULT_BLOCKS_PATH),
        Path::new(DEFAULT_CONTENT_PACKS_DIR),
    )
}

fn default_block_registry() -> BlockRegistry {
    BlockRegistry::new(vec![
        BlockDescriptor::simple("air", false),
        BlockDescriptor::simple("stone", true),
        BlockDescriptor::simple("dirt", true),
        BlockDescriptor::simple("grass", true),
    ])
}

/// Load the block registry from base config + content packs, returning errors to the caller.
///
/// This is intended for tests/validation. The game uses [`load_block_registry`] instead, which
/// logs and skips invalid packs.
#[cfg(test)]
pub fn load_block_registry_strict() -> Result<BlockRegistry> {
    load_block_registry_strict_from_paths(
        Path::new(DEFAULT_BLOCKS_PATH),
        Path::new(DEFAULT_CONTENT_PACKS_DIR),
    )
}

fn load_block_registry_lenient(base_path: &Path, packs_root: &Path) -> BlockRegistry {
    let mut descriptors = match load_block_descriptors_from_file(base_path) {
        Ok(descriptors) => descriptors,
        Err(err) => {
            warn!(
                "Failed to load block pack {}: {err:#}. Using defaults",
                base_path.display()
            );
            return default_block_registry();
        }
    };

    let mut used_keys: BTreeSet<RegistryKey> = BTreeSet::new();
    for descriptor in &descriptors {
        if !used_keys.insert(descriptor.key.clone()) {
            warn!(
                "Duplicate block key {} while loading {}",
                descriptor.key,
                base_path.display()
            );
        }
    }

    for pack in content_packs::discover_packs_lenient(packs_root) {
        let blocks_path = pack.dir.join("blocks.json");
        if !blocks_path.exists() {
            continue;
        }

        match load_block_descriptors_from_file(&blocks_path) {
            Ok(pack_descriptors) => {
                for descriptor in pack_descriptors {
                    if !used_keys.insert(descriptor.key.clone()) {
                        warn!(
                            "Ignoring duplicate block key {} from {}",
                            descriptor.key,
                            blocks_path.display()
                        );
                        continue;
                    }
                    descriptors.push(descriptor);
                }
            }
            Err(err) => {
                warn!(
                    "Failed to load content pack blocks {}: {err:#}",
                    blocks_path.display()
                );
            }
        }
    }

    BlockRegistry::new(descriptors)
}

#[cfg(test)]
fn load_block_registry_strict_from_paths(
    base_path: &Path,
    packs_root: &Path,
) -> Result<BlockRegistry> {
    let mut descriptors = load_block_descriptors_from_file(base_path)?;
    let mut used_keys: BTreeSet<RegistryKey> = BTreeSet::new();
    for descriptor in &descriptors {
        if !used_keys.insert(descriptor.key.clone()) {
            anyhow::bail!(
                "Duplicate block key {} while loading {}",
                descriptor.key,
                base_path.display()
            );
        }
    }

    for pack in content_packs::discover_packs_strict(packs_root)? {
        let blocks_path = pack.dir.join("blocks.json");
        if !blocks_path.exists() {
            continue;
        }

        for descriptor in load_block_descriptors_from_file(&blocks_path)? {
            if !used_keys.insert(descriptor.key.clone()) {
                anyhow::bail!(
                    "Duplicate block key {} while loading {}",
                    descriptor.key,
                    blocks_path.display()
                );
            }
            descriptors.push(descriptor);
        }
    }

    Ok(BlockRegistry::new(descriptors))
}

fn load_block_descriptors_from_file(path: &Path) -> Result<Vec<BlockDescriptor>> {
    let contents = fs::read_to_string(path)?;
    let defs = mdminecraft_assets::load_blocks_from_str(&contents)?;
    let mut descriptors = Vec::with_capacity(defs.len());
    for def in defs {
        descriptors.push(BlockDescriptor::try_from_definition(def)?);
    }
    Ok(descriptors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_assets::BlockFace;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn content_pack_blocks_load_and_append_deterministically() {
        let registry = load_block_registry_strict().expect("block registry should load");

        // Base registry IDs stay stable (as defined by config/blocks.json).
        assert_eq!(registry.id_by_name("stone"), Some(1));

        // Example pack block is present and uses the expected metadata.
        let id = registry
            .id_by_name("example:polished_stone")
            .expect("example_pack block should be registered");
        let desc = registry.descriptor(id).expect("descriptor should exist");
        assert_eq!(desc.name, "polished_stone");
        assert!(desc.opaque);
        assert_eq!(desc.texture_for(BlockFace::Up), "blocks/stone");
    }

    #[test]
    fn disabled_packs_are_ignored_when_loading_block_registry() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let packs_root = std::env::temp_dir().join(format!("mdminecraft_pack_blocks_{timestamp}"));
        fs::create_dir_all(&packs_root).expect("packs root create");

        let enabled_pack = packs_root.join("enabled_pack");
        fs::create_dir_all(&enabled_pack).expect("enabled pack create");
        fs::write(
            enabled_pack.join("blocks.json"),
            r#"[{"name":"enabled_block","key":"test:enabled_block","opaque":true,"texture":"blocks/stone"}]"#,
        )
        .expect("write enabled blocks");

        let disabled_pack = packs_root.join("disabled_pack");
        fs::create_dir_all(&disabled_pack).expect("disabled pack create");
        fs::write(disabled_pack.join("pack.json"), r#"{"enabled":false}"#)
            .expect("write disabled manifest");
        fs::write(
            disabled_pack.join("blocks.json"),
            r#"[{"name":"disabled_block","key":"test:disabled_block","opaque":true,"texture":"blocks/stone"}]"#,
        )
        .expect("write disabled blocks");

        let registry =
            load_block_registry_strict_from_paths(Path::new(DEFAULT_BLOCKS_PATH), &packs_root)
                .expect("registry loads with custom packs root");

        assert!(
            registry.id_by_name("test:enabled_block").is_some(),
            "enabled pack block should be present"
        );
        assert!(
            registry.id_by_name("test:disabled_block").is_none(),
            "disabled pack block should not be present"
        );

        let _ = fs::remove_dir_all(&packs_root);
    }
}
