//! Audio manager for sound playback and music.

use crate::{AudioSettings, MusicTrack, SoundId};
use anyhow::Result;
use std::collections::HashMap;
use tracing::debug;

#[cfg(feature = "rodio_backend")]
mod backend {
    use super::*;
    use anyhow::Context;
    use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    /// Audio data for a loaded sound.
    pub struct SoundData {
        /// Raw audio bytes
        pub data: Vec<u8>,
    }

    /// Active sound playback state.
    pub struct PlaybackState {
        /// The audio sink for playback control
        pub sink: Sink,
    }

    /// Backend state for rodio audio.
    pub struct BackendState {
        /// Output stream (must be kept alive)
        pub _stream: OutputStream,
        /// Stream handle for creating sinks
        pub stream_handle: OutputStreamHandle,
        /// Music playback sink
        pub music_sink: Option<Sink>,
        /// Active one-shot sound effects
        pub active_sounds: Arc<Mutex<Vec<PlaybackState>>>,
    }

    impl BackendState {
        pub fn new() -> Result<Self> {
            let (stream, stream_handle) =
                OutputStream::try_default().context("Failed to create audio output stream")?;

            Ok(Self {
                _stream: stream,
                stream_handle,
                music_sink: None,
                active_sounds: Arc::new(Mutex::new(Vec::new())),
            })
        }

        pub fn play_sound(&self, data: &SoundData, volume: f32) -> Result<()> {
            let cursor = Cursor::new(data.data.clone());
            let source = rodio::Decoder::new(cursor).context("Failed to decode audio")?;

            let sink = Sink::try_new(&self.stream_handle).context("Failed to create audio sink")?;
            sink.set_volume(volume);
            sink.append(source);

            if let Ok(mut active) = self.active_sounds.lock() {
                active.retain(|s| !s.sink.empty());
                active.push(PlaybackState { sink });
            }

            Ok(())
        }

        pub fn stop_music(&mut self) {
            if let Some(sink) = self.music_sink.take() {
                sink.stop();
            }
        }

        pub fn pause_music(&self) {
            if let Some(sink) = &self.music_sink {
                sink.pause();
            }
        }

        pub fn resume_music(&self) {
            if let Some(sink) = &self.music_sink {
                sink.play();
            }
        }

        pub fn is_music_playing(&self) -> bool {
            self.music_sink
                .as_ref()
                .map(|s| !s.empty() && !s.is_paused())
                .unwrap_or(false)
        }

        pub fn set_music_volume(&self, volume: f32) {
            if let Some(sink) = &self.music_sink {
                sink.set_volume(volume);
            }
        }

        pub fn update(&mut self) {
            if let Ok(mut active) = self.active_sounds.lock() {
                active.retain(|s| !s.sink.empty());
            }
        }

        pub fn active_sound_count(&self) -> usize {
            self.active_sounds.lock().map(|a| a.len()).unwrap_or(0)
        }

        pub fn stop_all(&mut self) {
            self.stop_music();
            if let Ok(mut active) = self.active_sounds.lock() {
                for state in active.drain(..) {
                    state.sink.stop();
                }
            }
        }
    }
}

#[cfg(not(feature = "rodio_backend"))]
mod backend {
    use super::*;

    /// Audio data for a loaded sound (stub).
    #[allow(dead_code)]
    pub struct SoundData {
        /// Raw audio bytes (unused in stub mode)
        pub data: Vec<u8>,
    }

    /// Backend state stub when rodio is not available.
    pub struct BackendState;

    impl BackendState {
        pub fn new() -> Result<Self> {
            debug!("Audio backend: stub (no rodio)");
            Ok(Self)
        }

        pub fn play_sound(&self, _data: &SoundData, _volume: f32) -> Result<()> {
            Ok(())
        }

        pub fn stop_music(&mut self) {}

        pub fn pause_music(&self) {}

        pub fn resume_music(&self) {}

