//! Deterministic replay harness for testing and debugging.
//!
//! Records game inputs and network events to enable replay and validation of determinism.

use crate::protocol::{EntityId, InputBundle, Transform};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// Input log entry for JSONL format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputLogEntry {
    /// Simulation tick when input was recorded.
    pub tick: u64,

    /// Player entity ID.
    pub player_id: EntityId,

    /// Input bundle.
    pub input: InputBundle,
}

/// Network event log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetworkEvent {
    /// Player position update.
    PlayerPosition {
        /// Tick when event occurred.
        tick: u64,
        /// Player entity ID.
        player_id: EntityId,
        /// Player transform.
        transform: Transform,
    },

    /// Entity spawn event.
    EntitySpawn {
        /// Tick when event occurred.
        tick: u64,
        /// Entity ID.
        entity_id: EntityId,
        /// Entity type.
        entity_type: String,
        /// Initial transform.
        transform: Transform,
    },

    /// Entity update event.
    EntityUpdate {
        /// Tick when event occurred.
        tick: u64,
        /// Entity ID.
        entity_id: EntityId,
        /// Updated transform.
        transform: Transform,
    },

    /// Entity despawn event.
    EntityDespawn {
        /// Tick when event occurred.
        tick: u64,
        /// Entity ID.
        entity_id: EntityId,
    },
}

impl NetworkEvent {
    /// Get the tick of this event.
    pub fn tick(&self) -> u64 {
        match self {
            NetworkEvent::PlayerPosition { tick, .. } => *tick,
            NetworkEvent::EntitySpawn { tick, .. } => *tick,
            NetworkEvent::EntityUpdate { tick, .. } => *tick,
            NetworkEvent::EntityDespawn { tick, .. } => *tick,
        }
    }
}

/// Input logger that writes to JSONL format.
pub struct InputLogger {
    writer: BufWriter<File>,
    entries_written: u64,
}

impl InputLogger {
    /// Create a new input logger.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::create(path.as_ref())
            .with_context(|| format!("Failed to create input log: {:?}", path.as_ref()))?;
        Ok(Self {
            writer: BufWriter::new(file),
            entries_written: 0,
        })
    }

    /// Log an input entry.
    pub fn log(&mut self, tick: u64, player_id: EntityId, input: InputBundle) -> Result<()> {
        let entry = InputLogEntry {
            tick,
            player_id,
            input,
        };
        serde_json::to_writer(&mut self.writer, &entry)?;
        writeln!(&mut self.writer)?;
        self.entries_written += 1;
        Ok(())
    }

    /// Flush buffered writes.
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    /// Get number of entries written.
    pub fn entries_written(&self) -> u64 {
        self.entries_written
    }
}

/// Event logger that writes network events to JSONL format.
pub struct EventLogger {
    writer: BufWriter<File>,
    events_written: u64,
}

impl EventLogger {
    /// Create a new event logger.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::create(path.as_ref())
            .with_context(|| format!("Failed to create event log: {:?}", path.as_ref()))?;
        Ok(Self {
            writer: BufWriter::new(file),
            events_written: 0,
        })
    }

    /// Log a network event.
    pub fn log(&mut self, event: NetworkEvent) -> Result<()> {
        serde_json::to_writer(&mut self.writer, &event)?;
        writeln!(&mut self.writer)?;
        self.events_written += 1;
        Ok(())
    }

    /// Flush buffered writes.
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    /// Get number of events written.
    pub fn events_written(&self) -> u64 {
        self.events_written
    }
}

/// Replay player that reads and replays inputs from a log file.
pub struct ReplayPlayer {
    entries: Vec<InputLogEntry>,
    current_index: usize,
}

impl ReplayPlayer {
    /// Load a replay from a JSONL file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open replay file: {:?}", path.as_ref()))?;
        let reader = BufReader::new(file);

        let mut entries = Vec::new();
        for (line_num, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("Failed to read line {}", line_num + 1))?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: InputLogEntry = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse line {}: {}", line_num + 1, line))?;
            entries.push(entry);
        }

        Ok(Self {
            entries,
            current_index: 0,
        })
    }

    /// Get next input for a specific tick.
    ///
    /// Returns None if no more inputs or if next input is for a later tick.
    pub fn next_input(&mut self, tick: u64) -> Option<InputLogEntry> {
        if self.current_index >= self.entries.len() {
            return None;
        }

        let entry = &self.entries[self.current_index];
        if entry.tick == tick {
            self.current_index += 1;
            Some(entry.clone())
        } else {
            None
        }
    }

    /// Get all inputs for a specific tick.
    pub fn inputs_for_tick(&mut self, tick: u64) -> Vec<InputLogEntry> {
        let mut inputs = Vec::new();

        while self.current_index < self.entries.len() {
            let entry = &self.entries[self.current_index];
            if entry.tick == tick {
                inputs.push(entry.clone());
                self.current_index += 1;
            } else if entry.tick > tick {
                break;
            } else {
                // Skip inputs from past ticks (shouldn't happen with sequential playback)
                self.current_index += 1;
            }
        }

        inputs
    }

    /// Reset playback to beginning.
    pub fn reset(&mut self) {
        self.current_index = 0;
    }

    /// Get total number of entries in replay.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get current playback position.
    pub fn current_position(&self) -> usize {
        self.current_index
    }

    /// Check if replay is complete.
    pub fn is_finished(&self) -> bool {
        self.current_index >= self.entries.len()
    }
}

