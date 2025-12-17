//! Entity state replication with delta encoding and visibility tracking.
//!
//! Efficiently replicates entity state from server to clients using delta encoding.

use crate::protocol::{EntityDeltaMessage, EntityId, EntityUpdate, EntityUpdateType, Transform};
use std::collections::{BTreeMap, BTreeSet};

/// Tracks entity state for delta encoding.
/// Uses BTreeMap/BTreeSet for deterministic iteration order (critical for multiplayer sync).
pub struct EntityReplicationTracker {
    /// Last known state for each entity (per client).
    /// BTreeMap ensures deterministic iteration order.
    last_states: BTreeMap<EntityId, EntityState>,

    /// Entities visible to this client.
    /// BTreeSet ensures deterministic iteration order.
    visible_entities: BTreeSet<EntityId>,

    /// View distance in chunks.
    view_distance: u32,
}

/// Cached entity state for delta encoding.
///
/// This is deliberately a small, quantized representation suitable for network
/// delta encoding and deterministic iteration.
#[derive(Debug, Clone)]
pub struct EntityState {
    transform: Transform,
    health: Option<(f32, f32)>, // (current, max)
    entity_type: String,
}

impl EntityReplicationTracker {
    /// Create a new entity replication tracker.
    pub fn new(view_distance: u32) -> Self {
        Self {
            last_states: BTreeMap::new(),
            visible_entities: BTreeSet::new(),
            view_distance,
        }
    }

    /// Update visibility based on player position.
    ///
    /// Entities within view_distance chunks of the player are visible.
    pub fn update_visibility(
        &mut self,
        player_pos: &Transform,
        entity_positions: &BTreeMap<EntityId, Transform>,
    ) {
        let player_dimension = player_pos.dimension;
        let player_chunk_x = player_pos.x / (16 * 16); // 16 blocks * 16 subunits
        let player_chunk_z = player_pos.z / (16 * 16);

        let mut new_visible = BTreeSet::new();

        for (&entity_id, entity_pos) in entity_positions {
            if entity_pos.dimension != player_dimension {
                continue;
            }
            let entity_chunk_x = entity_pos.x / (16 * 16);
            let entity_chunk_z = entity_pos.z / (16 * 16);

            let dx = (entity_chunk_x - player_chunk_x).abs();
            let dz = (entity_chunk_z - player_chunk_z).abs();
            let distance = dx.max(dz) as u32;

            if distance <= self.view_distance {
                new_visible.insert(entity_id);
            }
        }

        self.visible_entities = new_visible;
    }

    /// Generate entity delta message for visible entities.
    ///
    /// Only includes entities that have changed since last update.
    pub fn generate_delta(
        &mut self,
        tick: u64,
        entities: &BTreeMap<EntityId, EntityState>,
    ) -> EntityDeltaMessage {
        let mut updates = Vec::new();

        // Process visible entities
        for &entity_id in &self.visible_entities {
            if let Some(current_state) = entities.get(&entity_id) {
                if let Some(last_state) = self.last_states.get(&entity_id) {
                    // Entity exists - check for changes
                    if Self::has_changed(last_state, current_state) {
                        updates.push(Self::create_update(entity_id, last_state, current_state));
                    }
                } else {
                    // New entity - send spawn update
                    updates.push(EntityUpdate {
                        entity_id,
                        update: EntityUpdateType::Spawn {
                            transform: current_state.transform.clone(),
                            entity_type: current_state.entity_type.clone(),
                        },
                    });
                }

                // Update cached state
                self.last_states.insert(entity_id, current_state.clone());
            }
        }

        // Check for despawned entities (in cache but not in visible set)
        let mut despawned = Vec::new();
        for entity_id in self.last_states.keys() {
            if !self.visible_entities.contains(entity_id) {
                despawned.push(*entity_id);
                updates.push(EntityUpdate {
                    entity_id: *entity_id,
                    update: EntityUpdateType::Despawn,
                });
            }
        }

        // Remove despawned entities from cache
        for entity_id in despawned {
            self.last_states.remove(&entity_id);
        }

        EntityDeltaMessage {
            tick,
            entities: updates,
        }
    }

    /// Check if entity state has changed significantly.
    fn has_changed(last: &EntityState, current: &EntityState) -> bool {
        // Check transform (already quantized, so any change matters)
        if last.transform.dimension != current.transform.dimension
            || last.transform.x != current.transform.x
            || last.transform.y != current.transform.y
            || last.transform.z != current.transform.z
            || last.transform.yaw != current.transform.yaw
            || last.transform.pitch != current.transform.pitch
        {
            return true;
        }

        // Check health
        if last.health != current.health {
            return true;
        }

        false
    }

    /// Create entity update from state difference.
    fn create_update(
        entity_id: EntityId,
        last: &EntityState,
        current: &EntityState,
    ) -> EntityUpdate {
        // For now, always send full transform if anything changed
        // Future optimization: send only changed fields
        if last.health != current.health && current.health.is_some() {
            let (current_health, max_health) = current.health.unwrap();
            EntityUpdate {
                entity_id,
                update: EntityUpdateType::Health {
                    current: current_health,
                    max: max_health,
                },
            }
        } else {
            EntityUpdate {
                entity_id,
                update: EntityUpdateType::Transform(current.transform.clone()),
            }
        }
    }

    /// Get number of tracked entities.
    pub fn tracked_count(&self) -> usize {
        self.last_states.len()
    }

    /// Get number of visible entities.
    pub fn visible_count(&self) -> usize {
        self.visible_entities.len()
    }

    /// Clear all tracked state.
    pub fn clear(&mut self) {
        self.last_states.clear();
        self.visible_entities.clear();
    }
}

