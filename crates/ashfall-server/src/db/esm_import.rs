//! ESM/ESP plugin import — populate the SQLite database from Fallout 3 /
//! New Vegas plugin files (.esm/.esp).
//!
//! Implements the minimal TES4 plugin-format walker needed for record
//! extraction (esplugin's record/subrecord APIs are `pub(crate)`, so a native
//! parser is required). Real masters compress some records (LAND, NPC_, ...)
//! with the 0x00040000 flag — those are zlib-decompressed before parsing.
//!
//! ## Record → table mapping
//!
//! | Record | Table | Extracted subrecords |
//! |--------|-------|----------------------|
//! | WEAP   | weapons | FULL (name), DATA (damage/crit) |
//! | NPC_/CREA | npcs | FULL, RNAM (race), ACBS (female), ACDT (health/level) |
//! | RACE   | races | FULL |
//! | CONT   | base_containers | FULL |
//! | MISC/ALCH/AMMO/ARMO/BOOK/KEYM/NOTE/SLGM | base_items | FULL, DATA (weight/value) |
//! | TERM   | terminals | FULL |
//! | FACT   | factions | FULL, DATA (hostility) |
//! | QUST   | quest_stages | INDX (one row per stage) |
//! | CELL   | interiors / exteriors | FULL (interior name), XCLC (exterior coords) |
//! | REFR/ACHR/ACRE | references | NAME (baseID); cell from enclosing cell-children group |
//! | other  | records | FULL (name), DESC (description), type code |
//!
//! Data-field byte offsets follow the FO3/FNV shared layouts (damage@0,
//! item weight@0/value@4, etc.) and are bounds-checked; unverifiable fields
//! default to 0.

use super::Database;
use std::path::Path;

/// Which game's plugin file is being imported (affects a few record layouts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameId {
    Fallout3,
    FalloutNV,
}

impl std::str::FromStr for GameId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "fo3" | "fallout3" | "fallout 3" => Ok(GameId::Fallout3),
            "fnv" | "falloutnv" | "fallout nv" | "newvegas" | "new vegas" => {
                Ok(GameId::FalloutNV)
            }
            other => Err(anyhow::anyhow!("unknown game '{other}' (expected fo3 or fnv)")),
        }
    }
}

/// Import counts for reporting.
#[derive(Debug, Default, Clone, Copy)]
pub struct EsmImportStats {
    pub records: usize,
    pub weapons: usize,
    pub npcs: usize,
    pub races: usize,
    pub items: usize,
    pub containers: usize,
    pub terminals: usize,
    pub factions: usize,
    pub quest_stages: usize,
    pub interiors: usize,
    pub exteriors: usize,
    pub references: usize,
    /// Compressed records whose zlib stream failed to decompress — skipped
    /// (rare; e.g. one corrupt LAND record in the GOG FNV build).
    pub skipped_compressed: usize,
}

/// One parsed subrecord.
struct Subrecord {
    ty: [u8; 4],
    data: Vec<u8>,
}

/// Minimal forward-only reader over the plugin bytes.
struct PluginReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> PluginReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        PluginReader { data, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn peek_4cc(&self) -> Option<[u8; 4]> {
        self.data.get(self.pos..self.pos + 4).map(|s| s.try_into().unwrap())
    }

    fn read(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return None;
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(slice)
    }

    fn read_u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.read(4)?.try_into().unwrap()))
    }
}

/// Parse the subrecord list of one record body.
fn parse_subrecords(body: &[u8]) -> Vec<Subrecord> {
    let mut subs = Vec::new();
    let mut pos = 0;
    while pos + 6 <= body.len() {
        let ty: [u8; 4] = body[pos..pos + 4].try_into().unwrap();
        let size = u16::from_le_bytes([body[pos + 4], body[pos + 5]]) as usize;
        if pos + 6 + size > body.len() {
            break; // truncated trailing subrecord
        }
        subs.push(Subrecord { ty, data: body[pos + 6..pos + 6 + size].to_vec() });
        pos += 6 + size;
    }
    subs
}

// ── Subrecord field helpers ──

