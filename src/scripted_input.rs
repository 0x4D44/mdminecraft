use crate::input::ActionState;
use mdminecraft_render::InputContext;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
struct ScriptedInputFile {
    steps: Vec<ScriptedStep>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ScriptedStep {
    duration: f32,
    #[serde(default)]
    move_x: f32,
    #[serde(default)]
    move_y: f32,
    #[serde(default)]
    move_z: f32,
    #[serde(default)]
    sprint: bool,
    #[serde(default)]
    crouch: bool,
    #[serde(default)]
    jump: bool,
    #[serde(default)]
    toggle_fly: bool,
    #[serde(default)]
    look_x: f32,
    #[serde(default)]
    look_y: f32,
}

pub struct ScriptedInputPlayer {
    steps: Vec<ScriptedStep>,
    index: usize,
    time_in_step: f32,
}

impl ScriptedInputPlayer {
    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)?;
        let file: ScriptedInputFile = serde_json::from_str(&contents)?;
        if file.steps.is_empty() {
            anyhow::bail!("scripted input file contains no steps");
        }
        Ok(Self {
            steps: file.steps,
            index: 0,
            time_in_step: 0.0,
        })
    }

    pub fn advance(&mut self, dt: f32) -> ActionState {
        if self.steps.is_empty() {
            return ActionState::default();
        }

        self.time_in_step += dt;
        while self.index < self.steps.len() && self.time_in_step >= self.steps[self.index].duration
        {
            self.time_in_step -= self.steps[self.index].duration;
            if self.index + 1 < self.steps.len() {
                self.index += 1;
            } else {
                self.time_in_step = 0.0;
                break;
            }
        }

        let step = self.steps.get(self.index).cloned().unwrap_or_default();
        step.into_action_state()
    }
}

impl ScriptedStep {
    fn into_action_state(self) -> ActionState {
        ActionState {
            context: InputContext::Gameplay,
            move_x: self.move_x,
            move_y: self.move_y,
            move_z: self.move_z,
            sprint: self.sprint,
            crouch: self.crouch,
            jump: self.jump,
            jump_pressed: self.jump,
            toggle_fly: self.toggle_fly,
            toggle_cursor: false,
            drop_item: false,
            drop_stack: false,
            hotbar_slot: None,
            hotbar_scroll: 0,
            scroll_delta: 0.0,
            look_delta: (self.look_x, self.look_y),
            raw_look_delta: (0.0, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("mdm-scripted-input-{label}-{nanos}.json"));
        path
    }

    #[test]
    fn from_path_rejects_empty_steps() {
        let path = temp_path("empty");
        std::fs::write(&path, r#"{"steps":[]}"#).expect("write");
        assert!(ScriptedInputPlayer::from_path(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn advance_moves_through_steps() {
        let path = temp_path("advance");
        let json = r#"{"steps":[{"duration":0.5,"move_x":1.0},{"duration":0.5,"move_z":-1.0,"jump":true}]}"#;
        std::fs::write(&path, json).expect("write");

        let mut player = ScriptedInputPlayer::from_path(&path).expect("load");
        let state_first = player.advance(0.25);
        assert_eq!(state_first.context, InputContext::Gameplay);
        assert_eq!(state_first.move_x, 1.0);
        assert_eq!(state_first.move_z, 0.0);

        let state_second = player.advance(0.30);
        assert_eq!(state_second.move_x, 0.0);
        assert_eq!(state_second.move_z, -1.0);
        assert!(state_second.jump);

        let _ = std::fs::remove_file(&path);
    }
}
