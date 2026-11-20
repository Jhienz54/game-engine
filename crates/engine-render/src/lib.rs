//! `engine-render`: the wgpu-based 2D renderer.
//!
//! Reads from `engine-core` scene data but never owns it — the game
//! owns the `Scene`, the renderer draws it. A future 3D module will be
//! a sibling (`Renderer3D`) enabled by a feature flag; the two do not
//! share public APIs.
//!
//! ## Texture upload contract
//!
//! Textures are loaded *decoded* by `engine-assets` and then explicitly
//! handed to the renderer:
//!
//! ```text
//!   engine-assets::load_png(path)  ->  (TextureHandle, DecodedImage)
//!   renderer.upload_texture(handle, &image)  ->  GpuTexture
//! ```
//!
//! The renderer keeps `HashMap<TextureHandle, wgpu::Texture>`. This
//! boundary lets us swap decoding or add async/hot-reload without
//! touching the renderer.

#![deny(unsafe_code)]
#![warn(missing_docs)]