#![warn(missing_docs)]
//! Placeholder scripting hooks (WASM-based API TBD).

use anyhow::Result;
use mdminecraft_core::SimTick;

/// A script context invoked each tick.
pub trait ScriptContext {
    /// Called once per tick with the deterministic simulation tick.
    fn on_tick(&mut self, tick: SimTick) -> Result<()>;
}

/// No-op script used until a proper WASM host lands.
pub struct NoopScript;

impl ScriptContext for NoopScript {
    fn on_tick(&mut self, _tick: SimTick) -> Result<()> {
        Ok(())
    }
}
