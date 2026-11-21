//! `engine-assets`: synchronous image loading, decoding, and caching.
//!
//! CPU-side asset pipeline. Reads PNG/JPEG off disk, decodes to RGBA8,
//! and hands out `TextureHandle`s. `engine-render` (M6) will consume
//! those handles and upload the pixels to the GPU.
//!
//! ## Placeholders
//!
//! Everything the engine renders must be drawable with zero real art.
//! [`AssetCache::missing_texture`] yields a shared magenta/black
//! checker, and [`ImageAsset::solid`] makes single-colour tiles. A game
//! can ship M6 sprites on day one with no PNG on disk.
//!
//! ## Hot reload (future)
//!
//! Each image’s [`TextureMeta::label`] carries its canonical path (or
//! `"placeholder:missing"`). A future `poll_for_changes()` re-decodes
//! into the same slot; games holding a `TextureHandle` keep it. Adding
//! that method will not change this crate’s public API.

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod cache;
mod error;

pub use cache::{AssetCache, ImageAsset};
pub use error::AssetError;

// Re-export the core types this crate’s users will need.
pub use engine_core::{FilterMode, TextureHandle, TextureMeta};