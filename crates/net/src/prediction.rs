//! Client-side prediction and server reconciliation.
//!
//! Implements client prediction with rollback and replay for responsive multiplayer.

use crate::protocol::{EntityId, InputBundle, Transform};
use std::collections::{HashMap, VecDeque};

/// Maximum number of snapshots to keep in the circular buffer.
const DEFAULT_SNAPSHOT_CAPACITY: usize = 256;

/// Error tolerance for position mismatch (in quantized units, 1/16 block).
const POSITION_ERROR_TOLERANCE: i32 = 2; // ~1/8 block = 12.5mm

/// Server-authoritative snapshot of world state at a specific tick.
#[derive(Debug, Clone)]
pub struct ServerSnapshot {
    /// Simulation tick this snapshot represents.
    pub tick: u64,

    /// Player transform confirmed by server.
    pub player_transform: Transform,

    /// Other entity transforms (visible to player).
    pub entities: HashMap<EntityId, Transform>,
}

/// Circular buffer for storing server snapshots.
pub struct SnapshotBuffer {
    snapshots: VecDeque<ServerSnapshot>,
    capacity: usize,
}

impl SnapshotBuffer {
    /// Create a new snapshot buffer with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_SNAPSHOT_CAPACITY)
    }

    /// Create a new snapshot buffer with custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add a snapshot to the buffer.
    ///
    /// If buffer is full, oldest snapshot is removed.
    pub fn push(&mut self, snapshot: ServerSnapshot) {
        if self.snapshots.len() >= self.capacity {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot);
    }

    /// Get snapshot for a specific tick.
    pub fn get(&self, tick: u64) -> Option<&ServerSnapshot> {
        self.snapshots.iter().find(|s| s.tick == tick)
    }

    /// Get the most recent snapshot.
    pub fn latest(&self) -> Option<&ServerSnapshot> {
        self.snapshots.back()
    }

    /// Get the oldest snapshot.
    pub fn oldest(&self) -> Option<&ServerSnapshot> {
        self.snapshots.front()
    }

    /// Clear all snapshots.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Get number of snapshots stored.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Remove snapshots older than specified tick.
    pub fn prune_before(&mut self, tick: u64) {
        self.snapshots.retain(|s| s.tick >= tick);
    }
}

impl Default for SnapshotBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Client-side predictor with rollback and replay.
pub struct ClientPredictor {
    /// Circular buffer of server snapshots.
    snapshot_buffer: SnapshotBuffer,

    /// Last tick confirmed by server.
    last_confirmed_tick: u64,

    /// Pending inputs not yet confirmed by server.
    pending_inputs: VecDeque<(u64, InputBundle)>, // (tick, input)

    /// Current client tick (may be ahead of server).
    client_tick: u64,

    /// Prediction metrics.
    metrics: PredictionMetrics,
}

/// Metrics for tracking prediction accuracy.
#[derive(Debug, Clone, Default)]
pub struct PredictionMetrics {
    /// Total number of predictions made.
    pub total_predictions: u64,

    /// Total number of mismatches detected.
    pub total_mismatches: u64,

    /// Total number of rewinds performed.
    pub total_rewinds: u64,

    /// Average prediction error distance (in blocks).
    pub avg_error_distance: f32,

    /// Maximum prediction error distance (in blocks).
    pub max_error_distance: f32,
}

impl ClientPredictor {
    /// Create a new client predictor.
    pub fn new() -> Self {
        Self {
            snapshot_buffer: SnapshotBuffer::new(),
            last_confirmed_tick: 0,
            pending_inputs: VecDeque::new(),
            client_tick: 0,
            metrics: PredictionMetrics::default(),
        }
    }

