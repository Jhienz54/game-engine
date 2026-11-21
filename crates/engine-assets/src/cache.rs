//! CPU-side image cache and placeholder generation.
//!
//! This is intentionally synchronous. Startup loads everything the game
//! needs; hot reload (later) re-runs the same code path on the changed
//! file. Do not move decoding onto a background thread until a game
//! actually stalls on it — that changes the API surface for no measured
//! win.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use engine_core::{FilterMode, TextureHandle, TextureMeta};
use image::ImageReader;

use crate::error::AssetError;

/// One loaded image: RGBA8 pixels plus metadata. Not yet uploaded to the
/// GPU; `engine-render` (M6) will consume these via
/// `upload_texture(handle, &ImageAsset)`.
#[derive(Debug, Clone)]
pub struct ImageAsset {
    /// Descriptor (dimensions, filter, label).
    pub meta: TextureMeta,
    /// RGBA8 pixels, row-major, top-left origin.
    /// Length is exactly `width * height * 4`.
    pub pixels: Vec<u8>,
}

impl ImageAsset {
    /// Create a single-colour image. Useful for card backgrounds, debug
    /// tints, and generated card faces.
    ///
    /// Uses 4×4 rather than 1×1 so that nearest vs. linear filtering has
    /// something to interpolate when stretched — a 1×1 stretched to N×N
    /// looks identical either way and would hide filter bugs in tests.
    pub fn solid(rgba: [u8; 4], filter: FilterMode, label: impl Into<String>) -> Self {
        const SIZE: u32 = 4;
        let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for _ in 0..(SIZE * SIZE) {
            pixels.extend_from_slice(&rgba);
        }
        Self {
            meta: TextureMeta {
                width: SIZE,
                height: SIZE,
                filter,
                label: label.into(),
            },
            pixels,
        }
    }

    /// Magenta-and-black checkerboard used when a texture is missing.
    ///
    /// Loud on purpose: you cannot mistake it for intentional art.
    pub fn missing_texture() -> Self {
        const SIZE: u32 = 16;
        const CHECKER: u32 = 4;
        const MAGENTA: [u8; 4] = [255, 0, 255, 255];
        const BLACK: [u8; 4] = [0, 0, 0, 255];

        let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let even = ((x / CHECKER) + (y / CHECKER)) % 2 == 0;
                let px = if even { MAGENTA } else { BLACK };
                pixels.extend_from_slice(&px);
            }
        }
        Self {
            meta: TextureMeta {
                width: SIZE,
                height: SIZE,
                filter: FilterMode::Nearest,
                label: "placeholder:missing".to_string(),
            },
            pixels,
        }
    }
}

/// Cache of loaded image assets, keyed by canonicalized path.
///
/// Uses `HashMap` deliberately. Asset loading is a startup / hot-reload
/// concern, not a per-step simulation concern; the `BTreeMap` rule in
/// `engine-core` applies to game state, not to this table. If you ever
/// need deterministic iteration over loaded assets (build tools,
/// manifests), add a sorted view alongside — do not switch the backing
/// store.
#[derive(Default)]
pub struct AssetCache {
    images: Vec<ImageAsset>,
    by_path: HashMap<PathBuf, TextureHandle>,
    /// Lazily-created shared placeholder. `None` until first requested.
    missing_handle: Option<TextureHandle>,
}

