//! Inventory management — add/remove/equip items from containers.

use ashfall_core::id::NetworkID;
use std::collections::HashMap;

/// Inventory operations on container item lists.
pub struct Inventory;

impl Inventory {
    /// Add an item to a container's item list.
    pub fn add(container_items: &mut Vec<NetworkID>, item_id: NetworkID) {
        if !container_items.contains(&item_id) {
            container_items.push(item_id);
        }
    }

    /// Remove an item from a container's item list.
    pub fn remove(container_items: &mut Vec<NetworkID>, item_id: NetworkID) -> bool {
        if let Some(pos) = container_items.iter().position(|i| *i == item_id) {
            container_items.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if an item is in a container.
    pub fn contains(container_items: &[NetworkID], item_id: NetworkID) -> bool {
        container_items.contains(&item_id)
    }

    /// Get count of items with a given base_id in a container.
    /// Requires looking up each item in the registry.
    pub fn count_by_base(
        container_items: &[NetworkID],
        base_id: u32,
        item_base_map: &HashMap<NetworkID, u32>,
    ) -> u32 {
        container_items
            .iter()
            .filter(|id| item_base_map.get(id).copied() == Some(base_id))
            .count() as u32
    }

    /// Find first item with a given base_id in a container.
    pub fn find_by_base(
        container_items: &[NetworkID],
        base_id: u32,
        item_base_map: &HashMap<NetworkID, u32>,
    ) -> Option<NetworkID> {
        container_items
            .iter()
            .find(|id| item_base_map.get(id).copied() == Some(base_id))
            .copied()
    }
}
