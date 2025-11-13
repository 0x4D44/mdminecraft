use std::collections::HashMap;

use serde::Deserialize;

/// Block metadata loaded from packs.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockDescriptor {
    /// Human-readable identifier (e.g., "stone").
    pub name: String,
    /// Whether the block blocks light/vision.
    #[serde(default)]
    pub opaque: bool,
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
}
