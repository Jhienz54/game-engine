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

/// On-the-wire representation used only during deserialization.
///
/// Persists the free list so that `pop()` returns the same index on a
/// freshly loaded scene as it would on the scene that was saved. The
/// despawn queue is not persisted (it's transient, drained each frame).
#[derive(Deserialize)]
struct SceneRaw {
    slots: Vec<Slot>,
    free: Vec<u32>,
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
///
/// ## Determinism across save/load
///
/// The free list is **persisted**. If you save a scene, load it, and
/// then spawn a node, the new `NodeId` will match what would have been
/// produced had you never saved. This keeps game-owned side tables
/// (which deserialize alongside the scene) stable across reload cycles.
///
/// ## Validation on load
///
/// [`Scene::try_from`] validates every structural invariant — dangling
/// parent/child links, mismatched bidirectional links, duplicate
/// children, out-of-range or inconsistent free indices, and hierarchy
/// cycles are all rejected. Hand-edited RON files fail loudly instead
/// of producing an unusable scene.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(try_from = "SceneRaw")]
pub struct Scene {
    slots: Vec<Slot>,
    /// Indices into `slots` currently free for reuse (LIFO).
    ///
    /// Persisted: preserves the exact reuse order across save/load.
    free: Vec<u32>,
    /// `NodeId`s removed since the last `take_despawned`. Transient.
    #[serde(skip)]
    despawned: Vec<NodeId>,
}

impl TryFrom<SceneRaw> for Scene {
    type Error = SceneError;

    fn try_from(raw: SceneRaw) -> Result<Self, Self::Error> {
        let slot_count = raw.slots.len();

        // 1. Free-list structural validation.
        let mut seen_free = vec![false; slot_count];
        for &idx in &raw.free {
            let i = idx as usize;
            if i >= slot_count {
                return Err(SceneError::FreeIndexOutOfRange { index: idx });
            }
            if seen_free[i] {
                return Err(SceneError::DuplicateFreeIndex { index: idx });
            }
            seen_free[i] = true;
            if raw.slots[i].value.is_some() {
                return Err(SceneError::FreeSlotOccupied { index: idx });
            }
        }
        for (i, slot) in raw.slots.iter().enumerate() {
            if slot.value.is_none() && !seen_free[i] {
                return Err(SceneError::LeakedEmptySlot { index: i as u32 });
            }
        }

        // 2. Build the scene (needed for cross-slot lookups below).
        let scene = Scene {
            slots: raw.slots,
            free: raw.free,
            despawned: Vec::new(),
        };

        // 3. Bidirectional parent/child consistency for every live node.
        for (i, slot) in scene.slots.iter().enumerate() {
            let Some(node) = &slot.value else { continue };
            let this_id = NodeId {
                index: i as u32,
                generation: slot.generation,
            };

            // Parent link must point at a live node that lists us back.
            if let Some(p) = node.parent {
                let parent_node = scene
                    .slots
                    .get(p.index as usize)
                    .filter(|s| s.generation == p.generation)
                    .and_then(|s| s.value.as_ref())
                    .ok_or(SceneError::DanglingParent {
                        child: this_id,
                        parent: p,
                    })?;
                if !parent_node.children.contains(&this_id) {
                    return Err(SceneError::ParentChildMismatch {
                        child: this_id,
                        parent: p,
                    });
                }
            }

            // Every listed child must be live and point back at us.
            let mut seen_children: Vec<NodeId> = Vec::with_capacity(node.children.len());
            for &c in &node.children {
                if seen_children.contains(&c) {
                    return Err(SceneError::DuplicateChild {
                        parent: this_id,
                        child: c,
                    });
                }
                seen_children.push(c);
                let child_node = scene
                    .slots
                    .get(c.index as usize)
                    .filter(|s| s.generation == c.generation)
                    .and_then(|s| s.value.as_ref())
                    .ok_or(SceneError::DanglingChild {
                        parent: this_id,
                        child: c,
                    })?;
                if child_node.parent != Some(this_id) {
                    return Err(SceneError::ChildParentMismatch {
                        parent: this_id,
                        child: c,
                    });
                }
            }
        }

        // 4. Cycle detection. The bidirectional check above catches
        //    self-references but not longer cycles (A↔B, A→B→C→A).
        //    A chain cannot exceed `slot_count` edges without revisiting,
        //    so a step counter suffices — no allocation per node.
        let max_steps = slot_count as u32;
        for start in 0..slot_count {
            if scene.slots[start].value.is_none() {
                continue;
            }
            let mut cur = Some(start as u32);
            let mut steps = 0u32;
            while let Some(i) = cur {
                let node = match &scene.slots[i as usize].value {
                    Some(n) => n,
                    None => break,
                };
                match node.parent {
                    None => break,
                    Some(p) => {
                        steps += 1;
                        if steps > max_steps {
                            return Err(SceneError::HierarchyCycle {
                                node: NodeId {
                                    index: start as u32,
                                    generation: scene.slots[start].generation,
                                },
                            });
                        }
                        cur = Some(p.index);
                    }
                }
            }
        }

        Ok(scene)
    }
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