fn sub<'a>(subs: &'a [Subrecord], ty: &[u8; 4]) -> Option<&'a [u8]> {
    subs.iter().find(|s| &s.ty == ty).map(|s| s.data.as_slice())
}

/// FULL name, trimmed at the NUL terminator.
fn full_name(subs: &[Subrecord]) -> String {
    sub(subs, b"FULL")
        .and_then(|d| {
            let end = d.iter().position(|&b| b == 0).unwrap_or(d.len());
            Some(String::from_utf8_lossy(&d[..end]).into_owned())
        })
        .unwrap_or_default()
}

fn desc(subs: &[Subrecord]) -> String {
    sub(subs, b"DESC")
        .and_then(|d| {
            let end = d.iter().position(|&b| b == 0).unwrap_or(d.len());
            Some(String::from_utf8_lossy(&d[..end]).into_owned())
        })
        .unwrap_or_default()
}

fn sub_f32_at(subs: &[Subrecord], ty: &[u8; 4], offset: usize) -> f32 {
    sub(subs, ty)
        .filter(|d| d.len() >= offset + 4)
        .map(|d| f32::from_le_bytes(d[offset..offset + 4].try_into().unwrap()))
        .unwrap_or(0.0)
}

fn sub_u32_at(subs: &[Subrecord], ty: &[u8; 4], offset: usize) -> u32 {
    sub(subs, ty)
        .filter(|d| d.len() >= offset + 4)
        .map(|d| u32::from_le_bytes(d[offset..offset + 4].try_into().unwrap()))
        .unwrap_or(0)
}

fn sub_u16_at(subs: &[Subrecord], ty: &[u8; 4], offset: usize) -> u16 {
    sub(subs, ty)
        .filter(|d| d.len() >= offset + 2)
        .map(|d| u16::from_le_bytes([d[offset], d[offset + 1]]))
        .unwrap_or(0)
}

impl Database {
    /// Import a plugin file into the database (tool-time, not server startup).
    pub fn import_plugin(&self, path: &Path, game: GameId) -> anyhow::Result<EsmImportStats> {
        let data = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        let stats = self.import_plugin_bytes(&data, game)?;
        tracing::info!(
            "Imported {}: {} records, {} weapons, {} npcs, {} items, {} references",
            path.display(),
            stats.records,
            stats.weapons,
            stats.npcs,
            stats.items,
            stats.references,
        );
        Ok(stats)
    }

    /// Import plugin bytes (also used by tests with synthetic plugins).
    pub fn import_plugin_bytes(&self, data: &[u8], game: GameId) -> anyhow::Result<EsmImportStats> {
        let mut reader = PluginReader::new(data);
        let mut stats = EsmImportStats::default();

        // TES4 file header record — 24-byte record header: type(4) size(4)
        // flags(4) formID(4) timestamp(4) vcs(2) internal(2) = 4+4+16.
        if reader.peek_4cc() == Some(*b"TES4") {
            reader.read(4); // record type
            let size = reader.read_u32().unwrap_or(0) as usize;
            reader.read(16); // flags, formID, timestamp, vcs, internal
            reader.read(size); // header subrecords
        }

        // Walk top-level groups
        let mut world_ctx: u32 = 0;
        let mut cell_ctx: u32 = 0;
        while !reader.is_empty() {
            if reader.peek_4cc() != Some(*b"GRUP") {
                return Err(anyhow::anyhow!(
                    "malformed plugin: expected GRUP at offset {}",
                    reader.pos
                ));
            }
            self.parse_group(&mut reader, game, &mut stats, &mut world_ctx, &mut cell_ctx)?;
        }
        Ok(stats)
    }

