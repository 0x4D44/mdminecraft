//! Dimension identifiers.
//!
//! Vanilla Minecraft gameplay rules and persistence/network protocols are
//! dimension-scoped (Overworld, Nether, End). Even if a build only supports a
//! single dimension today, threading a dimension identifier through the core
//! types prevents later large-scale rewrites.

use serde::{Deserialize, Serialize};

/// Stable identifier for a world dimension.
///
/// This is intentionally small (u8) for efficient persistence/network encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum DimensionId {
    /// The Overworld dimension.
    Overworld = 0,
    /// The Nether dimension.
    Nether = 1,
    /// The End dimension.
    End = 2,
}

impl DimensionId {
    /// Default (Overworld) dimension.
    pub const DEFAULT: Self = Self::Overworld;

    /// Convert to a stable numeric representation.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Try to convert from the stable numeric representation.
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Overworld),
            1 => Some(Self::Nether),
            2 => Some(Self::End),
            _ => None,
        }
    }

    /// Canonical string key used in configs/logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overworld => "overworld",
            Self::Nether => "nether",
            Self::End => "end",
        }
    }
}

impl Default for DimensionId {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_id_is_stable() {
        assert_eq!(DimensionId::Overworld.as_u8(), 0);
        assert_eq!(DimensionId::Nether.as_u8(), 1);
        assert_eq!(DimensionId::End.as_u8(), 2);
        assert_eq!(DimensionId::from_u8(0), Some(DimensionId::Overworld));
        assert_eq!(DimensionId::from_u8(1), Some(DimensionId::Nether));
        assert_eq!(DimensionId::from_u8(2), Some(DimensionId::End));
        assert_eq!(DimensionId::from_u8(3), None);
    }
}
