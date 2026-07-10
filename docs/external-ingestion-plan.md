# Ashfall — External Repo Ingestion & Improvement Plan

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

### 1. Fix `NVSEPlugin_Load` signature — `plugin.rs`
**Problem:** Takes `*const c_void`. Real NVSE passes `NVSEInterface*` with SafeWrite/trampoline bootstrap functions.
**Fix:** Change to `*const NVSEInterface`, expose SafeWrite via interface.
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
```toml
esplugin = "4"
```

### 28. Create `db/esm_import.rs` — ~200 lines
`Database::import_plugin(path, GameId)` → iterate ESM records → extract into all 17 tables:
- `WEAP` → weapons (FULL→name, DATA→damage/crit/type)
- `NPC_`/`CREA` → npcs (FULL→name, RNAM→race, ACBS→flags, ACDT→stats)
- `RACE` → races (FULL→name)
- `CONT` → base_containers (FULL→name)
- `MISC`/`ALCH`/`AMMO`/`ARMO`/`BOOK`/`KEYM`/`NOTE`/`SLGM` → base_items
- `TERM` → terminals (FULL→name)
- `FACT` → factions (FULL→name, DATA→hostility)
- `QUST` → quest_stages (INDX→stages)
- `CELL` → interiors + exteriors (EDID/name, XCLC→coords)
- `REFR`/`ACHR`/`ACRE` → references (NAME→baseID, DATA→XYZ, cell context)
- Unrecognized → records (generic: baseID, FULL→name, type code)

### 29. Add `--import-esm` CLI flag to `ashfall-server`
Import runs at tool-time (not server startup). CLI subcommand: `ashfall-server --import-esm Fallout3.esm --db fallout3.sqlite3`.

### 30. Deprecate `tools/esm-reader/`
Remove empty tool directory. esplugin direct ESM→DB replaces it entirely.

---

## Priority 4 — Consolidation & Cleanup

### 31. Add `core/src/protocol/console.rs`
Move console command type definitions from scattered locations into single module. Add opcode range documentation table.

### 32. Replace `.csv` database population in tests
Update test setup to use in-memory ESM import instead of canned SQL inserts. Validates import pipeline.

### 33. Add `#[allow(dead_code)]` annotations
Mark intentionally-stubbed hook functions (Havok physics, quest aliases, dialog) with explicit `#[allow(dead_code)]` + `// ponytail: deferred to post-MVP` comment.

### 34. Run `cargo fix` workspace-wide
Clean up 54 compiler warnings (unused imports, unused mut, dead_code) across ashfall-client, ashfall-server, ashfall-bridge.

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
Batch 1 (today, ~60 min):
  P0: #1-3 (fix signature, remove duplicates, consolidate events)

Batch 2 (next session, ~2 hrs):
  P1: #4-5-6-7-8-15 (opcode array, get_cell/enabled/name, write_rel_jump_padded, wire stubs)

Batch 3 (next session, ~3 hrs):
  P1: #9-10-11-12-13-14-16-17-18 (find_pattern, get_lock/parent_cell/combat_target, 
       AV constants, new events, version guard, mask fix)

Batch 4 (dedicated session, ~4 hrs):
  P2: #19-20-21-22 (RTT fix, Jacobson RTO, retransmit, send window)
  P2: #23-24-25-26 (channel queues, NACK, rate limit, VarInt seq)

Batch 5 (dedicated session, ~4 hrs):
  P3: #27-28-29-30 (esplugin import, CLI flag, deprecate esm-reader)

Batch 6 (cleanup, ~2 hrs):
  P4: #31-34 (console protocol module, csv deprecation, dead_code annotations, cargo fix)
```

**Total: 34 actionable items, ~16 hours of work.**
