//! Quest handler — quest stages, dialogue flags, FO3/FNV globals.

use ashfall_core::protocol::Packet;
use crate::quest::QuestManager;

/// Handle QuestStage.
pub fn handle_quest_stage(quests: &QuestManager, quest_id: u32, stage: u16) -> Packet {
    quests.set_stage(quest_id, stage);
    Packet::QuestStage { quest_id, stage }
}

/// Handle DialogueFlag.
pub fn handle_dialogue_flag(quests: &QuestManager, flag_id: u32, value: bool) -> Packet {
    quests.set_flag(flag_id, value);
    Packet::DialogueFlag { flag_id, value }
}

/// Handle DialogueChoice.
pub fn handle_dialogue_choice(_quests: &QuestManager, flag_id: u32, choice: u32) -> Packet {
    // ponytail: forward to script callback in Phase 5
    Packet::DialogueChoice { flag_id, choice }
}

/// Handle KarmaUpdate (FO3).
pub fn handle_karma(value: i32) -> Packet {
    Packet::KarmaUpdate { value }
}

/// Handle ReputationUpdate (FNV).
pub fn handle_reputation(faction: u32, value: i32) -> Packet {
    Packet::ReputationUpdate { faction, value }
}

/// Handle HardcoreStats (FNV).
pub fn handle_hardcore_stats(hunger: f32, thirst: f32, sleep: f32) -> Packet {
    Packet::HardcoreStats { hunger, thirst, sleep }
}
