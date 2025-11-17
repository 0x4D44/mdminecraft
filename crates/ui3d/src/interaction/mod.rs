//! 3D UI Interaction System
//!
//! This module handles raycasting against UI elements, hover detection,
//! and click handling for interactive 3D UI components.

pub mod raycaster;

pub use raycaster::{raycast_billboard_quad, screen_to_ray, UIRaycastHit, UIAABB};
