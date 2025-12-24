use std::collections::HashMap;

use mdminecraft_core::RegistryKey;
use mdminecraft_world::BlockOpacityProvider;

use crate::AssetError;
use crate::BlockTextureConfig;
use std::collections::BTreeSet;

/// Minimum tool tier required to successfully harvest a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HarvestLevel {
    /// Wooden tools or better
    Wood = 0,
    /// Stone tools or better
    Stone = 1,
    /// Iron tools or better
    Iron = 2,
    /// Diamond tools required
    Diamond = 3,
}

impl HarvestLevel {
    /// Parse a harvest level from a string (e.g., "wood", "stone", "iron", "diamond").
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "wood" => Some(HarvestLevel::Wood),
            "stone" => Some(HarvestLevel::Stone),
            "iron" => Some(HarvestLevel::Iron),
            "diamond" => Some(HarvestLevel::Diamond),
            _ => None,
        }
    }

    /// Get the numeric tier value (0-3).
    /// This matches the values returned by ToolMaterial::harvest_tier() in the core crate.
    pub fn tier(self) -> u8 {
        self as u8
    }

    /// Check if a tool harvest tier can successfully harvest blocks with this requirement.
    /// Returns true if tool_tier >= required tier.
    pub fn can_harvest_with_tier(self, tool_tier: u8) -> bool {
        tool_tier >= self.tier()
    }
}

/// Block metadata loaded from packs.
#[derive(Debug, Clone)]
pub struct BlockDescriptor {
    /// Stable namespaced registry key (e.g., "mdm:stone").
    pub key: RegistryKey,
    /// Human-readable identifier (e.g., "stone").
    pub name: String,
    /// Whether the block blocks light/vision.
    pub opaque: bool,
    /// How strongly this block attenuates light passing through it (0-15).
    ///
    /// - `0` means fully transparent (light passes through).
    /// - `15` means fully opaque (light does not pass through).
    pub light_opacity: u8,
    /// Block-light emitted by this block (0-15).
    pub light_emission: u8,
    /// Tag keys applied to this block.
    pub tags: BTreeSet<RegistryKey>,
    textures: BlockTextures,
    /// Required tool tier to harvest this block (None = no tool required).
    pub harvest_level: Option<HarvestLevel>,
}

impl BlockDescriptor {
    /// Resolve the atlas entry for the supplied face.
    pub fn texture_for(&self, face: BlockFace) -> &str {
        self.textures.texture_for(face)
    }

    /// Construct descriptor from the JSON definition.
    pub fn try_from_definition(def: crate::BlockDefinition) -> Result<Self, AssetError> {
        let raw_key = def.key.as_deref().unwrap_or(&def.name);
        let key = RegistryKey::parse(raw_key)
            .map_err(|err| AssetError::InvalidRegistryKey(err.to_string()))?;

        // Display name remains the short "path" part.
        let name = key.path().to_string();

        let base_name = def.texture.clone().unwrap_or_else(|| name.clone());
        let textures = BlockTextures::from_config(&base_name, def.textures);
        let harvest_level = def.harvest_level.and_then(|s| HarvestLevel::parse(&s));
        let light_opacity = match def.light_opacity {
            Some(value) if value <= 15 => value,
            Some(value) => return Err(AssetError::InvalidLightOpacity(value)),
            None => {
                if def.opaque {
                    15
                } else {
                    0
                }
            }
        };
        let light_emission = match def.light_emission {
            Some(value) if value <= 15 => value,
            Some(value) => return Err(AssetError::InvalidLightEmission(value)),
            None => {
                if def.emissive.unwrap_or(false) {
                    15
                } else {
                    0
                }
            }
        };

        let mut tags = BTreeSet::new();
        for raw_tag in def.tags {
            let tag = RegistryKey::parse(&raw_tag)
                .map_err(|err| AssetError::InvalidTagKey(err.to_string()))?;
            tags.insert(tag);
        }

        Ok(Self {
            key,
            name,
            opaque: def.opaque,
            light_opacity,
            light_emission,
            tags,
            textures,
            harvest_level,
        })
    }

    /// Construct descriptor from the JSON definition.
    ///
    /// Prefer [`BlockDescriptor::try_from_definition`] when loading untrusted pack data.
    pub fn from_definition(def: crate::BlockDefinition) -> Self {
        Self::try_from_definition(def).expect("invalid BlockDefinition")
    }

