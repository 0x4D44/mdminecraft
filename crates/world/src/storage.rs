use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use lru::LruCache;

use mdminecraft_core::DimensionId;

use crate::{Chunk, ChunkKey, ChunkPos};

/// In-memory chunk arena with an LRU eviction policy.
/// Uses BTreeMap for deterministic iteration order (critical for multiplayer sync).
pub struct ChunkStorage {
    /// Chunks stored with deterministic key ordering.
    chunks: BTreeMap<ChunkKey, Chunk>,
    lru: LruCache<ChunkKey, ()>,
    capacity: usize,
}

impl ChunkStorage {
    /// Create a storage with the desired maximum chunk count.
    pub fn new(capacity: usize) -> Self {
        // Capacity is always at least 1, so NonZeroUsize is guaranteed valid
        let cap = NonZeroUsize::new(capacity.max(1)).expect("capacity is at least 1 after max(1)");
        Self {
            chunks: BTreeMap::new(),
            lru: LruCache::new(cap),
            capacity,
        }
    }

    /// Number of resident chunks.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Returns true when no chunks are currently stored.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Obtain mutable access to a chunk, creating it if necessary.
    pub fn ensure_chunk(&mut self, pos: ChunkPos) -> &mut Chunk {
        self.ensure_chunk_in_dimension(DimensionId::DEFAULT, pos)
    }

    /// Obtain mutable access to a chunk in the specified dimension, creating it if necessary.
    pub fn ensure_chunk_in_dimension(
        &mut self,
        dimension: DimensionId,
        pos: ChunkPos,
    ) -> &mut Chunk {
        let key = ChunkKey::from_pos(dimension, pos);
        if !self.chunks.contains_key(&key) {
            self.evict_if_needed();
            let chunk = Chunk::new(pos);
            self.chunks.insert(key, chunk);
        }
        self.touch(key);
        self.chunks.get_mut(&key).expect("chunk present")
    }

    /// Attempt to fetch a chunk immutably.
    pub fn get(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.get_in_dimension(DimensionId::DEFAULT, pos)
    }

    /// Attempt to fetch a chunk immutably in the specified dimension.
    pub fn get_in_dimension(&self, dimension: DimensionId, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&ChunkKey::from_pos(dimension, pos))
    }

    /// Fetch a chunk mutably (without creating it).
    pub fn get_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
        self.get_mut_in_dimension(DimensionId::DEFAULT, pos)
    }

    /// Fetch a chunk mutably in the specified dimension (without creating it).
    pub fn get_mut_in_dimension(
        &mut self,
        dimension: DimensionId,
        pos: ChunkPos,
    ) -> Option<&mut Chunk> {
        let key = ChunkKey::from_pos(dimension, pos);
        if self.chunks.contains_key(&key) {
            self.touch(key);
        }
        self.chunks.get_mut(&key)
    }

    /// Iterate over currently resident chunk positions.
    pub fn iter_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.chunks
            .keys()
            .filter(|key| key.dimension == DimensionId::DEFAULT)
            .map(|key| key.pos)
    }

    /// Iterate over currently resident chunk keys (all dimensions).
    pub fn iter_keys(&self) -> impl Iterator<Item = ChunkKey> + '_ {
        self.chunks.keys().copied()
    }

    fn touch(&mut self, key: ChunkKey) {
        self.lru.put(key, ());
    }

    fn evict_if_needed(&mut self) {
        while self.chunks.len() >= self.capacity {
            if let Some((oldest, _)) = self.lru.pop_lru() {
                self.chunks.remove(&oldest);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_evicts_old_chunks() {
        let mut storage = ChunkStorage::new(2);
        let a = ChunkPos::new(0, 0);
        let b = ChunkPos::new(1, 0);
        let c = ChunkPos::new(2, 0);
        storage.ensure_chunk(a);
        storage.ensure_chunk(b);
        assert_eq!(storage.len(), 2);
        storage.ensure_chunk(c);
        assert_eq!(storage.len(), 2);
        // `a` should have been evicted (least recently used).
        assert!(storage.get(a).is_none());
        assert!(storage.get(b).is_some());
        assert!(storage.get(c).is_some());
    }

    #[test]
    fn iter_positions_covers_resident_chunks() {
        let mut storage = ChunkStorage::new(2);
        storage.ensure_chunk(ChunkPos::new(0, 0));
        storage.ensure_chunk(ChunkPos::new(1, 0));
        let positions: Vec<_> = storage.iter_positions().collect();
        assert_eq!(positions.len(), storage.len());
    }

    #[test]
    fn iter_positions_is_deterministic() {
        // BTreeMap provides deterministic iteration order
        let mut storage = ChunkStorage::new(10);

        // Insert in non-sorted order
        storage.ensure_chunk(ChunkPos::new(5, 5));
        storage.ensure_chunk(ChunkPos::new(1, 2));
        storage.ensure_chunk(ChunkPos::new(3, 0));
        storage.ensure_chunk(ChunkPos::new(0, 0));
        storage.ensure_chunk(ChunkPos::new(2, 1));

        // Collect positions multiple times - should always be same order
        let order1: Vec<_> = storage.iter_positions().collect();
        let order2: Vec<_> = storage.iter_positions().collect();

        assert_eq!(order1, order2);

        // BTreeMap should iterate in sorted order (by ChunkPos's Ord impl)
        // ChunkPos is (x, z) so it sorts by x first, then z
        let expected = vec![
            ChunkPos::new(0, 0),
            ChunkPos::new(1, 2),
            ChunkPos::new(2, 1),
            ChunkPos::new(3, 0),
            ChunkPos::new(5, 5),
        ];
        assert_eq!(order1, expected);
    }

    #[test]
    fn get_returns_none_for_missing_chunk() {
        let storage = ChunkStorage::new(2);
        assert!(storage.get(ChunkPos::new(999, 999)).is_none());
    }

    #[test]
    fn get_mut_returns_none_for_missing_chunk() {
        let mut storage = ChunkStorage::new(2);
        assert!(storage.get_mut(ChunkPos::new(999, 999)).is_none());
    }
}
