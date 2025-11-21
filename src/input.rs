use std::collections::HashMap;

use crate::config::{BindingOverrides, ControlsConfig};
use mdminecraft_render::{InputContext, InputSnapshot};
use tracing::warn;
use winit::{event::MouseButton, keyboard::KeyCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Jump,
    Sprint,
    Crouch,
    ToggleFly,
    ToggleCursor,
    HotbarSlot(u8),
    HotbarScrollUp,
    HotbarScrollDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputBinding {
    Key(KeyCode),
    Mouse(MouseButton),
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone)]
struct BindingLayer {
    map: HashMap<Action, Vec<InputBinding>>,
}

impl BindingLayer {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn insert(&mut self, action: Action, bindings: Vec<InputBinding>) {
        self.map.insert(action, bindings);
    }

    fn get(&self, action: &Action) -> Option<&[InputBinding]> {
        self.map.get(action).map(|v| v.as_slice())
    }
}

#[derive(Debug, Clone)]
pub struct Bindings {
    base: BindingLayer,
    gameplay: BindingLayer,
    ui: BindingLayer,
}

impl Bindings {
    pub fn from_config(config: &ControlsConfig) -> Self {
        let mut base = BindingLayer::new();
        for (action, binds) in default_base_bindings() {
            base.insert(action, binds);
        }

        let mut gameplay = BindingLayer::new();
        let mut ui = BindingLayer::new();

        apply_overrides(&mut base, &mut gameplay, &mut ui, &config.bindings);

        Self { base, gameplay, ui }
    }

    fn layer_for(&self, context: InputContext) -> &BindingLayer {
        match context {
            InputContext::Gameplay => &self.gameplay,
            InputContext::UiOverlay => &self.ui,
            InputContext::Menu => &self.ui,
        }
    }

    fn bindings_for(&self, action: &Action, context: InputContext) -> Option<&[InputBinding]> {
        self.layer_for(context)
            .get(action)
            .or_else(|| self.base.get(action))
    }
}

#[derive(Debug, Clone)]
pub struct ActionState {
    pub context: InputContext,
    pub move_x: f32,
    pub move_y: f32,
    pub move_z: f32,
    pub sprint: bool,
    pub crouch: bool,
    pub jump: bool,
    pub jump_pressed: bool,
    pub toggle_fly: bool,
    pub toggle_cursor: bool,
    pub hotbar_slot: Option<u8>,
    pub hotbar_scroll: i32,
    #[allow(dead_code)]
    pub scroll_delta: f32,
    pub look_delta: (f32, f32),
    pub raw_look_delta: (f32, f32),
}

impl Default for ActionState {
    fn default() -> Self {
        Self {
            context: InputContext::Menu,
            move_x: 0.0,
            move_y: 0.0,
            move_z: 0.0,
            sprint: false,
            crouch: false,
            jump: false,
            jump_pressed: false,
            toggle_fly: false,
            toggle_cursor: false,
            hotbar_slot: None,
            hotbar_scroll: 0,
            scroll_delta: 0.0,
            look_delta: (0.0, 0.0),
            raw_look_delta: (0.0, 0.0),
        }
    }
}

#[derive(Debug)]
pub struct InputProcessor {
    bindings: Bindings,
    prev_keys: std::collections::HashSet<KeyCode>,
    prev_mouse: std::collections::HashSet<MouseButton>,
}

impl InputProcessor {
    pub fn new(config: &ControlsConfig) -> Self {
        Self {
            bindings: Bindings::from_config(config),
            prev_keys: std::collections::HashSet::new(),
            prev_mouse: std::collections::HashSet::new(),
        }
    }

    pub fn process(&mut self, snapshot: &InputSnapshot) -> ActionState {
        let mut state = ActionState {
            context: snapshot.context,
            look_delta: (snapshot.mouse_delta.0 as f32, snapshot.mouse_delta.1 as f32),
            raw_look_delta: (
                snapshot.raw_mouse_delta.0 as f32,
                snapshot.raw_mouse_delta.1 as f32,
            ),
            scroll_delta: snapshot.scroll_delta,
            ..ActionState::default()
        };

        state.move_y = axis_value(
            &self.bindings,
            Action::MoveForward,
            Action::MoveBackward,
            snapshot,
        );
        state.move_x = axis_value(
            &self.bindings,
            Action::MoveRight,
            Action::MoveLeft,
            snapshot,
        );
        state.move_z = axis_value(&self.bindings, Action::MoveUp, Action::MoveDown, snapshot);

        state.sprint = self.action_active(Action::Sprint, snapshot);
        state.crouch = self.action_active(Action::Crouch, snapshot);
        state.jump = self.action_active(Action::Jump, snapshot);
        state.jump_pressed = self.action_triggered(Action::Jump, snapshot);
        state.toggle_fly = self.action_triggered(Action::ToggleFly, snapshot);
        state.toggle_cursor = self.action_triggered(Action::ToggleCursor, snapshot);
        state.hotbar_slot = self.detect_hotbar_slot(snapshot);
        state.hotbar_scroll = self.detect_hotbar_scroll(snapshot);

        self.prev_keys = snapshot.keys_pressed.clone();
        self.prev_mouse = snapshot.mouse_buttons.clone();

        state
    }

