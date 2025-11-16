//! Message encoding and decoding with framing.
//!
//! Provides length-prefixed encoding for reliable delivery over QUIC streams.

use crate::protocol::{ClientMessage, ServerMessage, PROTOCOL_MAGIC, PROTOCOL_VERSION};
use anyhow::{Context, Result};
use blake3::Hash;

/// Compute schema hash from protocol definitions.
///
/// This hash is used to ensure client and server have compatible protocol versions.
pub fn compute_schema_hash() -> u64 {
    // Hash the serialized message type definitions
    let mut hasher = blake3::Hasher::new();

    // Include protocol version
    hasher.update(&PROTOCOL_VERSION.to_le_bytes());

    // Include protocol magic
    hasher.update(PROTOCOL_MAGIC);

    // Include message type names (deterministic)
    hasher.update(b"ClientMessage");
    hasher.update(b"ServerMessage");
    hasher.update(b"InputBundle");
    hasher.update(b"ChunkDataMessage");
    hasher.update(b"EntityDeltaMessage");

    let hash: Hash = hasher.finalize();
    u64::from_le_bytes(hash.as_bytes()[0..8].try_into().unwrap())
}

/// Encode a client message with length prefix.
///
/// Frame format: [length: u32][message_type: u8][payload: bytes]
pub fn encode_client_message(msg: &ClientMessage) -> Result<Vec<u8>> {
    // Serialize message with postcard
    let payload = postcard::to_allocvec(msg).context("Failed to serialize client message")?;

    // Build frame: length + message type + payload
    let mut frame = Vec::with_capacity(4 + 1 + payload.len());

    // Length (excluding length field itself)
    let length = (1 + payload.len()) as u32;
    frame.extend_from_slice(&length.to_le_bytes());

    // Message type tag (for multiplexing if needed)
    frame.push(message_type_tag(msg));

    // Payload
    frame.extend_from_slice(&payload);

    Ok(frame)
}

/// Encode a server message with length prefix.
///
/// Frame format: [length: u32][message_type: u8][payload: bytes]
pub fn encode_server_message(msg: &ServerMessage) -> Result<Vec<u8>> {
    // Serialize message with postcard
    let payload = postcard::to_allocvec(msg).context("Failed to serialize server message")?;

    // Build frame: length + message type + payload
    let mut frame = Vec::with_capacity(4 + 1 + payload.len());

    // Length (excluding length field itself)
    let length = (1 + payload.len()) as u32;
    frame.extend_from_slice(&length.to_le_bytes());

    // Message type tag
    frame.push(server_message_type_tag(msg));

    // Payload
    frame.extend_from_slice(&payload);

    Ok(frame)
}

/// Decode a client message from frame data.
///
/// Expects data to start with length prefix.
pub fn decode_client_message(data: &[u8]) -> Result<ClientMessage> {
    if data.len() < 5 {
        return Err(anyhow::anyhow!(
            "Frame too short: {} bytes (minimum 5)",
            data.len()
        ));
    }

    // Read length
    let length = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    if data.len() < 4 + length {
        return Err(anyhow::anyhow!(
            "Incomplete frame: expected {} bytes, got {}",
            4 + length,
            data.len()
        ));
    }

    // Skip message type tag (data[4])
    let payload = &data[5..4 + length];

    // Deserialize with postcard
    let msg = postcard::from_bytes(payload).context("Failed to deserialize client message")?;

    Ok(msg)
}

/// Decode a server message from frame data.
///
/// Expects data to start with length prefix.
pub fn decode_server_message(data: &[u8]) -> Result<ServerMessage> {
    if data.len() < 5 {
        return Err(anyhow::anyhow!(
            "Frame too short: {} bytes (minimum 5)",
            data.len()
        ));
    }

    // Read length
    let length = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    if data.len() < 4 + length {
        return Err(anyhow::anyhow!(
            "Incomplete frame: expected {} bytes, got {}",
            4 + length,
            data.len()
        ));
    }

    // Skip message type tag (data[4])
    let payload = &data[5..4 + length];

    // Deserialize with postcard
    let msg = postcard::from_bytes(payload).context("Failed to deserialize server message")?;

    Ok(msg)
}

/// Get message type tag for client messages.
fn message_type_tag(msg: &ClientMessage) -> u8 {
    match msg {
        ClientMessage::Handshake { .. } => 0,
        ClientMessage::Input(_) => 1,
        ClientMessage::Chat { .. } => 2,
        ClientMessage::DiagnosticsRequest => 3,
        ClientMessage::Disconnect { .. } => 4,
    }
}

/// Get message type tag for server messages.
fn server_message_type_tag(msg: &ServerMessage) -> u8 {
    match msg {
        ServerMessage::HandshakeResponse { .. } => 0,
        ServerMessage::ChunkData(_) => 1,
        ServerMessage::EntityDelta(_) => 2,
        ServerMessage::Chat { .. } => 3,
        ServerMessage::ServerState { .. } => 4,
        ServerMessage::DiagnosticsResponse { .. } => 5,
        ServerMessage::Disconnect { .. } => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{InputBundle, MovementInput};

    #[test]
    fn test_schema_hash_deterministic() {
        let hash1 = compute_schema_hash();
        let hash2 = compute_schema_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_schema_hash_non_zero() {
        let hash = compute_schema_hash();
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_encode_decode_client_handshake() {
        let msg = ClientMessage::Handshake {
            version: PROTOCOL_VERSION,
            schema_hash: 0xDEADBEEF,
        };

        let encoded = encode_client_message(&msg).expect("Failed to encode");
        let decoded = decode_client_message(&encoded).expect("Failed to decode");

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_encode_decode_server_handshake() {
        let msg = ServerMessage::HandshakeResponse {
            accepted: true,
            reason: None,
            player_entity_id: Some(42),
        };

        let encoded = encode_server_message(&msg).expect("Failed to encode");
        let decoded = decode_server_message(&encoded).expect("Failed to decode");

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_encode_decode_input_bundle() {
        let msg = ClientMessage::Input(InputBundle {
            tick: 1000,
            sequence: 42,
            last_ack_tick: 995,
            movement: MovementInput::zero(),
            block_actions: vec![],
            inventory_actions: vec![],
        });

        let encoded = encode_client_message(&msg).expect("Failed to encode");
        let decoded = decode_client_message(&encoded).expect("Failed to decode");

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_decode_incomplete_frame() {
        let data = vec![10, 0, 0, 0]; // Length says 10 bytes, but no data
        let result = decode_client_message(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_too_short() {
        let data = vec![1, 2, 3]; // Less than 5 bytes
        let result = decode_client_message(&data);
        assert!(result.is_err());
    }
}
