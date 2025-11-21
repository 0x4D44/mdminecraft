//! Fuzz-style property tests for network codec
//!
//! These tests validate that message decoders handle arbitrary
//! network input gracefully without crashing.

use mdminecraft_net::{
    decode_client_message, decode_server_message, encode_client_message, encode_server_message,
    ClientMessage, InputBundle, MovementInput, ServerMessage, PROTOCOL_VERSION,
};
use proptest::prelude::*;

proptest! {
    /// Property: Arbitrary bytes don't crash client decoder
    #[test]
    fn arbitrary_bytes_dont_crash_client(
        random_bytes in prop::collection::vec(any::<u8>(), 0..2000),
    ) {
        let _result = decode_client_message(&random_bytes);
        // No panic = success
    }

    /// Property: Arbitrary bytes don't crash server decoder
    #[test]
    fn arbitrary_bytes_dont_crash_server(
        random_bytes in prop::collection::vec(any::<u8>(), 0..2000),
    ) {
        let _result = decode_server_message(&random_bytes);
        // No panic = success
    }

    /// Property: Handshake messages roundtrip
    #[test]
    fn handshake_roundtrips(
        version in any::<u16>(),
        schema_hash in any::<u64>(),
    ) {
        let msg = ClientMessage::Handshake {
            version,
            schema_hash,
        };

        let encoded = encode_client_message(&msg).unwrap();
        let decoded = decode_client_message(&encoded).unwrap();

        prop_assert_eq!(msg, decoded);
    }

    /// Property: Input bundles roundtrip
    #[test]
    fn input_bundle_roundtrips(
        tick in any::<u64>(),
        sequence in any::<u32>(),
        last_ack in any::<u64>(),
    ) {
        let msg = ClientMessage::Input(InputBundle {
            tick,
            sequence,
            last_ack_tick: last_ack,
            movement: MovementInput::zero(),
            block_actions: vec![],
            inventory_actions: vec![],
        });

        let encoded = encode_client_message(&msg).unwrap();
        let decoded = decode_client_message(&encoded).unwrap();

        prop_assert_eq!(msg, decoded);
    }

    /// Property: Server handshake roundtrips
    #[test]
    fn server_handshake_roundtrips(
        accepted in any::<bool>(),
        player_id in any::<u64>(),
    ) {
        let msg = ServerMessage::HandshakeResponse {
            accepted,
            reason: if accepted { None } else { Some("Test".to_string()) },
            player_entity_id: if accepted { Some(player_id) } else { None },
        };

        let encoded = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&encoded).unwrap();

        prop_assert_eq!(msg, decoded);
    }

    /// Property: Truncated frames don't crash
    #[test]
    fn truncated_frames_handled(
        truncate_at in 0usize..50,
    ) {
        let msg = ClientMessage::Handshake {
            version: PROTOCOL_VERSION,
            schema_hash: 0x12345678,
        };

        let mut encoded = encode_client_message(&msg).unwrap();

        if truncate_at < encoded.len() {
            encoded.truncate(truncate_at);
            let _result = decode_client_message(&encoded);
            // May fail or succeed - just shouldn't panic
        }
    }

    /// Property: Oversized length prefix handled
    #[test]
    fn oversized_length_handled(
        claimed_length in 100u32..5000u32,
    ) {
        let mut frame = Vec::new();
        frame.extend_from_slice(&claimed_length.to_le_bytes());
        frame.push(0);
        frame.extend_from_slice(&[0, 1, 2, 3, 4]);

        let _result = decode_client_message(&frame);
        // Should fail gracefully, not panic
    }

    /// Property: Corrupted payload handled
    #[test]
    fn corrupted_payload_handled(
        flip_pos in 0usize..30,
        flip_bit in 0u8..8,
    ) {
        let msg = ClientMessage::Handshake {
            version: PROTOCOL_VERSION,
            schema_hash: 0xDEADBEEF,
        };

        let mut encoded = encode_client_message(&msg).unwrap();

        if flip_pos + 5 < encoded.len() {
            encoded[flip_pos + 5] ^= 1 << flip_bit;
            let _result = decode_client_message(&encoded);
            // May succeed or fail - just shouldn't panic
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn empty_frame_fails() {
        assert!(decode_client_message(&[]).is_err());
        assert!(decode_server_message(&[]).is_err());
    }

    #[test]
    fn too_short_fails() {
        assert!(decode_client_message(&[1, 2, 3]).is_err());
    }

    #[test]
    fn valid_roundtrip() {
        let msg = ClientMessage::Handshake {
            version: 1,
            schema_hash: 0x123,
        };

        let encoded = encode_client_message(&msg).unwrap();
        let decoded = decode_client_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }
}
