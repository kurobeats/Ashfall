//! Base containers table — baseID + name.

use super::Database;

#[derive(Debug, Clone)]
pub struct BaseContainer {
    pub base_id: u32,
    pub name: String,
}

impl Database {
    pub fn get_container(&self, base_id: u32) -> Option<BaseContainer> {
        let mut stmt = self.conn()
            .prepare("SELECT baseID, name FROM base_containers WHERE baseID = ?1")
            .ok()?;
        stmt.query_row(rusqlite::params![base_id], |row| {
            Ok(BaseContainer { base_id: row.get(0)?, name: row.get(1)? })
        }).ok()
    }

    pub fn load_all_containers(&self) -> Vec<BaseContainer> {
        let mut stmt = match self.conn().prepare("SELECT baseID, name FROM base_containers") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(BaseContainer { base_id: row.get(0)?, name: row.get(1)? })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    pub fn insert_container(&self, c: &BaseContainer) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO base_containers (baseID, name) VALUES (?1, ?2)",
            rusqlite::params![c.base_id, c.name],
        );
    }
}
