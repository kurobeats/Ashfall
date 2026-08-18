//! Ashfall shared-quest game mode — example of a server-wide quest.
//!
//! Demonstrates the shared-quest pattern on top of the WASM script stack:
//! all players' NPC kills count toward ONE shared quest; progress is
//! broadcast; completion advances to the next stage and notifies everyone.
//!
//! Built with: `cargo build --target wasm32-unknown-unknown --release`
//! Copy `target/wasm32-unknown-unknown/release/ashfall_shared_quest.wasm`
//! to the server's `scripts/` directory (loads alongside other modes).
//!
//! Host import names must match crates/ashfall-server/src/script/host.rs.
//! wasm32 is single-threaded — plain `static mut` is safe.

#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(level: u32, ptr: *const u8, len: u32);
    fn chat_message(player_id: u64, ptr: *const u8, len: u32);
    fn ui_message(player_id: u64, ptr: *const u8, len: u32);
}

// ═══════════════════════════════════════════════════════════════
// Shared quest state
// ═══════════════════════════════════════════════════════════════

/// Kills needed to clear each stage. Shared across all players.
const STAGE_TARGETS: [u32; 3] = [3, 5, 8];
/// Stage descriptions shown to players.
const STAGE_NAMES: [&[u8]; 3] = [
    b"Stage 1/3: Clear the way - 3 kills",
    b"Stage 2/3: Thin the herd - 5 kills",
    b"Stage 3/3: Wipe them out - 8 kills",
];

static mut STAGE: u32 = 0;
static mut KILLS: u32 = 0;
static mut PLAYERS: [u64; 16] = [0; 16];

fn log(msg: &[u8]) {
    unsafe { host_log(3, msg.as_ptr(), msg.len() as u32) };
}

fn tell(player_id: u64, msg: &[u8]) {
    unsafe { ui_message(player_id, msg.as_ptr(), msg.len() as u32) };
}

fn broadcast(msg: &[u8]) {
    unsafe {
        for &p in PLAYERS.iter() {
            if p != 0 {
                chat_message(p, msg.as_ptr(), msg.len() as u32);
            }
        }
    }
}

fn is_player(id: u64) -> bool {
    unsafe { PLAYERS.iter().any(|&p| p == id) }
}

fn register_player(id: u64) {
    unsafe {
        for slot in PLAYERS.iter_mut() {
            if *slot == 0 {
                *slot = id;
                return;
            }
        }
    }
}

