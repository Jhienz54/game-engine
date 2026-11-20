use serde::{Deserialize, Serialize};

/// A stable, generation-checked handle to a node in a [`Scene`](crate::Scene).
///
/// `NodeId`s are cheap to copy and cheap to compare. They are **not**
/// valid forever: when the node they refer to is despawned, a later
/// `spawn` may reuse the same `index` with an incremented `generation`,
/// and the old `NodeId` becomes stale.
///
/// Two IDs compare equal iff both `index` and `generation` match.
/// `Ord` is derived from `(index, generation)` — a deterministic total
/// order you can rely on for `BTreeMap` side tables.
///
/// Serialization round-trips preserve identity exactly, which is why
/// game-owned side tables keyed by `NodeId` can be saved and loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl NodeId {
    /// Raw arena index. Use this to index `Vec<Option<T>>`-style side
    /// tables, checking `generation()` on access.
    #[inline]
    pub fn index(self) -> u32 {
        self.index
    }

    /// Generation counter. A `NodeId` is only valid if the scene's slot
    /// at `index()` carries this generation *and* is occupied.
    #[inline]
    pub fn generation(self) -> u32 {
        self.generation
    }
}