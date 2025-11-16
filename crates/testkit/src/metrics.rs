//! Standardized metrics collection and reporting for CI/CD integration.
//!
//! This module defines a comprehensive metrics schema for tracking performance,
//! quality, and system behavior across all subsystems. Metrics are exported as
//! JSON for automated analysis, regression detection, and dashboarding.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Top-level metrics report containing all subsystem metrics.
///
/// This is the standardized format for metrics.json files exported by tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsReport {
    /// Test/benchmark identifier
    pub test_name: String,

    /// Timestamp when metrics were collected (ISO 8601)
    pub timestamp: String,

    /// Git commit hash (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,

    /// Overall test result
    pub result: TestResult,

    /// Terrain generation metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terrain: Option<TerrainMetrics>,

    /// Lighting system metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lighting: Option<LightingMetrics>,

    /// Mob simulation metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobs: Option<MobMetrics>,

    /// Dropped item metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<ItemMetrics>,

    /// Rendering/meshing metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendering: Option<RenderMetrics>,

    /// Network performance metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkMetrics>,

    /// Persistence/save metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence: Option<PersistenceMetrics>,

    /// Test execution metrics
    pub test_execution: TestExecutionMetrics,
}

/// Overall test result status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestResult {
    /// Test passed all validations
    Pass,
    /// Test failed
    Fail,
    /// Test was skipped
    Skip,
}

/// Terrain generation performance and quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainMetrics {
    /// Total chunks generated
    pub chunks_generated: usize,

    /// Total blocks generated
    pub blocks_generated: usize,

    /// Average generation time per chunk (microseconds)
    pub avg_gen_time_us: f64,

    /// Min generation time (microseconds)
    pub min_gen_time_us: u128,

    /// Max generation time (microseconds)
    pub max_gen_time_us: u128,

    /// Total generation time (milliseconds)
    pub total_gen_time_ms: f64,

    /// Chunks per second throughput
    pub chunks_per_second: f64,

    /// Number of unique biomes present
    pub unique_biomes: usize,

    /// Chunk seam validation results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seam_validation: Option<SeamValidation>,
}

/// Chunk boundary seam validation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeamValidation {
    /// Total seams checked
    pub total_seams: usize,

    /// Seams that passed validation
    pub seams_valid: usize,

    /// Seams that failed validation
    pub seams_failed: usize,

    /// Maximum height difference observed at seams
    pub max_seam_diff: i32,

    /// Average height difference at seams
    pub avg_seam_diff: f64,
}

/// Lighting system performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingMetrics {
    /// Total light propagation operations
    pub total_operations: usize,

    /// Average time per propagation (microseconds)
    pub avg_propagation_time_us: f64,

    /// Total voxels processed
    pub voxels_processed: usize,

    /// Light updates per second
    pub updates_per_second: f64,
}

/// Mob simulation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobMetrics {
    /// Total mobs spawned
    pub total_spawned: usize,

    /// Total mob updates processed
    pub total_updates: usize,

    /// Average update time per mob (microseconds)
    pub avg_update_time_us: f64,

    /// Mobs alive at end of test
    pub mobs_alive: usize,

    /// Breakdown by mob type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_type: Option<HashMap<String, usize>>,
}

/// Dropped item simulation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemMetrics {
    /// Total items spawned
    pub total_spawned: usize,

    /// Total item updates processed
    pub total_updates: usize,

    /// Average update time per item (microseconds)
    pub avg_update_time_us: f64,

    /// Items active at end of test
    pub items_active: usize,

    /// Items despawned
    pub items_despawned: usize,

    /// Items merged (stacking)
    pub items_merged: usize,
}

/// Rendering and meshing performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderMetrics {
    /// Total chunks meshed
    pub chunks_meshed: usize,

    /// Average meshing time per chunk (microseconds)
    pub avg_mesh_time_us: f64,

    /// Total triangles generated
    pub total_triangles: usize,

    /// Average triangles per chunk
    pub avg_triangles_per_chunk: f64,

    /// Total vertices generated
    pub total_vertices: usize,

    /// Mesh cache hit rate (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit_rate: Option<f64>,
}

/// Network performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Total messages sent
    pub messages_sent: usize,

    /// Total messages received
    pub messages_received: usize,

    /// Total bytes sent (uncompressed)
    pub bytes_sent_uncompressed: u64,

    /// Total bytes sent (compressed)
    pub bytes_sent_compressed: u64,

    /// Compression ratio
    pub compression_ratio: f64,

    /// Average message encoding time (microseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_encode_time_us: Option<f64>,

    /// Average message decoding time (microseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_decode_time_us: Option<f64>,

    /// Prediction mismatches (client-server)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction_mismatches: Option<u64>,
}

/// Persistence and save/load metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceMetrics {
    /// Chunks saved
    pub chunks_saved: usize,

    /// Chunks loaded
    pub chunks_loaded: usize,

    /// Average save time per chunk (microseconds)
    pub avg_save_time_us: f64,

    /// Average load time per chunk (microseconds)
    pub avg_load_time_us: f64,

    /// Total bytes written
    pub bytes_written: u64,

    /// Total bytes read
    pub bytes_read: u64,

    /// Compression ratio (save files)
    pub compression_ratio: f64,
}

