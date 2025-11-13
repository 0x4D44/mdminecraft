use std::fs;
use std::path::Path;

use crate::AssetError;
use crate::{BlockDescriptor, BlockRegistry};

/// Load a block registry from the provided JSON file path.
pub fn registry_from_file(path: &Path) -> Result<BlockRegistry, AssetError> {
    let data = fs::read_to_string(path)?;
    registry_from_str(&data)
}

/// Load a block registry from an in-memory JSON string.
pub fn registry_from_str(input: &str) -> Result<BlockRegistry, AssetError> {
    let defs = crate::load_blocks_from_str(input)?;
    Ok(BlockRegistry::new(
        defs.into_iter()
            .map(|def| BlockDescriptor {
                name: def.name,
                opaque: def.opaque,
            })
            .collect(),
    ))
}
