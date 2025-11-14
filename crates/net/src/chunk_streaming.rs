//! Chunk streaming with priority-based delivery and bandwidth throttling.
//!
//! Manages efficient chunk data delivery to clients with distance-based priority
//! and configurable bandwidth limits.

use crate::chunk_encoding::encode_chunk_data;
use crate::protocol::{BlockId, ChunkDataMessage};
use anyhow::Result;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::time::{Duration, Instant};

/// Maximum number of chunks to queue per client.
const MAX_QUEUE_SIZE: usize = 256;

/// Default bandwidth limit: 1 MB/s.
const DEFAULT_BANDWIDTH_LIMIT: u64 = 1024 * 1024;

/// Chunk streaming manager for a single client connection.
pub struct ChunkStreamer {
    /// Priority queue of chunks to send (ordered by priority).
    send_queue: BinaryHeap<ChunkPriority>,

    /// Set of chunk coordinates currently queued.
    queued_chunks: HashSet<(i32, i32)>,

    /// Set of chunk coordinates already sent to client.
    sent_chunks: HashSet<(i32, i32)>,

    /// Current player position for priority calculation.
    player_chunk_x: i32,
    player_chunk_z: i32,

    /// Bandwidth throttling state.
    bandwidth_limit: u64,
    bytes_sent_this_second: u64,
    last_reset_time: Instant,

    /// Metrics.
    metrics: StreamingMetrics,
}

/// Priority entry for chunk send queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChunkPriority {
    chunk_x: i32,
    chunk_z: i32,
    priority: u32, // Lower = higher priority (distance from player)
}

impl Ord for ChunkPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering: lower priority value = higher priority
        other.priority.cmp(&self.priority)
    }
}

impl PartialOrd for ChunkPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Streaming metrics for monitoring and debugging.
#[derive(Debug, Clone, Default)]
pub struct StreamingMetrics {
    /// Total bytes sent (before compression).
    pub total_bytes_uncompressed: u64,

    /// Total bytes sent (after compression).
    pub total_bytes_compressed: u64,

    /// Total chunks sent.
    pub chunks_sent: u64,

    /// Current queue size.
    pub queue_size: usize,

    /// Bandwidth utilization (bytes/sec).
    pub bandwidth_used: u64,

    /// Average compression ratio (percentage).
    pub avg_compression_ratio: f32,
}

impl ChunkStreamer {
    /// Create a new chunk streamer with default bandwidth limit.
    pub fn new() -> Self {
        Self::with_bandwidth_limit(DEFAULT_BANDWIDTH_LIMIT)
    }

    /// Create a new chunk streamer with custom bandwidth limit (bytes/sec).
    pub fn with_bandwidth_limit(bandwidth_limit: u64) -> Self {
        Self {
            send_queue: BinaryHeap::new(),
            queued_chunks: HashSet::new(),
            sent_chunks: HashSet::new(),
            player_chunk_x: 0,
            player_chunk_z: 0,
            bandwidth_limit,
            bytes_sent_this_second: 0,
            last_reset_time: Instant::now(),
            metrics: StreamingMetrics::default(),
        }
    }

    /// Update player position and recalculate priorities.
    pub fn set_player_position(&mut self, chunk_x: i32, chunk_z: i32) {
        self.player_chunk_x = chunk_x;
        self.player_chunk_z = chunk_z;

        // Rebuild priority queue with updated priorities
        let chunks: Vec<_> = self.send_queue.drain().collect();
        for chunk in chunks {
            let priority = self.calculate_priority(chunk.chunk_x, chunk.chunk_z);
            self.send_queue.push(ChunkPriority {
                chunk_x: chunk.chunk_x,
                chunk_z: chunk.chunk_z,
                priority,
            });
        }
    }

    /// Enqueue a chunk for sending.
    ///
    /// Returns false if queue is full or chunk already queued/sent.
    pub fn enqueue_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> bool {
        // Check if already queued or sent
        if self.queued_chunks.contains(&(chunk_x, chunk_z))
            || self.sent_chunks.contains(&(chunk_x, chunk_z))
        {
            return false;
        }

        // Check queue size limit
        if self.queued_chunks.len() >= MAX_QUEUE_SIZE {
            return false;
        }

        let priority = self.calculate_priority(chunk_x, chunk_z);

        self.send_queue.push(ChunkPriority {
            chunk_x,
            chunk_z,
            priority,
        });
        self.queued_chunks.insert((chunk_x, chunk_z));

        self.metrics.queue_size = self.queued_chunks.len();

        true
    }