    /// Helper for tests/examples that need a simple descriptor.
    pub fn simple(name: &str, opaque: bool) -> Self {
        Self::from_definition(crate::BlockDefinition {
            name: name.to_string(),
            key: None,
            tags: Vec::new(),
            opaque,
            light_opacity: None,
            light_emission: None,
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        })
    }
}

/// Registry storing block descriptors keyed by id.
pub struct BlockRegistry {
    descriptors: Vec<BlockDescriptor>,
    key_to_id: HashMap<RegistryKey, u16>,
}

impl BlockRegistry {
    /// Construct a registry from the supplied descriptors.
    pub fn new(descriptors: Vec<BlockDescriptor>) -> Self {
        let mut key_to_id = HashMap::new();
        for (id, desc) in descriptors.iter().enumerate() {
            key_to_id.insert(desc.key.clone(), id as u16);
        }
        Self {
            descriptors,
            key_to_id,
        }
    }

    /// Look up a descriptor by numeric id.
    pub fn descriptor(&self, id: u16) -> Option<&BlockDescriptor> {
        self.descriptors.get(id as usize)
    }

    /// Resolve a block id by its name.
    pub fn id_by_name(&self, name: &str) -> Option<u16> {
        let key = RegistryKey::parse(name).ok()?;
        self.id_by_key(&key)
    }

    /// Resolve a block id by its registry key.
    pub fn id_by_key(&self, key: &RegistryKey) -> Option<u16> {
        self.key_to_id.get(key).copied()
    }

    /// Get the harvest level required for a block (None = no tool required).
    pub fn harvest_level(&self, block_id: u16) -> Option<HarvestLevel> {
        self.descriptor(block_id).and_then(|d| d.harvest_level)
    }

    /// Get the registry key for a numeric block id.
    pub fn key_by_id(&self, id: u16) -> Option<&RegistryKey> {
        self.descriptor(id).map(|d| &d.key)
    }

    /// Return whether the given block has the supplied tag.
    pub fn has_tag(&self, block_id: u16, tag: &RegistryKey) -> bool {
        self.descriptor(block_id)
            .is_some_and(|descriptor| descriptor.tags.contains(tag))
    }

    /// List all block ids that have a specific tag.
    pub fn blocks_with_tag(&self, tag: &RegistryKey) -> Vec<u16> {
        self.descriptors
            .iter()
            .enumerate()
            .filter_map(|(id, descriptor)| descriptor.tags.contains(tag).then_some(id as u16))
            .collect()
    }
}

impl BlockOpacityProvider for BlockRegistry {
    fn light_opacity(&self, block_id: u16) -> u8 {
        self.descriptor(block_id)
            .map(|d| d.light_opacity)
            .unwrap_or(15)
    }

    fn base_block_light_emission(&self, block_id: u16) -> u8 {
        self.descriptor(block_id)
            .map(|d| d.light_emission)
            .unwrap_or(0)
    }
}

/// Faces corresponding to the block's six sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFace {
    /// Positive Y / top face.
    Up,
    /// Negative Y / bottom face.
    Down,
    /// Negative Z face.
    North,
    /// Positive Z face.
    South,
    /// Positive X face.
    East,
    /// Negative X face.
    West,
}

#[derive(Debug, Clone)]
struct BlockTextures {
    up: String,
    down: String,
    north: String,
    south: String,
    east: String,
    west: String,
}

impl BlockTextures {
    fn uniform(name: &str) -> Self {
        Self {
            up: name.to_string(),
            down: name.to_string(),
            north: name.to_string(),
            south: name.to_string(),
            east: name.to_string(),
            west: name.to_string(),
        }
    }

    fn from_config(base: &str, config: Option<BlockTextureConfig>) -> Self {
        let mut textures = Self::uniform(base);
        if let Some(cfg) = config {
            if let Some(all) = cfg.all.as_ref() {
                textures.set_all(all);
            }
            if let Some(side) = cfg.side.as_ref() {
                textures.set_sides(side);
            }
            if let Some(top) = cfg.top.as_ref() {
                textures.up = top.clone();
            }
            if let Some(bottom) = cfg.bottom.as_ref() {
                textures.down = bottom.clone();
            }
            if let Some(north) = cfg.north.as_ref() {
                textures.north = north.clone();
            }
            if let Some(south) = cfg.south.as_ref() {
                textures.south = south.clone();
            }
            if let Some(east) = cfg.east.as_ref() {
                textures.east = east.clone();
            }
            if let Some(west) = cfg.west.as_ref() {
                textures.west = west.clone();
            }
        }
        textures
    }