impl AssetCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a PNG or JPEG from disk, decode to RGBA8, and cache it.
    /// Repeated calls with the same canonicalized path return the same
    /// handle without re-decoding.
    ///
    /// Uses `FilterMode::Linear`. For pixel art, use
    /// [`load_image_with_filter`](Self::load_image_with_filter).
    pub fn load_image(&mut self, path: impl AsRef<Path>) -> Result<TextureHandle, AssetError> {
        self.load_image_with_filter(path, FilterMode::Linear)
    }

    /// As [`load_image`](Self::load_image), but with an explicit filter.
    ///
    /// The filter is only applied on first load. A subsequent call with
    /// a different filter and the same path returns the cached handle
    /// unchanged — the filter is part of the texture, not of the call
    /// site. To change it later, use hot reload (future milestone) or a
    /// separate cache.
    pub fn load_image_with_filter(
        &mut self,
        path: impl AsRef<Path>,
        filter: FilterMode,
    ) -> Result<TextureHandle, AssetError> {
        let path = path.as_ref();

        // Canonicalize so `"art/foo.png"` and `"./art/foo.png"` dedup.
        // If canonicalization fails (file missing, symlink loop), fall
        // back to the given path — the read below will produce the real
        // error, which is more useful to the caller than a path-normalize
        // error.
        let key = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => path.to_path_buf(),
        };

        if let Some(&h) = self.by_path.get(&key) {
            return Ok(h);
        }

        let reader = ImageReader::open(path).map_err(|source| AssetError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let img = reader.decode().map_err(|source| AssetError::Decode {
            path: path.to_path_buf(),
            source,
        })?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let asset = ImageAsset {
            meta: TextureMeta {
                width,
                height,
                filter,
                label: key.to_string_lossy().into_owned(),
            },
            pixels: rgba.into_raw(),
        };

        let handle = self.push(asset);
        self.by_path.insert(key, handle);
        Ok(handle)
    }

    /// Get-or-create the shared missing-texture placeholder. All calls
    /// return the same handle.
    pub fn missing_texture(&mut self) -> TextureHandle {
        if let Some(h) = self.missing_handle {
            return h;
        }
        let handle = self.push(ImageAsset::missing_texture());
        self.missing_handle = Some(handle);
        handle
    }

    /// Insert a caller-supplied image (procedural card, test fixture,
    /// hot-reloaded bytes). Not deduplicated — each call allocates a new
    /// handle. The caller owns the label.
    pub fn insert(&mut self, asset: ImageAsset) -> TextureHandle {
        self.push(asset)
    }

    /// Look up a handle’s data. Returns `None` for unknown handles —
    /// e.g. one produced by a different cache.
    pub fn get(&self, handle: TextureHandle) -> Option<&ImageAsset> {
        self.images.get(handle.raw() as usize)
    }

    /// Number of loaded images, including placeholders.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// True iff nothing has been loaded.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    fn push(&mut self, asset: ImageAsset) -> TextureHandle {
        let handle = TextureHandle::from_raw(self.images.len() as u32);
        self.images.push(asset);
        handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_texture_is_stable_across_calls() {
        let mut cache = AssetCache::new();
        let h1 = cache.missing_texture();
        let h2 = cache.missing_texture();
        assert_eq!(h1, h2);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn missing_texture_is_checkerboard() {
        let mut cache = AssetCache::new();
        let h = cache.missing_texture();
        let img = cache.get(h).expect("missing texture must be cached");
        assert_eq!(img.meta.width, 16);
        assert_eq!(img.meta.height, 16);
        assert_eq!(img.meta.filter, FilterMode::Nearest);
        assert_eq!(img.pixels.len(), 16 * 16 * 4);
        // (0,0) → even checker → magenta.
        assert_eq!(&img.pixels[0..4], &[255, 0, 255, 255]);
        // (4,0) → odd checker → black. Pixel index = 4, byte offset = 16.
        assert_eq!(&img.pixels[16..20], &[0, 0, 0, 255]);
    }

    #[test]
    fn solid_color_asset() {
        let img = ImageAsset::solid([10, 20, 30, 255], FilterMode::Nearest, "test");
        assert_eq!(img.meta.width, 4);
        assert_eq!(img.meta.height, 4);
        assert_eq!(img.pixels.len(), 4 * 4 * 4);
        assert_eq!(&img.pixels[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn unknown_handle_returns_none() {
        let cache = AssetCache::new();
        assert!(cache.get(TextureHandle::from_raw(42)).is_none());
    }

    #[test]
    fn load_image_dedups_by_path() -> Result<(), Box<dyn std::error::Error>> {
        // Write a 2×2 red PNG to the OS temp dir, load it twice, and
        // assert both calls return the same handle with the expected
        // bytes. Uses `image` to write the fixture (the SUT is our
        // cache/dedup/meta extraction, not `image`).
        let path = std::env::temp_dir().join(format!(
            "engine-m4-test-{}.png",
            std::process::id()
        ));
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        img.save(&path)?;

        let mut cache = AssetCache::new();
        let a = cache.load_image(&path)?;
        let b = cache.load_image(&path)?;
        assert_eq!(a, b);
        assert_eq!(cache.len(), 1);

        let loaded = cache.get(a).expect("just loaded");
        assert_eq!(loaded.meta.width, 2);
        assert_eq!(loaded.meta.height, 2);
        assert_eq!(loaded.meta.filter, FilterMode::Linear);
        assert_eq!(&loaded.pixels[0..4], &[255, 0, 0, 255]);

        let _ = std::fs::remove_file(&path);
        Ok(())
    }
}