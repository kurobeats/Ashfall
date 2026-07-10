//! Faction system — hostility matrix.
//!
//! ponytail: simple lookup table. Loaded from DB in Phase 4.

use std::collections::HashMap;

/// Hostility levels between factions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hostility {
    Ally,
    Neutral,
    Enemy,
}

/// Faction hostility matrix.
#[derive(Clone)]
pub struct FactionMatrix {
    /// (faction_a, faction_b) → hostility
    relations: HashMap<(u32, u32), Hostility>,
}

impl FactionMatrix {
    pub fn new() -> Self {
        FactionMatrix {
            relations: HashMap::new(),
        }
    }

    pub fn set_relation(&mut self, faction_a: u32, faction_b: u32, hostility: Hostility) {
        self.relations.insert((faction_a, faction_b), hostility);
        self.relations.insert((faction_b, faction_a), hostility); // symmetric
    }

    pub fn get_hostility(&self, faction_a: u32, faction_b: u32) -> Hostility {
        if faction_a == faction_b {
            return Hostility::Ally;
        }
        self.relations
            .get(&(faction_a, faction_b))
            .copied()
            .unwrap_or(Hostility::Neutral)
    }

    pub fn are_hostile(&self, faction_a: u32, faction_b: u32) -> bool {
        self.get_hostility(faction_a, faction_b) == Hostility::Enemy
    }
}

impl Default for FactionMatrix {
    fn default() -> Self {
        Self::new()
    }
}
