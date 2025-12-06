//! Sound effect and music track definitions.

use serde::{Deserialize, Serialize};

/// Identifiers for sound effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SoundId {
    // Block sounds
    /// Block being broken
    BlockBreak,
    /// Block being placed
    BlockPlace,
    /// Block being hit during mining
    BlockHit,

    // Footstep sounds by surface
    /// Walking on grass
    FootstepGrass,
    /// Walking on stone/cobblestone
    FootstepStone,
    /// Walking on sand
    FootstepSand,
    /// Walking on wood
    FootstepWood,
    /// Walking on gravel
    FootstepGravel,

    // Tool sounds
    /// Swinging a tool/weapon
    ToolSwing,
    /// Tool breaking from durability loss
    ToolBreak,

    // Combat sounds
    /// Bow being drawn
    BowDraw,
    /// Arrow being released
    BowShoot,
    /// Arrow hitting a target
    ArrowHit,
    /// Arrow hitting the ground
    ArrowStick,
    /// Player taking damage
    PlayerHurt,
    /// Player dying
    PlayerDeath,
    /// Generic hit sound
    Hit,
    /// Critical hit sound
    CriticalHit,

    // Mob sounds
    /// Zombie ambient sound
    ZombieIdle,
    /// Zombie hurt sound
    ZombieHurt,
    /// Zombie death sound
    ZombieDeath,
    /// Skeleton ambient sound
    SkeletonIdle,
    /// Skeleton hurt sound
    SkeletonHurt,
    /// Skeleton death sound
    SkeletonDeath,
    /// Spider ambient sound
    SpiderIdle,
    /// Spider hurt sound
    SpiderHurt,
    /// Spider death sound
    SpiderDeath,
    /// Creeper hiss before explosion
    CreeperHiss,
    /// Explosion sound
    Explosion,
    /// Cow moo
    CowIdle,
    /// Pig oink
    PigIdle,
    /// Sheep baa
    SheepIdle,
    /// Chicken cluck
    ChickenIdle,

    // UI sounds
    /// Inventory opening
    InventoryOpen,
    /// Inventory closing
    InventoryClose,
    /// Item being picked up
    ItemPickup,
    /// Crafting successful
    CraftSuccess,
    /// Button click
    ButtonClick,

    // Environment sounds
    /// Water flowing
    WaterAmbient,
    /// Rain falling
    RainAmbient,
    /// Thunder crack
    Thunder,
    /// Wind ambient
    WindAmbient,
    /// Cave dripping water
    CaveDrip,

    // Eating/drinking
    /// Eating food
    Eat,
    /// Drinking potion
    Drink,
    /// Burp after eating
    Burp,

    // Experience
    /// XP orb pickup
    XpPickup,
    /// Level up sound
    LevelUp,
}

impl SoundId {
    /// Get the relative file path for this sound effect.
    /// Returns None if the sound doesn't have an associated file yet.
    pub fn file_path(&self) -> Option<&'static str> {
        // These would map to actual sound files in assets/sounds/
        // For now, we return None as placeholder sounds aren't implemented
        match self {
            SoundId::BlockBreak => Some("sounds/block/break.wav"),
            SoundId::BlockPlace => Some("sounds/block/place.wav"),
            SoundId::BlockHit => Some("sounds/block/hit.wav"),
            SoundId::FootstepGrass => Some("sounds/step/grass.wav"),
            SoundId::FootstepStone => Some("sounds/step/stone.wav"),
            SoundId::FootstepSand => Some("sounds/step/sand.wav"),
            SoundId::FootstepWood => Some("sounds/step/wood.wav"),
            SoundId::FootstepGravel => Some("sounds/step/gravel.wav"),
            SoundId::ToolSwing => Some("sounds/tool/swing.wav"),
            SoundId::BowDraw => Some("sounds/combat/bow_draw.wav"),
            SoundId::BowShoot => Some("sounds/combat/bow_shoot.wav"),
            SoundId::ArrowHit => Some("sounds/combat/arrow_hit.wav"),
            SoundId::PlayerHurt => Some("sounds/player/hurt.wav"),
            SoundId::PlayerDeath => Some("sounds/player/death.wav"),
            SoundId::Explosion => Some("sounds/combat/explosion.wav"),
            SoundId::InventoryOpen => Some("sounds/ui/inventory_open.wav"),
            SoundId::InventoryClose => Some("sounds/ui/inventory_close.wav"),
            SoundId::ItemPickup => Some("sounds/ui/item_pickup.wav"),
            SoundId::ButtonClick => Some("sounds/ui/button_click.wav"),
            SoundId::Eat => Some("sounds/player/eat.wav"),
            SoundId::XpPickup => Some("sounds/player/xp_pickup.wav"),
            SoundId::LevelUp => Some("sounds/player/level_up.wav"),
            // Other sounds not yet implemented
            _ => None,
        }
    }

    /// Get the default volume for this sound (0.0 to 1.0).
    pub fn default_volume(&self) -> f32 {
        match self {
            // UI sounds are quieter
            SoundId::ButtonClick | SoundId::InventoryOpen | SoundId::InventoryClose => 0.5,
            // Footsteps are subtle
            SoundId::FootstepGrass
            | SoundId::FootstepStone
            | SoundId::FootstepSand
            | SoundId::FootstepWood
            | SoundId::FootstepGravel => 0.4,
            // Combat sounds are prominent
            SoundId::Explosion | SoundId::PlayerHurt | SoundId::Hit => 0.9,
            // Default volume
            _ => 0.7,
        }
    }

    /// Whether this sound should use 3D positional audio.
    pub fn is_positional(&self) -> bool {
        match self {
            // UI sounds are not positional
            SoundId::InventoryOpen
            | SoundId::InventoryClose
            | SoundId::ButtonClick
            | SoundId::CraftSuccess
            | SoundId::LevelUp => false,
            // Player sounds are not positional (they're at the listener)
            SoundId::PlayerHurt | SoundId::PlayerDeath | SoundId::Eat | SoundId::Drink => false,
            // Everything else is positional
            _ => true,
        }
    }

    /// Get the maximum audible distance for positional sounds.
    pub fn max_distance(&self) -> f32 {
        match self {
            // Explosions can be heard from far away
            SoundId::Explosion | SoundId::Thunder => 64.0,
            // Mob sounds are moderately audible
            SoundId::ZombieIdle
            | SoundId::SkeletonIdle
            | SoundId::SpiderIdle
            | SoundId::CreeperHiss => 24.0,
            // Ambient sounds have shorter range
            SoundId::CaveDrip | SoundId::WaterAmbient => 12.0,
            // Default range
            _ => 16.0,
        }
    }
}

