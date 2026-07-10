//! Faction persistence.
use rusqlite::params;

#[derive(Debug, Clone)]
pub struct FactionRow { pub faction_id: u32, pub name: String, pub hostility_mask: u32 }

impl super::Database {
    pub fn load_all_factions(&self) -> Vec<FactionRow> {
        let mut stmt = match self.conn().prepare("SELECT faction_id, name, hostility_mask FROM factions") {
            Ok(s) => s, Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| Ok(FactionRow { faction_id: row.get(0)?, name: row.get(1)?, hostility_mask: row.get(2)? }));
        match rows { Ok(iter) => iter.filter_map(|r| r.ok()).collect(), Err(_) => vec![] }
    }
    #[allow(dead_code)]
    pub fn set_faction(&self, faction_id: u32, name: &str, hostility_mask: u32) {
        let _ = self.conn().execute("INSERT OR REPLACE INTO factions VALUES (?1,?2,?3)", params![faction_id, name, hostility_mask]);
    }
}
