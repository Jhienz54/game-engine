use crate::NodeId;

/// Errors returned by [`Scene`](crate::Scene) operations.
///
/// Deserialization errors (`FreeIndexOutOfRange`, `DanglingParent`, …)
/// are returned by [`Scene::try_from`] when loading a `.ron` file and
/// indicate the on-disk scene was corrupt or hand-edited inconsistently.
/// Runtime errors (`InvalidNodeId`, `WouldCreateCycle`, `SelfParent`)
/// indicate the game passed a stale or illegal `NodeId` to an API.
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    /// The `NodeId` is stale (its generation does not match the current
    /// slot) or refers to a never-allocated index.
    #[error("node id {0:?} is stale or was never valid")]
    InvalidNodeId(NodeId),

    /// Parenting `child` under `parent` would create a cycle.
    #[error("cannot parent {child:?} to {parent:?}: would create a cycle")]
    WouldCreateCycle {
        /// The proposed new parent.
        parent: NodeId,
        /// The node being re-parented.
        child: NodeId,
    },

    /// Attempting to parent a node to itself.
    #[error("cannot parent a node to itself")]
    SelfParent,

    // ---- deserialization-only errors --------------------------------

    /// The serialized free list references an index beyond the slot
    /// array.
    #[error("free list index {index} is out of range")]
    FreeIndexOutOfRange {
        /// The offending index.
        index: u32,
    },

    /// The serialized free list contains the same index more than once.
    #[error("free list contains duplicate index {index}")]
    DuplicateFreeIndex {
        /// The duplicated index.
        index: u32,
    },

    /// The serialized free list points at a slot that also holds a live
    /// node.
    #[error("free list index {index} points at a live node")]
    FreeSlotOccupied {
        /// The offending index.
        index: u32,
    },

    /// A slot is empty but was not listed in the free list — it would
    /// be unreachable forever.
    #[error("slot {index} is empty but missing from the free list")]
    LeakedEmptySlot {
        /// The offending slot index.
        index: u32,
    },

    /// A live node's `parent` field refers to a slot that is missing,
    /// stale, or empty.
    #[error("node {child:?} claims parent {parent:?}, which is not a live node")]
    DanglingParent {
        /// The node whose parent link is broken.
        child: NodeId,
        /// The invalid parent reference.
        parent: NodeId,
    },

    /// A live node lists a `child` that is missing, stale, or empty.
    #[error("node {parent:?} lists child {child:?}, which is not a live node")]
    DanglingChild {
        /// The node whose children list is broken.
        parent: NodeId,
        /// The invalid child reference.
        child: NodeId,
    },

    /// A node's `parent` does not list it as a child.
    #[error("node {child:?} says parent is {parent:?}, but that parent does not list it as a child")]
    ParentChildMismatch {
        /// The node whose parent link is broken.
        child: NodeId,
        /// The parent that is missing the back-link.
        parent: NodeId,
    },

    /// A node lists a `child` whose `parent` points somewhere else.
    #[error("node {parent:?} lists child {child:?}, but that child's parent is not {parent:?}")]
    ChildParentMismatch {
        /// The node whose children list is broken.
        parent: NodeId,
        /// The child whose back-link is broken.
        child: NodeId,
    },

    /// A node's `children` vector lists the same child twice.
    #[error("node {parent:?} lists child {child:?} more than once")]
    DuplicateChild {
        /// The parent with the duplicated entry.
        parent: NodeId,
        /// The duplicated child.
        child: NodeId,
    },

    /// The hierarchy contains a cycle, which the scene must never
    /// contain.
    #[error("hierarchy cycle detected involving node {node:?}")]
    HierarchyCycle {
        /// Any node participating in the cycle.
        node: NodeId,
    },
}