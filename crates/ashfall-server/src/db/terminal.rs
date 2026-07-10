//! Terminals table — baseID + name.

use super::Database;

#[derive(Debug, Clone)]
pub struct Terminal {
    pub base_id: u32,
    pub name: String,
}

impl Database {
    pub fn get_terminal(&self, base_id: u32) -> Option<Terminal> {
        let mut stmt = self.conn()
            .prepare("SELECT baseID, name FROM terminals WHERE baseID = ?1")
            .ok()?;
        stmt.query_row(rusqlite::params![base_id], |row| {
            Ok(Terminal { base_id: row.get(0)?, name: row.get(1)? })
        }).ok()
    }

    pub fn load_all_terminals(&self) -> Vec<Terminal> {
        let mut stmt = match self.conn().prepare("SELECT baseID, name FROM terminals") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(Terminal { base_id: row.get(0)?, name: row.get(1)? })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    pub fn insert_terminal(&self, t: &Terminal) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO terminals (baseID, name) VALUES (?1, ?2)",
            rusqlite::params![t.base_id, t.name],
        );
    }
}
