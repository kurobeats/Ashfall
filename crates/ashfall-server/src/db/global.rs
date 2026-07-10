//! FO3/FNV global persistence — karma, reputation, hardcore stats.
use rusqlite::params;

impl super::Database {
    pub fn get_karma(&self) -> i32 {
        self.conn().query_row("SELECT value FROM karma LIMIT 1", [], |r| r.get(0)).unwrap_or(0)
    }
    pub fn set_karma(&self, value: i32) {
        let _ = self.conn().execute("DELETE FROM karma", []);
        let _ = self.conn().execute("INSERT INTO karma VALUES (?1)", params![value]);
    }
    pub fn get_reputation(&self, faction_id: u32) -> i32 {
        self.conn().query_row("SELECT value FROM reputation WHERE faction_id=?1", params![faction_id], |r| r.get(0)).unwrap_or(0)
    }
    pub fn set_reputation(&self, faction_id: u32, value: i32) {
        let _ = self.conn().execute("INSERT OR REPLACE INTO reputation VALUES (?1,?2)", params![faction_id, value]);
    }
    pub fn load_all_reputation(&self) -> Vec<(u32, i32)> {
        let mut stmt = match self.conn().prepare("SELECT faction_id, value FROM reputation") { Ok(s) => s, Err(_) => return vec![] };
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)));
        match rows { Ok(iter) => iter.filter_map(|r| r.ok()).collect(), Err(_) => vec![] }
    }
    pub fn get_hardcore_stats(&self) -> (f32, f32, f32) {
        self.conn().query_row("SELECT hunger, thirst, sleep FROM hardcore_stats LIMIT 1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap_or((0.0,0.0,0.0))
    }
    pub fn set_hardcore_stats(&self, hunger: f32, thirst: f32, sleep: f32) {
        let _ = self.conn().execute("DELETE FROM hardcore_stats", []);
        let _ = self.conn().execute("INSERT INTO hardcore_stats VALUES (?1,?2,?3)", params![hunger,thirst,sleep]);
    }
}
