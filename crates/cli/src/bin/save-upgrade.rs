//! Save Upgrade Tool
//!
//! Upgrades on-disk save data to the latest supported formats.
//!
//! Today this focuses on:
//! - Creating `world.meta` if missing (stable world seed).
//! - Rewriting `world.state` using the latest schema version (migrates v1 -> v2).
//!
//! Usage:
//!   save-upgrade --world saves/default
//!   save-upgrade --world saves/default --seed 12345

use anyhow::{Context, Result};
use mdminecraft_world::{RegionStore, WorldMeta};
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::fmt;

#[derive(Debug)]
struct Config {
    world_dir: PathBuf,
    seed: Option<u64>,
    backup: bool,
}

fn print_help() {
    println!("save-upgrade - upgrade mdminecraft save data");
    println!();
    println!("Usage:");
    println!("  save-upgrade [--world <dir>] [--seed <u64>] [--no-backup]");
    println!();
    println!("Options:");
    println!("  --world <dir>     World directory (default: saves/default)");
    println!(
        "  --seed <u64>      Seed to write if world.meta is missing (default: MDM_WORLD_SEED or 0)"
    );
    println!("  --no-backup       Do not write .bak.<nanos> backups before rewriting world.state");
    println!();
}

fn parse_args() -> Result<Config> {
    parse_args_from_iter(std::env::args().skip(1))
}

fn parse_args_from_iter<I>(mut args: I) -> Result<Config>
where
    I: Iterator<Item = String>,
{
    let mut world_dir = PathBuf::from("saves/default");
    let mut seed: Option<u64> = None;
    let mut backup = true;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--world" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--world requires a directory path");
                };
                world_dir = PathBuf::from(value);
            }
            "--seed" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--seed requires a u64 value");
                };
                seed = Some(value.parse::<u64>().context("invalid --seed value")?);
            }
            "--no-backup" => backup = false,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => anyhow::bail!("Unknown argument: {}", other),
        }
    }

    Ok(Config {
        world_dir,
        seed,
        backup,
    })
}

fn resolve_world_seed(explicit: Option<u64>) -> u64 {
    explicit
        .or_else(|| {
            std::env::var("MDM_WORLD_SEED")
                .ok()
                .and_then(|raw| raw.parse::<u64>().ok())
        })
        .unwrap_or(0)
}

fn main() -> Result<()> {
    let _ = fmt().with_max_level(Level::INFO).try_init();
    let config = parse_args()?;

    tracing::info!(world_dir = %config.world_dir.display(), "Opening world directory");
    let store = RegionStore::new(&config.world_dir)
        .with_context(|| format!("failed to open {}", config.world_dir.display()))?;

    if store.world_meta_exists() {
        let meta = store
            .load_world_meta()
            .context("failed to load world.meta")?;
        tracing::info!(world_seed = meta.world_seed, "world.meta OK");
    } else {
        let world_seed = resolve_world_seed(config.seed);

        tracing::warn!(
            world_seed,
            "world.meta missing; creating (pass --seed to override)"
        );
        store
            .save_world_meta(&WorldMeta {
                world_seed,
                end_boss_defeated: false,
            })
            .context("failed to write world.meta")?;
    }

    let world_state_path = config.world_dir.join("world.state");
    if store.world_state_exists() {
        if config.backup && world_state_path.exists() {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let backup_path = config.world_dir.join(format!("world.state.bak.{}", ts));
            std::fs::copy(&world_state_path, &backup_path)
                .with_context(|| format!("failed to create backup {}", backup_path.display()))?;
            tracing::info!(backup = %backup_path.display(), "Backed up world.state");
        }

        let state = store
            .load_world_state()
            .context("failed to load world.state")?;
        store
            .save_world_state(&state)
            .context("failed to write upgraded world.state")?;
        tracing::info!("Upgraded world.state (rewritten using latest schema)");
    } else {
        tracing::info!("No world.state found; nothing to upgrade yet");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_defaults() {
        let config = parse_args_from_iter(std::iter::empty()).expect("config");
        assert_eq!(config.world_dir, PathBuf::from("saves/default"));
        assert!(config.seed.is_none());
        assert!(config.backup);
    }

    #[test]
    fn parse_args_overrides() {
        let args = vec![
            "--world".to_string(),
            "saves/test".to_string(),
            "--seed".to_string(),
            "42".to_string(),
            "--no-backup".to_string(),
        ];
        let config = parse_args_from_iter(args.into_iter()).expect("config");
        assert_eq!(config.world_dir, PathBuf::from("saves/test"));
        assert_eq!(config.seed, Some(42));
        assert!(!config.backup);
    }

    #[test]
    fn resolve_world_seed_prefers_explicit() {
        let original = std::env::var("MDM_WORLD_SEED").ok();
        std::env::set_var("MDM_WORLD_SEED", "99");
        assert_eq!(resolve_world_seed(Some(7)), 7);
        assert_eq!(resolve_world_seed(None), 99);
        match original {
            Some(value) => std::env::set_var("MDM_WORLD_SEED", value),
            None => std::env::remove_var("MDM_WORLD_SEED"),
        }
    }

    #[test]
    fn resolve_world_seed_invalid_env_falls_back() {
        let original = std::env::var("MDM_WORLD_SEED").ok();
        std::env::set_var("MDM_WORLD_SEED", "not-a-number");
        assert_eq!(resolve_world_seed(None), 0);
        match original {
            Some(value) => std::env::set_var("MDM_WORLD_SEED", value),
            None => std::env::remove_var("MDM_WORLD_SEED"),
        }
    }
}
