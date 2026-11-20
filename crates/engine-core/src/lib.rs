//! `engine-core`: the engine's pure-data layer.
//!
//! No OS, no GPU, no I/O, no threads. `#![forbid(unsafe_code)]` is
//! permanent.
//!
//! ## Determinism contract for game-owned side tables
//!
//! Games attach their own data to nodes by keeping a side table keyed
//! by [`NodeId`]. To keep simulation deterministic **on the same
//! machine/toolchain**, side tables must **not** use `HashMap`.
//!
//! Preferred forms:
//!
//! - `BTreeMap<NodeId, T>` — ordered iteration.
//! - `Vec<Option<T>>` indexed by `NodeId::index()`, with a
//!   `generation` check on access to reject stale IDs. Fastest; `NodeId`
//!   exposes `index()` and `generation()` for exactly this.
//!
//! Cross-platform bit-exact determinism is **not** promised — float
//! math may differ across CPUs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod node_id;
mod scene;
mod transform;

pub use error::SceneError;
pub use node_id::NodeId;
pub use scene::Scene;
pub use transform::Transform2D;

/// Engine version string, useful for logs and error reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");