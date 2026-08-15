//! ObjectRegistry — central in-memory object store.
//!
//! DashMap for concurrent read, Arc for shared ownership.
//! Replaces GameFactory from original C++.

use ashfall_core::id::NetworkID;
use ashfall_core::types::{GameObject, ObjectKind};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Central object registry. Thread-safe, lock-free reads for most operations.
pub struct ObjectRegistry {
    objects: DashMap<NetworkID, Arc<RwLock<dyn GameObject>>>,
    type_counts: DashMap<ObjectKind, u32>,
    deleted: DashMap<NetworkID, ()>,
    ref_to_id: DashMap<u32, NetworkID>,
    cell_refs: DashMap<u32, Vec<NetworkID>>,
    /// Simulation ownership: actor id → owning player id (STR OwnershipTransfer).
    /// The owner may mutate the actor via client packets; everyone else renders.
    owners: DashMap<NetworkID, NetworkID>,
    next_id: AtomicU64,
}

impl ObjectRegistry {
    pub fn new() -> Self {
        ObjectRegistry {
            objects: DashMap::new(),
            type_counts: DashMap::new(),
            deleted: DashMap::new(),
            ref_to_id: DashMap::new(),
            cell_refs: DashMap::new(),
            owners: DashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Allocate a fresh NetworkID.
    pub fn allocate_id(&self) -> NetworkID {
        NetworkID::new(self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    /// Insert an object into the registry.
    pub fn insert<T: GameObject + 'static>(&self, obj: T) -> NetworkID {
        let id = obj.id();
        let kind = obj.kind();
        self.objects.insert(id, Arc::new(RwLock::new(obj)));
        self.type_counts
            .entry(kind)
            .and_modify(|c| *c += 1)
            .or_insert(1);
        id
    }

    /// Get a read-locked reference to an object.
    /// Returns None if not found or deleted.
    pub fn get(&self, id: NetworkID) -> Option<Arc<RwLock<dyn GameObject>>> {
        if self.deleted.contains_key(&id) {
            return None;
        }
        self.objects.get(&id).map(|entry| entry.value().clone())
    }

    /// Get and downcast to a specific type.
    pub fn get_typed<T>(&self, id: NetworkID) -> Option<T>
    where
        T: 'static + Clone + Send + Sync,
    {
        let arc = self.get(id)?;
        let guard = arc.read();
        let obj: &dyn GameObject = &*guard;
        obj.as_any().downcast_ref::<T>().cloned()
    }

    /// Remove an object from the registry.
    pub fn remove(&self, id: NetworkID) -> bool {
        if let Some((_, arc)) = self.objects.remove(&id) {
            let guard = arc.read();
            let kind = guard.kind();
            self.type_counts
                .entry(kind)
                .and_modify(|c| *c = c.saturating_sub(1));
            self.deleted.insert(id, ());
            self.owners.remove(&id);

            // Remove from cell_refs
            // ponytail: scan all cell_refs to remove this id.
            // O(cells) but cells list is small. Optimize if needed.
            self.cell_refs.retain(|_, ids| {
                ids.retain(|&nid| nid != id);
                !ids.is_empty()
            });

            true
        } else {
            false
        }
    }

    pub fn is_deleted(&self, id: NetworkID) -> bool {
        self.deleted.contains_key(&id)
    }

    // ── simulation ownership (STR OwnershipTransfer pattern) ──

    /// Owning player id of an actor, if any.
    pub fn owner_of(&self, id: NetworkID) -> Option<NetworkID> {
        self.owners.get(&id).map(|r| *r.value())
    }

    /// Assign simulation ownership. Returns true if it was unowned.
    pub fn set_owner(&self, id: NetworkID, player_id: NetworkID) -> bool {
        if self.owners.contains_key(&id) {
            return false;
        }
        self.owners.insert(id, player_id);
        true
    }

    /// Release ownership, returning the released actor ids.
    pub fn take_owner(&self, id: NetworkID) -> bool {
        self.owners.remove(&id).is_some()
    }

    /// Release every actor owned by `player_id` (on disconnect).
    /// Returns the released actor ids so callers can broadcast the release.
    pub fn release_player_owned(&self, player_id: NetworkID) -> Vec<NetworkID> {
        let released: Vec<NetworkID> = self
            .owners
            .iter()
            .filter(|e| *e.value() == player_id)
            .map(|e| *e.key())
            .collect();
        for id in &released {
            self.owners.remove(id);
        }
        released
    }

    /// Register a reference ID → NetworkID mapping.
    pub fn map_ref(&self, ref_id: u32, id: NetworkID) {
        self.ref_to_id.insert(ref_id, id);
    }

    pub fn lookup_ref(&self, ref_id: u32) -> Option<NetworkID> {
        self.ref_to_id.get(&ref_id).map(|r| *r.value())
    }

    // ── cell management ──

    /// Register an object to a cell for visibility queries.
    pub fn add_to_cell(&self, cell: u32, id: NetworkID) {
        self.cell_refs
            .entry(cell)
            .and_modify(|ids| {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            })
            .or_insert_with(|| vec![id]);
    }

    /// Get all object IDs in a cell.
    pub fn get_by_cell(&self, cell: u32) -> Vec<NetworkID> {
        self.cell_refs
            .get(&cell)
            .map(|r| r.value().clone())
            .unwrap_or_default()
    }

    /// Get objects filtered by kind mask in a cell.
    pub fn get_by_cell_kind(&self, cell: u32, mask: u32) -> Vec<NetworkID> {
        self.get_by_cell(cell)
            .into_iter()
            .filter(|id| {
                self.objects
                    .get(id)
                    .map(|entry| {
                        let guard = entry.value().read();
                        guard.kind_mask() & mask != 0
                    })
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get all objects of a kind (across all cells).
    pub fn get_by_kind(&self, mask: u32) -> Vec<NetworkID> {
        self.objects
            .iter()
            .filter(|entry| {
                let guard = entry.value().read();
                guard.kind_mask() & mask != 0
            })
            .map(|entry| *entry.key())
            .collect()
    }

    /// Count objects of a type.
    pub fn type_count(&self, kind: ObjectKind) -> u32 {
        self.type_counts.get(&kind).map(|c| *c.value()).unwrap_or(0)
    }

    /// Total object count (excluding deleted).
    pub fn total_count(&self) -> usize {
        self.objects.len()
    }

    /// Iterate all objects in a set of cells (for cell context query).
    pub fn get_by_cells(&self, cells: &[u32]) -> Vec<NetworkID> {
        let mut result = Vec::new();
        for cell in cells {
            result.extend(self.get_by_cell(*cell));
        }
        result.sort_by_key(|id| id.as_u64());
        result.dedup_by_key(|id| id.as_u64());
        result
    }
}

impl Default for ObjectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::objects::Actor;

    fn actor(id: u64) -> Actor {
        Actor::new(NetworkID::new(id), 0x100 + id as u32, 0x7, 0x1)
    }

    #[test]
    fn insert_get_remove_deleted() {
        let r = ObjectRegistry::new();
        let id = r.insert(actor(1));
        assert!(r.get(id).is_some());
        assert!(r.remove(id));
        assert!(r.get(id).is_none()); // deleted → hidden
        assert!(r.is_deleted(id));
        assert!(!r.remove(id)); // already gone
    }

    #[test]
    fn allocate_id_is_unique() {
        let r = ObjectRegistry::new();
        let a = r.allocate_id();
        let b = r.allocate_id();
        assert_ne!(a, b);
    }

    #[test]
    fn owner_claim_release() {
        let r = ObjectRegistry::new();
        let id = r.insert(actor(1));
        let p = NetworkID::new(9);
        assert!(r.set_owner(id, p));
        assert!(!r.set_owner(id, p)); // already owned
        assert_eq!(r.owner_of(id), Some(p));
        assert!(r.take_owner(id));
        assert_eq!(r.owner_of(id), None);
    }

    #[test]
    fn release_player_owned_clears_many() {
        let r = ObjectRegistry::new();
        let p = NetworkID::new(9);
        let a = r.insert(actor(1));
        let b = r.insert(actor(2));
        let other = r.insert(actor(3));
        r.set_owner(a, p);
        r.set_owner(b, p);
        r.set_owner(other, NetworkID::new(10));
        let released = r.release_player_owned(p);
        assert_eq!(released.len(), 2);
        assert_eq!(r.owner_of(a), None);
        assert_eq!(r.owner_of(other), Some(NetworkID::new(10)));
    }

    #[test]
    fn cell_registration_and_lookup() {
        let r = ObjectRegistry::new();
        let a = r.insert(actor(1));
        let b = r.insert(actor(2));
        r.add_to_cell(0x5, a);
        r.add_to_cell(0x5, b);
        r.add_to_cell(0x6, a);
        assert_eq!(r.get_by_cell(0x5).len(), 2);
        assert_eq!(r.get_by_cell(0x6), vec![a]);
        // remove cleans cell refs
        r.remove(a);
        assert_eq!(r.get_by_cell(0x5), vec![b]);
        assert_eq!(r.get_by_cell(0x6), vec![]);
    }

    #[test]
    fn type_counts_and_ref_map() {
        let r = ObjectRegistry::new();
        let a = r.insert(actor(1));
        assert_eq!(r.type_count(ObjectKind::Actor), 1);
        r.map_ref(0x200, a);
        assert_eq!(r.lookup_ref(0x200), Some(a));
        r.remove(a);
        assert_eq!(r.type_count(ObjectKind::Actor), 0);
    }
}