    /// Walk one group (recursively for nested groups).
    fn parse_group(
        &self,
        reader: &mut PluginReader,
        game: GameId,
        stats: &mut EsmImportStats,
        world_ctx: &mut u32,
        cell_ctx: &mut u32,
    ) -> anyhow::Result<()> {
        let start = reader.pos;
        reader.read(4); // "GRUP"
        let size = reader.read_u32().ok_or_else(|| anyhow::anyhow!("truncated group header"))? as usize;
        let label = reader.read_u32().ok_or_else(|| anyhow::anyhow!("truncated group header"))?;
        let group_type = reader.read_u32().unwrap_or(0);
        reader.read(8); // timestamp + unknown

        if size < 24 {
            return Err(anyhow::anyhow!("invalid group size {size} at offset {start}"));
        }
        let end = start + size;
        if end > reader.data.len() {
            return Err(anyhow::anyhow!("group at offset {start} overruns file"));
        }

        // Group labels carry context: world children → world formID,
        // cell children → cell formID.
        match group_type {
            1 => *world_ctx = label,
            6 | 8 | 10 => *cell_ctx = label,
            _ => {}
        }

        while reader.pos < end {
            match reader.peek_4cc() {
                Some(g) if &g == b"GRUP" => {
                    self.parse_group(reader, game, stats, world_ctx, cell_ctx)?;
                }
                Some(_) => self.parse_record(reader, game, stats, *world_ctx, *cell_ctx)?,
                None => break,
            }
        }
        reader.pos = end;
        Ok(())
    }

    /// Parse one record and route it to the matching table.
    fn parse_record(
        &self,
        reader: &mut PluginReader,
        game: GameId,
        stats: &mut EsmImportStats,
        world_ctx: u32,
        cell_ctx: u32,
    ) -> anyhow::Result<()> {
        let rec_type: [u8; 4] = reader
            .read(4)
            .ok_or_else(|| anyhow::anyhow!("truncated record type"))?
            .try_into()
            .unwrap();
        let size = reader.read_u32().ok_or_else(|| anyhow::anyhow!("truncated record"))? as usize;
        let flags = reader.read_u32().unwrap_or(0);
        let form_id = reader.read_u32().unwrap_or(0);
        reader.read(8); // timestamp(4) + version control(2) + internal(2)
        let body = reader
            .read(size)
            .ok_or_else(|| anyhow::anyhow!("record 0x{form_id:08X} overruns file"))?;

        // Real masters compress some records (flag 0x00040000): the data
        // field is [u32 uncompressed_size][zlib stream] that decompresses to
        // the subrecords.
        let body: std::borrow::Cow<'_, [u8]> = if flags & 0x0004_0000 != 0 {
            use flate2::read::ZlibDecoder;
            use std::io::Read;
            if body.len() < 4 {
                return Err(anyhow::anyhow!(
                    "record 0x{form_id:08X}: truncated compressed body"
                ));
            }
            let expected = u32::from_le_bytes(body[..4].try_into().unwrap()) as usize;
            let mut out = Vec::with_capacity(expected);
            match ZlibDecoder::new(&body[4..]).read_to_end(&mut out) {
                Ok(_) => std::borrow::Cow::Owned(out),
                Err(e) => {
                    // One corrupt record must not abort the whole import
                    // (1 LAND record in the GOG FNV build fails zlib).
                    stats.skipped_compressed += 1;
                    tracing::warn!(
                        "record 0x{form_id:08X} zlib: {e} — skipped ({})",
                        stats.skipped_compressed
                    );
                    return Ok(());
                }
            }
        } else {
            std::borrow::Cow::Borrowed(body)
        };

