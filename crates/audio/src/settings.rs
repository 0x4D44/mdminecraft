//! Audio settings and volume controls.

use serde::{Deserialize, Serialize};

/// Audio volume settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Master volume (0.0 to 1.0)
    pub master: f32,
    /// Music volume (0.0 to 1.0)
    pub music: f32,
    /// Sound effects volume (0.0 to 1.0)
    pub sfx: f32,
    /// Ambient sounds volume (0.0 to 1.0)
    pub ambient: f32,
    /// Whether audio is muted
    pub muted: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 0.5,
            sfx: 1.0,
            ambient: 0.7,
            muted: false,
        }
    }
}

impl AudioSettings {
    /// Create new audio settings with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the effective music volume (master * music).
    pub fn effective_music_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master * self.music
        }
    }

    /// Get the effective SFX volume (master * sfx).
    pub fn effective_sfx_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master * self.sfx
        }
    }

    /// Get the effective ambient volume (master * ambient).
    pub fn effective_ambient_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master * self.ambient
        }
    }

    /// Toggle mute state.
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    /// Set master volume (clamped to 0.0-1.0).
    pub fn set_master(&mut self, volume: f32) {
        self.master = volume.clamp(0.0, 1.0);
    }

    /// Set music volume (clamped to 0.0-1.0).
    pub fn set_music(&mut self, volume: f32) {
        self.music = volume.clamp(0.0, 1.0);
    }

    /// Set SFX volume (clamped to 0.0-1.0).
    pub fn set_sfx(&mut self, volume: f32) {
        self.sfx = volume.clamp(0.0, 1.0);
    }

    /// Set ambient volume (clamped to 0.0-1.0).
    pub fn set_ambient(&mut self, volume: f32) {
        self.ambient = volume.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AudioSettings::default();
        assert_eq!(settings.master, 1.0);
        assert_eq!(settings.music, 0.5);
        assert_eq!(settings.sfx, 1.0);
        assert!(!settings.muted);
    }

    #[test]
    fn test_effective_volumes() {
        let settings = AudioSettings {
            master: 0.5,
            music: 0.8,
            sfx: 0.6,
            ..Default::default()
        };

        assert!((settings.effective_music_volume() - 0.4).abs() < 0.001);
        assert!((settings.effective_sfx_volume() - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_mute() {
        let mut settings = AudioSettings::default();
        settings.toggle_mute();
        assert!(settings.muted);
        assert_eq!(settings.effective_music_volume(), 0.0);
        assert_eq!(settings.effective_sfx_volume(), 0.0);

        settings.toggle_mute();
        assert!(!settings.muted);
        assert!(settings.effective_music_volume() > 0.0);
    }

    #[test]
    fn test_volume_clamping() {
        let mut settings = AudioSettings::default();
        settings.set_master(1.5);
        assert_eq!(settings.master, 1.0);

        settings.set_music(-0.5);
        assert_eq!(settings.music, 0.0);
    }
}
