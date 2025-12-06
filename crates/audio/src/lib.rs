//! Audio system for mdminecraft.
//!
//! Provides sound effect playback, background music, and 3D positional audio.
//! Uses rodio for cross-platform audio output.
//!
//! # Architecture
//!
//! - [`AudioManager`] - Main interface for playing sounds and music
//! - [`SoundId`] - Identifier for sound effects
//! - [`AudioSettings`] - Volume controls for master, music, and SFX
//!
//! # Example
//!
//! ```ignore
//! let audio = AudioManager::new()?;
//! audio.play_sfx(SoundId::BlockBreak);
//! audio.set_listener_position([0.0, 64.0, 0.0]);
//! ```

mod manager;
mod settings;
mod sounds;

pub use manager::AudioManager;
pub use settings::AudioSettings;
pub use sounds::{AmbientSound, MusicTrack, SoundId};
