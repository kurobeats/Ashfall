//! Records table — maps baseID → name/description/type.
//!
//! These are the static game data records (weapons, NPCs, items, etc.)
//! loaded from Fallout 3 / New Vegas ESM files via an ESM reader tool.

use super::Database;

/// A game data record.
#[derive(Debug, Clone)]
pub struct Record {
    pub base_id: u32,
    pub name: String,
    pub description: String,
    pub kind: u32,
}

impl Database {
    /// Get a single record by base ID.
    pub fn get_record(&self, base_id: u32) -> Option<Record> {
        let mut stmt = self.conn()
            .prepare("SELECT baseID, name, description, type FROM records WHERE baseID = ?1")
            .ok()?;
        stmt.query_row([base_id], |row| {
            Ok(Record {
                base_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                kind: row.get(3)?,
            })
        }).ok()
    }

    /// Get all records of a given type (kind filter).
    pub fn get_records_by_type(&self, kind: u32) -> Vec<Record> {
        let mut stmt = match self.conn()
            .prepare("SELECT baseID, name, description, type FROM records WHERE type = ?1")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([kind], |row| {
            Ok(Record {
                base_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                kind: row.get(3)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    /// Insert or replace a record.
    pub fn insert_record(&self, record: &Record) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO records (baseID, name, description, type) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![record.base_id, record.name, record.description, record.kind],
        );
    }

    /// Load all records (for server startup cache).
    pub fn load_all_records(&self) -> Vec<Record> {
        let mut stmt = match self.conn()
            .prepare("SELECT baseID, name, description, type FROM records")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(Record {
                base_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                kind: row.get(3)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }
}
