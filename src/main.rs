//! mdminecraft - A deterministic voxel sandbox engine
//!
//! Main executable with graphical menu system

mod commands;
mod config;
mod game;
mod input;
mod menu;
mod scripted_input;

use anyhow::Result;
use config::ControlsConfig;
use game::GameWorld;
use menu::MenuState;
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

    let controls = Arc::new(ControlsConfig::load());
    let cli = CliOptions::parse(env::args().skip(1));

    // Create event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    // Start with menu unless auto-play is requested
    let mut app_state = if cli.auto_play {
        match GameWorld::new(&event_loop, controls.clone(), cli.scripted_input.clone()) {
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
                        match GameWorld::new(elwt, controls, cli.scripted_input.clone()) {
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
    scripted_input: Option<PathBuf>,
}

impl CliOptions {
    fn parse<I: Iterator<Item = String>>(mut args: I) -> Self {
        let mut opts = CliOptions {
            auto_play: false,
            scripted_input: None,
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--auto-play" => opts.auto_play = true,
                "--scripted-input" => {
                    if let Some(path) = args.next() {
                        opts.auto_play = true;
                        opts.scripted_input = Some(PathBuf::from(path));
                    } else {
                        tracing::error!("--scripted-input requires a file path");
                    }
                }
                _ => {}
            }
        }

        opts
    }
}
