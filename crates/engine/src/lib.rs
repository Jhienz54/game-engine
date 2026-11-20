//! `engine`: the facade crate. **Games depend on this one.**
//!
//! Re-exports the public API of every sub-crate so games do:
//!
//! ```ignore
//! use engine::{run, PlatformApp, Window, WindowEvent, Renderer};
//! ```
//!
//! Feature flags will later let us trim unused sub-crates (e.g. a
//! `headless` feature that drops `engine-render`).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub use engine_core as core;
pub use engine_render as render;
pub use engine_platform as platform;
pub use engine_assets as assets;
pub use engine_audio as audio;

// Top-level re-exports for the parts games actually touch.
pub use engine_core::{NodeId, Scene, SceneError, Transform2D, VERSION};

pub use engine_platform::{run, PlatformApp, PlatformError, Window, WindowEvent};

pub use engine_render::{Renderer, RendererError};