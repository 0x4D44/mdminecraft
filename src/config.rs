use anyhow::Result;
use mdminecraft_assets::{registry_from_file, BlockDescriptor, BlockRegistry};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use tracing::warn;

const DEFAULT_CONTROLS_PATH: &str = "config/controls.toml";
const DEFAULT_BLOCKS_PATH: &str = "config/blocks.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ControlsConfig {
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    /// Field of view in degrees.
    pub fov_degrees: f32,
    /// Chunk radius used for loading/unloading the world around the player.
    pub render_distance: i32,
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
    load_block_registry_from_path(Path::new(DEFAULT_BLOCKS_PATH))
}

fn load_block_registry_from_path(path: &Path) -> BlockRegistry {
    match registry_from_file(path) {
        Ok(registry) => registry,
        Err(err) => {
            warn!(
                "Failed to load block pack {}: {err}. Using defaults",
                path.display()
            );
            default_block_registry()
        }
    }
}

fn default_block_registry() -> BlockRegistry {
    BlockRegistry::new(vec![
        BlockDescriptor::simple("air", false),
        BlockDescriptor::simple("stone", true),
        BlockDescriptor::simple("dirt", true),
        BlockDescriptor::simple("grass", true),
    ])
}
