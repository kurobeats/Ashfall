//! Weapons table — baseID + combat stats.

use super::Database;

#[derive(Debug, Clone)]
pub struct Weapon {
    pub base_id: u32,
    pub name: String,
    pub damage: f32,
    pub crit_damage: f32,
    pub crit_chance: f32,
    pub weapon_type: u32,
}

impl Database {
    pub fn get_weapon(&self, base_id: u32) -> Option<Weapon> {
        let mut stmt = self.conn()
            .prepare("SELECT baseID, name, damage, crit_damage, crit_chance, weapon_type FROM weapons WHERE baseID = ?1")
            .ok()?;
        stmt.query_row(rusqlite::params![base_id], |row| {
            Ok(Weapon {
                base_id: row.get(0)?,
                name: row.get(1)?,
                damage: row.get(2)?,
                crit_damage: row.get(3)?,
                crit_chance: row.get(4)?,
                weapon_type: row.get(5)?,
            })
        }).ok()
    }

    pub fn load_all_weapons(&self) -> Vec<Weapon> {
        let mut stmt = match self.conn()
            .prepare("SELECT baseID, name, damage, crit_damage, crit_chance, weapon_type FROM weapons")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], |row| {
            Ok(Weapon {
                base_id: row.get(0)?,
                name: row.get(1)?,
                damage: row.get(2)?,
                crit_damage: row.get(3)?,
                crit_chance: row.get(4)?,
                weapon_type: row.get(5)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    pub fn insert_weapon(&self, w: &Weapon) {
        let _ = self.conn().execute(
            "INSERT OR REPLACE INTO weapons (baseID, name, damage, crit_damage, crit_chance, weapon_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![w.base_id, w.name, w.damage, w.crit_damage, w.crit_chance, w.weapon_type],
        );
    }
}
