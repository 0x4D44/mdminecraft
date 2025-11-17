//! mdminecraft - A deterministic voxel sandbox engine
//!
//! Main executable with graphical menu system

mod menu;
mod game;
mod font_utils;

use anyhow::Result;
use menu::MenuState;
use game::GameWorld;
use tracing::info;
use winit::event_loop::{EventLoop, ControlFlow};

/// Application state
enum AppState {
    /// Main menu
    Menu(MenuState),
    /// In-game (playing)
    InGame(GameWorld),
    /// Quitting
    Quit,
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting mdminecraft v{}", env!("CARGO_PKG_VERSION"));

    // Create event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    // Start with main menu
    let mut app_state = AppState::Menu(MenuState::new(&event_loop)?);

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
                        match GameWorld::new(elwt) {
                            Ok(game) => {
                                app_state = AppState::InGame(game);
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
                                app_state = AppState::Menu(menu);
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
