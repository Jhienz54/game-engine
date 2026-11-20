use crate::{NodeId, SceneError, Transform2D};
use glam::Mat3;
use serde::{Deserialize, Serialize};

/// One node's data in the scene. All fields are `serde`-able; the
/// entire `Scene` can be written to RON and read back with identical
/// `NodeId`s.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    name: String,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// Current simulation state.
    local: Transform2D,
    /// Previous fixed-step snapshot, used for render interpolation.
    /// Set equal to `local` on spawn so newly spawned nodes don't
    /// interpolate from an arbitrary origin.
    prev_local: Transform2D,
}

/// One arena slot. `value: None` means the slot is free; `generation`
/// is the generation that was last handed out for this index.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Slot {
    generation: u32,
    value: Option<Node>,
}

/// On-the-wire representation: only the slot list. The free list and
/// despawn queue are derived / transient.
#[derive(Deserialize)]
struct SceneRaw {
    slots: Vec<Slot>,
}

impl From<SceneRaw> for Scene {
    fn from(raw: SceneRaw) -> Self {
        let mut free = Vec::new();
        for (i, slot) in raw.slots.iter().enumerate() {
            if slot.value.is_none() {
                free.push(i as u32);
            }
        }
        Scene {
            slots: raw.slots,
            free,
            despawned: Vec::new(),
        }
    }
}

/// The scene: an arena of nodes forming a parent/child hierarchy.
///
/// ## Interpolation contract
///
/// Each fixed step, **before** calling your `fixed_update`:
///
/// ```ignore
/// scene.snapshot_transforms();  // prev_local := local, per node
/// game.fixed_update(&mut scene, ...);
/// ```
///
/// Then at draw time, ask the renderer to draw using
/// [`interpolated_world_transform`](Self::interpolated_world_transform)
/// with `alpha = accumulator / fixed_dt`.
///
/// If you never call `snapshot_transforms`, `prev_local` stays fixed at
/// spawn time and interpolation will snap — safe, but not smooth.
///
/// ## Despawn contract
///
/// [`despawn`](Self::despawn) removes the node and its entire subtree
/// in pre-order and pushes every removed `NodeId` into the despawn
/// queue. Drain it with [`take_despawned`](Self::take_despawned) and
/// prune any game-owned side tables in the same frame. If you don't
/// drain it, the queue grows unboundedly.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(from = "SceneRaw")]
pub struct Scene {
    slots: Vec<Slot>,
    /// Indices into `slots` currently free for reuse (LIFO).
    #[serde(skip)]
    free: Vec<u32>,
    /// `NodeId`s removed since the last `take_despawned`.
    #[serde(skip)]
    despawned: Vec<NodeId>,
}

impl Scene {
    /// Create an empty scene.
    pub fn new() -> Self {
        Self::default()
    }

    // -------- spawning -------------------------------------------------

    /// Spawn a root node with the identity transform.
    pub fn spawn(&mut self, name: impl Into<String>) -> NodeId {
        self.spawn_with(name, Transform2D::IDENTITY)
    }

    /// Spawn a root node with a given local transform.
    pub fn spawn_with(&mut self, name: impl Into<String>, transform: Transform2D) -> NodeId {
        let node = Node {
            name: name.into(),
            parent: None,
            children: Vec::new(),
            local: transform,
            prev_local: transform,
        };
        self.insert_slot(node)
    }

