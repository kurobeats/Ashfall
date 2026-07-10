//! Quest stages + dialogue flags persistence.
use rusqlite::params;

#[derive(Debug, Clone)]
pub struct QuestStageRow { pub quest_id: u32, pub stage: u16 }
#[derive(Debug, Clone)]
pub struct DialogueFlagRow { pub flag_id: u32, pub value: bool }

impl super::Database {
    pub fn load_quest_stages(&self) -> Vec<QuestStageRow> {
        let mut stmt = match self.conn().prepare("SELECT quest_id, stage FROM quest_stages") { Ok(s) => s, Err(_) => return vec![] };
        let rows = stmt.query_map([], |row| Ok(QuestStageRow { quest_id: row.get(0)?, stage: row.get(1)? }));
        match rows { Ok(iter) => iter.filter_map(|r| r.ok()).collect(), Err(_) => vec![] }
    }
    pub fn set_quest_stage(&self, quest_id: u32, stage: u16) {
        let _ = self.conn().execute("INSERT OR REPLACE INTO quest_stages VALUES (?1,?2)", params![quest_id, stage]);
    }
    pub fn clear_quest_stage(&self, quest_id: u32) {
        let _ = self.conn().execute("DELETE FROM quest_stages WHERE quest_id=?1", params![quest_id]);
    }
    pub fn load_dialogue_flags(&self) -> Vec<DialogueFlagRow> {
        let mut stmt = match self.conn().prepare("SELECT flag_id, value FROM dialogue_flags") { Ok(s) => s, Err(_) => return vec![] };
        let rows = stmt.query_map([], |row| Ok(DialogueFlagRow { flag_id: row.get(0)?, value: row.get::<_,i32>(1)? != 0 }));
        match rows { Ok(iter) => iter.filter_map(|r| r.ok()).collect(), Err(_) => vec![] }
    }
    pub fn set_dialogue_flag(&self, flag_id: u32, value: bool) {
        let iv = if value { 1i32 } else { 0 };
        let _ = self.conn().execute("INSERT OR REPLACE INTO dialogue_flags VALUES (?1,?2)", params![flag_id, iv]);
    }
}
