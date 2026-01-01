//! mdminecraft - A deterministic voxel sandbox engine
//!
//! Main executable with graphical menu system

mod automation;
mod commentary;
mod command_script;
mod commands;
mod config;
mod content_pack_loot;
mod content_pack_spawns;
mod content_packs;
mod game;
mod headless;
mod input;
mod menu;
mod scripted_input;

use anyhow::Result;
use commentary::{CommentaryConfig, CommentaryStyle};
use config::ControlsConfig;
use game::{GameWorld, GameWorldOptions, RecordConfig, ScreenshotConfig};
use menu::MenuState;
use std::net::SocketAddr;
use std::{env, path::PathBuf, sync::Arc};
use tracing::info;
use winit::event_loop::{ControlFlow, EventLoop};

/// Application state
enum AppState {
    /// Main menu
    Menu(Box<MenuState>),
    /// In-game (playing)
    InGame(Box<GameWorld>),
    /// Quitting
    Quit,
}

fn main() -> Result<()> {
    // Initialize tracing with WARN level by default (can be overridden via RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    info!("Starting mdminecraft v{}", env!("CARGO_PKG_VERSION"));

    let cli = CliOptions::parse(env::args().skip(1));
    let mut controls = ControlsConfig::load();
    if cli.headless {
        if let Some(value) = cli.headless_render_distance {
            controls.render_distance = value.clamp(2, 16);
        }
    } else if cli.headless_render_distance.is_some() {
        tracing::warn!("--headless-render-distance has no effect without --headless");
    }
    let controls = Arc::new(controls);

    let screenshot = match cli.screenshot_dir.clone() {
        Some(dir) => Some(ScreenshotConfig {
            dir,
            every_ticks: cli.screenshot_every_ticks,
            max: cli.screenshot_max,
        }),
        None => {
            if cli.screenshot_every_ticks.is_some() || cli.screenshot_max.is_some() {
                tracing::error!(
                    "--screenshot-every-ticks/--screenshot-max require --screenshot-dir"
                );
            }
            None
        }
    };

    let record_resolution = match (cli.record_width, cli.record_height) {
        (Some(width), Some(height)) => Some((width, height)),
        (Some(_), None) | (None, Some(_)) => {
            tracing::error!("--record-width and --record-height must be set together");
            None
        }
        _ => None,
    };
    let resolution = record_resolution.unwrap_or(cli.resolution);

    let record = match cli.record_dir.clone() {
        Some(dir) => {
            if !cli.headless {
                anyhow::bail!("--record-dir requires --headless");
            }
            if cli.no_render {
                anyhow::bail!("--record-dir requires rendering (do not use --no-render)");
            }
            let requested_fps = cli.record_fps.unwrap_or(20).max(1);
            let fps = requested_fps.min(20);
            if requested_fps != fps {
                tracing::warn!(requested_fps, fps, "--record-fps clamped for headless capture");
            }
            let duration_seconds = cli.record_duration_seconds.unwrap_or(30).max(1);
            Some(RecordConfig {
                dir,
                fps,
                duration_seconds,
            })
        }
        None => {
            if cli.record_fps.is_some()
                || cli.record_duration_seconds.is_some()
                || cli.record_width.is_some()
                || cli.record_height.is_some()
            {
                tracing::error!("--record-* flags require --record-dir");
            }
            None
        }
    };

    let default_commentary_log = record
        .as_ref()
        .map(|cfg| cfg.dir.join("commentary.jsonl"));
    let commentary_log = cli.commentary_log.clone().or(default_commentary_log);
    let commentary = match commentary_log {
        Some(path) => {
            let style = cli
                .commentary_style
                .as_deref()
                .and_then(CommentaryStyle::parse)
                .unwrap_or(CommentaryStyle::Teen);
            let min_interval_ms = cli.commentary_min_interval_ms.unwrap_or(2000);
            let max_interval_ms = cli.commentary_max_interval_ms.unwrap_or(5000);
            let (min_interval_ms, max_interval_ms) = if min_interval_ms > max_interval_ms {
                (max_interval_ms, min_interval_ms)
            } else {
                (min_interval_ms, max_interval_ms)
            };
            Some(CommentaryConfig {
                log_path: path,
                style,
                min_interval_ms,
                max_interval_ms,
            })
        }
        None => None,
    };

    if commentary.is_none()
        && (cli.commentary_style.is_some()
            || cli.commentary_min_interval_ms.is_some()
            || cli.commentary_max_interval_ms.is_some())
    {
        tracing::error!("--commentary-* flags require --commentary-log or --record-dir");
    }

    if cli.automation_listen.is_some() && cli.automation_uds.is_some() {
        anyhow::bail!("--automation-listen and --automation-uds are mutually exclusive");
    }

    let mut automation_endpoint = None;
    if let Some(addr) = cli.automation_listen {
        match automation::server::AutomationServer::start(
            addr,
            cli.automation_token.clone(),
            cli.automation_log.clone(),
        ) {
            Ok(handle) => {
                automation_endpoint = Some(handle.endpoint);
            }
            Err(err) => {
                if cli.headless {
                    return Err(err);
                }
                tracing::error!(%err, "Failed to start automation server");
            }
        }
    } else if let Some(path) = cli.automation_uds.clone() {
        #[cfg(unix)]
        {
            match automation::server::AutomationServer::start_uds(
                path,
                cli.automation_token.clone(),
                cli.automation_log.clone(),
            ) {
                Ok(handle) => {
                    automation_endpoint = Some(handle.endpoint);
                }
                Err(err) => {
                    if cli.headless {
                        return Err(err);
                    }
                    tracing::error!(%err, "Failed to start automation server");
                }
            }
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--automation-uds is only supported on unix");
        }
    }

    if cli.headless {
        let record_max_ticks = record
            .as_ref()
            .map(|cfg| cfg.duration_seconds as u64 * 20);
        let max_ticks = match (cli.max_ticks, record_max_ticks) {
            (Some(user), Some(record_ticks)) => Some(user.min(record_ticks)),
            (None, Some(record_ticks)) => Some(record_ticks),
            (Some(user), None) => Some(user),
            (None, None) => None,
        };
        if cli.automation_exit_when_disconnected
            && cli.automation_listen.is_none()
            && cli.automation_uds.is_none()
        {
            tracing::warn!(
                "--automation-exit-when-disconnected has no effect without automation control"
            );
        }
        if cli.automation_step && cli.automation_listen.is_none() && cli.automation_uds.is_none() {
            anyhow::bail!("--automation-step requires --automation-listen or --automation-uds");
        }
        if cli.exit_when_script_finished && cli.command_script.is_none() {
            tracing::warn!("--exit-when-script-finished has no effect without --command-script");
        }
        if cli.scripted_input.is_some()
            && (cli.automation_listen.is_some() || cli.automation_uds.is_some())
        {
            anyhow::bail!("--scripted-input is not supported with automation control");
        }

        return headless::run(headless::HeadlessConfig {
            controls,
            scripted_input: cli.scripted_input.clone(),
            command_script: cli.command_script.clone(),
            width: resolution.0,
            height: resolution.1,
            screenshot,
            record,
            commentary,
            automation: automation_endpoint,
            save_dir: cli.save_dir.clone(),
            no_save: cli.no_save,
            reset_world: cli.reset_world,
            world_seed: cli.world_seed,
            no_render: cli.no_render,
            no_audio: cli.no_audio,
            max_ticks,
            exit_when_script_finished: cli.exit_when_script_finished,
            automation_step: cli.automation_step,
            automation_exit_when_disconnected: cli.automation_exit_when_disconnected,
        });
    }

    // Create event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    // Start with menu unless auto-play is requested
    let mut app_state = if cli.auto_play {
        let options = GameWorldOptions {
            width: resolution.0,
            height: resolution.1,
            automation: automation_endpoint,
            screenshot: screenshot.clone(),
            record: record.clone(),
            commentary: commentary.clone(),
        };
        match GameWorld::new(
            &event_loop,
            controls.clone(),
            cli.scripted_input.clone(),
            cli.command_script.clone(),
            options,
        ) {
            Ok(game) => AppState::InGame(Box::new(game)),
            Err(err) => {
                tracing::error!(%err, "Failed to start auto-play mode, falling back to menu");
                AppState::Menu(Box::new(MenuState::new(&event_loop)?))
            }
        }
    } else {
        AppState::Menu(Box::new(MenuState::new(&event_loop)?))
    };

    // Run event loop
    event_loop.run(move |event, elwt| {
        match &mut app_state {
            AppState::Menu(menu) => {
                match menu.handle_event(&event, elwt) {
                    menu::MenuAction::Continue => {
                        // Stay in menu
                    }
                    menu::MenuAction::StartGame => {
                        info!("Starting game...");
                        // Transition to game
                        // Reload controls so menu changes take effect immediately.
                        let controls = Arc::new(ControlsConfig::load());
                        let options = GameWorldOptions {
                            width: resolution.0,
                            height: resolution.1,
                            automation: None,
                            screenshot: screenshot.clone(),
                            record: record.clone(),
                            commentary: commentary.clone(),
                        };
                        match GameWorld::new(
                            elwt,
                            controls,
                            cli.scripted_input.clone(),
                            cli.command_script.clone(),
                            options,
                        ) {
                            Ok(game) => {
                                app_state = AppState::InGame(Box::new(game));
                            }
                            Err(e) => {
                                tracing::error!("Failed to start game: {}", e);
                            }
                        }
                    }
                    menu::MenuAction::Quit => {
                        info!("Quitting from menu");
                        elwt.exit();
                        app_state = AppState::Quit;
                    }
                }
            }
            AppState::InGame(game) => {
                match game.handle_event(&event, elwt) {
                    game::GameAction::Continue => {
                        // Stay in game
                    }
                    game::GameAction::ReturnToMenu => {
                        info!("Returning to menu...");
                        // Transition back to menu
                        match MenuState::new(elwt) {
                            Ok(menu) => {
                                app_state = AppState::Menu(Box::new(menu));
                            }
                            Err(e) => {
                                tracing::error!("Failed to return to menu: {}", e);
                                elwt.exit();
                            }
                        }
                    }
                    game::GameAction::Quit => {
                        info!("Quitting from game");
                        elwt.exit();
                        app_state = AppState::Quit;
                    }
                }
            }
            AppState::Quit => {
                elwt.exit();
            }
        }
    })?;

    info!("mdminecraft shutting down");
    Ok(())
}