        let subs = parse_subrecords(&body);
        self.import_record(&rec_type, form_id, &subs, game, stats, world_ctx, cell_ctx);
        Ok(())
    }

    /// Route one record into its table (pure extraction + inserts).
    fn import_record(
        &self,
        rec_type: &[u8; 4],
        form_id: u32,
        subs: &[Subrecord],
        _game: GameId,
        stats: &mut EsmImportStats,
        world_ctx: u32,
        cell_ctx: u32,
    ) {
        match rec_type {
            b"WEAP" => {
                let weapon = super::weapon::Weapon {
                    base_id: form_id,
                    name: full_name(subs),
                    damage: sub_f32_at(subs, b"DATA", 0),
                    crit_damage: sub_f32_at(subs, b"DATA", 28),
                    crit_chance: sub_f32_at(subs, b"DATA", 32),
                    weapon_type: 0, // per-game offset, unverifiable without real files
                };
                self.insert_weapon(&weapon);
                stats.weapons += 1;
            }
            b"NPC_" | b"CREA" => {
                let npc = super::npc::Npc {
                    base_id: form_id,
                    name: full_name(subs),
                    race: sub_u32_at(subs, b"RNAM", 0),
                    female: sub_u32_at(subs, b"ACBS", 0) & 0x1 != 0,
                    health: sub_u32_at(subs, b"ACDT", 0),
                    level: sub_u16_at(subs, b"ACDT", 32) as u32,
                };
                self.insert_npc(&npc);
                stats.npcs += 1;
            }
            b"RACE" => {
                self.insert_race(&super::race::Race { base_id: form_id, name: full_name(subs) });
                stats.races += 1;
            }
            b"CONT" => {
                self.insert_container(&super::container::BaseContainer {
                    base_id: form_id,
                    name: full_name(subs),
                });
                stats.containers += 1;
            }
            b"MISC" | b"ALCH" | b"AMMO" | b"ARMO" | b"BOOK" | b"KEYM" | b"NOTE" | b"SLGM" => {
                let item = super::item::BaseItem {
                    base_id: form_id,
                    name: full_name(subs),
                    weight: sub_f32_at(subs, b"DATA", 0),
                    value: sub_u32_at(subs, b"DATA", 4),
                };
                self.insert_item(&item);
                stats.items += 1;
            }
            b"TERM" => {
                self.insert_terminal(&super::terminal::Terminal {
                    base_id: form_id,
                    name: full_name(subs),
                });
                stats.terminals += 1;
            }
            b"FACT" => {
                let name = full_name(subs);
                let mask = sub_u32_at(subs, b"DATA", 0);
                self.set_faction(form_id, &name, mask);
                stats.factions += 1;
            }
            b"QUST" => {
                // One row per INDX subrecord (quest stage)
                let stages: Vec<u16> = subs
                    .iter()
                    .filter(|s| &s.ty == b"INDX" && s.data.len() >= 2)
                    .map(|s| u16::from_le_bytes([s.data[0], s.data[1]]))
                    .collect();
                for stage in &stages {
                    self.set_quest_stage(form_id, *stage);
                }
                stats.quest_stages += stages.len();
            }
            b"CELL" => {
                if sub(subs, b"XCLC").is_some() {
                    // Exterior cell: world + grid coords
                    let x = sub(subs, b"XCLC")
                        .and_then(|d| {
                            d.get(0..4).map(|s| i32::from_le_bytes(s.try_into().unwrap()))
                        })
                        .unwrap_or(0);
                    let y = sub(subs, b"XCLC")
                        .and_then(|d| {
                            d.get(4..8).map(|s| i32::from_le_bytes(s.try_into().unwrap()))
                        })
                        .unwrap_or(0);
                    self.insert_exterior(&super::exterior::Exterior {
                        world_id: world_ctx,
                        x,
                        y,
                    });
                    stats.exteriors += 1;
                } else if !full_name(subs).is_empty() {
                    self.insert_interior(form_id, &full_name(subs));
                    stats.interiors += 1;
                }
            }
            b"REFR" | b"ACHR" | b"ACRE" => {
                let reference = super::reference::RefData {
                    ref_id: form_id,
                    base_id: sub_u32_at(subs, b"NAME", 0),
                    cell_id: cell_ctx,
                    object_id: 0,
                };
                self.insert_reference(&reference);
                stats.references += 1;
            }
            _ => {
                // Generic record
                let record = super::record::Record {
                    base_id: form_id,
                    name: full_name(subs),
                    description: desc(subs),
                    kind: u32::from_le_bytes(*rec_type),
                };
                self.insert_record(&record);
                stats.records += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Synthetic plugin builders ──

    fn sub(ty: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(6 + data.len());
        v.extend_from_slice(ty);
        v.extend_from_slice(&(data.len() as u16).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    fn record(ty: &[u8; 4], form_id: u32, subs: &[Vec<u8>]) -> Vec<u8> {
        let body_len: usize = subs.iter().map(|s| s.len()).sum();
        let mut v = Vec::with_capacity(24 + body_len);
        v.extend_from_slice(ty);
        v.extend_from_slice(&(body_len as u32).to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
        v.extend_from_slice(&form_id.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // timestamp
        v.extend_from_slice(&0u16.to_le_bytes()); // version control
        v.extend_from_slice(&0u16.to_le_bytes()); // internal
        for s in subs {
            v.extend_from_slice(s);
        }
        v
    }

    fn group(label: [u8; 4], group_type: u32, body: &[u8]) -> Vec<u8> {
        let size = 24 + body.len();
        let mut v = Vec::with_capacity(size);
        v.extend_from_slice(b"GRUP");
        v.extend_from_slice(&(size as u32).to_le_bytes());
        v.extend_from_slice(&label);
        v.extend_from_slice(&group_type.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // timestamp
        v.extend_from_slice(&0u32.to_le_bytes()); // unknown
        v.extend_from_slice(body);
        v
    }

    fn header(num_records: u32) -> Vec<u8> {
        let hed_data: Vec<u8> = [1.7f32.to_le_bytes(), num_records.to_le_bytes(), 0x2000u32.to_le_bytes()]
            .concat();
        record(b"TES4", 0, &[sub(b"HEDR", &hed_data), sub(b"CNAM", b"Fallout3.esm\0")])
    }

    /// Build a small but representative plugin: one record per table.
    fn synthetic_plugin() -> Vec<u8> {
        // WEAP
        let weap_data: Vec<u8> = [10.0f32.to_le_bytes(), 12u32.to_le_bytes(), 5.0f32.to_le_bytes()]
            .concat();
        let weap = record(b"WEAP", 0x00001000, &[sub(b"FULL", b"10mm Pistol\0"), sub(b"DATA", &weap_data)]);

        // NPC_
        let acbs: Vec<u8> = [0x01u32.to_le_bytes()].concat(); // female bit
        let mut acdt: Vec<u8> = vec![0u8; 36];
        acdt[0..4].copy_from_slice(&100u32.to_le_bytes()); // health
        acdt[32..34].copy_from_slice(&5u16.to_le_bytes()); // level
        let npc = record(
            b"NPC_",
            0x00001001,
            &[sub(b"FULL", b"Charon\0"), sub(b"RNAM", &0x1234u32.to_le_bytes()), sub(b"ACBS", &acbs), sub(b"ACDT", &acdt)],
        );

        // RACE
        let race = record(b"RACE", 0x00001002, &[sub(b"FULL", b"Ghoul\0")]);

        // CONT
        let cont = record(b"CONT", 0x00001003, &[sub(b"FULL", b"Footlocker\0")]);

        // MISC item
        let item_data: Vec<u8> = [1.5f32.to_le_bytes(), 500u32.to_le_bytes()].concat();
        let misc = record(b"MISC", 0x00001004, &[sub(b"FULL", b"Pip-Boy\0"), sub(b"DATA", &item_data)]);

        // TERM
        let term = record(b"TERM", 0x00001005, &[sub(b"FULL", b"RobCo Terminal\0")]);

        // FACT
        let fact = record(b"FACT", 0x00001006, &[sub(b"FULL", b"Brotherhood\0"), sub(b"DATA", &0xFFu32.to_le_bytes())]);

        // QUST with two stages
        let qust = record(
            b"QUST",
            0x00001007,
            &[sub(b"FULL", b"Tutorial\0"), sub(b"INDX", &10u16.to_le_bytes()), sub(b"INDX", &20u16.to_le_bytes())],
        );

        // Interior CELL
        let interior_cell = record(b"CELL", 0x00001010, &[sub(b"FULL", b"Vault 101\0")]);

        // Generic record (e.g. SNDR)
        let generic = record(b"SNDR", 0x00001008, &[sub(b"FULL", b"Sound Marker\0"), sub(b"DESC", b"desc\0")]);

        // Exterior WRLD group (type 1, label = world formID) → CELL with XCLC
        let xclc: Vec<u8> = [(-5i32).to_le_bytes(), 7i32.to_le_bytes()].concat();
        let exterior_cell = record(b"CELL", 0x00001011, &[sub(b"XCLC", &xclc)]);
        let world = group(0x00002000u32.to_le_bytes(), 1, &exterior_cell);

        // Cell-children group (type 6, label = cell formID) → REFR
        let refr_data: Vec<u8> = [0x1234u32.to_le_bytes()].concat(); // NAME = baseID
        let refr = record(b"REFR", 0x00002001, &[sub(b"NAME", &refr_data), sub(b"DATA", &[0u8; 12])]);
        let cell_children = group(0x00001010u32.to_le_bytes(), 6, &refr);

        let mut plugin = header(8);
        plugin.extend_from_slice(&group(*b"WEAP", 0, &weap));
        plugin.extend_from_slice(&group(*b"NPC_", 0, &npc));
        plugin.extend_from_slice(&group(*b"RACE", 0, &race));
        plugin.extend_from_slice(&group(*b"CONT", 0, &cont));
        plugin.extend_from_slice(&group(*b"MISC", 0, &misc));
        plugin.extend_from_slice(&group(*b"TERM", 0, &term));
        plugin.extend_from_slice(&group(*b"FACT", 0, &fact));
        plugin.extend_from_slice(&group(*b"QUST", 0, &qust));
        plugin.extend_from_slice(&group(*b"CELL", 0, &interior_cell));
        plugin.extend_from_slice(&group(*b"SNDR", 0, &generic));
        plugin.extend_from_slice(&world);
        plugin.extend_from_slice(&cell_children);
        plugin
    }

    // ── Tests ──

    #[test]
    fn test_game_id_from_str() {
        assert_eq!("fo3".parse::<GameId>().unwrap(), GameId::Fallout3);
        assert_eq!("FalloutNV".parse::<GameId>().unwrap(), GameId::FalloutNV);
        assert!("skyrim".parse::<GameId>().is_err());
    }

    #[test]
    fn test_import_synthetic_plugin() {
        let db = Database::open_in_memory().unwrap();
        let stats = db.import_plugin_bytes(&synthetic_plugin(), GameId::Fallout3).unwrap();

        assert_eq!(stats.weapons, 1);
        assert_eq!(stats.npcs, 1);
        assert_eq!(stats.races, 1);
        assert_eq!(stats.containers, 1);
        assert_eq!(stats.items, 1);
        assert_eq!(stats.terminals, 1);
        assert_eq!(stats.factions, 1);
        assert_eq!(stats.quest_stages, 2);
        assert_eq!(stats.interiors, 1);
        assert_eq!(stats.exteriors, 1);
        assert_eq!(stats.references, 1);
        assert_eq!(stats.records, 1);

        // Spot-check extracted values
        let weapon = db.get_weapon(0x00001000).unwrap();
        assert_eq!(weapon.name, "10mm Pistol");
        assert_eq!(weapon.damage, 10.0);

        let npc = db.get_npc(0x00001001).unwrap();
        assert_eq!(npc.name, "Charon");
        assert_eq!(npc.race, 0x1234);
        assert!(npc.female, "ACBS bit 0 = female");
        assert_eq!(npc.health, 100);
        assert_eq!(npc.level, 5);

        let item = db.get_item(0x00001004).unwrap();
        assert_eq!(item.name, "Pip-Boy");
        assert_eq!(item.weight, 1.5);
        assert_eq!(item.value, 500);

        let stages = db.load_quest_stages();
        assert!(stages.iter().any(|s| s.quest_id == 0x00001007 && s.stage == 10));
        assert!(stages.iter().any(|s| s.quest_id == 0x00001007 && s.stage == 20));

        let exterior = db.get_exterior(0x2000, -5, 7).unwrap();
        assert_eq!((exterior.world_id, exterior.x, exterior.y), (0x2000, -5, 7));

        let reference = db.get_reference(0x00002001).unwrap();
        assert_eq!(reference.base_id, 0x1234);
        assert_eq!(reference.cell_id, 0x00001010, "REFR inherits cell-children group label");

        let generic = db.get_record(0x00001008).unwrap();
        assert_eq!(generic.name, "Sound Marker");
        assert_eq!(generic.kind, u32::from_le_bytes(*b"SNDR"));

        let faction = db.get_faction(0x00001006).unwrap();
        assert_eq!(faction.hostility_mask, 0xFF);
    }

    #[test]
    fn test_import_plugin_from_file() {
        // Exercises the CLI entry point (import_plugin reads from disk)
        let dir = std::env::temp_dir().join(format!("ashfall-esm-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("synthetic.esm");
        std::fs::write(&path, synthetic_plugin()).unwrap();

        let db = Database::open_in_memory().unwrap();
        let stats = db.import_plugin(&path, GameId::Fallout3).unwrap();
        assert_eq!(stats.weapons, 1);
        assert_eq!(stats.npcs, 1);
        assert_eq!(stats.references, 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_import_truncated_plugin_errors_gracefully() {
        let db = Database::open_in_memory().unwrap();
        let plugin = synthetic_plugin();
        // Truncate mid-file → walker must error, not panic
        let truncated = &plugin[..plugin.len() / 2];
        let result = db.import_plugin_bytes(truncated, GameId::Fallout3);
        assert!(result.is_err(), "truncated plugin must fail cleanly");
    }

    #[test]
    fn test_import_empty_plugin() {
        let db = Database::open_in_memory().unwrap();
        let stats = db.import_plugin_bytes(&header(0), GameId::FalloutNV).unwrap();
        assert_eq!(stats.weapons, 0);
        assert_eq!(stats.npcs, 0);
    }

    #[test]
    fn test_compressed_record_decompressed() {
        let db = Database::open_in_memory().unwrap();
        // WEAP record with the compressed flag (0x00040000): the data field
        // is a zlib stream that decompresses to the real subrecords.
        use flate2::write::ZlibEncoder;
        use std::io::Write;
        let raw = record(b"WEAP", 0x100, &[sub(b"FULL", b"Zzz\0")]);
        let raw_body = &raw[24..]; // [type 4][size 4][flags 4][formID 4][ts 4][vcs 2][int 2]
        let mut enc = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(raw_body).unwrap();
        let zlib_body = enc.finish().unwrap();
        // TES4 compressed record data: [u32 uncompressed_size][zlib stream]
        let mut compressed = Vec::with_capacity(4 + zlib_body.len());
        compressed.extend_from_slice(&(raw_body.len() as u32).to_le_bytes());
        compressed.extend_from_slice(&zlib_body);

        let mut rec = Vec::new();
        rec.extend_from_slice(b"WEAP");
        rec.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        rec.extend_from_slice(&0x0004_0000u32.to_le_bytes()); // compressed flag
        rec.extend_from_slice(&0x100u32.to_le_bytes()); // formID
        rec.extend_from_slice(&[0u8; 8]); // timestamp(4) + vcs(2) + internal(2)
        rec.extend_from_slice(&compressed);

        let mut plugin = header(1);
        plugin.extend_from_slice(&group(*b"WEAP", 0, &rec));
        let stats = db.import_plugin_bytes(&plugin, GameId::Fallout3).unwrap();
        assert_eq!(stats.weapons, 1, "compressed WEAP record imported");
    }
    #[test]
    fn test_corrupt_compressed_record_skipped_not_abort() {
        let db = Database::open_in_memory().unwrap();
        // WEAP record flagged compressed, but the payload is not valid zlib —
        // the import must skip it and continue, not error out.
        let mut rec = Vec::new();
        rec.extend_from_slice(b"WEAP");
        rec.extend_from_slice(&20u32.to_le_bytes()); // size
        rec.extend_from_slice(&0x0004_0000u32.to_le_bytes()); // compressed flag
        rec.extend_from_slice(&0x100u32.to_le_bytes()); // formID
        rec.extend_from_slice(&[0u8; 8]); // timestamp + vcs + internal
        rec.extend_from_slice(&[0x78, 0x9C, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                               0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); // garbage
        let mut plugin = header(1);
        plugin.extend_from_slice(&group(*b"WEAP", 0, &rec));
        let stats = db.import_plugin_bytes(&plugin, GameId::Fallout3).unwrap();
        assert_eq!(stats.weapons, 0, "corrupt record not imported");
        assert_eq!(stats.skipped_compressed, 1, "corrupt record counted as skipped");
    }
}