    fn insert_slot(&mut self, node: Node) -> NodeId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(slot.value.is_none(), "free list out of sync");
            slot.value = Some(node);
            NodeId {
                index,
                generation: slot.generation,
            }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 0,
                value: Some(node),
            });
            NodeId {
                index,
                generation: 0,
            }
        }
    }

    // -------- liveness -------------------------------------------------

    /// Whether `id` refers to a live node.
    pub fn contains(&self, id: NodeId) -> bool {
        self.slots
            .get(id.index as usize)
            .is_some_and(|s| s.generation == id.generation && s.value.is_some())
    }

    /// Number of live nodes.
    pub fn node_count(&self) -> usize {
        self.slots.iter().filter(|s| s.value.is_some()).count()
    }

    fn node(&self, id: NodeId) -> Result<&Node, SceneError> {
        self.slots
            .get(id.index as usize)
            .filter(|s| s.generation == id.generation)
            .and_then(|s| s.value.as_ref())
            .ok_or(SceneError::InvalidNodeId(id))
    }

    fn node_mut(&mut self, id: NodeId) -> Result<&mut Node, SceneError> {
        self.slots
            .get_mut(id.index as usize)
            .filter(|s| s.generation == id.generation)
            .and_then(|s| s.value.as_mut())
            .ok_or(SceneError::InvalidNodeId(id))
    }

    // -------- data accessors -------------------------------------------

    /// Node name.
    pub fn name(&self, id: NodeId) -> Result<&str, SceneError> {
        Ok(&self.node(id)?.name)
    }

    /// Rename a node.
    pub fn set_name(&mut self, id: NodeId, name: impl Into<String>) -> Result<(), SceneError> {
        self.node_mut(id)?.name = name.into();
        Ok(())
    }

    /// The node's local transform (relative to its parent).
    pub fn local_transform(&self, id: NodeId) -> Result<Transform2D, SceneError> {
        Ok(self.node(id)?.local)
    }

    /// Mutable access to the local transform. Mutations affect the next
    /// fixed step's world transform.
    pub fn local_transform_mut(&mut self, id: NodeId) -> Result<&mut Transform2D, SceneError> {
        Ok(&mut self.node_mut(id)?.local)
    }

    /// Overwrite the local transform.
    pub fn set_local_transform(
        &mut self,
        id: NodeId,
        transform: Transform2D,
    ) -> Result<(), SceneError> {
        self.node_mut(id)?.local = transform;
        Ok(())
    }

    // -------- hierarchy -------------------------------------------------

    /// Parent of `id`, if any.
    pub fn parent(&self, id: NodeId) -> Result<Option<NodeId>, SceneError> {
        Ok(self.node(id)?.parent)
    }

    /// Children of `id` in insertion order.
    pub fn children(&self, id: NodeId) -> Result<&[NodeId], SceneError> {
        Ok(&self.node(id)?.children)
    }

    /// Parent `child` under `parent`. If `child` already has a parent,
    /// it is detached from that parent first.
    ///
    /// Returns [`SceneError::SelfParent`] if `parent == child`, and
    /// [`SceneError::WouldCreateCycle`] if `parent` is `child`'s
    /// descendant.
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), SceneError> {
        if parent == child {
            return Err(SceneError::SelfParent);
        }
        // Validate both.
        self.node(parent)?;
        self.node(child)?;

        // Cycle check: walk up from `parent`; if we hit `child`, the
        // proposed parent is a descendant of the node we'd re-parent.
        let mut cur = Some(parent);
        while let Some(id) = cur {
            if id == child {
                return Err(SceneError::WouldCreateCycle { parent, child });
            }
            cur = self.node(id)?.parent;
        }

        // Detach from old parent.
        if let Some(old) = self.node(child)?.parent {
            self.node_mut(old)?.children.retain(|&c| c != child);
        }

        // Attach.
        self.node_mut(child)?.parent = Some(parent);
        self.node_mut(parent)?.children.push(child);
        Ok(())
    }

    /// Detach `child` from its current parent; `child` becomes a root.
    /// No-op if `child` has no parent.
    pub fn remove_from_parent(&mut self, child: NodeId) -> Result<(), SceneError> {
        if let Some(parent) = self.node(child)?.parent {
            self.node_mut(parent)?.children.retain(|&c| c != child);
            self.node_mut(child)?.parent = None;
        }
        Ok(())
    }

    // -------- world transforms -----------------------------------------

    /// Current world transform of `id` — the composition of `local`
    /// transforms from the root down.
    pub fn world_transform(&self, id: NodeId) -> Result<Mat3, SceneError> {
        let mut chain: Vec<Transform2D> = Vec::new();
        let mut cur = Some(id);
        while let Some(n) = cur {
            let node = self.node(n)?;
            chain.push(node.local);
            cur = node.parent;
        }
        let mut m = Mat3::IDENTITY;
        for t in chain.iter().rev() {
            m *= (*t).to_matrix();
        }
        Ok(m)
    }

    /// World transform interpolated between `prev_local` and `local`
    /// by `alpha`. Pass `alpha = accumulator / fixed_dt` at draw time.
    pub fn interpolated_world_transform(
        &self,
        id: NodeId,
        alpha: f32,
    ) -> Result<Mat3, SceneError> {
        let mut chain: Vec<Transform2D> = Vec::new();
        let mut cur = Some(id);
        while let Some(n) = cur {
            let node = self.node(n)?;
            chain.push(Transform2D::lerp(node.prev_local, node.local, alpha));
            cur = node.parent;
        }
        let mut m = Mat3::IDENTITY;
        for t in chain.iter().rev() {
            m *= (*t).to_matrix();
        }
        Ok(m)
    }

    // -------- interpolation snapshot -----------------------------------

    /// Snapshot every node's `local` into `prev_local`. Call **once per
    /// fixed step, before `fixed_update`**, so the renderer has the
    /// previous state to interpolate from.
    pub fn snapshot_transforms(&mut self) {
        for slot in &mut self.slots {
            if let Some(node) = slot.value.as_mut() {
                node.prev_local = node.local;
            }
        }
    }

    // -------- despawn --------------------------------------------------

    /// Despawn `id` **and all its descendants**, in pre-order (parent
    /// first, then children in their insertion order). Every removed
    /// `NodeId` is pushed onto the despawn queue.
    pub fn despawn(&mut self, id: NodeId) -> Result<(), SceneError> {
        self.node(id)?;

        // Collect the subtree while every slot is still valid.
        let mut to_remove: Vec<NodeId> = Vec::new();
        self.collect_subtree_preorder(id, &mut to_remove);

        for nid in to_remove {
            // Detach from parent (no-op if the parent was already freed).
            if let Ok(node) = self.node(nid) {
                if let Some(parent) = node.parent {
                    if let Ok(pn) = self.node_mut(parent) {
                        pn.children.retain(|&c| c != nid);
                    }
                }
            }
            // Free the slot: bump generation, push to free list.
            if let Some(slot) = self.slots.get_mut(nid.index as usize) {
                slot.value = None;
                slot.generation = slot.generation.wrapping_add(1);
                self.free.push(nid.index);
            }
            self.despawned.push(nid);
        }
        Ok(())
    }

    fn collect_subtree_preorder(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        if let Ok(node) = self.node(id) {
            for &c in &node.children {
                self.collect_subtree_preorder(c, out);
            }
        }
    }

    /// Drain the despawn queue. Call once per frame (or per fixed step)
    /// and use the returned IDs to prune any game-owned side tables.
    pub fn take_despawned(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.despawned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn spawn_assigns_distinct_ids() {
        let mut s = Scene::new();
        let a = s.spawn("a");
        let b = s.spawn("b");
        assert_ne!(a, b);
        assert_eq!(s.node_count(), 2);
    }

    #[test]
    fn reused_index_bumps_generation() {
        let mut s = Scene::new();
        let a = s.spawn("a");
        assert_eq!(a.generation(), 0);
        s.despawn(a).unwrap();
        let b = s.spawn("b");
        assert_eq!(b.index(), a.index());
        assert_eq!(b.generation(), a.generation() + 1);
        // Old handle is stale.
        assert!(!s.contains(a));
        assert!(s.contains(b));
    }

    #[test]
    fn stale_id_rejected() {
        let mut s = Scene::new();
        let a = s.spawn("a");
        s.despawn(a).unwrap();
        assert!(matches!(
            s.local_transform(a),
            Err(SceneError::InvalidNodeId(_))
        ));
    }

    #[test]
    fn parent_child_basics() {
        let mut s = Scene::new();
        let p = s.spawn("p");
        let c = s.spawn("c");
        s.add_child(p, c).unwrap();
        assert_eq!(s.parent(c).unwrap(), Some(p));
        assert_eq!(s.children(p).unwrap(), &[c]);
    }

    #[test]
    fn reparenting_moves_child() {
        let mut s = Scene::new();
        let p1 = s.spawn("p1");
        let p2 = s.spawn("p2");
        let c = s.spawn("c");
        s.add_child(p1, c).unwrap();
        s.add_child(p2, c).unwrap();
        assert_eq!(s.parent(c).unwrap(), Some(p2));
        assert!(s.children(p1).unwrap().is_empty());
        assert_eq!(s.children(p2).unwrap(), &[c]);
    }

    #[test]
    fn cycle_rejected() {
        let mut s = Scene::new();
        let a = s.spawn("a");
        let b = s.spawn("b");
        let c = s.spawn("c");
        s.add_child(a, b).unwrap();
        s.add_child(b, c).unwrap();
        // c is a descendant of a; parenting a under c would cycle.
        assert!(matches!(
            s.add_child(c, a),
            Err(SceneError::WouldCreateCycle { .. })
        ));
    }

    #[test]
    fn self_parent_rejected() {
        let mut s = Scene::new();
        let a = s.spawn("a");
        assert!(matches!(s.add_child(a, a), Err(SceneError::SelfParent)));
    }

    #[test]
    fn world_transform_composes() {
        let mut s = Scene::new();
        let p = s.spawn_with("p", Transform2D::from_translation(Vec2::new(10.0, 0.0)));
        let c = s.spawn_with("c", Transform2D::from_translation(Vec2::new(5.0, 0.0)));
        s.add_child(p, c).unwrap();
        let w = s.world_transform(c).unwrap();
        let origin = w.transform_point2(Vec2::ZERO);
        assert!((origin.x - 15.0).abs() < 1e-5);
        assert!(origin.y.abs() < 1e-5);
    }

    #[test]
    fn despawn_removes_subtree_in_preorder() {
        let mut s = Scene::new();
        let root = s.spawn("root");
        let c1 = s.spawn("c1");
        let c2 = s.spawn("c2");
        let gc = s.spawn("gc");
        s.add_child(root, c1).unwrap();
        s.add_child(root, c2).unwrap();
        s.add_child(c1, gc).unwrap();

        s.despawn(root).unwrap();
        assert_eq!(s.node_count(), 0);
        let drained = s.take_despawned();
        assert_eq!(drained, vec![root, c1, gc, c2]);
        // Second drain is empty.
        assert!(s.take_despawned().is_empty());
    }

    #[test]
    fn snapshot_freezes_prev_state() {
        let mut s = Scene::new();
        let n = s.spawn_with("n", Transform2D::from_translation(Vec2::ZERO));

        s.snapshot_transforms();
        s.set_local_transform(n, Transform2D::from_translation(Vec2::new(10.0, 0.0)))
            .unwrap();

        // Midway interpolation should be at (5, 0).
        let mid = s
            .interpolated_world_transform(n, 0.5)
            .unwrap()
            .transform_point2(Vec2::ZERO);
        assert!((mid.x - 5.0).abs() < 1e-5);
    }

    #[test]
    fn ron_roundtrip_preserves_ids_and_hierarchy() {
        let mut s = Scene::new();
        let p = s.spawn_with("p", Transform2D::from_translation(Vec2::new(1.0, 2.0)));
        let c = s.spawn_with("c", Transform2D::from_translation(Vec2::new(3.0, 4.0)));
        s.add_child(p, c).unwrap();
        // Free one slot so the free list is non-trivial on re-load.
        let tmp = s.spawn("tmp");
        s.despawn(tmp).unwrap();
        let _ = s.take_despawned();

        let text = ron::to_string(&s).unwrap();
        let s2: Scene = ron::from_str(&text).unwrap();

        assert_eq!(s2.node_count(), 2);
        assert_eq!(s2.parent(c).unwrap(), Some(p));
        assert_eq!(s2.children(p).unwrap(), &[c]);
        assert_eq!(s2.name(p).unwrap(), "p");
        assert_eq!(s2.name(c).unwrap(), "c");
        // Same NodeIds work on the deserialized scene.
        let w = s2.world_transform(c).unwrap();
        let origin = w.transform_point2(Vec2::ZERO);
        assert!((origin.x - 4.0).abs() < 1e-5);
        assert!((origin.y - 6.0).abs() < 1e-5);
    }
}