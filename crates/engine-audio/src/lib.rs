//! `engine-audio`: audio interface. **Stub in v0.1.**
//!
//! The `AudioBackend` trait lands in M10 as a no-op implementation. Games
//! already write calls against `AudioHandle` and the trait; swapping in
//! a real backend later is a one-crate change.

#![deny(unsafe_code)]
#![warn(missing_docs)]