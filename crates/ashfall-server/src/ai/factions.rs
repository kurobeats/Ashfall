//! Faction system — hostility matrix.
//!
//! ponytail: simple lookup table. Loaded from DB in Phase 4.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Hostility levels between factions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hostility {
    Ally,
    Neutral,
    Enemy,
}

/// Faction hostility matrix.
#[derive(Clone)]
/// Faction hostility matrix — shared interior mutability (Arc<Mutex>) so
/// WASM script instances all observe each other's `set_faction_relation`
/// calls (same pattern as QuestManager: instances hold clones of the state,
/// the matrix is the shared object).
#[derive(Default)]
pub struct FactionMatrix {
    /// (faction_a, faction_b) → hostility
    relations: Arc<Mutex<HashMap<(u32, u32), Hostility>>>,
}

impl FactionMatrix {
    pub fn new() -> Self {
        FactionMatrix::default()
    }

    pub fn set_relation(&self, faction_a: u32, faction_b: u32, hostility: Hostility) {
        let mut rel = self.relations.lock().unwrap();
        rel.insert((faction_a, faction_b), hostility);
        rel.insert((faction_b, faction_a), hostility); // symmetric
    }

    pub fn get_hostility(&self, faction_a: u32, faction_b: u32) -> Hostility {
        if faction_a == faction_b {
            return Hostility::Ally;
        }
        self.relations
            .lock()
            .unwrap()
            .get(&(faction_a, faction_b))
            .copied()
            .unwrap_or(Hostility::Neutral)
    }

    pub fn are_hostile(&self, faction_a: u32, faction_b: u32) -> bool {
        self.get_hostility(faction_a, faction_b) == Hostility::Enemy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_faction_is_ally() {
        let m = FactionMatrix::new();
        assert_eq!(m.get_hostility(0x1, 0x1), Hostility::Ally);
        assert!(!m.are_hostile(0x1, 0x1));
    }

    #[test]
    fn default_is_neutral() {
        let m = FactionMatrix::new();
        assert_eq!(m.get_hostility(0x1, 0x2), Hostility::Neutral);
    }

    #[test]
    fn relation_is_symmetric() {
        let m = FactionMatrix::new();
        m.set_relation(0x1, 0x2, Hostility::Enemy);
        assert!(m.are_hostile(0x1, 0x2));
        assert!(m.are_hostile(0x2, 0x1));
    }

    #[test]
    fn relation_overwrite() {
        let m = FactionMatrix::new();
        m.set_relation(0x1, 0x2, Hostility::Enemy);
        m.set_relation(0x1, 0x2, Hostility::Ally);
        assert!(!m.are_hostile(0x1, 0x2));
    }

    #[test]
    fn clone_shares_matrix() {
        let m = FactionMatrix::new();
        let m2 = m.clone();
        m.set_relation(0x1, 0x3, Hostility::Enemy);
        assert!(m2.are_hostile(0x1, 0x3));
    }
}
