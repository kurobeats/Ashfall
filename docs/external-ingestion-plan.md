# Ashfall — External Repo Ingestion & Improvement Plan

> ✅ **STATUS: COMPLETE (2026-08-05).** All 34 items across P0–P4 implemented.
> Final state: **303 tests, 0 warnings, zero-warning build.** See the
> per-item ✅ notes below; the implementation-order section is fully checked off.
>
> Note: this is a historical record of that plan's completion. The repo has
> since grown — see README for the current test count (600) and status.

> Date: 2026-07-10
> Scope: Incremental improvements only. No redesigns, no new architectures.
> Sources: xNVSE/NVSE, FOSE, JIP LN, lStewieAl, GECK Extender, RakNet, ENet, GameNetworkingSockets, Bethesda-Plugin-Tools

---

## Summary

39 actionable improvements across 4 layers. All additive, non-breaking, <50 lines each.

| Layer | Priority Items | Total |
|-------|---------------|-------|
| Bridge hooks (plugin, events, memory, vtable, opcode) | 15 | 23 |
| Networking (RTT fix, retransmit, throttle, channels) | 6 | 8 |
| Database (ESM→DB population) | 1 | 1 |
| Consolidation (duplicate code removal) | 3 | 3 |
| Documentation/constants | 4 | 4 |

---

## Priority 0 — Bugs & Duplicates (Fix Now)

### 1. Fix `NVSEPlugin_Load` signature — `plugin.rs`  ✅ DONE (superseded 2026-08-06)
**Problem:** Takes `*const c_void`. Real NVSE passes `NVSEInterface*`.
**Fix:** Change to `*const NVSEInterface`.

> ⚠️ **Correction (verified against xFOSE/xNVSE PluginAPI.h, 2026-08-06):** the
> original premise — "NVSEInterface carries SafeWrite/trampoline bootstrap
> functions" — is FALSE. The real interface is version fields + RegisterCommand,
> SetOpcodeBase, QueryInterface, GetPluginHandle, RegisterTypedCommand,
> GetRuntimeDirectory, isNogore. The first implementation mirrored the wrong
> layout (every field after nvse_version read garbage) and used a 264-byte
> inline-array PluginInfo (real: 12 bytes, `const char*` name). All corrected in
> `bc841b4`/`f92f9fe`; FOSEPlugin_* exports added (FOSE loads FO3 plugins by the
> FOSEPlugin_ name).
```rust
#[repr(C)]
pub struct NVSEInterface {
    pub interface_version: u32,
    pub get_plugin_info: unsafe extern "C" fn() -> *mut PluginInfo,
    pub query_interface: unsafe extern "C" fn(id: u32) -> *mut c_void,
    pub register_listener: unsafe extern "C" fn(*mut NVSEInterface, *const u8, EventListener),
    pub dispatch_message: unsafe extern "C" fn(*mut NVSEInterface, *const u8, *const u8, *mut u8, u32, *const u8) -> bool,
    pub safe_write8: unsafe extern "C" fn(u32, u32),
    pub safe_write16: unsafe extern "C" fn(u32, u32),
    pub safe_write32: unsafe extern "C" fn(u32, u32),
    pub safe_write_buf: unsafe extern "C" fn(u32, *mut u8, u32),
    pub write_rel_jump: unsafe extern "C" fn(u32, u32) -> *mut u8,
    pub write_rel_call: unsafe extern "C" fn(u32, u32) -> *mut u8,
}
```

### 2. Remove duplicate `PluginInfo` from `hooks/mod.rs`
**Problem:** Both `plugin.rs` and `hooks/mod.rs` export `PluginInfo`. hooks/mod.rs version lacks `version: u32` field.
**Fix:** Delete lines 314-339 in hooks/mod.rs. plugin.rs is authoritative.

### 3. Consolidate event sink registries
**Problem:** `events.rs` has `register_event_sink(EventCallback)` with `(u32, *const c_void)`. `hooks/mod.rs` has `register_event_sink(EventSinkCallback)` with `(u32, u32, u32, u32)`. Incompatible signatures, both `pub`.
**Fix:** Remove duplicate from hooks/mod.rs. Keep events.rs (matches NVSE `BSTEventSink<T>` pattern). Hooks/mod.rs becomes a shim that bridges events.rs → pipe commands.