    fn detect_hotbar_slot(&self, snapshot: &InputSnapshot) -> Option<u8> {
        (0..9u8).find(|&slot| self.action_triggered(Action::HotbarSlot(slot), snapshot))
    }

    fn detect_hotbar_scroll(&self, snapshot: &InputSnapshot) -> i32 {
        if snapshot.scroll_delta > 0.1 {
            1
        } else if snapshot.scroll_delta < -0.1 {
            -1
        } else if self.action_triggered(Action::HotbarScrollUp, snapshot) {
            1
        } else if self.action_triggered(Action::HotbarScrollDown, snapshot) {
            -1
        } else {
            0
        }
    }

    fn action_active(&self, action: Action, snapshot: &InputSnapshot) -> bool {
        if let Some(bindings) = self.bindings.bindings_for(&action, snapshot.context) {
            bindings
                .iter()
                .any(|binding| binding_active(binding, snapshot))
        } else {
            false
        }
    }

    fn action_triggered(&self, action: Action, snapshot: &InputSnapshot) -> bool {
        if let Some(bindings) = self.bindings.bindings_for(&action, snapshot.context) {
            bindings.iter().any(|binding| {
                binding_triggered(binding, snapshot, &self.prev_keys, &self.prev_mouse)
            })
        } else {
            false
        }
    }
}

fn axis_value(
    bindings: &Bindings,
    positive: Action,
    negative: Action,
    snapshot: &InputSnapshot,
) -> f32 {
    let pos = bindings
        .bindings_for(&positive, snapshot.context)
        .map(|list| list.iter().any(|binding| binding_active(binding, snapshot)))
        .unwrap_or(false);
    let neg = bindings
        .bindings_for(&negative, snapshot.context)
        .map(|list| list.iter().any(|binding| binding_active(binding, snapshot)))
        .unwrap_or(false);

    (pos as i32 - neg as i32) as f32
}

fn binding_active(binding: &InputBinding, snapshot: &InputSnapshot) -> bool {
    match binding {
        InputBinding::Key(code) => snapshot.keys_pressed.contains(code),
        InputBinding::Mouse(btn) => snapshot.mouse_buttons.contains(btn),
        InputBinding::ScrollUp => snapshot.scroll_delta > 0.0,
        InputBinding::ScrollDown => snapshot.scroll_delta < 0.0,
    }
}

fn binding_triggered(
    binding: &InputBinding,
    snapshot: &InputSnapshot,
    prev_keys: &std::collections::HashSet<KeyCode>,
    prev_mouse: &std::collections::HashSet<MouseButton>,
) -> bool {
    match binding {
        InputBinding::Key(code) => {
            snapshot.keys_pressed.contains(code) && !prev_keys.contains(code)
        }
        InputBinding::Mouse(btn) => {
            snapshot.mouse_buttons.contains(btn) && !prev_mouse.contains(btn)
        }
        InputBinding::ScrollUp => snapshot.scroll_delta > 0.0,
        InputBinding::ScrollDown => snapshot.scroll_delta < 0.0,
    }
}

fn default_base_bindings() -> Vec<(Action, Vec<InputBinding>)> {
    use InputBinding::Key;
    vec![
        (Action::MoveForward, vec![Key(KeyCode::KeyW)]),
        (Action::MoveBackward, vec![Key(KeyCode::KeyS)]),
        (Action::MoveLeft, vec![Key(KeyCode::KeyA)]),
        (Action::MoveRight, vec![Key(KeyCode::KeyD)]),
        (Action::MoveUp, vec![Key(KeyCode::KeyE)]),
        (Action::MoveDown, vec![Key(KeyCode::KeyQ)]),
        (Action::Jump, vec![Key(KeyCode::Space)]),
        (Action::Sprint, vec![Key(KeyCode::ShiftLeft)]),
        (Action::Crouch, vec![Key(KeyCode::ControlLeft)]),
        (Action::ToggleFly, vec![Key(KeyCode::F4)]),
        (Action::ToggleCursor, vec![Key(KeyCode::Tab)]),
        (Action::HotbarScrollUp, vec![InputBinding::ScrollUp]),
        (Action::HotbarScrollDown, vec![InputBinding::ScrollDown]),
        (Action::HotbarSlot(0), vec![Key(KeyCode::Digit1)]),
        (Action::HotbarSlot(1), vec![Key(KeyCode::Digit2)]),
        (Action::HotbarSlot(2), vec![Key(KeyCode::Digit3)]),
        (Action::HotbarSlot(3), vec![Key(KeyCode::Digit4)]),
        (Action::HotbarSlot(4), vec![Key(KeyCode::Digit5)]),
        (Action::HotbarSlot(5), vec![Key(KeyCode::Digit6)]),
        (Action::HotbarSlot(6), vec![Key(KeyCode::Digit7)]),
        (Action::HotbarSlot(7), vec![Key(KeyCode::Digit8)]),
        (Action::HotbarSlot(8), vec![Key(KeyCode::Digit9)]),
    ]
}