        pub fn is_music_playing(&self) -> bool {
            false
        }

        pub fn set_music_volume(&self, _volume: f32) {}

        pub fn update(&mut self) {}

        pub fn active_sound_count(&self) -> usize {
            0
        }

        pub fn stop_all(&mut self) {}
    }
}

use backend::{BackendState, SoundData};
use std::sync::Arc;

/// Main audio manager for the game.
///
/// Handles sound effects, background music, and 3D positional audio.
/// Uses rodio for cross-platform audio output when the `rodio_backend` feature is enabled.
pub struct AudioManager {
    /// Backend state
    backend: Option<BackendState>,
    /// Current audio settings
    settings: AudioSettings,
    /// Loaded sound effects cache
    sounds: HashMap<SoundId, Arc<SoundData>>,
    /// Currently playing music track
    current_music: Option<MusicTrack>,
    /// Listener position for 3D audio
    listener_pos: [f32; 3],
}

impl AudioManager {
    /// Create a new audio manager.
    ///
    /// Initializes the audio output device and prepares for playback.
    /// Falls back to a stub if audio initialization fails.
    pub fn new() -> Result<Self> {
        let backend = match BackendState::new() {
            Ok(b) => {
                debug!("Audio manager initialized");
                Some(b)
            }
            Err(e) => {
                tracing::warn!("Failed to initialize audio: {}. Using stub.", e);
                None
            }
        };

        Ok(Self {
            backend,
            settings: AudioSettings::default(),
            sounds: HashMap::new(),
            current_music: None,
            listener_pos: [0.0, 64.0, 0.0],
        })
    }

    /// Create a stub audio manager that doesn't actually play audio.
    ///
    /// Useful for testing or headless operation.
    pub fn stub() -> Self {
        Self {
            backend: None,
            settings: AudioSettings::default(),
            sounds: HashMap::new(),
            current_music: None,
            listener_pos: [0.0, 64.0, 0.0],
        }
    }

    /// Check if audio playback is available.
    pub fn is_available(&self) -> bool {
        self.backend.is_some()
    }

    /// Get the current audio settings.
    pub fn settings(&self) -> &AudioSettings {
        &self.settings
    }

    /// Get mutable access to audio settings.
    pub fn settings_mut(&mut self) -> &mut AudioSettings {
        &mut self.settings
    }

    /// Update audio settings.
    pub fn update_settings(&mut self, settings: AudioSettings) {
        self.settings = settings;

        // Update music volume if playing
        if let Some(backend) = &self.backend {
            backend.set_music_volume(self.settings.effective_music_volume());
        }
    }

    /// Set the listener position for 3D audio.
    pub fn set_listener_position(&mut self, pos: [f32; 3]) {
        self.listener_pos = pos;
    }

    /// Load a sound effect into memory.
    pub fn load_sound(&mut self, id: SoundId, data: Vec<u8>) {
        self.sounds.insert(id, Arc::new(SoundData { data }));
        debug!("Loaded sound: {:?}", id);
    }

    /// Play a sound effect.
    ///
    /// The sound plays at the listener's position (non-positional).
    pub fn play_sfx(&self, id: SoundId) {
        self.play_sfx_at(id, self.listener_pos);
    }

    /// Play a sound effect at a specific world position.
    ///
    /// Volume is attenuated based on distance from the listener.
    pub fn play_sfx_at(&self, id: SoundId, position: [f32; 3]) {
        let volume = self.calculate_volume(id, position);
        if volume < 0.01 {
            return; // Too quiet to hear
        }

        // Check if we have the sound loaded
        if let Some(sound_data) = self.sounds.get(&id) {
            if let Some(backend) = &self.backend {
                if let Err(e) = backend.play_sound(
                    sound_data.as_ref(),
                    volume * self.settings.effective_sfx_volume(),
                ) {
                    tracing::warn!("Failed to play sound {:?}: {}", id, e);
                }
            }
        } else {
            // Sound not loaded - this is normal during development
            debug!("Sound not loaded: {:?}", id);
        }
    }

