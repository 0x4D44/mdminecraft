use crate::automation::controller::AutomationEndpoint;
use crate::config::ControlsConfig;
use crate::game::{GameWorld, GameWorldOptions, ScreenshotConfig};
use anyhow::{Context, Result};
use rand::RngCore;
use std::path::PathBuf;
use std::sync::Arc;

pub struct HeadlessConfig {
    pub controls: Arc<ControlsConfig>,
    pub scripted_input: Option<PathBuf>,
    pub command_script: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub screenshot: Option<ScreenshotConfig>,
    pub automation: Option<AutomationEndpoint>,
    pub save_dir: Option<PathBuf>,
    pub no_save: bool,
    pub reset_world: bool,
    pub world_seed: Option<u64>,
    pub no_render: bool,
    pub no_audio: bool,
    pub max_ticks: Option<u64>,
    pub exit_when_script_finished: bool,
    pub automation_step: bool,
    pub automation_exit_when_disconnected: bool,
}

pub fn run(cfg: HeadlessConfig) -> Result<()> {
    let (save_path, cleanup_save_path) =
        prepare_save_dir(cfg.save_dir.as_deref(), cfg.no_save, cfg.reset_world)?;

    let options = GameWorldOptions {
        width: cfg.width,
        height: cfg.height,
        automation: cfg.automation,
        screenshot: cfg.screenshot,
    };

    let mut world = GameWorld::new_headless(
        cfg.controls,
        cfg.scripted_input,
        cfg.command_script,
        options,
        save_path.clone(),
        cfg.world_seed,
        cfg.no_render,
        cfg.no_audio,
    )?;

    let result = if cfg.automation_step {
        run_step_mode(
            &mut world,
            cfg.max_ticks,
            cfg.exit_when_script_finished,
            cfg.automation_exit_when_disconnected,
        )
    } else {
        run_free_mode(
            &mut world,
            cfg.max_ticks,
            cfg.exit_when_script_finished,
            cfg.automation_exit_when_disconnected,
        )
    };

    if cleanup_save_path {
        if let Err(err) = std::fs::remove_dir_all(&save_path) {
            tracing::warn!(%err, path = %save_path.display(), "Failed to remove ephemeral save dir");
        }
    }

    result
}

fn prepare_save_dir(
    save_dir: Option<&std::path::Path>,
    no_save: bool,
    reset_world: bool,
) -> Result<(PathBuf, bool)> {
    let save_dir = if no_save { None } else { save_dir };
    let cleanup = save_dir.is_none();

    let save_path = match save_dir {
        Some(path) => path.to_path_buf(),
        None => {
            let mut rng = rand::thread_rng();
            let suffix = rng.next_u64();
            std::env::temp_dir()
                .join("mdminecraft_headless")
                .join(format!("run_{suffix:016x}"))
        }
    };

    if reset_world && save_dir.is_some() {
        if save_path.parent().is_none() {
            anyhow::bail!(
                "refusing to reset save dir with no parent: {}",
                save_path.display()
            );
        }
        if save_path.exists() {
            std::fs::remove_dir_all(&save_path)
                .with_context(|| format!("failed to reset save dir {}", save_path.display()))?;
        }
    }

    std::fs::create_dir_all(&save_path)
        .with_context(|| format!("failed to create save dir {}", save_path.display()))?;

    Ok((save_path, cleanup))
}

fn run_free_mode(
    world: &mut GameWorld,
    max_ticks: Option<u64>,
    exit_when_script_finished: bool,
    exit_when_disconnected: bool,
) -> Result<()> {
    world.run_headless_free(max_ticks, exit_when_script_finished, exit_when_disconnected)
}

fn run_step_mode(
    world: &mut GameWorld,
    max_ticks: Option<u64>,
    exit_when_script_finished: bool,
    exit_when_disconnected: bool,
) -> Result<()> {
    world.run_headless_step(max_ticks, exit_when_script_finished, exit_when_disconnected)
}