/// Replay validator that compares recorded vs replayed events.
pub struct ReplayValidator {
    /// Expected events from original recording.
    expected_events: Vec<NetworkEvent>,

    /// Current event index.
    current_index: usize,

    /// Validation errors found.
    errors: Vec<ValidationError>,
}

/// Validation error during replay.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Tick when error occurred.
    pub tick: u64,

    /// Error message.
    pub message: String,

    /// Expected event (if applicable).
    pub expected: Option<String>,

    /// Actual event (if applicable).
    pub actual: Option<String>,
}

impl ReplayValidator {
    /// Load expected events from a JSONL file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open event log: {:?}", path.as_ref()))?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        for (line_num, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("Failed to read line {}", line_num + 1))?;
            if line.trim().is_empty() {
                continue;
            }
            let event: NetworkEvent = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse line {}: {}", line_num + 1, line))?;
            events.push(event);
        }

        Ok(Self {
            expected_events: events,
            current_index: 0,
            errors: Vec::new(),
        })
    }

    /// Validate a network event against expected events.
    pub fn validate_event(&mut self, actual: &NetworkEvent) {
        if self.current_index >= self.expected_events.len() {
            self.errors.push(ValidationError {
                tick: actual.tick(),
                message: "Unexpected event (no more expected events)".to_string(),
                expected: None,
                actual: Some(format!("{:?}", actual)),
            });
            return;
        }

        let expected = &self.expected_events[self.current_index];

        if !events_match(expected, actual) {
            self.errors.push(ValidationError {
                tick: actual.tick(),
                message: "Event mismatch".to_string(),
                expected: Some(format!("{:?}", expected)),
                actual: Some(format!("{:?}", actual)),
            });
        }

        self.current_index += 1;
    }

    /// Finish validation and check for missing events.
    pub fn finish(&mut self) {
        while self.current_index < self.expected_events.len() {
            let expected = &self.expected_events[self.current_index];
            self.errors.push(ValidationError {
                tick: expected.tick(),
                message: "Missing event (expected but not replayed)".to_string(),
                expected: Some(format!("{:?}", expected)),
                actual: None,
            });
            self.current_index += 1;
        }
    }

    /// Get validation errors.
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Check if validation passed (no errors).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get number of events validated.
    pub fn events_validated(&self) -> usize {
        self.current_index
    }
}

/// Check if two network events match.
fn events_match(expected: &NetworkEvent, actual: &NetworkEvent) -> bool {
    match (expected, actual) {
        (
            NetworkEvent::PlayerPosition {
                tick: t1,
                player_id: p1,
                transform: tr1,
            },
            NetworkEvent::PlayerPosition {
                tick: t2,
                player_id: p2,
                transform: tr2,
            },
        ) => t1 == t2 && p1 == p2 && transforms_equal(tr1, tr2),

        (
            NetworkEvent::EntitySpawn {
                tick: t1,
                entity_id: e1,
                entity_type: et1,
                transform: tr1,
            },
            NetworkEvent::EntitySpawn {
                tick: t2,
                entity_id: e2,
                entity_type: et2,
                transform: tr2,
            },
        ) => t1 == t2 && e1 == e2 && et1 == et2 && transforms_equal(tr1, tr2),

        (
            NetworkEvent::EntityUpdate {
                tick: t1,
                entity_id: e1,
                transform: tr1,
            },
            NetworkEvent::EntityUpdate {
                tick: t2,
                entity_id: e2,
                transform: tr2,
            },
        ) => t1 == t2 && e1 == e2 && transforms_equal(tr1, tr2),

        (
            NetworkEvent::EntityDespawn {
                tick: t1,
                entity_id: e1,
            },
            NetworkEvent::EntityDespawn {
                tick: t2,
                entity_id: e2,
            },
        ) => t1 == t2 && e1 == e2,

        _ => false, // Different event types
    }
}

