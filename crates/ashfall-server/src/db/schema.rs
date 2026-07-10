//! SQLite schema — CREATE TABLE statements for all tables.
//!
//! Ported from original fallout3.sqlite3, extended for FO3/FNV support.

use rusqlite::Connection;

/// Run all CREATE TABLE IF NOT EXISTS migrations.
pub fn run_migrations(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS records (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    description TEXT,
    type INTEGER
);

CREATE TABLE IF NOT EXISTS refs (
    refID INTEGER PRIMARY KEY,
    baseID INTEGER,
    cellID INTEGER,
    objectID INTEGER
);

CREATE TABLE IF NOT EXISTS exteriors (
    worldID INTEGER,
    x INTEGER,
    y INTEGER,
    PRIMARY KEY (worldID, x, y)
);

CREATE TABLE IF NOT EXISTS weapons (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    damage REAL,
    crit_damage REAL,
    crit_chance REAL,
    weapon_type INTEGER
);

CREATE TABLE IF NOT EXISTS races (
    baseID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS npcs (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    race INTEGER,
    female INTEGER,
    health INTEGER,
    level INTEGER
);

CREATE TABLE IF NOT EXISTS base_containers (
    baseID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS base_items (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    weight REAL,
    value INTEGER
);

CREATE TABLE IF NOT EXISTS terminals (
    baseID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS interiors (
    cellID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS ac_references (
    refID INTEGER PRIMARY KEY,
    baseID INTEGER,
    cellID INTEGER
);

CREATE TABLE IF NOT EXISTS quest_stages (
    quest_id INTEGER,
    stage INTEGER,
    PRIMARY KEY (quest_id)
);

CREATE TABLE IF NOT EXISTS dialogue_flags (
    flag_id INTEGER PRIMARY KEY,
    value INTEGER
);

CREATE TABLE IF NOT EXISTS karma (
    value INTEGER
);

CREATE TABLE IF NOT EXISTS reputation (
    faction_id INTEGER,
    value INTEGER,
    PRIMARY KEY (faction_id)
);

CREATE TABLE IF NOT EXISTS hardcore_stats (
    hunger REAL,
    thirst REAL,
    sleep REAL
);

CREATE TABLE IF NOT EXISTS factions (
    faction_id INTEGER PRIMARY KEY,
    name TEXT,
    hostility_mask INTEGER
);
";
