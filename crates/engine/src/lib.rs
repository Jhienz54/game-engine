//! `engine`: the facade crate. **Games depend on this one.**
//!
//! Re-exports the public API of every sub-crate so games do:
//!
//! ```ignore
//! use engine::{Game, Scene, NodeId, Transform2D, InputState, /* ... */};
//! ```
//!
//! and never need to add four dependencies by hand. Feature flags will
//! later let us trim unused sub-crates (e.g. `default-features = false`
//! for a headless build with no `engine-render`).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub use engine_core as core;
pub use engine_render as render;
pub use engine_platform as platform;
pub use engine_assets as assets;
pub use engine_audio as audio;

pub use engine_core::{ NodeId, Scene, Transform2D, SceneError, VERSION };
// Sub-crate public items re-exported at the top level land in M1+
// as each crate gains its API.