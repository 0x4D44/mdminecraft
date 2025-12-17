//! Deterministic metadata "components".
//!
//! Vanilla Minecraft persists and synchronizes many gameplay objects using
//! structured metadata (block entities, entities, player state). mdminecraft
//! uses a deterministic, ordered representation to avoid nondeterministic map
//! iteration and unstable serialization.

use crate::RegistryKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Deterministic component map keyed by [`RegistryKey`].
///
/// This is intended for persistence/network payloads, not for hot-path gameplay
/// logic (prefer typed fields there).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentMap {
    components: BTreeMap<RegistryKey, ComponentValue>,
}

impl ComponentMap {
    /// Create an empty component map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a component value.
    pub fn insert(&mut self, key: RegistryKey, value: ComponentValue) -> Option<ComponentValue> {
        self.components.insert(key, value)
    }

    /// Get a component value.
    pub fn get(&self, key: &RegistryKey) -> Option<&ComponentValue> {
        self.components.get(key)
    }

    /// Remove a component value.
    pub fn remove(&mut self, key: &RegistryKey) -> Option<ComponentValue> {
        self.components.remove(key)
    }

    /// Iterate over components in deterministic key order.
    pub fn iter(&self) -> impl Iterator<Item = (&RegistryKey, &ComponentValue)> {
        self.components.iter()
    }

    /// Returns true if the map contains no components.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Number of stored components.
    pub fn len(&self) -> usize {
        self.components.len()
    }
}

/// Deterministic component value.
///
/// Floats are intentionally omitted; gameplay-affecting values should be stored
/// as integers (e.g., fixed-point) to preserve determinism across platforms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ComponentValue {
    /// Boolean value.
    Bool(bool),
    /// Signed integer value.
    I64(i64),
    /// Unsigned integer value.
    U64(u64),
    /// UTF-8 string value.
    String(String),
    /// Opaque bytes (e.g., compressed payload).
    Bytes(Vec<u8>),
    /// Ordered list of values.
    List(Vec<ComponentValue>),
    /// Ordered map of values (string keys).
    Map(BTreeMap<String, ComponentValue>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryKey;

    #[test]
    fn component_map_orders_keys() {
        let mut map = ComponentMap::new();
        map.insert(
            RegistryKey::parse("mdm:b").unwrap(),
            ComponentValue::Bool(true),
        );
        map.insert(
            RegistryKey::parse("mdm:a").unwrap(),
            ComponentValue::Bool(false),
        );

        let keys: Vec<_> = map.iter().map(|(k, _)| k.to_string()).collect();
        assert_eq!(keys, vec!["mdm:a".to_string(), "mdm:b".to_string()]);
    }
}

