//! Comprehensive test suite for GPU initialization and resource management.
//!
//! These tests follow TDD methodology and are written BEFORE implementation.
//! They cover both success and failure scenarios, and ensure deterministic
//! initialization for CI compatibility.

mod device_tests;
mod pipeline_tests;
mod resource_tests;
mod texture_atlas_tests;