    /// Process server snapshot and reconcile with client state.
    ///
    /// Returns true if a mismatch was detected and rollback occurred.
    pub fn reconcile(
        &mut self,
        server_snapshot: ServerSnapshot,
        current_player_transform: &Transform,
    ) -> ReconciliationResult {
        let server_tick = server_snapshot.tick;

        // Store snapshot
        self.snapshot_buffer.push(server_snapshot.clone());
        self.last_confirmed_tick = server_tick;

        // Remove confirmed inputs
        self.pending_inputs
            .retain(|(tick, _)| *tick > server_tick);

        // Check for mismatch
        let error = calculate_transform_error(
            &server_snapshot.player_transform,
            current_player_transform,
        );

        if error > POSITION_ERROR_TOLERANCE {
            // Mismatch detected - need to rollback and replay
            self.metrics.total_mismatches += 1;
            self.metrics.total_rewinds += 1;

            let error_blocks = error as f32 / 16.0;
            self.metrics.avg_error_distance = (self.metrics.avg_error_distance
                * (self.metrics.total_mismatches - 1) as f32
                + error_blocks)
                / self.metrics.total_mismatches as f32;

            if error_blocks > self.metrics.max_error_distance {
                self.metrics.max_error_distance = error_blocks;
            }

            ReconciliationResult::Mismatch {
                server_tick,
                server_transform: server_snapshot.player_transform,
                inputs_to_replay: self.pending_inputs.iter().cloned().collect(),
                error_distance: error_blocks,
            }
        } else {
            // State matches server, no rollback needed
            ReconciliationResult::Match { server_tick }
        }
    }

    /// Record a client input for future reconciliation.
    pub fn record_input(&mut self, tick: u64, input: InputBundle) {
        self.pending_inputs.push_back((tick, input));
        self.client_tick = tick;
        self.metrics.total_predictions += 1;

        // Limit pending inputs to reasonable buffer size
        const MAX_PENDING_INPUTS: usize = 128;
        while self.pending_inputs.len() > MAX_PENDING_INPUTS {
            self.pending_inputs.pop_front();
        }
    }

    /// Get the last confirmed tick from server.
    pub fn last_confirmed_tick(&self) -> u64 {
        self.last_confirmed_tick
    }

    /// Get current client tick.
    pub fn client_tick(&self) -> u64 {
        self.client_tick
    }

    /// Get number of pending (unconfirmed) inputs.
    pub fn pending_input_count(&self) -> usize {
        self.pending_inputs.len()
    }

    /// Get current prediction metrics.
    pub fn metrics(&self) -> &PredictionMetrics {
        &self.metrics
    }

    /// Get snapshot buffer.
    pub fn snapshot_buffer(&self) -> &SnapshotBuffer {
        &self.snapshot_buffer
    }

    /// Reset predictor state.
    pub fn reset(&mut self) {
        self.snapshot_buffer.clear();
        self.pending_inputs.clear();
        self.last_confirmed_tick = 0;
        self.client_tick = 0;
        self.metrics = PredictionMetrics::default();
    }
}

impl Default for ClientPredictor {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of reconciliation with server state.
#[derive(Debug, Clone)]
pub enum ReconciliationResult {
    /// Client state matches server, no action needed.
    Match {
        /// Server tick that was confirmed.
        server_tick: u64,
    },

    /// Mismatch detected, rollback and replay required.
    Mismatch {
        /// Server tick that detected the mismatch.
        server_tick: u64,
        /// Server-authoritative transform to rollback to.
        server_transform: Transform,
        /// List of inputs to replay (tick, input).
        inputs_to_replay: Vec<(u64, InputBundle)>,
        /// Error distance in blocks.
        error_distance: f32,
    },
}

/// Calculate error distance between two transforms (in quantized units).
fn calculate_transform_error(server: &Transform, client: &Transform) -> i32 {
    let dx = (server.x - client.x).abs();
    let dy = (server.y - client.y).abs();
    let dz = (server.z - client.z).abs();

    // Use 3D distance
    ((dx * dx + dy * dy + dz * dz) as f64).sqrt() as i32
}

/// Entity interpolator for smooth remote entity movement.
pub struct EntityInterpolator {
    /// Target transforms for each entity.
    targets: HashMap<EntityId, Transform>,

    /// Interpolation alpha per entity (0.0 = current, 1.0 = target).
    alphas: HashMap<EntityId, f32>,

    /// Interpolation speed (alpha increment per tick).
    interpolation_speed: f32,
}

impl EntityInterpolator {
    /// Create a new entity interpolator.
    pub fn new(interpolation_speed: f32) -> Self {
        Self {
            targets: HashMap::new(),
            alphas: HashMap::new(),
            interpolation_speed,
        }
    }