    fn texture_for(&self, face: BlockFace) -> &str {
        match face {
            BlockFace::Up => &self.up,
            BlockFace::Down => &self.down,
            BlockFace::North => &self.north,
            BlockFace::South => &self.south,
            BlockFace::East => &self.east,
            BlockFace::West => &self.west,
        }
    }

    fn set_all(&mut self, value: &str) {
        let val = value.to_string();
        self.up = val.clone();
        self.down = val.clone();
        self.north = val.clone();
        self.south = val.clone();
        self.east = val.clone();
        self.west = val;
    }

    fn set_sides(&mut self, value: &str) {
        let val = value.to_string();
        self.north = val.clone();
        self.south = val.clone();
        self.east = val.clone();
        self.west = val;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockDefinition;

    #[test]
    fn test_harvest_level_tier() {
        // Verify tier values match enum discriminants
        assert_eq!(HarvestLevel::Wood.tier(), 0);
        assert_eq!(HarvestLevel::Stone.tier(), 1);
        assert_eq!(HarvestLevel::Iron.tier(), 2);
        assert_eq!(HarvestLevel::Diamond.tier(), 3);
    }

    #[test]
    fn test_harvest_level_parse() {
        // Test parsing from strings
        assert_eq!(HarvestLevel::parse("wood"), Some(HarvestLevel::Wood));
        assert_eq!(HarvestLevel::parse("stone"), Some(HarvestLevel::Stone));
        assert_eq!(HarvestLevel::parse("iron"), Some(HarvestLevel::Iron));
        assert_eq!(HarvestLevel::parse("diamond"), Some(HarvestLevel::Diamond));

        // Test case insensitivity
        assert_eq!(HarvestLevel::parse("WOOD"), Some(HarvestLevel::Wood));
        assert_eq!(HarvestLevel::parse("Stone"), Some(HarvestLevel::Stone));

        // Test invalid input
        assert_eq!(HarvestLevel::parse("invalid"), None);
        assert_eq!(HarvestLevel::parse(""), None);
    }

    #[test]
    fn test_can_harvest_with_tier() {
        // Wood tier (0) can harvest wood-level blocks
        assert!(HarvestLevel::Wood.can_harvest_with_tier(0));
        assert!(HarvestLevel::Wood.can_harvest_with_tier(1));
        assert!(HarvestLevel::Wood.can_harvest_with_tier(2));
        assert!(HarvestLevel::Wood.can_harvest_with_tier(3));

        // Stone tier (1) requires stone tools or better
        assert!(!HarvestLevel::Stone.can_harvest_with_tier(0));
        assert!(HarvestLevel::Stone.can_harvest_with_tier(1));
        assert!(HarvestLevel::Stone.can_harvest_with_tier(2));
        assert!(HarvestLevel::Stone.can_harvest_with_tier(3));

        // Iron tier (2) requires iron tools or better
        assert!(!HarvestLevel::Iron.can_harvest_with_tier(0));
        assert!(!HarvestLevel::Iron.can_harvest_with_tier(1));
        assert!(HarvestLevel::Iron.can_harvest_with_tier(2));
        assert!(HarvestLevel::Iron.can_harvest_with_tier(3));

        // Diamond tier (3) requires diamond tools
        assert!(!HarvestLevel::Diamond.can_harvest_with_tier(0));
        assert!(!HarvestLevel::Diamond.can_harvest_with_tier(1));
        assert!(!HarvestLevel::Diamond.can_harvest_with_tier(2));
        assert!(HarvestLevel::Diamond.can_harvest_with_tier(3));
    }

    #[test]
    fn test_harvest_level_ordering() {
        // Verify enum ordering matches tier progression
        assert!(HarvestLevel::Diamond > HarvestLevel::Iron);
        assert!(HarvestLevel::Iron > HarvestLevel::Stone);
        assert!(HarvestLevel::Stone > HarvestLevel::Wood);
    }

    #[test]
    fn test_block_tags_query() {
        let defs = vec![
            BlockDefinition {
                name: "air".to_string(),
                key: None,
                tags: Vec::new(),
                opaque: false,
                light_opacity: None,
                light_emission: None,
                emissive: None,
                texture: None,
                textures: None,
                harvest_level: None,
            },
            BlockDefinition {
                name: "stone".to_string(),
                key: Some("mdm:stone".to_string()),
                tags: vec!["mdm:mineable/pickaxe".to_string()],
                opaque: true,
                light_opacity: None,
                light_emission: None,
                emissive: None,
                texture: None,
                textures: None,
                harvest_level: Some("wood".to_string()),
            },
        ];

        let descriptors = defs
            .into_iter()
            .map(BlockDescriptor::try_from_definition)
            .collect::<Result<Vec<_>, _>>()
            .expect("valid definitions");
        let registry = BlockRegistry::new(descriptors);

        let tag = RegistryKey::parse("mdm:mineable/pickaxe").unwrap();
        assert!(!registry.has_tag(0, &tag));
        assert!(registry.has_tag(1, &tag));
        assert_eq!(registry.blocks_with_tag(&tag), vec![1]);
    }

    #[test]
    fn test_light_opacity_defaults_follow_opaque_flag() {
        let stone = BlockDescriptor::try_from_definition(BlockDefinition {
            name: "stone".to_string(),
            key: None,
            tags: Vec::new(),
            opaque: true,
            light_opacity: None,
            light_emission: None,
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        })
        .expect("stone parses");
        assert_eq!(stone.light_opacity, 15);

        let glass = BlockDescriptor::try_from_definition(BlockDefinition {
            name: "glass".to_string(),
            key: None,
            tags: Vec::new(),
            opaque: false,
            light_opacity: None,
            light_emission: None,
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        })
        .expect("glass parses");
        assert_eq!(glass.light_opacity, 0);
    }

    #[test]
    fn test_light_emission_defaults_to_zero() {
        let stone = BlockDescriptor::try_from_definition(BlockDefinition {
            name: "stone".to_string(),
            key: None,
            tags: Vec::new(),
            opaque: true,
            light_opacity: None,
            light_emission: None,
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        })
        .expect("stone parses");
        assert_eq!(stone.light_emission, 0);
    }

    #[test]
    fn test_emissive_flag_falls_back_to_full_light_emission() {
        let torch = BlockDescriptor::try_from_definition(BlockDefinition {
            name: "torch".to_string(),
            key: None,
            tags: Vec::new(),
            opaque: false,
            light_opacity: None,
            light_emission: None,
            emissive: Some(true),
            texture: None,
            textures: None,
            harvest_level: None,
        })
        .expect("torch parses");
        assert_eq!(torch.light_emission, 15);

        let dim = BlockDescriptor::try_from_definition(BlockDefinition {
            name: "dim_torch".to_string(),
            key: None,
            tags: Vec::new(),
            opaque: false,
            light_opacity: None,
            light_emission: Some(7),
            emissive: Some(true),
            texture: None,
            textures: None,
            harvest_level: None,
        })
        .expect("dim torch parses");
        assert_eq!(dim.light_emission, 7);
    }

    #[test]
    fn test_invalid_light_emission_errors() {
        let def = BlockDefinition {
            name: "torch".to_string(),
            key: None,
            tags: Vec::new(),
            opaque: false,
            light_opacity: None,
            light_emission: Some(16),
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        };

        let err = BlockDescriptor::try_from_definition(def).unwrap_err();
        match err {
            AssetError::InvalidLightEmission(16) => {}
            other => panic!("expected InvalidLightEmission(16), got {other:?}"),
        }
    }

    #[test]
    fn test_invalid_light_opacity_errors() {
        let def = BlockDefinition {
            name: "stone".to_string(),
            key: None,
            tags: Vec::new(),
            opaque: true,
            light_opacity: Some(16),
            light_emission: None,
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        };

        let err = BlockDescriptor::try_from_definition(def).unwrap_err();
        match err {
            AssetError::InvalidLightOpacity(16) => {}
            other => panic!("expected InvalidLightOpacity(16), got {other:?}"),
        }
    }

    #[test]
    fn test_invalid_tag_key_errors() {
        let def = BlockDefinition {
            name: "stone".to_string(),
            key: None,
            tags: vec!["NotAllowed".to_string()],
            opaque: true,
            light_opacity: None,
            light_emission: None,
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        };

        let err = BlockDescriptor::try_from_definition(def).unwrap_err();
        match err {
            AssetError::InvalidTagKey(_) => {}
            other => panic!("expected InvalidTagKey, got {other:?}"),
        }
    }
}
