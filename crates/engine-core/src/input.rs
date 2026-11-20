//! Plain-data input snapshot. Populated by `engine-platform` from winit
//! events, consumed by the game in `fixed_update`.
//!
//! ## Determinism
//!
//! Backing collections are `BTreeSet`, whose iteration order is stable
//! on a given machine. Serializing `InputState` yields deterministic
//! bytes; replay systems can use it directly.

use std::collections::BTreeSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// A physical keyboard key, layout-independent.
///
/// Deliberately a closed set. Games needing unusual keys can ask us to
/// extend; a catch-all variant would reintroduce layout dependence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Digit0, Digit1, Digit2, Digit3, Digit4,
    Digit5, Digit6, Digit7, Digit8, Digit9,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Space, Enter, Tab, Backspace, Escape,
    ShiftLeft, ShiftRight,
    ControlLeft, ControlRight,
    AltLeft, AltRight,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
}

/// A physical mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// A point-in-time snapshot of all input devices.
///
/// Games always see this via `&InputState` and cannot mutate it.
///
/// Edges (`pressed_keys`, `released_keys`, …) are **per fixed step**.
/// The platform clears them after each step, so a single event fires
/// `key_pressed` exactly once. If a frame runs 0 fixed steps, edges
/// persist until a step does run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputState {
    held_keys: BTreeSet<KeyCode>,
    pressed_keys: BTreeSet<KeyCode>,
    released_keys: BTreeSet<KeyCode>,
    held_mouse_buttons: BTreeSet<MouseButton>,
    pressed_mouse_buttons: BTreeSet<MouseButton>,
    released_mouse_buttons: BTreeSet<MouseButton>,
    mouse_position: Vec2,
    mouse_delta: Vec2,
    scroll_delta: Vec2,
}

impl InputState {
    // ---- keyboard queries ------------------------------------------------

    /// Is `key` currently held down?
    pub fn key_down(&self, key: KeyCode) -> bool {
        self.held_keys.contains(&key)
    }

    /// Did `key` transition to down since the last fixed step?
    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    /// Did `key` transition to up since the last fixed step?
    pub fn key_released(&self, key: KeyCode) -> bool {
        self.released_keys.contains(&key)
    }

    /// All currently held keys, in deterministic `Ord` order.
    pub fn keys_down(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.held_keys.iter().copied()
    }

    /// All keys newly pressed since the last fixed step.
    pub fn keys_pressed(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.pressed_keys.iter().copied()
    }

    // ---- mouse queries ---------------------------------------------------

    /// Is `button` currently held down?
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        self.held_mouse_buttons.contains(&button)
    }

    /// Did `button` transition to down since the last fixed step?
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.pressed_mouse_buttons.contains(&button)
    }

    /// Did `button` transition to up since the last fixed step?
    pub fn mouse_released(&self, button: MouseButton) -> bool {
        self.released_mouse_buttons.contains(&button)
    }

    /// Cursor position in pixels, top-left origin.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_position
    }

    /// Mouse movement since the last fixed step, in pixels.
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    /// Scroll-wheel delta accumulated since the last fixed step.
    pub fn scroll_delta(&self) -> Vec2 {
        self.scroll_delta
    }

    // ---- construction (platform + tests only) ---------------------------

    /// Construct an `InputState`. `engine-platform` calls this from
    /// `PendingInput::take_snapshot`; tests construct synthetic inputs.
    /// Game code should never call it — but it is public because the
    /// platform layer is a separate crate.
    #[doc(hidden)]
    pub fn from_parts(
        held_keys: BTreeSet<KeyCode>,
        pressed_keys: BTreeSet<KeyCode>,
        released_keys: BTreeSet<KeyCode>,
        held_mouse_buttons: BTreeSet<MouseButton>,
        pressed_mouse_buttons: BTreeSet<MouseButton>,
        released_mouse_buttons: BTreeSet<MouseButton>,
        mouse_position: Vec2,
        mouse_delta: Vec2,
        scroll_delta: Vec2,
    ) -> Self {
        Self {
            held_keys,
            pressed_keys,
            released_keys,
            held_mouse_buttons,
            pressed_mouse_buttons,
            released_mouse_buttons,
            mouse_position,
            mouse_delta,
            scroll_delta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_queries() {
        let s = InputState::default();
        assert!(!s.key_down(KeyCode::A));
        assert!(!s.key_pressed(KeyCode::A));
        assert!(!s.mouse_down(MouseButton::Left));
        assert_eq!(s.mouse_position(), Vec2::ZERO);
    }

    #[test]
    fn from_parts_populates_queries() {
        let mut held = BTreeSet::new();
        held.insert(KeyCode::W);
        let mut pressed = BTreeSet::new();
        pressed.insert(KeyCode::Space);
        let s = InputState::from_parts(
            held,
            pressed,
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            Vec2::new(100.0, 50.0),
            Vec2::new(3.0, -2.0),
            Vec2::ZERO,
        );
        assert!(s.key_down(KeyCode::W));
        assert!(s.key_pressed(KeyCode::Space));
        assert!(!s.key_down(KeyCode::Space));
        assert_eq!(s.mouse_position(), Vec2::new(100.0, 50.0));
        assert_eq!(s.mouse_delta(), Vec2::new(3.0, -2.0));
    }

    #[test]
    fn keys_down_is_deterministic() {
        let mut held = BTreeSet::new();
        // Insert in reverse of Ord order; iteration must still be ascending.
        held.insert(KeyCode::Z);
        held.insert(KeyCode::A);
        held.insert(KeyCode::M);
        let s = InputState::from_parts(
            held,
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::ZERO,
        );
        let order: Vec<_> = s.keys_down().collect();
        assert_eq!(order, vec![KeyCode::A, KeyCode::M, KeyCode::Z]);
    }
}