---

## Priority 1 — Bridge Hook Improvements (High Impact, Low Effort)

### 4. Replace HashMap with static array in `opcode.rs`
**Problem:** `HashMap<u16, OpcodeHandler>` — heap allocations, hashing, lock contention per `intercept()`.
**Fix:** `[Option<OpcodeHandler>; 0x2000]` — direct index by `opcode & 0x1FFF`. 128KB static, zero allocs, no hashing.
```rust
static OPCODE_HANDLERS: LazyLock<Mutex<[Option<OpcodeHandler>; 0x2000]>> =
    LazyLock::new(|| Mutex::new([None; 0x2000]));

pub fn intercept(opcode: u16, params: &[u32]) -> Option<Vec<u8>> {
    let idx = (opcode & 0x1FFF) as usize;
    OPCODE_HANDLERS.lock().unwrap()[idx].map(|h| h(opcode, params)).flatten()
}
```

### 5. Implement `get_cell` — `vtable.rs`
**Offset:** `TESObjectREFR+0x3C` → `TESObjectCELL*`. Read cell refID at cell+0x14.
```rust
pub unsafe fn get_cell(ref_id: u32) -> u32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() { return 0; }
    let cell_ptr: u32 = read_field(obj, 0x3C);
    if cell_ptr == 0 { return 0; }
    read_field(cell_ptr as *mut u8, 0x14) // TESForm::refID
}
```

### 6. Implement `get_enabled` — `mod.rs` → `vtable.rs`
**Offset:** `TESObjectREFR+0x50`, bit 0x02 (FO3) / `+0x54` (FNV).
```rust
pub unsafe fn get_enabled(ref_id: u32) -> bool {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() { return true; }
    let flags_offset: usize = if is_fnv() { 0x54 } else { 0x50 };
    let flags: u32 = read_field(obj, flags_offset);
    (flags & 0x02) == 0
}
```

### 7. Implement `get_name` — `vtable.rs`
**Path:** `TESObjectREFR::GetBaseForm()` vtable[4] → `TESForm::GetFullName()` vtable[7] → `CStr`.
```rust
pub unsafe fn get_name(ref_id: u32) -> String {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() { return "unnamed".into(); }
    let base_form: u32 = vcall_0(obj, vtable_index(0x10));
    if base_form == 0 { return "unnamed".into(); }
    let name_ptr: *const i8 = vcall_0(base_form as *mut u8, vtable_index(0x1C));
    if name_ptr.is_null() { return "unnamed".into(); }
    CStr::from_ptr(name_ptr).to_string_lossy().into_owned()
}
```

### 8. Add `write_rel_jump_padded` — `memory.rs`
**Problem:** 5-byte JMP over a 6-byte instruction leaves trailing garbage byte.
**Fix:** NOP-pad after jump.
```rust
pub unsafe fn write_rel_jump_padded(from: usize, to: usize, original_len: usize) {
    assert!(original_len >= 5);
    write_rel_jump(from, to);
    for i in 5..original_len {
        safe_write8(from + i, 0x90);
    }
}
```

### 9. Add `find_pattern` signature scanner — `memory.rs`
**Problem:** All addresses in `vtable.rs` hardcoded for FO3 1.7. FNV has different addresses.
**Fix:** Byte-pattern scanner for version-independent address resolution.
```rust
pub unsafe fn find_pattern(base: usize, size: usize, pattern: &[u8], mask: &str) -> usize {
    let end = base + size - pattern.len();
    let mask_bytes = mask.as_bytes();
    'outer: for addr in (base..end) {
        for i in 0..pattern.len() {
            if mask_bytes[i] == b'x' && *(addr as *const u8).add(i) != pattern[i] {
                continue 'outer;
            }
        }
        return addr;
    }
    0
}
```

