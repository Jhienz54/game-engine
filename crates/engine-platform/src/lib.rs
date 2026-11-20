//! `engine-platform`: window creation, event loop, and input mapping
//! via `winit`.
//!
//! Produces `engine_core::InputState` snapshots for the simulation and
//! `PlatformEvent`s for the game to react to (resize, close, file drop).
//!
//! Gamepads (`gilrs`) are deliberately deferred past v0.1; the input
//! state reserves fields for them so adding them later is additive.

#![deny(unsafe_code)]
#![warn(missing_docs)]