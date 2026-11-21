//! Errors returned by asset loading.

use std::path::PathBuf;

/// Errors returned by `engine-assets`.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// Reading the file off disk failed.
    #[error("failed to read {path:?}: {source}")]
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The `image` crate could not decode the file as any supported
    /// format. The `source` carries the specific reason (bad magic,
    /// truncated stream, unsupported format, …).
    #[error("failed to decode {path:?}: {source}")]
    Decode {
        /// The file that could not be decoded.
        path: PathBuf,
        /// Underlying decode error.
        #[source]
        source: image::ImageError,
    },
}