/// Test execution and infrastructure metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecutionMetrics {
    /// Total test duration (seconds)
    pub duration_seconds: f64,

    /// Memory usage peak (MB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_memory_mb: Option<f64>,

    /// Number of assertions checked
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertions_checked: Option<usize>,

    /// Number of validations passed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validations_passed: Option<usize>,
}

/// Builder for constructing metrics reports
pub struct MetricsReportBuilder {
    report: MetricsReport,
}

impl MetricsReportBuilder {
    /// Create a new builder with test name
    pub fn new(test_name: impl Into<String>) -> Self {
        Self {
            report: MetricsReport {
                test_name: test_name.into(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                commit_hash: None,
                result: TestResult::Pass,
                terrain: None,
                lighting: None,
                mobs: None,
                items: None,
                rendering: None,
                network: None,
                persistence: None,
                test_execution: TestExecutionMetrics {
                    duration_seconds: 0.0,
                    peak_memory_mb: None,
                    assertions_checked: None,
                    validations_passed: None,
                },
            },
        }
    }

    /// Set test result
    pub fn result(mut self, result: TestResult) -> Self {
        self.report.result = result;
        self
    }

    /// Set commit hash
    pub fn commit_hash(mut self, hash: impl Into<String>) -> Self {
        self.report.commit_hash = Some(hash.into());
        self
    }

    /// Set terrain metrics
    pub fn terrain(mut self, metrics: TerrainMetrics) -> Self {
        self.report.terrain = Some(metrics);
        self
    }

    /// Set lighting metrics
    pub fn lighting(mut self, metrics: LightingMetrics) -> Self {
        self.report.lighting = Some(metrics);
        self
    }

    /// Set mob metrics
    pub fn mobs(mut self, metrics: MobMetrics) -> Self {
        self.report.mobs = Some(metrics);
        self
    }

    /// Set item metrics
    pub fn items(mut self, metrics: ItemMetrics) -> Self {
        self.report.items = Some(metrics);
        self
    }

    /// Set render metrics
    pub fn rendering(mut self, metrics: RenderMetrics) -> Self {
        self.report.rendering = Some(metrics);
        self
    }

    /// Set network metrics
    pub fn network(mut self, metrics: NetworkMetrics) -> Self {
        self.report.network = Some(metrics);
        self
    }

    /// Set persistence metrics
    pub fn persistence(mut self, metrics: PersistenceMetrics) -> Self {
        self.report.persistence = Some(metrics);
        self
    }

    /// Set test execution metrics
    pub fn execution(mut self, metrics: TestExecutionMetrics) -> Self {
        self.report.test_execution = metrics;
        self
    }

    /// Build the metrics report
    pub fn build(self) -> MetricsReport {
        self.report
    }
}

/// Sink for writing metrics reports to JSON files
pub struct MetricsSink {
    path: std::path::PathBuf,
}

impl MetricsSink {
    /// Create a new metrics sink at the specified path
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(Self { path })
    }

    /// Write metrics report to file
    pub fn write(&self, report: &MetricsReport) -> Result<()> {
        let json = serde_json::to_string_pretty(report)?;
        let mut file = File::create(&self.path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn metrics_report_roundtrip() {
        let report = MetricsReportBuilder::new("test_example")
            .result(TestResult::Pass)
            .terrain(TerrainMetrics {
                chunks_generated: 100,
                blocks_generated: 409600,
                avg_gen_time_us: 3970.0,
                min_gen_time_us: 2500,
                max_gen_time_us: 8000,
                total_gen_time_ms: 397.0,
                chunks_per_second: 252.0,
                unique_biomes: 8,
                seam_validation: Some(SeamValidation {
                    total_seams: 400,
                    seams_valid: 400,
                    seams_failed: 0,
                    max_seam_diff: 12,
                    avg_seam_diff: 3.5,
                }),
            })
            .execution(TestExecutionMetrics {
                duration_seconds: 2.5,
                peak_memory_mb: Some(128.5),
                assertions_checked: Some(500),
                validations_passed: Some(500),
            })
            .build();

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&report).unwrap();

        // Deserialize back
        let parsed: MetricsReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.test_name, "test_example");
        assert_eq!(parsed.result, TestResult::Pass);
        assert!(parsed.terrain.is_some());
        assert_eq!(parsed.terrain.as_ref().unwrap().chunks_generated, 100);
    }

    #[test]
    fn metrics_sink_writes_file() {
        let path = std::env::temp_dir().join(format!(
            "metrics-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let report = MetricsReportBuilder::new("sink_test")
            .result(TestResult::Pass)
            .execution(TestExecutionMetrics {
                duration_seconds: 1.0,
                peak_memory_mb: None,
                assertions_checked: None,
                validations_passed: None,
            })
            .build();

        let sink = MetricsSink::create(&path).unwrap();
        sink.write(&report).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("sink_test"));
        assert!(contents.contains("\"result\": \"pass\""));

        // Cleanup
        fs::remove_file(&path).ok();
    }
}
