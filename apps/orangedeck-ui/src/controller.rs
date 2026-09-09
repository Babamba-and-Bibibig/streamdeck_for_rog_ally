use std::time::{Duration, Instant};

use eframe::egui;
use gilrs::{Axis, Button, EventType, Gilrs};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlAction {
    NavigateLeft,
    NavigateRight,
    NavigateUp,
    NavigateDown,
    Activate,
    Back,
    Context,
    Detail,
    PreviousPage,
    NextPage,
    PreviousThread,
    NextThread,
}

pub struct ControllerInput {
    gilrs: Option<Gilrs>,
    detected: Vec<String>,
    last_axis_action: Instant,
    last_left_x: f32,
    last_left_y: f32,
    was_focused: bool,
}

impl ControllerInput {
    pub fn new() -> Self {
        match Gilrs::new() {
            Ok(gilrs) => {
                let detected = gilrs
                    .gamepads()
                    .map(|(_, gamepad)| gamepad.name().to_owned())
                    .collect();
                Self {
                    gilrs: Some(gilrs),
                    detected,
                    last_axis_action: Instant::now()
                        .checked_sub(Duration::from_secs(1))
                        .unwrap_or_else(Instant::now),
                    last_left_x: 0.0,
                    last_left_y: 0.0,
                    was_focused: false,
                }
            }
            Err(error) => {
                tracing::warn!(%error, "게임패드를 초기화하지 못했습니다. 키보드와 터치는 계속 사용할 수 있습니다.");
                Self {
                    gilrs: None,
                    detected: Vec::new(),
                    last_axis_action: Instant::now(),
                    last_left_x: 0.0,
                    last_left_y: 0.0,
                    was_focused: false,
                }
            }
        }
    }

    pub fn detected(&self) -> &[String] {
        &self.detected
    }

    pub fn poll(
        &mut self,
        ctx: &egui::Context,
        focused: bool,
        approval_pending: bool,
    ) -> Vec<ControlAction> {
        let accept_input = focused && self.was_focused;
        self.was_focused = focused;
        let mut actions = if accept_input {
            keyboard_actions(ctx)
        } else {
            Vec::new()
        };
        if approval_pending {
            // Approval requires a touch decision or a fresh physical A/B press.
            actions
                .retain(|action| !matches!(action, ControlAction::Activate | ControlAction::Back));
        }
        let Some(gilrs) = &mut self.gilrs else {
            return actions;
        };
        while let Some(event) = gilrs.next_event() {
            if !accept_input {
                continue;
            }
            match event.event {
                EventType::ButtonPressed(button, _) => {
                    if let Some(action) = button_action(button) {
                        actions.push(action);
                    }
                }
                EventType::AxisChanged(axis, value, _) => match axis {
                    Axis::LeftStickX => self.last_left_x = value,
                    Axis::LeftStickY => self.last_left_y = value,
                    _ => {}
                },
                EventType::Connected => {
                    self.detected = gilrs
                        .gamepads()
                        .map(|(_, gamepad)| gamepad.name().to_owned())
                        .collect();
                }
                EventType::Disconnected => {
                    self.detected = gilrs
                        .gamepads()
                        .filter(|(_, gamepad)| gamepad.is_connected())
                        .map(|(_, gamepad)| gamepad.name().to_owned())
                        .collect();
                }
                _ => {}
            }
        }
        if !accept_input {
            self.last_left_x = 0.0;
            self.last_left_y = 0.0;
            return actions;
        }
        if self.last_axis_action.elapsed() >= Duration::from_millis(170) {
            let action = if self.last_left_x > 0.62 {
                Some(ControlAction::NavigateRight)
            } else if self.last_left_x < -0.62 {
                Some(ControlAction::NavigateLeft)
            } else if self.last_left_y > 0.62 {
                Some(ControlAction::NavigateUp)
            } else if self.last_left_y < -0.62 {
                Some(ControlAction::NavigateDown)
            } else {
                None
            };
            if let Some(action) = action {
                actions.push(action);
                self.last_axis_action = Instant::now();
            }
        }
        actions
    }
}

impl Default for ControllerInput {
    fn default() -> Self {
        Self::new()
    }
}

fn button_action(button: Button) -> Option<ControlAction> {
    match button {
        Button::South => Some(ControlAction::Activate),
        Button::East => Some(ControlAction::Back),
        Button::West => Some(ControlAction::Context),
        Button::North => Some(ControlAction::Detail),
        Button::DPadLeft => Some(ControlAction::NavigateLeft),
        Button::DPadRight => Some(ControlAction::NavigateRight),
        Button::DPadUp => Some(ControlAction::NavigateUp),
        Button::DPadDown => Some(ControlAction::NavigateDown),
        Button::LeftTrigger => Some(ControlAction::PreviousPage),
        Button::RightTrigger => Some(ControlAction::NextPage),
        Button::LeftTrigger2 => Some(ControlAction::PreviousThread),
        Button::RightTrigger2 => Some(ControlAction::NextThread),
        _ => None,
    }
}

fn keyboard_actions(ctx: &egui::Context) -> Vec<ControlAction> {
    if ctx.text_edit_focused() {
        return Vec::new();
    }
    ctx.input(|input| {
        let mut actions = Vec::new();
        let mut push = |key, action| {
            if input.key_pressed(key) {
                actions.push(action);
            }
        };
        push(egui::Key::ArrowLeft, ControlAction::NavigateLeft);
        push(egui::Key::ArrowRight, ControlAction::NavigateRight);
        push(egui::Key::ArrowUp, ControlAction::NavigateUp);
        push(egui::Key::ArrowDown, ControlAction::NavigateDown);
        push(egui::Key::Enter, ControlAction::Activate);
        push(egui::Key::Escape, ControlAction::Back);
        push(egui::Key::Q, ControlAction::PreviousPage);
        push(egui::Key::E, ControlAction::NextPage);
        push(egui::Key::Z, ControlAction::PreviousThread);
        push(egui::Key::C, ControlAction::NextThread);
        push(egui::Key::X, ControlAction::Context);
        push(egui::Key::Y, ControlAction::Detail);
        actions
    })
}
