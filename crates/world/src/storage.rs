use std::collections::HashMap;
use std::num::NonZeroUsize;

use lru::LruCache;

use crate::{Chunk, ChunkPos};

/// In-memory chunk arena with an LRU eviction policy.
pub struct ChunkStorage {
    chunks: HashMap<ChunkPos, Chunk>,
    lru: LruCache<ChunkPos, ()>,
    capacity: usize,
}

impl ChunkStorage {
    /// Create a storage with the desired maximum chunk count.
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).expect("capacity > 0");
        Self {
            chunks: HashMap::new(),
            lru: LruCache::new(cap),
            capacity,
        }
    }

    /// Number of resident chunks.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Obtain mutable access to a chunk, creating it if necessary.
    pub fn ensure_chunk(&mut self, pos: ChunkPos) -> &mut Chunk {
        if !self.chunks.contains_key(&pos) {
            self.evict_if_needed();
            let chunk = Chunk::new(pos);
            self.chunks.insert(pos, chunk);
        }
        self.touch(pos);
        self.chunks.get_mut(&pos).expect("chunk present")
    }

    /// Attempt to fetch a chunk immutably.
    pub fn get(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }

    /// Fetch a chunk mutably (without creating it).
    pub fn get_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
        if self.chunks.contains_key(&pos) {
            self.touch(pos);
        }
        self.chunks.get_mut(&pos)
    }

    /// Iterate over currently resident chunk positions.
    pub fn iter_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.chunks.keys().copied()
    }

    fn touch(&mut self, pos: ChunkPos) {
        self.lru.put(pos, ());
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
}
