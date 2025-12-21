use crate::chunk::BlockState;

/// The maximum number of charges a respawn anchor can hold.
pub const RESPAWN_ANCHOR_MAX_CHARGES: u8 = 4;

const RESPAWN_ANCHOR_CHARGE_MASK: BlockState = 0b111;

/// Extract the charge count (0..=4) from a respawn anchor's packed block state.
pub fn respawn_anchor_charges(state: BlockState) -> u8 {
    ((state & RESPAWN_ANCHOR_CHARGE_MASK) as u8).min(RESPAWN_ANCHOR_MAX_CHARGES)
}

/// Set the charge count (0..=4) in a respawn anchor's packed block state.
pub fn set_respawn_anchor_charges(state: BlockState, charges: u8) -> BlockState {
    let charges = charges.min(RESPAWN_ANCHOR_MAX_CHARGES) as BlockState;
    (state & !RESPAWN_ANCHOR_CHARGE_MASK) | charges
}

/// Compute the emitted block-light level (0..=15) from a respawn anchor charge count.
pub fn respawn_anchor_light_level(charges: u8) -> u8 {
    match charges.min(RESPAWN_ANCHOR_MAX_CHARGES) {
        0 => 0,
        1 => 3,
        2 => 7,
        3 => 11,
        _ => 15,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges_roundtrip_and_clamp() {
        let state = 0u16;
        assert_eq!(respawn_anchor_charges(state), 0);

        let state = set_respawn_anchor_charges(state, 3);
        assert_eq!(respawn_anchor_charges(state), 3);

        let state = set_respawn_anchor_charges(state, 99);
        assert_eq!(respawn_anchor_charges(state), RESPAWN_ANCHOR_MAX_CHARGES);
    }

    #[test]
    fn light_level_matches_vanillaish_steps() {
        assert_eq!(respawn_anchor_light_level(0), 0);
        assert_eq!(respawn_anchor_light_level(1), 3);
        assert_eq!(respawn_anchor_light_level(2), 7);
        assert_eq!(respawn_anchor_light_level(3), 11);
        assert_eq!(respawn_anchor_light_level(4), 15);
        assert_eq!(respawn_anchor_light_level(99), 15);
    }
}