    /// Try to send the next chunk from the queue.
    ///
    /// Returns Some(ChunkDataMessage) if a chunk was sent, None if bandwidth limit reached
    /// or queue is empty.
    pub fn try_send_next_chunk(
        &mut self,
        chunk_data_provider: &dyn Fn(i32, i32) -> Option<Vec<BlockId>>,
    ) -> Result<Option<ChunkDataMessage>> {
        // Reset bandwidth counter if a second has passed
        if self.last_reset_time.elapsed() >= Duration::from_secs(1) {
            self.bytes_sent_this_second = 0;
            self.last_reset_time = Instant::now();
        }

        // Check if queue is empty
        if self.send_queue.is_empty() {
            return Ok(None);
        }

        // Peek at next chunk to estimate size
        let next = self.send_queue.peek().unwrap();
        let chunk_x = next.chunk_x;
        let chunk_z = next.chunk_z;

        // Get chunk data from provider
        let block_data = match chunk_data_provider(chunk_x, chunk_z) {
            Some(data) => data,
            None => {
                // Chunk not available, remove from queue
                self.send_queue.pop();
                self.queued_chunks.remove(&(chunk_x, chunk_z));
                self.metrics.queue_size = self.queued_chunks.len();
                return Ok(None);
            }
        };

        // Encode chunk data
        let encoded = encode_chunk_data(chunk_x, chunk_z, &block_data)?;

        // Calculate compressed size
        let uncompressed_size = 65536 * 2; // 2 bytes per BlockId
        let compressed_size = encoded.compressed_data.len() + encoded.palette.len() * 2;

        // Check bandwidth limit (use compressed size)
        if self.bytes_sent_this_second + compressed_size as u64 > self.bandwidth_limit {
            // Would exceed bandwidth limit, wait for next second
            return Ok(None);
        }

        // Send chunk (remove from queue)
        self.send_queue.pop();
        self.queued_chunks.remove(&(chunk_x, chunk_z));
        self.sent_chunks.insert((chunk_x, chunk_z));

        // Update metrics
        self.bytes_sent_this_second += compressed_size as u64;
        self.metrics.total_bytes_uncompressed += uncompressed_size;
        self.metrics.total_bytes_compressed += compressed_size as u64;
        self.metrics.chunks_sent += 1;
        self.metrics.queue_size = self.queued_chunks.len();
        self.metrics.bandwidth_used = self.bytes_sent_this_second;

        // Calculate average compression ratio
        if self.metrics.total_bytes_uncompressed > 0 {
            self.metrics.avg_compression_ratio = ((self.metrics.total_bytes_uncompressed
                - self.metrics.total_bytes_compressed)
                as f32
                / self.metrics.total_bytes_uncompressed as f32)
                * 100.0;
        }

        Ok(Some(encoded))
    }

    /// Calculate priority for a chunk based on distance from player.
    ///
    /// Lower value = higher priority (closer to player).
    fn calculate_priority(&self, chunk_x: i32, chunk_z: i32) -> u32 {
        let dx = (chunk_x - self.player_chunk_x).abs();
        let dz = (chunk_z - self.player_chunk_z).abs();

        // Chebyshev distance (max of dx, dz)
        dx.max(dz) as u32
    }

    /// Get current streaming metrics.
    pub fn metrics(&self) -> &StreamingMetrics {
        &self.metrics
    }

    /// Get number of chunks in send queue.
    pub fn queue_size(&self) -> usize {
        self.queued_chunks.len()
    }

    /// Get number of chunks sent to client.
    pub fn sent_count(&self) -> usize {
        self.sent_chunks.len()
    }

    /// Check if a chunk has been sent.
    pub fn is_chunk_sent(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.sent_chunks.contains(&(chunk_x, chunk_z))
    }

    /// Clear sent chunk history (useful when client moves far away).
    pub fn clear_sent_history(&mut self) {
        self.sent_chunks.clear();
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.send_queue.clear();
        self.queued_chunks.clear();
        self.sent_chunks.clear();
        self.metrics = StreamingMetrics::default();
        self.bytes_sent_this_second = 0;
        self.last_reset_time = Instant::now();
    }
}