        /// Set the node's local transform **and** its interpolation snapshot
    /// simultaneously, so the next draw has no glide.
    ///
    /// Use this for spawn, respawn, screen-warp, and any "this should
    /// look like it jumped" event. Use [`set_local_transform`] for the
    /// normal "I want to see a smooth transition" case — which is what
    /// most game logic should do.
    ///
    /// [`set_local_transform`]: Self::set_local_transform
    pub fn teleport(&mut self, id: NodeId, transform: Transform2D) -> Result<(), SceneError> {
        let node = self.node_mut(id)?;
        node.local = transform;
        node.prev_local = transform;
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
        self.node(parent)?;
        self.node(child)?;

        let mut cur = Some(parent);
        while let Some(id) = cur {
            if id == child {
                return Err(SceneError::WouldCreateCycle { parent, child });
            }
            cur = self.node(id)?.parent;
        }

        if let Some(old) = self.node(child)?.parent {
            self.node_mut(old)?.children.retain(|&c| c != child);
        }

        // Attach, and reset the child's interpolation snapshot so its
        // own local contribution doesn't glide across the reparent. The
        // world transform will still change because the parent chain
        // changed — that jump is inherent to reparenting and is what the
        // game asked for.
        {
            let child_node = self.node_mut(child)?;
            child_node.parent = Some(parent);
            child_node.prev_local = child_node.local;
        }
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

        let mut to_remove: Vec<NodeId> = Vec::new();
        self.collect_subtree_preorder(id, &mut to_remove);

        for nid in to_remove {
            if let Ok(node) = self.node(nid) {
                if let Some(parent) = node.parent {
                    if let Ok(pn) = self.node_mut(parent) {
                        pn.children.retain(|&c| c != nid);
                    }
                }
            }
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

    // ---- basic arena / hierarchy ----------------------------------------

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
        assert!(s.take_despawned().is_empty());
    }

    #[test]
    fn snapshot_freezes_prev_state() {
        let mut s = Scene::new();
        let n = s.spawn_with("n", Transform2D::from_translation(Vec2::ZERO));
        s.snapshot_transforms();
        s.set_local_transform(n, Transform2D::from_translation(Vec2::new(10.0, 0.0)))
            .unwrap();
        let mid = s
            .interpolated_world_transform(n, 0.5)
            .unwrap()
            .transform_point2(Vec2::ZERO);
        assert!((mid.x - 5.0).abs() < 1e-5);
    }

    // ---- round-trip with free list preserved ----------------------------

    #[test]
    fn ron_roundtrip_preserves_ids_and_hierarchy() {
        let mut s = Scene::new();
        let p = s.spawn_with("p", Transform2D::from_translation(Vec2::new(1.0, 2.0)));
        let c = s.spawn_with("c", Transform2D::from_translation(Vec2::new(3.0, 4.0)));
        s.add_child(p, c).unwrap();
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
        let w = s2.world_transform(c).unwrap();
        let origin = w.transform_point2(Vec2::ZERO);
        assert!((origin.x - 4.0).abs() < 1e-5);
        assert!((origin.y - 6.0).abs() < 1e-5);
    }

    /// The user-requested determinism test: after save + load, the *next
    /// spawn* must produce the same `NodeId` as it would have on the
    /// unsaved scene.
    #[test]
    fn save_load_then_spawn_matches_unsaved() {
        let mut s = Scene::new();
        let a = s.spawn("a");
        let _b = s.spawn("b");
        s.despawn(a).unwrap();
        let _ = s.take_despawned();

        let text = ron::to_string(&s).unwrap();
        let mut s2: Scene = ron::from_str(&text).unwrap();

        let from_unsaved = s.spawn("new");
        let from_loaded = s2.spawn("new");

        assert_eq!(from_unsaved, from_loaded);
        assert_eq!(from_unsaved.index(), a.index());
        assert_eq!(from_unsaved.generation(), a.generation() + 1);
    }

    /// Multi-slot reuse ordering must survive the round-trip, not just
    /// the first pop.
    #[test]
    fn save_load_preserves_multi_free_order() {
        let mut s = Scene::new();
        let a = s.spawn("a"); // index 0
        let _b = s.spawn("b"); // index 1
        let c = s.spawn("c"); // index 2
        let d = s.spawn("d"); // index 3
        s.despawn(a).unwrap(); // free = [0]
        s.despawn(c).unwrap(); // free = [0, 2]
        s.despawn(d).unwrap(); // free = [0, 2, 3]
        let _ = s.take_despawned();

        let text = ron::to_string(&s).unwrap();
        let mut s2: Scene = ron::from_str(&text).unwrap();

        // LIFO: next reuse is 3, then 2, then 0 — on both scenes.
        let a1 = s.spawn("n1");
        let a2 = s.spawn("n2");
        let a3 = s.spawn("n3");
        let b1 = s2.spawn("n1");
        let b2 = s2.spawn("n2");
        let b3 = s2.spawn("n3");

        assert_eq!(a1.index(), 3);
        assert_eq!(a2.index(), 2);
        assert_eq!(a3.index(), 0);
        assert_eq!(a1, b1);
        assert_eq!(a2, b2);
        assert_eq!(a3, b3);
    }

    // ---- deserialization validation ------------------------------------

    fn make_node(
        name: &str,
        parent: Option<NodeId>,
        children: Vec<NodeId>,
    ) -> Node {
        Node {
            name: name.to_string(),
            parent,
            children,
            local: Transform2D::IDENTITY,
            prev_local: Transform2D::IDENTITY,
        }
    }

    #[test]
    fn reject_free_index_out_of_range() {
        let raw = SceneRaw {
            slots: vec![],
            free: vec![7],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::FreeIndexOutOfRange { index: 7 })
        ));
    }

