use crate::chunk::{
    BlockId, BlockState, BLOCK_CAVE_VINES, BLOCK_FURNACE_LIT, BLOCK_GLOW_LICHEN, BLOCK_MAGMA_BLOCK,
    BLOCK_SCULK_CATALYST, BLOCK_SCULK_SENSOR,
};
use crate::fluid::{BLOCK_LAVA, BLOCK_LAVA_FLOWING};
use crate::interaction::interactive_blocks;
use crate::redstone::{is_active, redstone_blocks};

/// Return the block light emission level for a block-state (0-15).
///
/// This is used to seed block-light propagation. The returned value matches vanilla-style
/// semantics: light is emitted from the block itself and decays by 1 per block traveled.
pub fn block_light_emission(block_id: BlockId, state: BlockState) -> u8 {
    match block_id {
        // Vanilla torch (14).
        interactive_blocks::TORCH => 14,

        // Redstone torch emits when active (7).
        redstone_blocks::REDSTONE_TORCH => {
            if is_active(state) {
                7
            } else {
                0
            }
        }

        // Redstone lamp emits when lit (15).
        redstone_blocks::REDSTONE_LAMP_LIT => 15,

        // Lit furnace emits some light (13).
        BLOCK_FURNACE_LIT => 13,

        // Lava emits max light (15).
        BLOCK_LAVA | BLOCK_LAVA_FLOWING => 15,

        // Cave decorations / sculk family (vanilla-ish).
        BLOCK_GLOW_LICHEN => 7,
        BLOCK_CAVE_VINES => 14,
        BLOCK_MAGMA_BLOCK => 3,
        BLOCK_SCULK_SENSOR => 1,
        BLOCK_SCULK_CATALYST => 6,

        _ => 0,
    }
}

/// Helper for checking whether a block emits block-light.
pub fn emits_block_light(block_id: BlockId, state: BlockState) -> bool {
    block_light_emission(block_id, state) > 0
}