/// Helper to create entity state for testing.
pub fn create_entity_state(
    transform: Transform,
    entity_type: &str,
    health: Option<(f32, f32)>,
) -> EntityState {
    EntityState {
        transform,
        health,
        entity_type: entity_type.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_core::DimensionId;

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
    fn test_visibility_within_range() {
        let mut tracker = EntityReplicationTracker::new(5);

        let player_pos = make_transform(0, 0, 0);
        let mut entities = BTreeMap::new();
        entities.insert(1, make_transform(16 * 16, 0, 0)); // 1 chunk away
        entities.insert(2, make_transform(16 * 16 * 10, 0, 0)); // 10 chunks away

        tracker.update_visibility(&player_pos, &entities);

        assert_eq!(tracker.visible_count(), 1);
        assert!(tracker.visible_entities.contains(&1));
        assert!(!tracker.visible_entities.contains(&2));
    }

    #[test]
    fn test_spawn_update() {
        let mut tracker = EntityReplicationTracker::new(5);
        tracker.visible_entities.insert(1);

        let mut entities = BTreeMap::new();
        let state = create_entity_state(
            make_transform(100, 200, 300),
            "Player",
            Some((100.0, 100.0)),
        );
        entities.insert(1, state);

        let delta = tracker.generate_delta(1000, &entities);

        assert_eq!(delta.tick, 1000);
        assert_eq!(delta.entities.len(), 1);

        match &delta.entities[0].update {
            EntityUpdateType::Spawn { entity_type, .. } => {
                assert_eq!(entity_type, "Player");
            }
            _ => panic!("Expected Spawn update"),
        }
    }

    #[test]
    fn test_transform_update() {
        let mut tracker = EntityReplicationTracker::new(5);
        tracker.visible_entities.insert(1);

        // Initial state
        let mut entities = BTreeMap::new();
        let state1 = create_entity_state(make_transform(100, 200, 300), "Player", None);
        entities.insert(1, state1);

        let delta1 = tracker.generate_delta(1000, &entities);
        assert_eq!(delta1.entities.len(), 1); // Spawn

        // Move entity
        let state2 = create_entity_state(make_transform(200, 200, 300), "Player", None);
        entities.insert(1, state2);

        let delta2 = tracker.generate_delta(1001, &entities);
        assert_eq!(delta2.entities.len(), 1); // Transform update

        match &delta2.entities[0].update {
            EntityUpdateType::Transform(transform) => {
                assert_eq!(transform.x, 200);
            }
            _ => panic!("Expected Transform update"),
        }
    }

    #[test]
    fn test_despawn_update() {
        let mut tracker = EntityReplicationTracker::new(5);
        tracker.visible_entities.insert(1);

        // Spawn entity
        let mut entities = BTreeMap::new();
        let state = create_entity_state(make_transform(100, 200, 300), "Player", None);
        entities.insert(1, state);

        tracker.generate_delta(1000, &entities);

        // Remove from visibility
        tracker.visible_entities.remove(&1);

        let delta = tracker.generate_delta(1001, &entities);

        assert_eq!(delta.entities.len(), 1);
        match &delta.entities[0].update {
            EntityUpdateType::Despawn => {}
            _ => panic!("Expected Despawn update"),
        }

        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_no_update_when_unchanged() {
        let mut tracker = EntityReplicationTracker::new(5);
        tracker.visible_entities.insert(1);

        let mut entities = BTreeMap::new();
        let state = create_entity_state(make_transform(100, 200, 300), "Player", None);
        entities.insert(1, state.clone());

        // First update - spawn
        let delta1 = tracker.generate_delta(1000, &entities);
        assert_eq!(delta1.entities.len(), 1);

        // Second update - no changes
        let delta2 = tracker.generate_delta(1001, &entities);
        assert_eq!(delta2.entities.len(), 0);
    }

    #[test]
    fn test_health_update() {
        let mut tracker = EntityReplicationTracker::new(5);
        tracker.visible_entities.insert(1);

        let mut entities = BTreeMap::new();
        let state1 = create_entity_state(
            make_transform(100, 200, 300),
            "Player",
            Some((100.0, 100.0)),
        );
        entities.insert(1, state1);

        tracker.generate_delta(1000, &entities);

        // Damage entity
        let state2 =
            create_entity_state(make_transform(100, 200, 300), "Player", Some((50.0, 100.0)));
        entities.insert(1, state2);

        let delta = tracker.generate_delta(1001, &entities);

        assert_eq!(delta.entities.len(), 1);
        match &delta.entities[0].update {
            EntityUpdateType::Health { current, max } => {
                assert_eq!(*current, 50.0);
                assert_eq!(*max, 100.0);
            }
            _ => panic!("Expected Health update"),
        }
    }

    #[test]
    fn test_clear() {
        let mut tracker = EntityReplicationTracker::new(5);
        tracker.visible_entities.insert(1);

        let mut entities = BTreeMap::new();
        let state = create_entity_state(make_transform(100, 200, 300), "Player", None);
        entities.insert(1, state);

        tracker.generate_delta(1000, &entities);
        assert_eq!(tracker.tracked_count(), 1);

        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
        assert_eq!(tracker.visible_count(), 0);
    }

    #[test]
    fn test_deterministic_iteration() {
        // Verify that iteration order is deterministic
        let mut tracker = EntityReplicationTracker::new(5);

        // Insert entities in random order
        for id in [5, 2, 8, 1, 9, 3, 7, 4, 6] {
            tracker.visible_entities.insert(id);
        }

        // Collect order multiple times - should always be the same
        let order1: Vec<EntityId> = tracker.visible_entities.iter().copied().collect();
        let order2: Vec<EntityId> = tracker.visible_entities.iter().copied().collect();

        assert_eq!(order1, order2);
        // BTreeSet should iterate in sorted order
        assert_eq!(order1, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
