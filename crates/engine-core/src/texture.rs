//! Texture *handles* and *descriptors*. No pixels, no GPU.
//!
//! `engine-assets` produces handles; `engine-render` (M6) consumes them.
//! `engine-core` stays free of `image`, `wgpu`, and file I/O.

use serde::{Deserialize, Serialize};

/// Stable handle to a loaded texture.
///
/// A plain newtype over `u32` for now. If asset unloading ever lands
/// (hot reload does **not** require it — handles survive reloads), this
/// becomes generational the same way `NodeId` is. Games treat the inner
/// value as opaque; only `engine-assets` and `engine-render` construct
/// and unwrap it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TextureHandle(u32);

impl TextureHandle {
    /// Construct a handle from a raw index. Only `engine-assets` calls
    /// this. `#[doc(hidden)]` because it is an implementation detail of
    /// the handle-allocation strategy.
    #[doc(hidden)]
    pub const fn from_raw(idx: u32) -> Self {
        Self(idx)
    }

    /// Raw index. Only `engine-render` calls this to look up GPU state.
    #[doc(hidden)]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Per-texture sampling filter.
///
/// Stored on the texture, not in a global setting, because a single
/// scene routinely mixes pixel art (nearest) with high-res portraits
/// (linear).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FilterMode {
    /// Nearest-neighbour; no blending. The right choice for pixel art.
    Nearest,
    /// Bilinear; smooth interpolation. Sensible default for photographs
    /// and high-resolution art.
    #[default]
    Linear,
}

/// Descriptor for a loaded texture: everything the renderer needs to
/// create a sampler and bind a texture, minus the pixel bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureMeta {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Sampling filter.
    pub filter: FilterMode,
    /// Human-readable source: a canonical file path, or
    /// `"placeholder:missing"` for the generated fallback. Used by logs
    /// and by the future editor’s asset inspector.
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_default_is_linear() {
        assert_eq!(FilterMode::default(), FilterMode::Linear);
    }

    #[test]
    fn texture_meta_ron_roundtrip() {
        let meta = TextureMeta {
            width: 64,
            height: 32,
            filter: FilterMode::Nearest,
            label: "art/card.png".into(),
        };
        let s = ron::to_string(&meta).unwrap();
        let back: TextureMeta = ron::from_str(&s).unwrap();
        assert_eq!(meta, back);
    }
}