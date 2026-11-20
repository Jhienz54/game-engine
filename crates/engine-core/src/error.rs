use crate::NodeId;

/// Errors returned by [`Scene`](crate::Scene) operations.
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    /// The `NodeId` is stale (its generation does not match the current
    /// slot) or refers to a never-allocated index.
    #[error("node id {0:?} is stale or was never valid")]
    InvalidNodeId(NodeId),

    /// Parenting `child` under `parent` would create a cycle.
    #[error("cannot parent {child:?} to {parent:?}: would create a cycle")]
    WouldCreateCycle { parent: NodeId, child: NodeId },

    /// Attempting to parent a node to itself.
    #[error("cannot parent a node to itself")]
    SelfParent,
}