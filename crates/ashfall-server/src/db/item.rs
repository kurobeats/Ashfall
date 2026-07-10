//! Base items table — baseID + weight/value.

use super::Database;

#[derive(Debug, Clone)]
pub struct BaseItem {
    pub base_id: u32,
    pub name: String,
    pub weight: f32,
    pub value: u32,
}

impl Database {
    pub fn get_item(&self, base_id: u32) -> Option<BaseItem> {
        let mut stmt = self.conn()
            .prepare("SELECT baseID, name, weight, value FROM base_items WHERE baseID = ?1")
            .ok()?;
        stmt.query_row(rusqlite::params![base_id], |row| {
            Ok(BaseItem {
                base_id: row.get(0)?,
                name: row.get(1)?,
                weight: row.get(2)?,
                value: row.get::<_, i32>(3)? as u32,
            })
        }).ok()
    }

    pub fn load_all_items(&self) -> Vec<BaseItem> {
        let mut stmt = match self.conn().prepare("SELECT baseID, name, weight, value FROM base_items") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(BaseItem {
                base_id: row.get(0)?,
                name: row.get(1)?,
                weight: row.get(2)?,
                value: row.get::<_, i32>(3)? as u32,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    pub fn insert_item(&self, i: &BaseItem) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO base_items (baseID, name, weight, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![i.base_id, i.name, i.weight, i.value],
        );
    }
}
