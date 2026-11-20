//! `engine-assets`: file I/O, image decoding, and cache.
//!
//! Kept deliberately **out** of `engine-core` so the core stays pure
//! data. Here we read bytes off disk, decode PNG/JPEG into RGBA8, and
//! return the result paired with an opaque `TextureHandle` that the
//! renderer can later consume via `Renderer2D::upload_texture`.
//!
//! ## Hot-reload future
//!
//! Each loaded asset records `(path, modification_time)`. A future
//! `poll_for_changes()` can detect new mtimes and re-decode without
//! changing the public API — games that hold `TextureHandle` keep the
//! same handle; only the decoded bytes change.

#![deny(unsafe_code)]
#![warn(missing_docs)]