fn unregister_player(id: u64) {
    unsafe {
        for slot in PLAYERS.iter_mut() {
            if *slot == id {
                *slot = 0;
                return;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Exported callbacks
// ═══════════════════════════════════════════════════════════════

#[no_mangle]
pub extern "C" fn on_server_init() {
    log(b"Shared-quest game mode loaded");
    broadcast(b"[QUEST] Shared quest started!");
    broadcast(STAGE_NAMES[0]);
}

#[no_mangle]
pub extern "C" fn on_server_exit(_shutdown: bool) {}

/// Permissive auth (same as freeroam — any non-empty short name).
#[no_mangle]
pub extern "C" fn on_client_authenticate(
    name_ptr: *const u8,
    name_len: u32,
    _pwd_ptr: *const u8,
    _pwd_len: u32,
) -> u32 {
    if name_ptr.is_null() || name_len == 0 || name_len > 16 {
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn on_player_disconnect(player_id: u64, _reason: u32) {
    unregister_player(player_id);
}

#[no_mangle]
pub extern "C" fn on_player_request_game(_player_id: u64) -> u32 {
    0x0001A26E // Megaton exterior (same spawn as freeroam)
}

#[no_mangle]
pub extern "C" fn on_spawn(player_id: u64) {
    register_player(player_id);
    unsafe {
        let stage = STAGE;
        let kills = KILLS;
        let target = STAGE_TARGETS[stage as usize];
        let mut buf = [0u8; 64];
        let s = format!("[QUEST] Stage {}/3: {}/{} kills - type !quest", stage + 1, kills, target);
        let n = s.len().min(buf.len());
        buf[..n].copy_from_slice(&s.as_bytes()[..n]);
        tell(player_id, &buf[..n]);
    }
}

#[no_mangle]
pub extern "C" fn on_player_chat(
    player_id: u64,
    msg_ptr: *const u8,
    msg_len: u32,
) -> u32 {
    let msg = read_message(msg_ptr, msg_len);
    if msg == b"!quest" {
        unsafe {
            let stage = STAGE;
            let kills = KILLS;
            let target = STAGE_TARGETS[stage as usize];
            let mut buf = [0u8; 64];
            let s = format!("[QUEST] Stage {}/3: {}/{} kills", stage + 1, kills, target);
            let n = s.len().min(buf.len());
            buf[..n].copy_from_slice(&s.as_bytes()[..n]);
            tell(player_id, &buf[..n]);
        }
        return 0; // consume
    }
    if msg == b"!reset" {
        // Admin-ish debug reset.
        unsafe {
            STAGE = 0;
            KILLS = 0;
        }
        broadcast(b"[QUEST] Reset to stage 1");
        return 0;
    }
    1 // relay
}

/// Shared quest progress: every NPC kill counts toward the current stage.
/// Player deaths don't count. On completion: broadcast + advance.
#[no_mangle]
pub extern "C" fn on_actor_death(
    actor_id: u64,
    _killer_id: u64,
    _limbs: u32,
    _cause: u32,
) {
    if is_player(actor_id) {
        return; // player death — no quest credit
    }
    unsafe {
        KILLS += 1;
        let stage = STAGE;
        let kills = KILLS;
        let target = STAGE_TARGETS[stage as usize];
        if kills >= target {
            broadcast(STAGE_NAMES[stage as usize]);
            let done = stage + 1 >= STAGE_TARGETS.len() as u32;
            if done {
                broadcast(b"[QUEST] ALL STAGES COMPLETE! The wasteland is yours.");
                STAGE = 0;
                KILLS = 0;
                broadcast(STAGE_NAMES[0]);
            } else {
                broadcast(b"[QUEST] Stage complete!");
                STAGE = stage + 1;
                KILLS = 0;
                broadcast(STAGE_NAMES[(stage + 1) as usize]);
            }
        } else {
            let mut buf = [0u8; 64];
            let s = format!("[QUEST] {}/{} kills", kills, target);
            let n = s.len().min(buf.len());
            buf[..n].copy_from_slice(&s.as_bytes()[..n]);
            broadcast(&buf[..n]);
        }
    }
}

// Unused callbacks kept for parity with the engine's export surface.
#[no_mangle]
pub extern "C" fn on_create(_object_id: u64) {}
#[no_mangle]
pub extern "C" fn on_destroy(_object_id: u64) {}
#[no_mangle]
pub extern "C" fn on_activate(_ref_id: u32, _actor_id: u64) {}
#[no_mangle]
pub extern "C" fn on_cell_change(_object_id: u64, _cell: u32) {}
#[no_mangle]
pub extern "C" fn on_item_count_change(_item_id: u64, _count: u32) {}
#[no_mangle]
pub extern "C" fn on_actor_value_change(_actor_id: u64, _index: u32, _value: f32) {}
#[no_mangle]
pub extern "C" fn on_hit(_target_id: u64, _attacker_id: u64, _limb: u32, _damage: f32) -> u32 {
    1
}
#[no_mangle]
pub extern "C" fn on_equip(_actor_id: u64, _item_id: u64, _equipped: u32) {}
#[no_mangle]
pub extern "C" fn on_game_time_change(_year: u32, _month: u32, _day: u32, _hour: u32) {}
#[no_mangle]
pub extern "C" fn on_quest_stage(_stage: u32) {}

fn read_message(ptr: *const u8, len: u32) -> &'static [u8] {
    static mut BUF: [u8; 256] = [0; 256];
    unsafe {
        let n = (len as usize).min(BUF.len());
        core::ptr::copy_nonoverlapping(ptr, BUF.as_mut_ptr(), n);
        &BUF[..n]
    }
}