impl Default for ChunkStreamer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uniform_chunk(block_id: BlockId) -> Vec<BlockId> {
        vec![block_id; 65536]
    }

    #[test]
    fn test_enqueue_chunk() {
        let mut streamer = ChunkStreamer::new();

        assert!(streamer.enqueue_chunk(0, 0));
        assert_eq!(streamer.queue_size(), 1);

        // Can't enqueue same chunk twice
        assert!(!streamer.enqueue_chunk(0, 0));
        assert_eq!(streamer.queue_size(), 1);
    }

    #[test]
    fn test_priority_ordering() {
        let mut streamer = ChunkStreamer::new();
        streamer.set_player_position(0, 0);

        // Enqueue chunks at various distances
        streamer.enqueue_chunk(5, 5); // distance = 5
        streamer.enqueue_chunk(1, 1); // distance = 1
        streamer.enqueue_chunk(3, 3); // distance = 3

        // Should dequeue in order of distance: (1,1), (3,3), (5,5)
        let next = streamer.send_queue.peek().unwrap();
        assert_eq!(next.chunk_x, 1);
        assert_eq!(next.chunk_z, 1);
    }

    #[test]
    fn test_send_chunk() {
        let mut streamer = ChunkStreamer::new();
        streamer.enqueue_chunk(0, 0);

        let provider = |x: i32, z: i32| {
            if x == 0 && z == 0 {
                Some(make_uniform_chunk(1))
            } else {
                None
            }
        };

        let result = streamer.try_send_next_chunk(&provider).expect("Send failed");
        assert!(result.is_some());

        let chunk_msg = result.unwrap();
        assert_eq!(chunk_msg.chunk_x, 0);
        assert_eq!(chunk_msg.chunk_z, 0);

        assert_eq!(streamer.queue_size(), 0);
        assert_eq!(streamer.sent_count(), 1);
        assert!(streamer.is_chunk_sent(0, 0));
    }

    #[test]
    fn test_bandwidth_limiting() {
        // Set limit to allow one uniform chunk but not two
        // Uniform chunk compresses to ~1050 bytes (516 RLE runs of 2 bytes each + palette)
        let mut streamer = ChunkStreamer::with_bandwidth_limit(1500);

        // Enqueue multiple chunks
        for i in 0..10 {
            streamer.enqueue_chunk(i, 0);
        }

        let provider = |_: i32, _: i32| Some(make_uniform_chunk(1));

        // First chunk should send
        let result1 = streamer.try_send_next_chunk(&provider).expect("Send failed");
        assert!(result1.is_some());

        // Second chunk should be blocked by bandwidth limit
        let result2 = streamer.try_send_next_chunk(&provider).expect("Send failed");
        assert!(result2.is_none()); // Bandwidth limit reached

        assert_eq!(streamer.sent_count(), 1);
    }

    #[test]
    fn test_metrics() {
        let mut streamer = ChunkStreamer::new();
        streamer.enqueue_chunk(0, 0);

        let provider = |x: i32, z: i32| {
            if x == 0 && z == 0 {
                Some(make_uniform_chunk(1))
            } else {
                None
            }
        };

        streamer.try_send_next_chunk(&provider).expect("Send failed");

        let metrics = streamer.metrics();
        assert_eq!(metrics.chunks_sent, 1);
        assert!(metrics.total_bytes_uncompressed > 0);
        assert!(metrics.total_bytes_compressed > 0);
        assert!(metrics.avg_compression_ratio > 0.0);
    }

    #[test]
    fn test_priority_update_on_player_move() {
        let mut streamer = ChunkStreamer::new();
        streamer.set_player_position(0, 0);

        streamer.enqueue_chunk(5, 5);
        streamer.enqueue_chunk(10, 10);

        // (5,5) should be higher priority
        let next = streamer.send_queue.peek().unwrap();
        assert_eq!(next.chunk_x, 5);

        // Move player closer to (10,10)
        streamer.set_player_position(9, 9);

        // Now (10,10) should be higher priority
        let next = streamer.send_queue.peek().unwrap();
        assert_eq!(next.chunk_x, 10);
    }

    #[test]
    fn test_chunk_not_available() {
        let mut streamer = ChunkStreamer::new();
        streamer.enqueue_chunk(0, 0);

        // Provider returns None
        let provider = |_: i32, _: i32| None;

        let result = streamer.try_send_next_chunk(&provider).expect("Send failed");
        assert!(result.is_none());

        // Chunk should be removed from queue
        assert_eq!(streamer.queue_size(), 0);
    }

    #[test]
    fn test_clear_sent_history() {
        let mut streamer = ChunkStreamer::new();
        streamer.enqueue_chunk(0, 0);

        let provider = |_: i32, _: i32| Some(make_uniform_chunk(1));

        streamer.try_send_next_chunk(&provider).expect("Send failed");
        assert_eq!(streamer.sent_count(), 1);

        streamer.clear_sent_history();
        assert_eq!(streamer.sent_count(), 0);

        // Can now enqueue same chunk again
        assert!(streamer.enqueue_chunk(0, 0));
    }

    #[test]
    fn test_reset() {
        let mut streamer = ChunkStreamer::new();
        streamer.enqueue_chunk(0, 0);
        streamer.enqueue_chunk(1, 1);

        streamer.reset();

        assert_eq!(streamer.queue_size(), 0);
        assert_eq!(streamer.sent_count(), 0);
        assert_eq!(streamer.metrics().chunks_sent, 0);
    }
}