    #[test]
    fn reject_duplicate_free_index() {
        let raw = SceneRaw {
            slots: vec![
                Slot { generation: 0, value: None },
                Slot { generation: 0, value: None },
            ],
            free: vec![0, 0, 1],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::DuplicateFreeIndex { index: 0 })
        ));
    }

    #[test]
    fn reject_free_slot_occupied() {
        let raw = SceneRaw {
            slots: vec![Slot {
                generation: 0,
                value: Some(make_node("a", None, vec![])),
            }],
            free: vec![0],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::FreeSlotOccupied { index: 0 })
        ));
    }

    #[test]
    fn reject_leaked_empty_slot() {
        let raw = SceneRaw {
            slots: vec![Slot { generation: 0, value: None }],
            free: vec![],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::LeakedEmptySlot { index: 0 })
        ));
    }

    #[test]
    fn reject_dangling_parent() {
        let bogus = NodeId { index: 9, generation: 0 };
        let raw = SceneRaw {
            slots: vec![Slot {
                generation: 0,
                value: Some(make_node("a", Some(bogus), vec![])),
            }],
            free: vec![],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::DanglingParent { .. })
        ));
    }

    #[test]
    fn reject_dangling_child() {
        let bogus = NodeId { index: 9, generation: 0 };
        let raw = SceneRaw {
            slots: vec![Slot {
                generation: 0,
                value: Some(make_node("a", None, vec![bogus])),
            }],
            free: vec![],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::DanglingChild { .. })
        ));
    }

    #[test]
    fn reject_parent_child_mismatch() {
        // a.parent = b, but b does not list a as a child.
        let id_b = NodeId { index: 1, generation: 0 };
        let raw = SceneRaw {
            slots: vec![
                Slot {
                    generation: 0,
                    value: Some(make_node("a", Some(id_b), vec![])),
                },
                Slot {
                    generation: 0,
                    value: Some(make_node("b", None, vec![])),
                },
            ],
            free: vec![],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::ParentChildMismatch { .. })
        ));
    }

    #[test]
    fn reject_child_parent_mismatch() {
        // a.children = [b], but b.parent != a.
        let id_b = NodeId { index: 1, generation: 0 };
        let raw = SceneRaw {
            slots: vec![
                Slot {
                    generation: 0,
                    value: Some(make_node("a", None, vec![id_b])),
                },
                Slot {
                    generation: 0,
                    // b claims no parent, but a lists it as a child.
                    value: Some(make_node("b", None, vec![])),
                },
            ],
            free: vec![],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::ChildParentMismatch { .. })
        ));
    }

    #[test]
    fn reject_duplicate_child() {
        let id_a = NodeId { index: 0, generation: 0 };
        let id_b = NodeId { index: 1, generation: 0 };
        let raw = SceneRaw {
            slots: vec![
                Slot {
                    generation: 0,
                    value: Some(make_node("a", None, vec![id_b, id_b])),
                },
                Slot {
                    generation: 0,
                    value: Some(make_node("b", Some(id_a), vec![])),
                },
            ],
            free: vec![],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::DuplicateChild { .. })
        ));
    }

    #[test]
    fn reject_hierarchy_cycle() {
        // A↔B bidirectional cycle: passes the "consistent back-links"
        // check but must fail cycle detection.
        let id_a = NodeId { index: 0, generation: 0 };
        let id_b = NodeId { index: 1, generation: 0 };
        let raw = SceneRaw {
            slots: vec![
                Slot {
                    generation: 0,
                    value: Some(make_node("a", Some(id_b), vec![id_b])),
                },
                Slot {
                    generation: 0,
                    value: Some(make_node("b", Some(id_a), vec![id_a])),
                },
            ],
            free: vec![],
        };
        assert!(matches!(
            Scene::try_from(raw),
            Err(SceneError::HierarchyCycle { .. })
        ));
    }
}