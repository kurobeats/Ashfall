//! References table — persistent world object placement data.
//!
//! Maps refID → baseID / cellID / objectID.
//! Used to spawn objects in cells at server startup.

use super::Database;

/// A persistent reference record.
#[derive(Debug, Clone)]
pub struct RefData {
    pub ref_id: u32,
    pub base_id: u32,
    pub cell_id: u32,
    pub object_id: u32,
}

impl Database {
    /// Get a single reference by refID.
    pub fn get_reference(&self, ref_id: u32) -> Option<RefData> {
        let mut stmt = self.conn()
            .prepare("SELECT refID, baseID, cellID, objectID FROM refs WHERE refID = ?1")
            .ok()?;
        stmt.query_row([ref_id], |row| {
            Ok(RefData {
                ref_id: row.get(0)?,
                base_id: row.get(1)?,
                cell_id: row.get(2)?,
                object_id: row.get(3)?,
            })
        }).ok()
    }

    /// Get all references in a specific cell.
    pub fn get_references_by_cell(&self, cell_id: u32) -> Vec<RefData> {
        let mut stmt = match self.conn()
            .prepare("SELECT refID, baseID, cellID, objectID FROM refs WHERE cellID = ?1")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([cell_id], |row| {
            Ok(RefData {
                ref_id: row.get(0)?,
                base_id: row.get(1)?,
                cell_id: row.get(2)?,
                object_id: row.get(3)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    /// Insert or replace a reference.
    pub fn insert_reference(&self, r: &RefData) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO refs (refID, baseID, cellID, objectID) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![r.ref_id, r.base_id, r.cell_id, r.object_id],
        );
    }

    /// Load all references (for server startup cache).
    pub fn load_all_references(&self) -> Vec<RefData> {
        let mut stmt = match self.conn()
            .prepare("SELECT refID, baseID, cellID, objectID FROM refs")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(RefData {
                ref_id: row.get(0)?,
                base_id: row.get(1)?,
                cell_id: row.get(2)?,
                object_id: row.get(3)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }
}
