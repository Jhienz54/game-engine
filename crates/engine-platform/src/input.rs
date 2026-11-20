//! Accumulates winit input events into pending sets, then snapshots
//! them into a fresh [`InputState`] at each fixed step.

use std::collections::BTreeSet;

use engine_core::{InputState, KeyCode, MouseButton};
use glam::Vec2;
use winit::event::{ElementState, MouseButton as WMButton, MouseScrollDelta};
use winit::keyboard::{KeyCode as WKeyCode};

/// Mutable accumulating state. Written by `window_event` as events
/// arrive; drained by [`take_snapshot`] once per fixed step.
#[derive(Default)]
pub(crate) struct PendingInput {
    held_keys: BTreeSet<KeyCode>,
    pressed_keys: BTreeSet<KeyCode>,
    released_keys: BTreeSet<KeyCode>,

    held_mouse_buttons: BTreeSet<MouseButton>,
    pressed_mouse_buttons: BTreeSet<MouseButton>,
    released_mouse_buttons: BTreeSet<MouseButton>,

    mouse_position: Vec2,
    /// Cursor position as of the last fixed-step snapshot; delta is
    /// `mouse_position - last_step_position`.
    last_step_position: Vec2,
    scroll: Vec2,
}

impl PendingInput {
    pub(crate) fn key_event(&mut self, code: WKeyCode, state: ElementState, repeat: bool) {
        let Some(kc) = map_keycode(code) else { return };
        match state {
            ElementState::Pressed => {
                if !repeat && self.held_keys.insert(kc) {
                    self.pressed_keys.insert(kc);
                }
            }
            ElementState::Released => {
                if self.held_keys.remove(&kc) {
                    self.released_keys.insert(kc);
                }
            }
        }
    }

    pub(crate) fn mouse_button_event(&mut self, button: WMButton, state: ElementState) {
        let Some(mb) = map_mouse_button(button) else { return };
        match state {
            ElementState::Pressed => {
                if self.held_mouse_buttons.insert(mb) {
                    self.pressed_mouse_buttons.insert(mb);
                }
            }
            ElementState::Released => {
                if self.held_mouse_buttons.remove(&mb) {
                    self.released_mouse_buttons.insert(mb);
                }
            }
        }
    }

    pub(crate) fn cursor_moved(&mut self, x: f64, y: f64) {
        self.mouse_position = Vec2::new(x as f32, y as f32);
    }

    pub(crate) fn scroll(&mut self, delta: MouseScrollDelta) {
        match delta {
            MouseScrollDelta::LineDelta(x, y) => {
                self.scroll += Vec2::new(x, y);
            }
            MouseScrollDelta::PixelDelta(p) => {
                self.scroll += Vec2::new(p.x as f32, p.y as f32);
            }
        }
    }

    /// Snapshot pending input into an `InputState` and clear edges.
    /// Called once per fixed step.
    pub(crate) fn take_snapshot(&mut self) -> InputState {
        let delta = self.mouse_position - self.last_step_position;
        self.last_step_position = self.mouse_position;
        InputState::from_parts(
            self.held_keys.clone(),
            std::mem::take(&mut self.pressed_keys),
            std::mem::take(&mut self.released_keys),
            self.held_mouse_buttons.clone(),
            std::mem::take(&mut self.pressed_mouse_buttons),
            std::mem::take(&mut self.released_mouse_buttons),
            self.mouse_position,
            delta,
            std::mem::take(&mut self.scroll),
        )
    }
}

fn map_keycode(k: WKeyCode) -> Option<KeyCode> {
    use KeyCode::*;
    use WKeyCode as W;
    Some(match k {
        W::KeyA => A, W::KeyB => B, W::KeyC => C, W::KeyD => D,
        W::KeyE => E, W::KeyF => F, W::KeyG => G, W::KeyH => H,
        W::KeyI => I, W::KeyJ => J, W::KeyK => K, W::KeyL => L,
        W::KeyM => M, W::KeyN => N, W::KeyO => O, W::KeyP => P,
        W::KeyQ => Q, W::KeyR => R, W::KeyS => S, W::KeyT => T,
        W::KeyU => U, W::KeyV => V, W::KeyW => W, W::KeyX => X,
        W::KeyY => Y, W::KeyZ => Z,
        W::Digit0 => Digit0, W::Digit1 => Digit1, W::Digit2 => Digit2,
        W::Digit3 => Digit3, W::Digit4 => Digit4, W::Digit5 => Digit5,
        W::Digit6 => Digit6, W::Digit7 => Digit7, W::Digit8 => Digit8,
        W::Digit9 => Digit9,
        W::ArrowUp => ArrowUp, W::ArrowDown => ArrowDown,
        W::ArrowLeft => ArrowLeft, W::ArrowRight => ArrowRight,
        W::Space => Space, W::Enter => Enter, W::Tab => Tab,
        W::Backspace => Backspace, W::Escape => Escape,
        W::ShiftLeft => ShiftLeft, W::ShiftRight => ShiftRight,
        W::ControlLeft => ControlLeft, W::ControlRight => ControlRight,
        W::AltLeft => AltLeft, W::AltRight => AltRight,
        W::F1 => F1, W::F2 => F2, W::F3 => F3, W::F4 => F4,
        W::F5 => F5, W::F6 => F6, W::F7 => F7, W::F8 => F8,
        W::F9 => F9, W::F10 => F10, W::F11 => F11, W::F12 => F12,
        _ => return None,
    })
}