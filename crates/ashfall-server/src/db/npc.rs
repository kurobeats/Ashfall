//! NPCs table — baseID + stats.

use super::Database;

#[derive(Debug, Clone)]
pub struct Npc {
    pub base_id: u32,
    pub name: String,
    pub race: u32,
    pub female: bool,
    pub health: u32,
    pub level: u32,
}

impl Database {
    pub fn get_npc(&self, base_id: u32) -> Option<Npc> {
        let mut stmt = self.conn()
            .prepare("SELECT baseID, name, race, female, health, level FROM npcs WHERE baseID = ?1")
            .ok()?;
        stmt.query_row(rusqlite::params![base_id], |row| {
            Ok(Npc {
                base_id: row.get(0)?,
                name: row.get(1)?,
                race: row.get::<_, u32>(2)?,
                female: row.get::<_, i32>(3)? != 0,
                health: row.get::<_, i32>(4)? as u32,
                level: row.get::<_, i32>(5)? as u32,
            })
        }).ok()
    }

    pub fn load_all_npcs(&self) -> Vec<Npc> {
        let mut stmt = match self.conn()
            .prepare("SELECT baseID, name, race, female, health, level FROM npcs")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(Npc {
                base_id: row.get(0)?,
                name: row.get(1)?,
                race: row.get::<_, u32>(2)?,
                female: row.get::<_, i32>(3)? != 0,
                health: row.get::<_, i32>(4)? as u32,
                level: row.get::<_, i32>(5)? as u32,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    pub fn insert_npc(&self, n: &Npc) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO npcs (baseID, name, race, female, health, level) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![n.base_id, n.name, n.race, n.female as i32, n.health, n.level],
        );
    }
}
