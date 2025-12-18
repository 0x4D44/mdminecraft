//! Deterministic snapshot testing utilities.
//!
//! This module provides a minimal "golden file" snapshot helper for tests.
//! Snapshots are serialized as canonical pretty JSON with object keys sorted.
//!
//! By default, tests compare against the golden file on disk. To update goldens,
//! rerun with `MDM_UPDATE_SNAPSHOTS=1`.

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Environment variable that enables snapshot updates.
pub const UPDATE_SNAPSHOTS_ENV: &str = "MDM_UPDATE_SNAPSHOTS";

/// Assert that `value` matches the JSON snapshot stored at `path`.
///
/// If `MDM_UPDATE_SNAPSHOTS=1` is set, the snapshot file is written/overwritten
/// with the current value instead.
pub fn assert_json_snapshot<P: AsRef<Path>, T: Serialize>(path: P, value: &T) -> Result<()> {
    let path = path.as_ref();
    let actual = canonical_json(value)?;

    if should_update_snapshots() {
        write_snapshot(path, &actual)?;
        return Ok(());
    }

    let expected = fs::read_to_string(path).with_context(|| {
        format!(
            "Snapshot missing at {} (run with {}=1 to create/update)",
            path.display(),
            UPDATE_SNAPSHOTS_ENV
        )
    })?;

    if expected != actual {
        anyhow::bail!(
            "Snapshot mismatch at {} (run with {}=1 to update)",
            path.display(),
            UPDATE_SNAPSHOTS_ENV
        );
    }

    Ok(())
}

fn should_update_snapshots() -> bool {
    matches!(
        std::env::var(UPDATE_SNAPSHOTS_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

fn write_snapshot(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create snapshot directory {}", parent.display()))?;
    }
    fs::write(path, contents)
        .with_context(|| format!("Failed to write snapshot {}", path.display()))
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value).context("Failed to serialize snapshot value")?;
    let value = canonicalize_value(value);
    let mut s = serde_json::to_string_pretty(&value).context("Failed to format snapshot JSON")?;
    s.push('\n');
    Ok(s)
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = serde_json::Map::with_capacity(entries.len());
            for (k, v) in entries {
                out.insert(k, canonicalize_value(v));
            }
            Value::Object(out)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_value).collect()),
        other => other,
    }
}
