//! Micro-worldtest harness for deterministic, tick-based snapshot tests.
//!
//! A micro-worldtest is intentionally small: it steps a tiny simulation for a
//! fixed number of ticks and snapshots selected state each tick. The resulting
//! report is compared against a golden JSON file on disk (or updated when
//! `MDM_UPDATE_SNAPSHOTS=1` is set).

use crate::snapshot::assert_json_snapshot;
use anyhow::Result;
use mdminecraft_core::SimTick;
use serde::Serialize;
use std::path::PathBuf;

/// Configuration for a micro-worldtest.
#[derive(Debug, Clone)]
pub struct MicroWorldtestConfig {
    /// Human-readable name (written into the snapshot report).
    pub name: String,
    /// Number of ticks to step (report includes the initial snapshot at tick 0).
    pub ticks: u64,
    /// Path to the golden JSON file.
    pub snapshot_path: PathBuf,
}

/// Single snapshot frame captured at a given tick.
#[derive(Debug, Clone, Serialize)]
pub struct MicroWorldtestFrame<S> {
    /// Tick number.
    pub tick: u64,
    /// Snapshot payload.
    pub snapshot: S,
}

#[derive(Debug, Clone, Serialize)]
struct MicroWorldtestReport<S> {
    name: String,
    frames: Vec<MicroWorldtestFrame<S>>,
}

/// Run a micro-worldtest and assert (or update) the snapshot at `config.snapshot_path`.
///
/// Captures the initial snapshot at tick 0, then steps `config.ticks` times,
/// capturing a snapshot after each step (so the report contains `ticks + 1` frames).
pub fn run_micro_worldtest<State, Snapshot, StepFn, SnapFn>(
    config: MicroWorldtestConfig,
    mut state: State,
    mut step: StepFn,
    mut snapshot: SnapFn,
) -> Result<()>
where
    Snapshot: Serialize,
    StepFn: FnMut(SimTick, &mut State),
    SnapFn: FnMut(SimTick, &State) -> Snapshot,
{
    let mut frames = Vec::with_capacity(config.ticks as usize + 1);

    let mut tick = SimTick::ZERO;
    frames.push(MicroWorldtestFrame {
        tick: tick.0,
        snapshot: snapshot(tick, &state),
    });

    for _ in 0..config.ticks {
        step(tick, &mut state);
        tick = tick.advance(1);
        frames.push(MicroWorldtestFrame {
            tick: tick.0,
            snapshot: snapshot(tick, &state),
        });
    }

    let report = MicroWorldtestReport {
        name: config.name,
        frames,
    };
    assert_json_snapshot(config.snapshot_path, &report)
}