### 10. Add `get_lock` — `vtable.rs`
**Path:** `TESObjectREFR::GetLocked()` vtable[40 (x86)] → `TESObjectLOCK*`.
```rust
const VTBL_REF_GET_LOCKED: usize = vtable_index(0xA0);
pub unsafe fn get_lock(ref_id: u32) -> u32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() { return 0; }
    vcall_0(obj, VTBL_REF_GET_LOCKED)
}
```

### 11. Add `get_parent_cell` — `vtable.rs`
**Offset:** `TESObjectREFR+0x28` (FO3) / `+0x2C` (FNV).
```rust
pub unsafe fn get_parent_cell(ref_id: u32) -> u32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() { return 0; }
    let offset = if is_fnv() { 0x2C } else { 0x28 };
    read_field::<u32>(obj, offset)
}
```

### 12. Add `get_combat_target` — `vtable.rs`
**Offset:** `Actor+0x4E0` (FO3) / `+0x430` (FNV), raw field read.
```rust
const OFFSET_COMBAT_TARGET_FO3: usize = 0x4E0;
const OFFSET_COMBAT_TARGET_FNV: usize = 0x430;

pub unsafe fn get_combat_target(ref_id: u32) -> u32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() { return 0; }
    let offset = if is_fnv() { OFFSET_COMBAT_TARGET_FNV } else { OFFSET_COMBAT_TARGET_FO3 };
    read_field::<u32>(obj, offset)
}
```

### 13. Add actor value index constants — `vtable.rs`
```rust
pub const AV_HEALTH: u8 = 0x14;
pub const AV_ACTION_POINTS: u8 = 0x15;
pub const AV_CARRY_WEIGHT: u8 = 0x05;
pub const AV_DAMAGE_RESIST: u8 = 0x29;
pub const AV_DAMAGE_THRESHOLD: u8 = 0x2A;  // FNV only
pub const AV_SPEED_MULT: u8 = 0x22;
pub const AV_RADIATION: u8 = 0x20;
// FNV hardcore
pub const AV_DEHYDRATION: u8 = 0x2B;
pub const AV_HUNGER: u8 = 0x2C;
pub const AV_SLEEP: u8 = 0x2D;
```

### 14. Add new event types — `events.rs`
```rust
#[repr(C)]
pub struct TESLoadGameEvent { pub loaded: bool }
pub const EVENT_ON_LOAD_GAME: u32 = 5;

#[repr(C)]
pub struct TESMagicEffectApplyEvent {
    pub caster: u32, pub target: u32, pub effect_code: u32, pub magnitude: f32,
}
pub const EVENT_ON_MAGIC_EFFECT: u32 = 6;
```

### 15. Wire real implementations in `hooks/mod.rs`
Replace stubs for `get_cell`, `get_enabled`, `get_lock`, `get_name`, `get_parent_cell`, `get_combat_target` with calls to vtable.rs implementations. Remove `// TODO` comments.

### 16. Add engine critical section comment — `opcode.rs`
Document thread safety requirement: all VTable calls from bridge thread must serialize through `std::sync::Mutex`. Real implementation needs Windows `CRITICAL_SECTION` or `parking_lot::Mutex`.

### 17. Version guard in `NVSEPlugin_Query` — `plugin.rs`
Change `if interface_version != 1` → `if interface_version < 1` for forward compatibility with xNVSE v6+.

### 18. Fix `vaultfunction_index` mask — `opcode.rs`
Change `opcode & !VAULTFUNCTION_MASK` → `opcode & 0x0FFF`. Current mask includes high nibble bits incorrectly.

---

## Priority 2 — Networking Fixes (Critical Gap)

### 19. Fix `ack_recv` RTT measurement — `network.rs:52`
**Bug:** `Instant::now().duration_since(Instant::now())` always zero. `send_buffer` timestamps never read.
**Fix:** Store send time in buffer, look up in `ack_recv`:
```rust
fn ack_recv(&mut self, ack_seq: u16) {
    if let Some(pos) = self.send_buffer.iter().position(|(s, _, _)| *s == ack_seq) {
        let (_, sent_at, _) = &self.send_buffer[pos];
        let rtt = Instant::now().duration_since(*sent_at);
        self.update_rtt(rtt);
    }
    self.send_buffer.retain(|(s, _, _)| s.wrapping_sub(ack_seq) > 0);
}
```

