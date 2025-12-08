use std::collections::HashMap;

use mdminecraft_world::BlockOpacityProvider;

use crate::BlockTextureConfig;

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
    /// Human-readable identifier (e.g., "stone").
    pub name: String,
    /// Whether the block blocks light/vision.
    pub opaque: bool,
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
    pub fn from_definition(def: crate::BlockDefinition) -> Self {
        let base_name = def.texture.clone().unwrap_or_else(|| def.name.clone());
        let textures = BlockTextures::from_config(&base_name, def.textures);
        let name = def.name;
        let harvest_level = def.harvest_level.and_then(|s| HarvestLevel::parse(&s));
        Self {
            name,
            opaque: def.opaque,
            textures,
            harvest_level,
        }
    }

    /// Helper for tests/examples that need a simple descriptor.
    pub fn simple(name: &str, opaque: bool) -> Self {
        Self::from_definition(crate::BlockDefinition {
            name: name.to_string(),
            opaque,
            texture: None,
            textures: None,
            harvest_level: None,
        })
    }
}

/// Registry storing block descriptors keyed by id.
pub struct BlockRegistry {
    descriptors: Vec<BlockDescriptor>,
    name_to_id: HashMap<String, u16>,
}

impl BlockRegistry {
    /// Construct a registry from the supplied descriptors.
    pub fn new(descriptors: Vec<BlockDescriptor>) -> Self {
        let mut name_to_id = HashMap::new();
        for (id, desc) in descriptors.iter().enumerate() {
            name_to_id.insert(desc.name.clone(), id as u16);
        }
        Self {
            descriptors,
            name_to_id,
        }
    }

    /// Look up a descriptor by numeric id.
    pub fn descriptor(&self, id: u16) -> Option<&BlockDescriptor> {
        self.descriptors.get(id as usize)
    }

    /// Resolve a block id by its name.
    pub fn id_by_name(&self, name: &str) -> Option<u16> {
        self.name_to_id.get(name).copied()
    }

    /// Get the harvest level required for a block (None = no tool required).
    pub fn harvest_level(&self, block_id: u16) -> Option<HarvestLevel> {
        self.descriptor(block_id).and_then(|d| d.harvest_level)
    }
}

impl BlockOpacityProvider for BlockRegistry {
    fn is_opaque(&self, block_id: u16) -> bool {
        self.descriptor(block_id).map(|d| d.opaque).unwrap_or(false)
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
}
