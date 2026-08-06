//! Database layer — SQLite persistence for server state.

use rusqlite::Connection;
use std::path::Path;

pub mod container;
pub mod esm_import;
pub mod exterior;
pub mod faction;
pub mod global;
pub mod interior;
pub mod item;
pub mod npc;
pub mod quest;
pub mod race;
pub mod record;
pub mod reference;
pub mod schema;
pub mod terminal;
pub mod weapon;

pub use esm_import::{EsmImportStats, GameId};

#[cfg(test)]
mod tests;

/// Database handle wrapping a rusqlite connection.
pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Database { conn };
        schema::run_migrations(&db.conn)?;
        tracing::info!("Database opened: {}", path.display());
        Ok(db)
    }

    /// Open an in-memory database (for tests).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Database { conn };
        schema::run_migrations(&db.conn)?;
        Ok(db)
    }

    #[inline]
    pub fn conn(&self) -> &Connection { &self.conn }

    pub fn close(self) -> anyhow::Result<()> {
        self.conn.execute_batch("PRAGMA optimize;")?;
        tracing::info!("Database closed");
        Ok(())
    }

    /// Load all data into server memory at startup.
    pub fn startup_load(
        &self,
        quests: &crate::quest::QuestManager,
        factions: &mut crate::ai::factions::FactionMatrix,
    ) {
        use crate::ai::factions::Hostility;

        tracing::info!("Loading database...");

        let records = self.load_all_records();
        tracing::info!("  {} records", records.len());

        let weapons = self.load_all_weapons();
        tracing::info!("  {} weapons", weapons.len());

        let npcs = self.load_all_npcs();
        tracing::info!("  {} NPCs", npcs.len());

        let items = self.load_all_items();
        tracing::info!("  {} items", items.len());

        let containers = self.load_all_containers();
        tracing::info!("  {} containers", containers.len());

        // Quest stages — the quest_stages table (populated by the ESM import)
        // holds the KNOWN stages per quest, not current progress. With no save
        // system, every quest starts unstarted (stage 0); the bridge/game
        // reports advances at runtime.
        let quest_stages = self.load_quest_stages();
        let mut quest_ids: Vec<u32> = quest_stages.iter().map(|qs| qs.quest_id).collect();
        quest_ids.sort_unstable();
        quest_ids.dedup();
        for quest_id in &quest_ids {
            quests.set_stage(*quest_id, 0);
        }
        tracing::info!("  {} quests (stage 0)", quest_ids.len());

        // Dialogue flags
        let flags = self.load_dialogue_flags();
        for df in &flags { quests.set_flag(df.flag_id, df.value); }
        tracing::info!("  {} dialogue flags", flags.len());

        // Factions
        let faction_rows = self.load_all_factions();
        for f in &faction_rows {
            for other in &faction_rows {
                if f.faction_id != other.faction_id {
                    let bit = other.faction_id % 32;
                    if (f.hostility_mask >> bit) & 1 != 0 {
                        factions.set_relation(f.faction_id, other.faction_id, Hostility::Enemy);
                    }
                }
            }
        }
        tracing::info!("  {} factions", faction_rows.len());

        tracing::info!(
            "Loaded: {} records, {} npcs, {} weapons, {} quests, {} factions",
            records.len(), npcs.len(), weapons.len(), quest_stages.len(), faction_rows.len()
        );
    }
}
