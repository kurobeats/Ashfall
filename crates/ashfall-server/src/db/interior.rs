//! Interiors table — interior cell index (cellID → name).

use super::Database;
use rusqlite::params;

/// An interior cell entry.
#[derive(Debug, Clone)]
pub struct InteriorRow {
    pub cell_id: u32,
    pub name: String,
}

impl Database {
    /// Get an interior cell by id.
    pub fn get_interior(&self, cell_id: u32) -> Option<InteriorRow> {
        let mut stmt = self
            .conn()
            .prepare("SELECT cellID, name FROM interiors WHERE cellID = ?1")
            .ok()?;
        stmt.query_row(params![cell_id], |row| {
            Ok(InteriorRow {
                cell_id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .ok()
    }

    /// Insert or replace an interior cell.
    pub fn insert_interior(&self, cell_id: u32, name: &str) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO interiors (cellID, name) VALUES (?1, ?2)",
            params![cell_id, name],
        );
    }

    /// Load all interior cells.
    pub fn load_all_interiors(&self) -> Vec<InteriorRow> {
        let mut stmt = match self.conn().prepare("SELECT cellID, name FROM interiors") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(InteriorRow {
                cell_id: row.get(0)?,
                name: row.get(1)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }
}
