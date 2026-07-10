//! Database persistence tests.

#[test]
fn test_record_roundtrip() {
    let db = super::Database::open_in_memory().unwrap();
    db.conn().execute("INSERT INTO records VALUES (1,'A','da',2)", []).unwrap();
    let records = db.load_all_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "A");
}

#[test]
fn test_weapon_roundtrip() {
    let db = super::Database::open_in_memory().unwrap();
    db.conn().execute("INSERT INTO weapons VALUES (42,'Laser',25,22,0.05,1)", []).unwrap();
    let ws = db.load_all_weapons();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].name, "Laser");
}

#[test]
fn test_npc_roundtrip() {
    let db = super::Database::open_in_memory().unwrap();
    db.conn().execute("INSERT INTO npcs VALUES (300,'Mutant',5,0,200,8)", []).unwrap();
    let npcs = db.load_all_npcs();
    assert_eq!(npcs.len(), 1);
    assert_eq!(npcs[0].health, 200);
}

#[test]
fn test_quest_stage_persistence() {
    let db = super::Database::open_in_memory().unwrap();
    db.set_quest_stage(0x1234, 10);
    let stages = db.load_quest_stages();
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].quest_id, 0x1234);
    assert_eq!(stages[0].stage, 10);
}

#[test]
fn test_dialogue_flag_persistence() {
    let db = super::Database::open_in_memory().unwrap();
    db.set_dialogue_flag(1, true);
    let flags = db.load_dialogue_flags();
    assert!(flags[0].value);
}

#[test]
fn test_karma_persistence() {
    let db = super::Database::open_in_memory().unwrap();
    assert_eq!(db.get_karma(), 0);
    db.set_karma(500);
    assert_eq!(db.get_karma(), 500);
}

#[test]
fn test_reputation_persistence() {
    let db = super::Database::open_in_memory().unwrap();
    db.set_reputation(1, 100);
    db.set_reputation(2, -50);
    assert_eq!(db.get_reputation(1), 100);
    assert_eq!(db.get_reputation(2), -50);
}

#[test]
fn test_hardcore_persistence() {
    let db = super::Database::open_in_memory().unwrap();
    db.set_hardcore_stats(250.0, 300.0, 150.0);
    let (h, t, s) = db.get_hardcore_stats();
    assert!((h - 250.0).abs() < 0.01);
    assert!((t - 300.0).abs() < 0.01);
}

#[test]
fn test_faction_persistence() {
    let db = super::Database::open_in_memory().unwrap();
    db.conn().execute("INSERT INTO factions VALUES (1,'NCR',0),(2,'Legion',1)", []).unwrap();
    let factions = db.load_all_factions();
    assert_eq!(factions.len(), 2);
}

#[test]
fn test_startup_load_integration() {
    let db = super::Database::open_in_memory().unwrap();
    db.conn().execute("INSERT INTO records VALUES (1,'R','d',1)", []).unwrap();
    db.conn().execute("INSERT INTO weapons VALUES (100,'10mm',9,0,0,0)", []).unwrap();
    db.conn().execute("INSERT INTO quest_stages VALUES (0xABCD,42)", []).unwrap();
    db.conn().execute("INSERT INTO factions VALUES (1,'Test',0)", []).unwrap();

    let quests = crate::quest::QuestManager::new();
    let mut factions = crate::ai::factions::FactionMatrix::new();
    db.startup_load(&quests, &mut factions);
    assert_eq!(quests.get_stage(0xABCD), 42);
}