### 20. Add Jacobson's RTO estimator — `network.rs`
Add `rttvar: Duration` field + `update_rtt()` method using:
```
srtt = srtt + 0.125 * (sample - srtt)
rttvar = rttvar + 0.25 * (|sample - srtt| - rttvar)
rto = srtt + 4 * rttvar (clamped 100ms–3000ms)
```

### 21. Add retransmission timer — `network.rs`
```rust
fn retransmit_expired(&mut self) -> Vec<(u16, Vec<u8>)> {
    let now = Instant::now();
    self.send_buffer.iter()
        .filter(|(_, sent, _)| now.duration_since(*sent) >= self.rto)
        .map(|(seq, _, data)| (*seq, data.clone()))
        .collect()
}
```
Call from 30Hz tick loop. Exponential backoff: double RTO on retransmit, reset on ACK.

### 22. Add send window throttle — `network.rs`
```rust
const MAX_INFLIGHT: usize = 32;
fn can_send(&self) -> bool { self.send_buffer.len() < MAX_INFLIGHT }
```
Guard in `send_reliable()` → return error on full window.

### 23. Split into per-channel priority queues — `network.rs`
Replace single `send_buffer` with 3 `VecDeque`s [System, Game, Chat]. Drain System first (weight 4), then Game (2), then Chat (1).

### 24. Add NACK fast retransmit — `network.rs`
In `recv()`, detect sequence gaps. Piggyback missing seqs on next ACK packet. Sender retransmits immediately on NACK.

### 25. Add token-bucket rate limiter — `network.rs`
```rust
struct RateLimiter { tokens: f64, last_refill: Instant, max_tokens: f64, rate: f64 }
fn check_rate(&mut self, addr: SocketAddr) -> bool;
```
Call in raw `recv_from` loop. Drop silently if rate exceeded. Default: 200 packets/sec, burst 100.

### 26. Add VarInt sequence encoding — `protocol/mod.rs`
Encode u16 seq as: if <128 → single byte with high bit set. Else → 0x00 marker + 2 LE bytes. Saves 1 byte per reliable packet ~50% of the time.

---

## Priority 3 — Database: ESM→DB Direct Import

