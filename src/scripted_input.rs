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
            hotbar_slot: None,
            hotbar_scroll: 0,
            scroll_delta: 0.0,
            look_delta: (self.look_x, self.look_y),
            raw_look_delta: (0.0, 0.0),
        }
    }
}
