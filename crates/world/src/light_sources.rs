use crate::chunk::{BlockId, BlockState, BLOCK_RESPAWN_ANCHOR};
use crate::redstone::{is_active, redstone_blocks};
use crate::BlockOpacityProvider;
use crate::{respawn_anchor_charges, respawn_anchor_light_level};

/// Return the block light emission level for a block-state (0-15).
///
/// This is used to seed block-light propagation. The returned value matches vanilla-style
/// semantics: light is emitted from the block itself and decays by 1 per block traveled.
fn block_light_emission_override(block_id: BlockId, state: BlockState) -> Option<u8> {
    match block_id {
        // Redstone torch emits when active (7).
        redstone_blocks::REDSTONE_TORCH => Some(if is_active(state) { 7 } else { 0 }),

        // Respawn anchors emit based on their charge level.
        BLOCK_RESPAWN_ANCHOR => Some(respawn_anchor_light_level(respawn_anchor_charges(state))),
        _ => None,
    }
}

/// Return the block light emission level for a block-state (0-15).
///
/// Uses registry metadata for base emission values, with state-dependent overrides for blocks
/// like redstone components and respawn anchors.
pub fn block_light_emission(
    block_id: BlockId,
    state: BlockState,
    registry: &dyn BlockOpacityProvider,
) -> u8 {
    block_light_emission_override(block_id, state)
        .unwrap_or_else(|| registry.base_block_light_emission(block_id))
}

/// Helper for checking whether a block emits block-light.
pub fn emits_block_light(
    block_id: BlockId,
    state: BlockState,
    registry: &dyn BlockOpacityProvider,
) -> bool {
    block_light_emission(block_id, state, registry) > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::BLOCK_AIR;
    use crate::redstone::set_active;
    use crate::respawn_anchor::{respawn_anchor_light_level, set_respawn_anchor_charges};

    struct BaseEmissiveRegistry;

    impl BlockOpacityProvider for BaseEmissiveRegistry {
        fn light_opacity(&self, block_id: BlockId) -> u8 {
            if block_id == BLOCK_AIR {
                0
            } else {
                15
            }
        }

        fn base_block_light_emission(&self, _block_id: BlockId) -> u8 {
            // Simulate a legacy config/pack that marked everything `emissive: true`.
            15
        }
    }

    #[test]
    fn stateful_overrides_ignore_base_emission() {
        let registry = BaseEmissiveRegistry;

        // Redstone torch off should emit 0 even if registry says it emits light.
        let off = block_light_emission(redstone_blocks::REDSTONE_TORCH, 0, &registry);
        assert_eq!(off, 0);

        let on_state = set_active(0, true);
        let on = block_light_emission(redstone_blocks::REDSTONE_TORCH, on_state, &registry);
        assert_eq!(on, 7);

        // Respawn anchor should follow its charge-derived emission even if base is non-zero.
        for charges in 0..=4 {
            let state = set_respawn_anchor_charges(0, charges);
            let expected = respawn_anchor_light_level(charges);
            let emission = block_light_emission(BLOCK_RESPAWN_ANCHOR, state, &registry);
            assert_eq!(emission, expected);
        }
    }
}
