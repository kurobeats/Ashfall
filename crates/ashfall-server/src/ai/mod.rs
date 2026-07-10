//! NPC AI — server-authoritative behavior packages + factions.

pub mod factions;
pub mod packages;

use ashfall_core::id::NetworkID;

/// AI state for a single NPC.
#[derive(Debug, Clone)]
pub struct AIState {
    pub combat_target: Option<NetworkID>,
    pub package_id: u32,
    pub package_flags: u8,
    pub last_package_change: std::time::Instant,
}

impl AIState {
    pub fn new() -> Self {
        AIState {
            combat_target: None,
            package_id: 0,
            package_flags: 0,
            last_package_change: std::time::Instant::now(),
        }
    }

    pub fn set_combat_target(&mut self, target: Option<NetworkID>) {
        self.combat_target = target;
    }

    pub fn set_package(&mut self, package_id: u32, flags: u8) {
        self.package_id = package_id;
        self.package_flags = flags;
        self.last_package_change = std::time::Instant::now();
    }
}

impl Default for AIState {
    fn default() -> Self {
        Self::new()
    }
}