#[derive(Clone)]
struct CliOptions {
    auto_play: bool,
    headless: bool,
    no_render: bool,
    no_audio: bool,
    save_dir: Option<PathBuf>,
    no_save: bool,
    reset_world: bool,
    world_seed: Option<u64>,
    max_ticks: Option<u64>,
    exit_when_script_finished: bool,
    scripted_input: Option<PathBuf>,
    command_script: Option<PathBuf>,
    resolution: (u32, u32),
    screenshot_dir: Option<PathBuf>,
    screenshot_every_ticks: Option<u64>,
    screenshot_max: Option<u64>,
    record_dir: Option<PathBuf>,
    record_fps: Option<u32>,
    record_duration_seconds: Option<u32>,
    record_width: Option<u32>,
    record_height: Option<u32>,
    automation_listen: Option<SocketAddr>,
    automation_uds: Option<PathBuf>,
    automation_token: Option<String>,
    automation_log: Option<PathBuf>,
    automation_step: bool,
    automation_exit_when_disconnected: bool,
    commentary_log: Option<PathBuf>,
    commentary_style: Option<String>,
    commentary_min_interval_ms: Option<u64>,
    commentary_max_interval_ms: Option<u64>,
    headless_render_distance: Option<i32>,
}

impl CliOptions {
    fn parse<I: Iterator<Item = String>>(mut args: I) -> Self {
        let mut opts = CliOptions {
            auto_play: false,
            headless: false,
            no_render: false,
            no_audio: false,
            save_dir: None,
            no_save: false,
            reset_world: false,
            world_seed: None,
            max_ticks: None,
            exit_when_script_finished: false,
            scripted_input: None,
            command_script: None,
            resolution: (1280, 720),
            screenshot_dir: None,
            screenshot_every_ticks: None,
            screenshot_max: None,
            record_dir: None,
            record_fps: None,
            record_duration_seconds: None,
            record_width: None,
            record_height: None,
            automation_listen: None,
            automation_uds: None,
            automation_token: None,
            automation_log: None,
            automation_step: false,
            automation_exit_when_disconnected: false,
            commentary_log: None,
            commentary_style: None,
            commentary_min_interval_ms: None,
            commentary_max_interval_ms: None,
            headless_render_distance: None,
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--auto-play" => opts.auto_play = true,
                "--headless" => opts.headless = true,
                "--no-render" => opts.no_render = true,
                "--no-audio" => opts.no_audio = true,
                "--save-dir" => {
                    if let Some(path) = args.next() {
                        opts.save_dir = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--save-dir requires a directory path");
                    }
                }
                "--no-save" => opts.no_save = true,
                "--reset-world" => opts.reset_world = true,
                "--world-seed" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u64>() {
                            Ok(value) => opts.world_seed = Some(value),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--world-seed must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--world-seed requires an integer");
                    }
                }
                "--max-ticks" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u64>() {
                            Ok(value) => opts.max_ticks = Some(value),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--max-ticks must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--max-ticks requires an integer");
                    }
                }
                "--headless-render-distance" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<i32>() {
                            Ok(value) => opts.headless_render_distance = Some(value),
                            Err(err) => {
                                tracing::error!(
                                    %err,
                                    value = %raw,
                                    "--headless-render-distance must be an integer"
                                );
                            }
                        }
                    } else {
                        tracing::error!("--headless-render-distance requires an integer");
                    }
                }
                "--exit-when-script-finished" => opts.exit_when_script_finished = true,
                "--resolution" => {
                    if let Some(raw) = args.next() {
                        match raw.split_once('x') {
                            Some((w, h)) => match (w.parse::<u32>(), h.parse::<u32>()) {
                                (Ok(width), Ok(height)) if width > 0 && height > 0 => {
                                    opts.resolution = (width, height);
                                }
                                _ => {
                                    tracing::error!(value = %raw, "--resolution must be like 1280x720");
                                }
                            },
                            None => {
                                tracing::error!(value = %raw, "--resolution must be like 1280x720");
                            }
                        }
                    } else {
                        tracing::error!("--resolution requires a value like 1280x720");
                    }
                }
                "--scripted-input" => {
                    if let Some(path) = args.next() {
                        opts.auto_play = true;
                        opts.scripted_input = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--scripted-input requires a file path");
                    }
                }
                "--command-script" => {
                    if let Some(path) = args.next() {
                        opts.command_script = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--command-script requires a file path");
                    }
                }
                "--screenshot-dir" => {
                    if let Some(path) = args.next() {
                        opts.screenshot_dir = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--screenshot-dir requires a directory path");
                    }
                }
                "--screenshot-every-ticks" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u64>() {
                            Ok(value) => opts.screenshot_every_ticks = Some(value.max(1)),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--screenshot-every-ticks must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--screenshot-every-ticks requires an integer");
                    }
                }
                "--screenshot-max" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u64>() {
                            Ok(value) => opts.screenshot_max = Some(value),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--screenshot-max must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--screenshot-max requires an integer");
                    }
                }
                "--record-dir" => {
                    if let Some(path) = args.next() {
                        opts.record_dir = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--record-dir requires a directory path");
                    }
                }
                "--record-fps" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u32>() {
                            Ok(value) => opts.record_fps = Some(value.max(1)),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--record-fps must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--record-fps requires an integer");
                    }
                }
                "--record-duration-seconds" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u32>() {
                            Ok(value) => opts.record_duration_seconds = Some(value.max(1)),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--record-duration-seconds must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--record-duration-seconds requires an integer");
                    }
                }
                "--record-width" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u32>() {
                            Ok(value) => opts.record_width = Some(value.max(1)),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--record-width must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--record-width requires an integer");
                    }
                }
                "--record-height" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u32>() {
                            Ok(value) => opts.record_height = Some(value.max(1)),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--record-height must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--record-height requires an integer");
                    }
                }
                "--automation-listen" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<SocketAddr>() {
                            Ok(addr) => {
                                opts.auto_play = true;
                                opts.automation_listen = Some(addr);
                            }
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--automation-listen must be a socket address");
                            }
                        }
                    } else {
                        tracing::error!(
                            "--automation-listen requires an address like 127.0.0.1:4242"
                        );
                    }
                }
                "--automation-uds" => {
                    if let Some(path) = args.next() {
                        opts.auto_play = true;
                        opts.automation_uds = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--automation-uds requires a socket path");
                    }
                }
                "--automation-token" => {
                    if let Some(token) = args.next() {
                        opts.automation_token = Some(token);
                    } else {
                        tracing::error!("--automation-token requires a token string");
                    }
                }
                "--automation-log" => {
                    if let Some(path) = args.next() {
                        opts.automation_log = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--automation-log requires a file path");
                    }
                }
                "--automation-step" => opts.automation_step = true,
                "--automation-exit-when-disconnected" => {
                    opts.automation_exit_when_disconnected = true;
                }
                "--commentary-log" => {
                    if let Some(path) = args.next() {
                        opts.commentary_log = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--commentary-log requires a file path");
                    }
                }
                "--commentary-style" => {
                    if let Some(style) = args.next() {
                        opts.commentary_style = Some(style);
                    } else {
                        tracing::error!("--commentary-style requires a style string");
                    }
                }
                "--commentary-min-interval-ms" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u64>() {
                            Ok(value) => opts.commentary_min_interval_ms = Some(value.max(250)),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--commentary-min-interval-ms must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--commentary-min-interval-ms requires an integer");
                    }
                }
                "--commentary-max-interval-ms" => {
                    if let Some(raw) = args.next() {
                        match raw.parse::<u64>() {
                            Ok(value) => opts.commentary_max_interval_ms = Some(value.max(250)),
                            Err(err) => {
                                tracing::error!(%err, value = %raw, "--commentary-max-interval-ms must be an integer");
                            }
                        }
                    } else {
                        tracing::error!("--commentary-max-interval-ms requires an integer");
                    }
                }
                _ => {}
            }
        }

        opts
    }
}
