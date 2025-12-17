#![warn(missing_docs)]
//! Deterministic testing surfaces (event stream + replay plumbing scaffolding).

mod metrics;
mod micro_worldtest;
mod snapshot;

use anyhow::Result;
use mdminecraft_core::SimTick;
use serde::Serialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub use metrics::*;
pub use micro_worldtest::*;
pub use snapshot::*;

/// Primary event record captured by headless tests.
#[derive(Debug, Serialize)]
pub struct EventRecord<'a> {
    /// Simulation tick when the event occurred.
    pub tick: SimTick,
    /// Human-readable kind label.
    pub kind: &'a str,
    /// Free-form payload for smoke tests.
    pub payload: &'a str,
}

/// A sink that writes newline-delimited JSON to disk.
pub struct JsonlSink {
    file: File,
}

impl JsonlSink {
    /// Create a new sink at `path`.
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::create(path)?;
        Ok(Self { file })
    }

    /// Append an event to the log.
    pub fn write(&mut self, event: &EventRecord<'_>) -> Result<()> {
        let line = serde_json::to_string(event)?;
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        Ok(())
    }
}

/// Mesh metric snapshot for a chunk.
#[derive(Debug, Serialize)]
pub struct ChunkMeshMetric {
    /// Chunk coordinates [x, z].
    pub chunk: [i32; 2],
    /// Triangle count for the chunk mesh.
    pub triangles: usize,
    /// Mesh hash (hex string) for deterministic comparisons.
    pub hash: String,
}

/// Writes chunk mesh metrics to JSON for CI artifacts.
pub struct MeshMetricSink {
    file: File,
}

impl MeshMetricSink {
    /// Create a sink pointed at the supplied path, creating parent dirs if needed.
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self {
            file: File::create(path)?,
        })
    }

    /// Persist the provided metrics as pretty JSON.
    pub fn write(&mut self, metrics: &[ChunkMeshMetric]) -> Result<()> {
        let json = serde_json::to_string_pretty(metrics)?;
        self.file.write_all(json.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn mesh_metric_sink_writes_file() {
        let path = std::env::temp_dir().join(format!(
            "mesh-metrics-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let metrics = vec![ChunkMeshMetric {
            chunk: [0, 0],
            triangles: 12,
            hash: "deadbeef".into(),
        }];
        let mut sink = MeshMetricSink::create(&path).expect("sink create");
        sink.write(&metrics).expect("write succeeds");
        let contents = fs::read_to_string(&path).expect("file readable");
        assert!(contents.contains("deadbeef"));
        assert!(contents.contains("triangles"));
    }
}
