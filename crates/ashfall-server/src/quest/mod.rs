//! Quest manager — quest stage and dialogue flag tracking.

use dashmap::DashMap;

/// Server-authoritative quest state.
pub struct QuestManager {
    /// quest_id → current stage
    stages: DashMap<u32, u16>,
    /// flag_id → value
    dialogue_flags: DashMap<u32, bool>,
}

impl Clone for QuestManager {
    fn clone(&self) -> Self {
        let stages: DashMap<u32, u16> = DashMap::new();
        for entry in &self.stages {
            stages.insert(*entry.key(), *entry.value());
        }
        let flags: DashMap<u32, bool> = DashMap::new();
        for entry in &self.dialogue_flags {
            flags.insert(*entry.key(), *entry.value());
        }
        QuestManager { stages, dialogue_flags: flags }
    }
}

impl QuestManager {
    pub fn new() -> Self {
        QuestManager {
            stages: DashMap::new(),
            dialogue_flags: DashMap::new(),
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
        self.dialogue_flags.get(&flag_id).map(|f| *f.value()).unwrap_or(false)
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