/// Background music tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MusicTrack {
    /// Calm daytime music
    Day1,
    /// Another daytime track
    Day2,
    /// Nighttime music
    Night1,
    /// Cave exploration music
    Cave1,
    /// Combat music
    Combat1,
    /// Menu/title screen music
    Menu,
}

impl MusicTrack {
    /// Get the relative file path for this music track.
    pub fn file_path(&self) -> Option<&'static str> {
        // Music files would be in assets/music/
        match self {
            MusicTrack::Day1 => Some("music/day1.ogg"),
            MusicTrack::Day2 => Some("music/day2.ogg"),
            MusicTrack::Night1 => Some("music/night1.ogg"),
            MusicTrack::Cave1 => Some("music/cave1.ogg"),
            MusicTrack::Combat1 => Some("music/combat1.ogg"),
            MusicTrack::Menu => Some("music/menu.ogg"),
        }
    }

    /// Get default volume for this track.
    pub fn default_volume(&self) -> f32 {
        0.3 // Music is typically quieter
    }
}

/// Ambient sound types for environmental audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmbientSound {
    /// Surface wind sounds
    Wind,
    /// Rain falling
    Rain,
    /// Heavy thunderstorm
    Thunder,
    /// Underground cave ambience
    Cave,
    /// Near water sounds
    Water,
    /// Forest bird sounds
    Forest,
    /// Night crickets
    Night,
}

impl AmbientSound {
    /// Get the file path for this ambient sound.
    pub fn file_path(&self) -> Option<&'static str> {
        match self {
            AmbientSound::Wind => Some("sounds/ambient/wind.ogg"),
            AmbientSound::Rain => Some("sounds/ambient/rain.ogg"),
            AmbientSound::Thunder => Some("sounds/ambient/thunder.ogg"),
            AmbientSound::Cave => Some("sounds/ambient/cave.ogg"),
            AmbientSound::Water => Some("sounds/ambient/water.ogg"),
            AmbientSound::Forest => Some("sounds/ambient/forest.ogg"),
            AmbientSound::Night => Some("sounds/ambient/night.ogg"),
        }
    }

    /// Whether this ambient sound should loop.
    pub fn loops(&self) -> bool {
        match self {
            AmbientSound::Thunder => false, // Thunder is a one-shot
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_id_properties() {
        assert!(SoundId::Explosion.is_positional());
        assert!(!SoundId::InventoryOpen.is_positional());
        assert!(SoundId::Explosion.max_distance() > SoundId::BlockBreak.max_distance());
    }

    #[test]
    fn test_music_track_paths() {
        assert!(MusicTrack::Day1.file_path().is_some());
        assert!(MusicTrack::Menu.file_path().is_some());
    }

    #[test]
    fn test_ambient_loops() {
        assert!(AmbientSound::Rain.loops());
        assert!(!AmbientSound::Thunder.loops());
    }
}
