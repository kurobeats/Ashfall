//! Exteriors table — worldspace cell index.
//!
//! Exterior cells are indexed by (worldID, x, y).
//! Used for cell-to-worldspace resolution and neighbor queries.

use super::Database;

/// An exterior cell entry.
#[derive(Debug, Clone)]
pub struct Exterior {
    pub world_id: u32,
    pub x: i32,
    pub y: i32,
}

impl Database {
    /// Get an exterior cell by world and coordinates.
    pub fn get_exterior(&self, world_id: u32, x: i32, y: i32) -> Option<Exterior> {
        let mut stmt = self.conn()
            .prepare("SELECT worldID, x, y FROM exteriors WHERE worldID = ?1 AND x = ?2 AND y = ?3")
            .ok()?;
        stmt.query_row(rusqlite::params![world_id, x, y], |row| {
            Ok(Exterior {
                world_id: row.get(0)?,
                x: row.get(1)?,
                y: row.get(2)?,
            })
        }).ok()
    }

    /// Get all exterior cells in a worldspace.
    pub fn get_exteriors_by_world(&self, world_id: u32) -> Vec<Exterior> {
        let mut stmt = match self.conn()
            .prepare("SELECT worldID, x, y FROM exteriors WHERE worldID = ?1")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([world_id], |row| {
            Ok(Exterior {
                world_id: row.get(0)?,
                x: row.get(1)?,
                y: row.get(2)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    /// Load all exteriors (for server startup cache).
    pub fn load_all_exteriors(&self) -> Vec<Exterior> {
        let mut stmt = match self.conn()
            .prepare("SELECT worldID, x, y FROM exteriors")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(Exterior {
                world_id: row.get(0)?,
                x: row.get(1)?,
                y: row.get(2)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }
}
