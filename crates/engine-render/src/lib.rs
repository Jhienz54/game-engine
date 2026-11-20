//! `engine-render`: the wgpu-based 2D renderer.
//!
//! Reads from `engine-core` scene data but never owns it. A future 3D
//! module will live alongside as `Renderer3D` under a feature flag.
//!
//! ## Backend selection
//!
//! By default, **all wgpu backends are enabled** (Vulkan, GL, Metal,
//! DX12, …). On Linux this means Vulkan is preferred, with GL as a
//! fallback. To force a specific backend:
//!
//! ```text
//! WGPU_BACKEND=gl cargo run -p topdown-demo
//! WGPU_BACKEND=vulkan,gl cargo run -p topdown-demo
//! ```
//!
//! The chosen adapter's name, backend, device type, and driver are
//! logged at `INFO` level during `Renderer::new`. This is how you
//! diagnose "is llvmpipe really running, or is it a Vulkan
//! software driver?"
//!
//! ## Texture upload contract (M6)
//!
//! ```text
//!   engine-assets::load_png(path)  ->  (TextureHandle, DecodedImage)
//!   renderer.upload_texture(handle, &image)  ->  GpuTexture
//! ```
//!
//! ## Limits
//!
//! We request `Limits::downlevel_webgl2_defaults()` — deliberately
//! conservative so the same binary runs on llvmpipe / GLES3 and on
//! modern Vulkan hardware without negotiation.

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod renderer;

pub use error::RendererError;
pub use renderer::Renderer;