    /// Calculate effective volume based on distance and sound properties.
    fn calculate_volume(&self, id: SoundId, position: [f32; 3]) -> f32 {
        if !id.is_positional() {
            return id.default_volume();
        }

        let dx = position[0] - self.listener_pos[0];
        let dy = position[1] - self.listener_pos[1];
        let dz = position[2] - self.listener_pos[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        let max_dist = id.max_distance();
        if distance >= max_dist {
            return 0.0;
        }

        // Linear falloff
        let falloff = 1.0 - (distance / max_dist);
        id.default_volume() * falloff
    }

    /// Play background music.
    ///
    /// Stops any currently playing music and starts the new track.
    pub fn play_music(&mut self, track: MusicTrack) {
        // Stop current music
        self.stop_music();

        // Check if we have the music file
        if let Some(path) = track.file_path() {
            debug!("Would play music: {} (loading not yet implemented)", path);
        }

        self.current_music = Some(track);
    }

    /// Stop the currently playing music.
    pub fn stop_music(&mut self) {
        if let Some(backend) = &mut self.backend {
            backend.stop_music();
        }
        self.current_music = None;
    }

    /// Pause the current music.
    pub fn pause_music(&self) {
        if let Some(backend) = &self.backend {
            backend.pause_music();
        }
    }

    /// Resume paused music.
    pub fn resume_music(&self) {
        if let Some(backend) = &self.backend {
            backend.resume_music();
        }
    }

    /// Check if music is currently playing.
    pub fn is_music_playing(&self) -> bool {
        self.backend
            .as_ref()
            .map(|b| b.is_music_playing())
            .unwrap_or(false)
    }

    /// Get the currently playing music track.
    pub fn current_music(&self) -> Option<MusicTrack> {
        self.current_music
    }

    /// Set the music volume.
    pub fn set_music_volume(&mut self, volume: f32) {
        self.settings.set_music(volume);
        if let Some(backend) = &self.backend {
            backend.set_music_volume(self.settings.effective_music_volume());
        }
    }

    /// Update audio state (call once per frame).
    ///
    /// Cleans up finished sounds and handles music transitions.
    pub fn update(&mut self) {
        if let Some(backend) = &mut self.backend {
            backend.update();
        }
    }

    /// Get the number of currently playing sounds.
    pub fn active_sound_count(&self) -> usize {
        self.backend
            .as_ref()
            .map(|b| b.active_sound_count())
            .unwrap_or(0)
    }

    /// Stop all sounds (including music).
    pub fn stop_all(&mut self) {
        if let Some(backend) = &mut self.backend {
            backend.stop_all();
        }
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::stub()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_manager() {
        let manager = AudioManager::stub();
        assert!(!manager.is_available());
        assert_eq!(manager.active_sound_count(), 0);
    }

    #[test]
    fn test_settings_update() {
        let mut manager = AudioManager::stub();
        manager.settings_mut().set_master(0.5);
        assert_eq!(manager.settings().master, 0.5);
    }

    #[test]
    fn test_listener_position() {
        let mut manager = AudioManager::stub();
        manager.set_listener_position([10.0, 64.0, 20.0]);
        assert_eq!(manager.listener_pos, [10.0, 64.0, 20.0]);
    }

    #[test]
    fn test_volume_calculation() {
        let manager = AudioManager::stub();
        // Test positional sound at listener position
        let volume = manager.calculate_volume(SoundId::BlockBreak, manager.listener_pos);
        assert!(volume > 0.0);

        // Test sound at max distance
        let far_pos = [
            manager.listener_pos[0] + 100.0,
            manager.listener_pos[1],
            manager.listener_pos[2],
        ];
        let volume_far = manager.calculate_volume(SoundId::BlockBreak, far_pos);
        assert_eq!(volume_far, 0.0);
    }
}