### 27. Add `esplugin` dependency — `ashfall-server/Cargo.toml`
**DONE — deviation:** esplugin (v4/v6) exposes no public record/subrecord iteration
(all record APIs are `pub(crate)`; it's built for load-order tooling). Instead
`db/esm_import.rs` implements a minimal native TES4 plugin walker (~24 lines of
record/group parsing) with bounds checks and synthetic-plugin tests.

### 28. Create `db/esm_import.rs` — ~200 lines  ✅ DONE
`Database::import_plugin(path, GameId)` → iterate ESM records → extract into all 17 tables:
- `WEAP` → weapons (FULL→name, DATA→damage/crit)
- `NPC_`/`CREA` → npcs (FULL→name, RNAM→race, ACBS→female, ACDT→health/level)
- `RACE` → races (FULL→name)
- `CONT` → base_containers (FULL→name)
- `MISC`/`ALCH`/`AMMO`/`ARMO`/`BOOK`/`KEYM`/`NOTE`/`SLGM` → base_items (FULL→name, DATA→weight/value)
- `TERM` → terminals (FULL→name)
- `FACT` → factions (FULL→name, DATA→hostility)
- `QUST` → quest_stages (INDX→stages; schema PK widened to (quest_id, stage))
- `CELL` → interiors + exteriors (FULL→interior name, XCLC→coords)
- `REFR`/`ACHR`/`ACRE` → references (NAME→baseID, cell from cell-children group label)
- Unrecognized → records (generic: baseID, FULL→name, DESC→description, type code)
Also added: `db/interior.rs` CRUD, `get_faction`, `insert_exterior`.

### 29. Add `--import-esm` CLI flag to `ashfall-server`  ✅ DONE
`ashfall-server --import-esm Fallout3.esm --import-game fo3 --import-db fallout3.sqlite3`
(import runs at tool-time, not server startup).

### 30. Deprecate `tools/esm-reader/`
N/A — no `tools/esm-reader/` directory exists in this repo. Native importer
replaces the need entirely.

---

## Priority 4 — Consolidation & Cleanup

### 31. Add `core/src/protocol/console.rs`  ✅ DONE (deviation)
Console command types were already single-module in `ashfall-bridge/src/console.rs`
(handler registry + defaults). Added the opcode range documentation table
(original 17 / Tier 1-4 / VAULTFUNCTION) there instead of core — console
commands are bridge-only, core/protocol is shared wire format.

### 32. Replace `.csv` database population in tests  ✅ DONE (deviation)
No CSV-based test setup exists. The intent — validating the ESM import
pipeline — is covered by `db/esm_import.rs` tests: synthetic plugins built
in memory, imported via `import_plugin_bytes`, asserted across all tables.

### 33. Add `#[allow(dead_code)]` annotations  ✅ DONE
Marked intentionally-stubbed/deferred items (Windows-only engine constants
`LOOKUP_FORM_FO3`/`VTBL_REF_GET_POS`, `allow_all`, client ui/ipc/world
architecture modules, never-read cache fields) with `#[allow(dead_code)]` +
`// ponytail:` comments.

### 34. Run `cargo fix` workspace-wide  ✅ DONE
Manual equivalent (clippy/fmt unavailable): workspace now builds with **0
warnings** (was 87) — removed unused imports/muts/vars/constants, deleted
dead methods (`timeout_for`, `read_u16`, UnreliableChannel seq), fixed a
unreachable pattern, underscored genuinely-unused bindings.

All 34 items complete. Total: 303 tests, 0 warnings, zero-warning build.

---

## What Was NOT Included (Architectural Changes Deferred)

| Item | Reason |
|------|--------|
| Signature scanning with fallback chains (ibds) | Too complex — needs per-version signature DB. Add `find_pattern()` utility first, build DB incrementally. |
| BSTEventSink vtable subclass in Rust | Requires C++ vtable allocation — not a Rust incremental change. |
| SEH crash guards (`__try/__except`) | Windows-only, Wine compatibility unknown. |
| Delta compression | postcard variable-length output makes byte-level delta infeasible. Needs field-level delta. |
| Client-side reliability layer | Server-driven model works for MVP. Add when client sends need reliability. |
| RakNet BitStream → postcard conversion | Already done. postcard is simpler. |
| ConsoleManager vtable hook | Needs exact address + version detection. Add when proton testing begins. |
| Full quest alias replication | Months of RE. Server-side quest stage sync covers 80% of use case. |

---

## Implementation Order

```
Batch 1 (today, ~60 min):                              ✅ DONE
  P0: #1-3 (fix signature, remove duplicates, consolidate events)

Batch 2 (next session, ~2 hrs):                        ✅ DONE
  P1: #4-5-6-7-8-15 (opcode array, get_cell/enabled/name, write_rel_jump_padded, wire stubs)

Batch 3 (next session, ~3 hrs):                        ✅ DONE
  P1: #9-10-11-12-13-14-16-17-18 (find_pattern, get_lock/parent_cell/combat_target, 
       AV constants, new events, version guard, mask fix)

Batch 4 (dedicated session, ~4 hrs):                   ✅ DONE
  P2: #19-20-21-22 (RTT fix, Jacobson RTO, retransmit, send window)
  P2: #23-24-25-26 (channel queues, NACK, rate limit, VarInt seq)

Batch 5 (dedicated session, ~4 hrs):                   ✅ DONE
  P3: #27-28-29-30 (ESM import, CLI flag, native TES4 parser)

Batch 6 (cleanup, ~2 hrs):                             ✅ DONE
  P4: #31-34 (opcode range docs, import-pipeline tests, dead_code annotations, zero warnings)
```

**Total: 34 actionable items — all complete (303 tests, 0 warnings).**