    /// Set target transform for an entity.
    pub fn set_target(&mut self, entity_id: EntityId, target: Transform) {
        self.targets.insert(entity_id, target);
        self.alphas.insert(entity_id, 0.0);
    }

    /// Update interpolation and get current transform for an entity.
    pub fn interpolate(
        &mut self,
        entity_id: EntityId,
        current: &Transform,
    ) -> Option<Transform> {
        let target = self.targets.get(&entity_id)?;
        let alpha = self.alphas.get_mut(&entity_id)?;

        // Increment alpha
        *alpha = (*alpha + self.interpolation_speed).min(1.0);

        // Interpolate transform
        let result = interpolate_transform(current, target, *alpha);

        // Remove if interpolation complete
        if *alpha >= 1.0 {
            self.targets.remove(&entity_id);
            self.alphas.remove(&entity_id);
        }

        Some(result)
    }

    /// Remove entity from interpolation.
    pub fn remove(&mut self, entity_id: EntityId) {
        self.targets.remove(&entity_id);
        self.alphas.remove(&entity_id);
    }

    /// Clear all interpolation state.
    pub fn clear(&mut self) {
        self.targets.clear();
        self.alphas.clear();
    }
}

/// Interpolate between two transforms.
fn interpolate_transform(from: &Transform, to: &Transform, alpha: f32) -> Transform {
    Transform {
        x: lerp_i32(from.x, to.x, alpha),
        y: lerp_i32(from.y, to.y, alpha),
        z: lerp_i32(from.z, to.z, alpha),
        yaw: lerp_u8(from.yaw, to.yaw, alpha),
        pitch: lerp_u8(from.pitch, to.pitch, alpha),
    }
}

/// Linear interpolation for i32.
fn lerp_i32(a: i32, b: i32, t: f32) -> i32 {
    (a as f32 + (b - a) as f32 * t) as i32
}

/// Linear interpolation for u8.
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as i16 - a as i16) as f32 * t) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MovementInput;

    fn make_transform(x: i32, y: i32, z: i32) -> Transform {
        Transform {
            x,
            y,
            z,
            yaw: 0,
            pitch: 0,
        }
    }

    fn make_input(forward: i8) -> InputBundle {
        InputBundle {
            tick: 0,
            sequence: 0,
            last_ack_tick: 0,
            movement: MovementInput {
                forward,
                strafe: 0,
                jump: false,
                sprint: false,
                yaw: 0,
                pitch: 0,
            },
            block_actions: Vec::new(),
            inventory_actions: Vec::new(),
        }
    }

    #[test]
    fn test_snapshot_buffer_push() {
        let mut buffer = SnapshotBuffer::with_capacity(3);

        buffer.push(ServerSnapshot {
            tick: 1,
            player_transform: make_transform(0, 0, 0),
            entities: HashMap::new(),
        });

        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.latest().unwrap().tick, 1);
    }

    #[test]
    fn test_snapshot_buffer_overflow() {
        let mut buffer = SnapshotBuffer::with_capacity(2);

        buffer.push(ServerSnapshot {
            tick: 1,
            player_transform: make_transform(0, 0, 0),
            entities: HashMap::new(),
        });
        buffer.push(ServerSnapshot {
            tick: 2,
            player_transform: make_transform(0, 0, 0),
            entities: HashMap::new(),
        });
        buffer.push(ServerSnapshot {
            tick: 3,
            player_transform: make_transform(0, 0, 0),
            entities: HashMap::new(),
        });

        // Should keep only latest 2
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.oldest().unwrap().tick, 2);
        assert_eq!(buffer.latest().unwrap().tick, 3);
    }

    #[test]
    fn test_snapshot_buffer_get() {
        let mut buffer = SnapshotBuffer::new();

        buffer.push(ServerSnapshot {
            tick: 5,
            player_transform: make_transform(100, 200, 300),
            entities: HashMap::new(),
        });

        let snapshot = buffer.get(5).unwrap();
        assert_eq!(snapshot.tick, 5);
        assert_eq!(snapshot.player_transform.x, 100);
    }

    #[test]
    fn test_predictor_record_input() {
        let mut predictor = ClientPredictor::new();

        predictor.record_input(1, make_input(1));
        predictor.record_input(2, make_input(0));

        assert_eq!(predictor.pending_input_count(), 2);
        assert_eq!(predictor.client_tick(), 2);
    }

    #[test]
    fn test_predictor_reconcile_match() {
        let mut predictor = ClientPredictor::new();

        predictor.record_input(1, make_input(1));

        let snapshot = ServerSnapshot {
            tick: 1,
            player_transform: make_transform(100, 200, 300),
            entities: HashMap::new(),
        };

        let result = predictor.reconcile(snapshot, &make_transform(100, 200, 300));

        match result {
            ReconciliationResult::Match { server_tick } => {
                assert_eq!(server_tick, 1);
            }
            _ => panic!("Expected Match result"),
        }

        assert_eq!(predictor.pending_input_count(), 0);
    }

    #[test]
    fn test_predictor_reconcile_mismatch() {
        let mut predictor = ClientPredictor::new();

        predictor.record_input(1, make_input(1));

        let snapshot = ServerSnapshot {
            tick: 1,
            player_transform: make_transform(100, 200, 300),
            entities: HashMap::new(),
        };

        // Client prediction is wrong
        let result = predictor.reconcile(snapshot, &make_transform(200, 200, 300));

        match result {
            ReconciliationResult::Mismatch {
                server_tick,
                error_distance,
                inputs_to_replay,
                ..
            } => {
                assert_eq!(server_tick, 1);
                assert!(error_distance > 0.0);
                assert_eq!(inputs_to_replay.len(), 0); // Input was for tick 1, which was confirmed
            }
            _ => panic!("Expected Mismatch result"),
        }

        assert_eq!(predictor.metrics().total_mismatches, 1);
    }

    #[test]
    fn test_predictor_pending_inputs_replay() {
        let mut predictor = ClientPredictor::new();

        // Record multiple inputs
        predictor.record_input(1, make_input(1));
        predictor.record_input(2, make_input(0));
        predictor.record_input(3, make_input(1));

        // Server confirms tick 1
        let snapshot = ServerSnapshot {
            tick: 1,
            player_transform: make_transform(100, 200, 300),
            entities: HashMap::new(),
        };

        let result = predictor.reconcile(snapshot, &make_transform(200, 200, 300));

        match result {
            ReconciliationResult::Mismatch {
                inputs_to_replay, ..
            } => {
                // Should replay inputs for ticks 2 and 3
                assert_eq!(inputs_to_replay.len(), 2);
                assert_eq!(inputs_to_replay[0].0, 2);
                assert_eq!(inputs_to_replay[1].0, 3);
            }
            _ => panic!("Expected Mismatch result"),
        }
    }

    #[test]
    fn test_entity_interpolator() {
        let mut interpolator = EntityInterpolator::new(0.5);

        let current = make_transform(0, 0, 0);
        let target = make_transform(100, 100, 100);

        interpolator.set_target(1, target);

        // First interpolation
        let result1 = interpolator.interpolate(1, &current).unwrap();
        assert!(result1.x > 0 && result1.x < 100);

        // Second interpolation (should reach target)
        let result2 = interpolator.interpolate(1, &result1).unwrap();
        assert_eq!(result2.x, 100);
        assert_eq!(result2.y, 100);
        assert_eq!(result2.z, 100);

        // Third interpolation (should return None, interpolation complete)
        let result3 = interpolator.interpolate(1, &result2);
        assert!(result3.is_none());
    }

    #[test]
    fn test_transform_error_calculation() {
        let server = make_transform(100, 200, 300);
        let client = make_transform(105, 200, 300);

        let error = calculate_transform_error(&server, &client);
        assert_eq!(error, 5); // Exact distance on X axis
    }

    #[test]
    fn test_predictor_reset() {
        let mut predictor = ClientPredictor::new();

        predictor.record_input(1, make_input(1));
        predictor.snapshot_buffer.push(ServerSnapshot {
            tick: 1,
            player_transform: make_transform(0, 0, 0),
            entities: HashMap::new(),
        });

        predictor.reset();

        assert_eq!(predictor.pending_input_count(), 0);
        assert_eq!(predictor.snapshot_buffer.len(), 0);
        assert_eq!(predictor.client_tick(), 0);
    }
}
