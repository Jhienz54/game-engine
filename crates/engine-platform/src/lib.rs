//! `engine-platform`: window creation, event loop, and input mapping
//! via `winit`.
//!
//! Produces `engine_core::InputState` snapshots for the fixed-step
//! simulation, and forwards window events for the game to react to.
//! Gamepads (`gilrs`) are deferred past v0.1.
//!
//! ## Frame loop
//!
//! ```text
//! about_to_wait:
//!   clock.begin_frame()
//!   while clock.consume_fixed():
//!       input := pending.take_snapshot()
//!       app.fixed_update(1/60, input)
//!   alpha := clock.alpha()
//!   request_redraw()
//! RedrawRequested:
//!   app.redraw(alpha)
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod app;
mod clock;
mod error;
mod input;

pub use app::{run, PlatformApp};
pub use error::PlatformError;

pub use winit::event::WindowEvent;
pub use winit::window::Window;