/// Check if two transforms are equal.
fn transforms_equal(a: &Transform, b: &Transform) -> bool {
    a.x == b.x && a.y == b.y && a.z == b.z && a.yaw == b.yaw && a.pitch == b.pitch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MovementInput;
    use mdminecraft_core::DimensionId;
    use std::fs;
    use tempfile::tempdir;

    fn make_input() -> InputBundle {
        InputBundle {
            tick: 100,
            sequence: 1,
            last_ack_tick: 99,
            movement: MovementInput {
                forward: 1,
                strafe: 0,
                jump: false,
                sprint: false,
                yaw: 128,
                pitch: 64,
            },
            block_actions: Vec::new(),
            inventory_actions: Vec::new(),
        }
    }

    fn make_transform(x: i32, y: i32, z: i32) -> Transform {
        Transform {
            dimension: DimensionId::DEFAULT,
            x,
            y,
            z,
            yaw: 0,
            pitch: 0,
        }
    }

    #[test]
    fn test_input_logger_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inputs.jsonl");

        let mut logger = InputLogger::create(&path).unwrap();
        logger.log(100, 1, make_input()).unwrap();
        logger.log(101, 1, make_input()).unwrap();
        logger.flush().unwrap();
        drop(logger);

        // Verify file contents
        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_event_logger_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        let mut logger = EventLogger::create(&path).unwrap();
        logger
            .log(NetworkEvent::PlayerPosition {
                tick: 100,
                player_id: 1,
                transform: make_transform(100, 200, 300),
            })
            .unwrap();
        logger.flush().unwrap();
        drop(logger);

        // Verify file contents
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("PlayerPosition"));
    }

    #[test]
    fn test_replay_player_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inputs.jsonl");

        let mut logger = InputLogger::create(&path).unwrap();
        logger.log(100, 1, make_input()).unwrap();
        logger.log(101, 1, make_input()).unwrap();
        logger.flush().unwrap();
        drop(logger);

        let player = ReplayPlayer::load(&path).unwrap();
        assert_eq!(player.entry_count(), 2);
    }

    #[test]
    fn test_replay_player_playback() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inputs.jsonl");

        let mut logger = InputLogger::create(&path).unwrap();
        logger.log(100, 1, make_input()).unwrap();
        logger.log(101, 1, make_input()).unwrap();
        logger.flush().unwrap();
        drop(logger);

        let mut player = ReplayPlayer::load(&path).unwrap();

        // Get input for tick 100
        let entry = player.next_input(100).unwrap();
        assert_eq!(entry.tick, 100);

        // Get input for tick 101
        let entry = player.next_input(101).unwrap();
        assert_eq!(entry.tick, 101);

        // No more inputs
        assert!(player.next_input(102).is_none());
        assert!(player.is_finished());
    }

    #[test]
    fn test_replay_player_reset() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inputs.jsonl");

        let mut logger = InputLogger::create(&path).unwrap();
        logger.log(100, 1, make_input()).unwrap();
        logger.flush().unwrap();
        drop(logger);

        let mut player = ReplayPlayer::load(&path).unwrap();
        player.next_input(100);
        assert!(player.is_finished());

        player.reset();
        assert!(!player.is_finished());
        assert_eq!(player.current_position(), 0);
    }

    #[test]
    fn test_validator_match() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        let event = NetworkEvent::PlayerPosition {
            tick: 100,
            player_id: 1,
            transform: make_transform(100, 200, 300),
        };

        let mut logger = EventLogger::create(&path).unwrap();
        logger.log(event.clone()).unwrap();
        logger.flush().unwrap();
        drop(logger);

        let mut validator = ReplayValidator::load(&path).unwrap();
        validator.validate_event(&event);
        validator.finish();

        assert!(validator.is_valid());
        assert_eq!(validator.events_validated(), 1);
    }

    #[test]
    fn test_validator_mismatch() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        let expected = NetworkEvent::PlayerPosition {
            tick: 100,
            player_id: 1,
            transform: make_transform(100, 200, 300),
        };

        let actual = NetworkEvent::PlayerPosition {
            tick: 100,
            player_id: 1,
            transform: make_transform(200, 200, 300), // Different position
        };

        let mut logger = EventLogger::create(&path).unwrap();
        logger.log(expected).unwrap();
        logger.flush().unwrap();
        drop(logger);

        let mut validator = ReplayValidator::load(&path).unwrap();
        validator.validate_event(&actual);
        validator.finish();

        assert!(!validator.is_valid());
        assert_eq!(validator.errors().len(), 1);
    }

    #[test]
    fn test_validator_missing_event() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        let event = NetworkEvent::PlayerPosition {
            tick: 100,
            player_id: 1,
            transform: make_transform(100, 200, 300),
        };

        let mut logger = EventLogger::create(&path).unwrap();
        logger.log(event).unwrap();
        logger.flush().unwrap();
        drop(logger);

        let mut validator = ReplayValidator::load(&path).unwrap();
        validator.finish(); // Finish without validating any events

        assert!(!validator.is_valid());
        assert_eq!(validator.errors().len(), 1);
        assert!(validator.errors()[0].message.contains("Missing event"));
    }
}
