use anyhow::{Context, Result};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::warn;

/// Default directory containing content packs.
pub const CONTENT_PACKS_DIR: &str = "content_packs";

/// Content pack manifest file name.
pub const CONTENT_PACK_MANIFEST_FILE: &str = "pack.json";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ContentPackManifest {
    /// Human-friendly pack name (defaults to the directory name).
    pub name: Option<String>,
    /// Optional description, purely informational.
    pub description: Option<String>,
    /// Optional author, purely informational.
    pub author: Option<String>,
    /// Optional version string, purely informational.
    pub version: Option<String>,
    /// If false, the pack is ignored.
    pub enabled: bool,
    /// Deterministic pack load ordering (lower loads earlier).
    pub priority: i32,
}

impl Default for ContentPackManifest {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            author: None,
            version: None,
            enabled: true,
            priority: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredContentPack {
    pub id: String,
    pub dir: PathBuf,
    pub manifest: ContentPackManifest,
}

fn load_manifest_strict(pack_dir: &Path, pack_id: &str) -> Result<ContentPackManifest> {
    let manifest_path = pack_dir.join(CONTENT_PACK_MANIFEST_FILE);
    let mut manifest = if !manifest_path.exists() {
        ContentPackManifest::default()
    } else {
        let contents = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", manifest_path.display()))?
    };

    if manifest.name.as_deref().unwrap_or("").is_empty() {
        manifest.name = Some(pack_id.to_string());
    }

    Ok(manifest)
}

/// Discover content pack directories under the given root.
///
/// Pack discovery is deterministic: directories are returned in sorted order.
pub fn discover_pack_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("Failed to read {}", root.display())),
    };

    let mut dirs = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("Failed to read dir entry in {}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    dirs.sort();
    Ok(dirs)
}

/// Discover content packs under the given root, applying manifest ordering and enablement.
///
/// This function is strict: it errors if a manifest exists but can't be read/parsed.
#[cfg(test)]
pub fn discover_packs_strict(root: &Path) -> Result<Vec<DiscoveredContentPack>> {
    let pack_dirs = discover_pack_dirs(root)?;
    let mut packs = Vec::with_capacity(pack_dirs.len());
    for pack_dir in pack_dirs {
        let id = pack_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| pack_dir.display().to_string());
        let manifest = load_manifest_strict(&pack_dir, &id)?;
        if !manifest.enabled {
            continue;
        }
        packs.push(DiscoveredContentPack {
            id,
            dir: pack_dir,
            manifest,
        });
    }

    packs.sort_by(|a, b| {
        a.manifest
            .priority
            .cmp(&b.manifest.priority)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(packs)
}

/// Discover content packs under the given root, applying manifest ordering and enablement.
///
/// This function is lenient: packs with unreadable/invalid manifests are skipped with a warning.
pub fn discover_packs_lenient(root: &Path) -> Vec<DiscoveredContentPack> {
    let pack_dirs = match discover_pack_dirs(root) {
        Ok(pack_dirs) => pack_dirs,
        Err(err) => {
            warn!(
                "Failed to scan content packs dir {}: {err:#}",
                root.display()
            );
            return Vec::new();
        }
    };

    let mut packs = Vec::with_capacity(pack_dirs.len());
    for pack_dir in pack_dirs {
        let id = pack_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| pack_dir.display().to_string());
        let manifest = match load_manifest_strict(&pack_dir, &id) {
            Ok(manifest) => manifest,
            Err(err) => {
                warn!(
                    "Skipping content pack {} due to invalid manifest: {err:#}",
                    pack_dir.display()
                );
                continue;
            }
        };
        if !manifest.enabled {
            continue;
        }
        packs.push(DiscoveredContentPack {
            id,
            dir: pack_dir,
            manifest,
        });
    }

    packs.sort_by(|a, b| {
        a.manifest
            .priority
            .cmp(&b.manifest.priority)
            .then_with(|| a.id.cmp(&b.id))
    });
    packs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mdminecraft_content_packs_{timestamp}"))
    }

    #[test]
    fn manifests_control_deterministic_pack_order_and_enablement() {
        let root = unique_temp_root();
        fs::create_dir_all(&root).expect("temp root create");

        let pack_b = root.join("b_pack");
        fs::create_dir_all(&pack_b).expect("pack create");
        fs::write(
            pack_b.join(CONTENT_PACK_MANIFEST_FILE),
            r#"{"priority":-5}"#,
        )
        .expect("write manifest");

        let pack_a = root.join("a_pack");
        fs::create_dir_all(&pack_a).expect("pack create");
        fs::write(
            pack_a.join(CONTENT_PACK_MANIFEST_FILE),
            r#"{"priority":10}"#,
        )
        .expect("write manifest");

        // No manifest → defaults.
        let pack_c = root.join("c_pack");
        fs::create_dir_all(&pack_c).expect("pack create");

        // Disabled pack should be ignored.
        let pack_d = root.join("d_pack");
        fs::create_dir_all(&pack_d).expect("pack create");
        fs::write(
            pack_d.join(CONTENT_PACK_MANIFEST_FILE),
            r#"{"enabled":false,"priority":-100}"#,
        )
        .expect("write manifest");

        let packs = discover_packs_strict(&root).expect("discover packs");
        let ids: Vec<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["b_pack", "c_pack", "a_pack"]);

        let c = packs
            .iter()
            .find(|p| p.id == "c_pack")
            .expect("c_pack present");
        assert_eq!(c.manifest.priority, 0);
        assert!(c.manifest.enabled);
        assert_eq!(c.manifest.name.as_deref(), Some("c_pack"));

        let _ = fs::remove_dir_all(&root);
    }
}