fn apply_overrides(
    base: &mut BindingLayer,
    gameplay: &mut BindingLayer,
    ui: &mut BindingLayer,
    overrides: &BindingOverrides,
) {
    for (action_name, bindings) in &overrides.base {
        if let Some(action) = parse_action(action_name) {
            base.insert(action, parse_bindings(bindings));
        } else {
            warn!("Unknown action '{}' in base bindings", action_name);
        }
    }
    for (action_name, bindings) in &overrides.gameplay {
        if let Some(action) = parse_action(action_name) {
            gameplay.insert(action, parse_bindings(bindings));
        } else {
            warn!("Unknown action '{}' in gameplay bindings", action_name);
        }
    }
    for (action_name, bindings) in &overrides.ui {
        if let Some(action) = parse_action(action_name) {
            ui.insert(action, parse_bindings(bindings));
        } else {
            warn!("Unknown action '{}' in UI bindings", action_name);
        }
    }
}

fn parse_bindings(tokens: &[String]) -> Vec<InputBinding> {
    tokens
        .iter()
        .filter_map(|token| {
            parse_binding(token).or_else(|| {
                warn!("Unknown binding token '{}'; ignoring", token);
                None
            })
        })
        .collect()
}

fn parse_binding(token: &str) -> Option<InputBinding> {
    if let Some(key) = parse_key_code(token) {
        return Some(InputBinding::Key(key));
    }
    match token {
        "MouseLeft" => Some(InputBinding::Mouse(MouseButton::Left)),
        "MouseRight" => Some(InputBinding::Mouse(MouseButton::Right)),
        "ScrollUp" => Some(InputBinding::ScrollUp),
        "ScrollDown" => Some(InputBinding::ScrollDown),
        _ => None,
    }
}

fn parse_key_code(name: &str) -> Option<KeyCode> {
    Some(match name {
        "KeyW" => KeyCode::KeyW,
        "KeyA" => KeyCode::KeyA,
        "KeyS" => KeyCode::KeyS,
        "KeyD" => KeyCode::KeyD,
        "KeyQ" => KeyCode::KeyQ,
        "KeyE" => KeyCode::KeyE,
        "KeyR" => KeyCode::KeyR,
        "KeyF" => KeyCode::KeyF,
        "KeyC" => KeyCode::KeyC,
        "KeyV" => KeyCode::KeyV,
        "Space" => KeyCode::Space,
        "ShiftLeft" => KeyCode::ShiftLeft,
        "ControlLeft" => KeyCode::ControlLeft,
        "Tab" => KeyCode::Tab,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "Digit1" => KeyCode::Digit1,
        "Digit2" => KeyCode::Digit2,
        "Digit3" => KeyCode::Digit3,
        "Digit4" => KeyCode::Digit4,
        "Digit5" => KeyCode::Digit5,
        "Digit6" => KeyCode::Digit6,
        "Digit7" => KeyCode::Digit7,
        "Digit8" => KeyCode::Digit8,
        "Digit9" => KeyCode::Digit9,
        _ => return None,
    })
}

fn parse_action(name: &str) -> Option<Action> {
    match name {
        "MoveForward" => Some(Action::MoveForward),
        "MoveBackward" => Some(Action::MoveBackward),
        "MoveLeft" => Some(Action::MoveLeft),
        "MoveRight" => Some(Action::MoveRight),
        "MoveUp" => Some(Action::MoveUp),
        "MoveDown" => Some(Action::MoveDown),
        "Jump" => Some(Action::Jump),
        "Sprint" => Some(Action::Sprint),
        "Crouch" => Some(Action::Crouch),
        "ToggleFly" => Some(Action::ToggleFly),
        "ToggleCursor" => Some(Action::ToggleCursor),
        "HotbarScrollUp" => Some(Action::HotbarScrollUp),
        "HotbarScrollDown" => Some(Action::HotbarScrollDown),
        _ => {
            if let Some(rest) = name.strip_prefix("Hotbar") {
                if let Ok(idx) = rest.parse::<u8>() {
                    if (1..=9).contains(&idx) {
                        return Some(Action::HotbarSlot(idx - 1));
                    }
                }
            }
            None
        }
    }
}
