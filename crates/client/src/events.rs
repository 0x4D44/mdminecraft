//! Game event system for deterministic replay and testing.
//!
//! All gameplay events (block placement, breaking, chunk updates) are
//! emitted to a stream that can be captured for replays and tests.

use mdminecraft_world::BlockId;
use serde::{Deserialize, Serialize};

/// Game events emitted during gameplay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameEvent {
    /// A block was placed by the player.
    BlockPlaced {
        /// World coordinates of the placed block.
        position: [i32; 3],
        /// The block ID that was placed.
        block_id: BlockId,
    },
    /// A block was broken by the player.
    BlockBroken {
        /// World coordinates of the broken block.
        position: [i32; 3],
        /// The block ID that was broken.
        block_id: BlockId,
    },
    /// A chunk mesh was generated.
    ChunkMeshed {
        /// Chunk position.
        chunk_pos: [i32; 2],
        /// Number of vertices in the mesh.
        vertex_count: usize,
    },
}

/// Event sink for writing gameplay events to JSONL format.
pub struct EventSink {
    events: Vec<GameEvent>,
}

impl EventSink {
    /// Create a new event sink.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Emit a game event to the sink.
    pub fn emit(&mut self, event: GameEvent) {
        self.events.push(event);
    }

    /// Get all events emitted so far.
    pub fn events(&self) -> &[GameEvent] {
        &self.events
    }

    /// Clear all events from the sink.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Serialize events to JSONL format.
    pub fn to_jsonl(&self) -> anyhow::Result<String> {
        let mut result = String::new();
        for event in &self.events {
            let line = serde_json::to_string(event)?;
            result.push_str(&line);
            result.push('\n');
        }
        Ok(result)
    }

    /// Write events to a file in JSONL format.
    pub fn write_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> anyhow::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;
        for event in &self.events {
            let line = serde_json::to_string(event)?;
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
        }
        Ok(())
    }
}

impl Default for EventSink {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_sink_creation() {
        let sink = EventSink::new();
        assert!(sink.events().is_empty());
    }

    #[test]
    fn emit_block_placed_event() {
        let mut sink = EventSink::new();
        let event = GameEvent::BlockPlaced {
            position: [10, 20, 30],
            block_id: 5,
        };

        sink.emit(event.clone());

        assert_eq!(sink.events().len(), 1);
        assert_eq!(sink.events()[0], event);
    }

    #[test]
    fn emit_block_broken_event() {
        let mut sink = EventSink::new();
        let event = GameEvent::BlockBroken {
            position: [5, 10, 15],
            block_id: 3,
        };

        sink.emit(event.clone());

        assert_eq!(sink.events().len(), 1);
        assert_eq!(sink.events()[0], event);
    }

    #[test]
    fn emit_chunk_meshed_event() {
        let mut sink = EventSink::new();
        let event = GameEvent::ChunkMeshed {
            chunk_pos: [2, 3],
            vertex_count: 1024,
        };

        sink.emit(event.clone());

        assert_eq!(sink.events().len(), 1);
        assert_eq!(sink.events()[0], event);
    }

    #[test]
    fn emit_multiple_events() {
        let mut sink = EventSink::new();

        sink.emit(GameEvent::BlockPlaced {
            position: [1, 2, 3],
            block_id: 1,
        });
        sink.emit(GameEvent::BlockBroken {
            position: [4, 5, 6],
            block_id: 2,
        });
        sink.emit(GameEvent::ChunkMeshed {
            chunk_pos: [0, 0],
            vertex_count: 512,
        });

        assert_eq!(sink.events().len(), 3);
    }

    #[test]
    fn clear_events() {
        let mut sink = EventSink::new();
        sink.emit(GameEvent::BlockPlaced {
            position: [1, 2, 3],
            block_id: 1,
        });

        assert_eq!(sink.events().len(), 1);

        sink.clear();
        assert!(sink.events().is_empty());
    }

    #[test]
    fn game_event_serialization() {
        let event = GameEvent::BlockPlaced {
            position: [10, 20, 30],
            block_id: 5,
        };

        let json = serde_json::to_string(&event).expect("serialize");
        let deserialized: GameEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(event, deserialized);
    }

    #[test]
    fn to_jsonl_empty() {
        let sink = EventSink::new();
        let jsonl = sink.to_jsonl().expect("to_jsonl");
        assert_eq!(jsonl, "");
    }

    #[test]
    fn to_jsonl_with_events() {
        let mut sink = EventSink::new();
        sink.emit(GameEvent::BlockPlaced {
            position: [1, 2, 3],
            block_id: 1,
        });
        sink.emit(GameEvent::BlockBroken {
            position: [4, 5, 6],
            block_id: 2,
        });

        let jsonl = sink.to_jsonl().expect("to_jsonl");

        // Should have two lines
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON
        let event1: GameEvent = serde_json::from_str(lines[0]).expect("parse line 1");
        let event2: GameEvent = serde_json::from_str(lines[1]).expect("parse line 2");

        assert_eq!(
            event1,
            GameEvent::BlockPlaced {
                position: [1, 2, 3],
                block_id: 1
            }
        );
        assert_eq!(
            event2,
            GameEvent::BlockBroken {
                position: [4, 5, 6],
                block_id: 2
            }
        );
    }

    #[test]
    fn write_to_file_creates_valid_jsonl() {
        let mut sink = EventSink::new();
        sink.emit(GameEvent::BlockPlaced {
            position: [1, 2, 3],
            block_id: 1,
        });
        sink.emit(GameEvent::ChunkMeshed {
            chunk_pos: [0, 0],
            vertex_count: 256,
        });

        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_events.jsonl");

        sink.write_to_file(&path).expect("write to file");

        // Read the file and verify contents
        let contents = std::fs::read_to_string(&path).expect("read file");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);

        // Clean up
        std::fs::remove_file(path).ok();
    }
}
