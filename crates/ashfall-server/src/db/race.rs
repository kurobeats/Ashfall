//! Races table — baseID + name.

use super::Database;

#[derive(Debug, Clone)]
pub struct Race {
    pub base_id: u32,
    pub name: String,
}

impl Database {
    pub fn get_race(&self, base_id: u32) -> Option<Race> {
        let mut stmt = self.conn()
            .prepare("SELECT baseID, name FROM races WHERE baseID = ?1")
            .ok()?;
        stmt.query_row(rusqlite::params![base_id], |row| {
            Ok(Race { base_id: row.get(0)?, name: row.get(1)? })
        }).ok()
    }

    pub fn load_all_races(&self) -> Vec<Race> {
        let mut stmt = match self.conn().prepare("SELECT baseID, name FROM races") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(Race { base_id: row.get(0)?, name: row.get(1)? })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    pub fn insert_race(&self, r: &Race) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO races (baseID, name) VALUES (?1, ?2)",
            rusqlite::params![r.base_id, r.name],
        );
    }
}
