//! Quest manager — quest stage and dialogue flag tracking.
//!
//! Clone shares the underlying maps (Arc) — server and scripts agree.

use dashmap::DashMap;
use std::sync::Arc;

/// Server-authoritative quest state.
pub struct QuestManager {
    /// quest_id → current stage
    stages: Arc<DashMap<u32, u16>>,
    /// flag_id → value
    dialogue_flags: Arc<DashMap<u32, bool>>,
}

impl Clone for QuestManager {
    fn clone(&self) -> Self {
        QuestManager {
            stages: self.stages.clone(),
            dialogue_flags: self.dialogue_flags.clone(),
        }
    }
}

impl QuestManager {
    pub fn new() -> Self {
        QuestManager {
            stages: Arc::new(DashMap::new()),
            dialogue_flags: Arc::new(DashMap::new()),
        }
    }

    pub fn get_stage(&self, quest_id: u32) -> u16 {
        self.stages.get(&quest_id).map(|s| *s.value()).unwrap_or(0)
    }

    pub fn set_stage(&self, quest_id: u32, stage: u16) {
        self.stages.insert(quest_id, stage);
        tracing::info!("Quest {quest_id:08X} → stage {stage}");
    }

    pub fn get_flag(&self, flag_id: u32) -> bool {
        self.dialogue_flags
            .get(&flag_id)
            .map(|f| *f.value())
            .unwrap_or(false)
    }

    pub fn set_flag(&self, flag_id: u32, value: bool) {
        self.dialogue_flags.insert(flag_id, value);
    }

    /// All quest stages for initial client sync.
    pub fn all_stages(&self) -> Vec<(u32, u16)> {
        self.stages.iter().map(|e| (*e.key(), *e.value())).collect()
    }
}

impl Default for QuestManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_defaults_zero_and_updates() {
        let q = QuestManager::new();
        assert_eq!(q.get_stage(0x100), 0);
        q.set_stage(0x100, 3);
        assert_eq!(q.get_stage(0x100), 3);
    }

    #[test]
    fn stage_sync_snapshot() {
        let q = QuestManager::new();
        q.set_stage(0xAAA, 1);
        q.set_stage(0xBBB, 2);
        let mut stages = q.all_stages();
        stages.sort();
        assert_eq!(stages, vec![(0xAAA, 1), (0xBBB, 2)]);
    }

    #[test]
    fn flag_defaults_false_and_updates() {
        let q = QuestManager::new();
        assert!(!q.get_flag(7));
        q.set_flag(7, true);
        assert!(q.get_flag(7));
        q.set_flag(7, false);
        assert!(!q.get_flag(7));
    }

    #[test]
    fn clone_shares_state() {
        // Clone shares the Arc'd maps — script and server agree.
        let q = QuestManager::new();
        let q2 = q.clone();
        q.set_stage(0x1, 5);
        assert_eq!(q2.get_stage(0x1), 5);
    }
}
