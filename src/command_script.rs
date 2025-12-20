use anyhow::Result;
use mdminecraft_core::SimTick;
use serde::Deserialize;
use std::{collections::VecDeque, fs, path::Path};

#[derive(Debug, Deserialize)]
struct CommandScriptFile {
    steps: Vec<CommandScriptStepDef>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommandScriptStepDef {
    tick: u64,
    command: String,
}

#[derive(Debug, Clone)]
struct CommandScriptStep {
    tick: SimTick,
    command: String,
}

/// Deterministic command script runner.
///
/// Scripts are a simple list of `{tick, command}` steps, executed in file order.
#[derive(Debug)]
pub struct CommandScriptPlayer {
    pending: VecDeque<CommandScriptStep>,
}

impl CommandScriptPlayer {
    /// Load a command script from a JSON file on disk.
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        Self::from_str(&contents)
    }

    /// Load a command script from an in-memory JSON string.
    pub fn from_str(contents: &str) -> Result<Self> {
        let file: CommandScriptFile = serde_json::from_str(contents)?;
        if file.steps.is_empty() {
            anyhow::bail!("command script contains no steps");
        }

        let mut pending = VecDeque::with_capacity(file.steps.len());
        let mut last_tick: Option<u64> = None;
        for step in file.steps {
            let command = step.command.trim().to_string();
            if command.is_empty() {
                anyhow::bail!("command script contains an empty command");
            }

            if let Some(prev) = last_tick {
                if step.tick < prev {
                    anyhow::bail!("command script steps must be sorted by tick");
                }
            }
            last_tick = Some(step.tick);

            pending.push_back(CommandScriptStep {
                tick: SimTick(step.tick),
                command,
            });
        }

        Ok(Self { pending })
    }

    /// Drain and return all commands scheduled for ticks `<= tick`.
    pub fn drain_ready_commands(&mut self, tick: SimTick) -> Vec<String> {
        let mut commands = Vec::new();
        while let Some(step) = self.pending.front() {
            if step.tick > tick {
                break;
            }
            let step = self.pending.pop_front().expect("front existed");
            commands.push(step.command);
        }
        commands
    }

    pub fn is_finished(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_script_rejects_unsorted_ticks() {
        let json = r#"{
            "steps": [
                {"tick": 2, "command": "/time set 0"},
                {"tick": 1, "command": "/time set 1"}
            ]
        }"#;
        let err = CommandScriptPlayer::from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("sorted by tick"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn command_script_drains_in_order_and_is_deterministic() {
        let json = r#"{
            "steps": [
                {"tick": 1, "command": "/time set 0"},
                {"tick": 1, "command": "/weather clear"},
                {"tick": 3, "command": "/tp 0 70 0"}
            ]
        }"#;
        let mut script = CommandScriptPlayer::from_str(json).expect("script should parse");

        assert_eq!(
            script.drain_ready_commands(SimTick(0)),
            Vec::<String>::new()
        );
        assert_eq!(
            script.drain_ready_commands(SimTick(1)),
            vec!["/time set 0".to_string(), "/weather clear".to_string()]
        );
        assert_eq!(
            script.drain_ready_commands(SimTick(2)),
            Vec::<String>::new()
        );
        assert_eq!(
            script.drain_ready_commands(SimTick(3)),
            vec!["/tp 0 70 0".to_string()]
        );
        assert!(script.is_finished());
    }
}
