# Phase 10 — Code Reuse Plan

> vaultmp-extended + FOSE/NVSE → Ashfall Rust bridge
> Date: 2026-07-10

---

## 1. Reusable Assets Summary

| Asset | Source | Already Ported? | Ashfall Module |
|-------|--------|-----------------|----------------|
| Pipe protocol (PIPE_OP_COMMAND/RETURN/RAW/CLOSE) | vaultmp.hpp lines 24-30 | ✅ Exact | `network.rs` |
| Opcode table (VAULTFUNCTION 0xE001–0xE036) | vaultmp.cpp vaultfunction() | ⚠️ Partial (17/117) | `commands.rs` |
| TESObjectREFR field offsets (pos+0x2C, angle+0x20, refID+0x0C) | vaultmp.cpp asm | ❌ Not wired | `hooks/mod.rs` (stubs only) |
| Actor animation struct (VTable+0x1E4 → anim+0x4E/0x54/0x118) | vaultmp.cpp GetActorState | ❌ Not wired | `hooks/mod.rs` (stubs only) |
| Engine function addresses (LOOKUP_FORM, FUNC_LOOKUP, SETPOS, etc.) | vaultmp.hpp | ❌ Not wired | Needs new `hooks/vtable.rs` |
| Patch addresses (AI suppression, MatchRace, PlayGroup, AVFix, etc.) | vaultmp.cpp PatchGame() | ❌ Not wired | Needs new `hooks/memory.rs` |
| Event sink structs (TESHitEvent, TESActivateEvent, TESEquipEvent, TESCellChangeEvent, TESDeathEvent) | xSE community DB | ✅ Done | `events.rs` |
| NVSE PluginInfo, NVSEPlugin_Query/Load | xNVSE SDK | ✅ Done | `plugin.rs` |
| Server architecture (Dedicated, Client, Session, GameFactory) | vaultserver/ | ✅ Full Rust rewrite | `ashfall-server/` |
| Object hierarchy bitmasks (Reference→Object→...→Player) | ReferenceTypes.hpp | ✅ Exact | `ashfall-core/types.rs` |
| Packet types (GameAuth, GameLoad, ObjectNew, UpdatePos, etc.) | Game.hpp PF_MAKE macros | ✅ Extended (140+) | `ashfall-core/protocol/` |
| Database schema (records, refs, weapons, races, npcs, etc.) | vaultserver/Database.hpp | ✅ Extended (17 tables) | `ashfall-server/db/` |
| FOSE memory write helpers (SafeWrite8/16/32/Buf, WriteRelJump/Call) | vaultmp.cpp | ❌ Not ported | Needs new `hooks/memory.rs` |
| FOSE opcode dispatch (FuncLookup → CommandInfo → callAddr chain) | vaultmp.cpp ExecuteCommand() | ❌ Not ported | Needs new `hooks/opcode.rs` |
| Delegate pattern (BethesdaDelegator thread-safe engine calls) | vaultmp.cpp | ❌ Not ported | Needs new `hooks/opcode.rs` |
| Event queues (qActivate, qFire, qGUI_OnClick) | vaultmp.cpp | ⚠️ Simpler design | `events.rs` (callback-based) |

---

## 2. Engine Address Reference (FO3 1.7.0.3 EN)

From `vaultmp/extended/source/vaultmpdll/vaultmp.hpp` + `vaultmp.cpp`:

| Symbol | Address | Type | Purpose |
|--------|---------|------|---------|
| `LOOKUP_FORM` | `0x00455190` | `fn(refID: u32) → *TESForm` | FormID → memory pointer |
| `LOOKUP_FUNC` | `0x00519AF0` | `fn(opcode: u16) → *CommandInfo` | Script opcode → handler info |
| `QUEUE_UI_MESSAGE` | `0x0061B850` | `fn(msg, emotion, dds, sound, time) → bool` | HUD message queue |
| `ALERTED_STATE` | `0x006F6C70` | VTable function | Actor alerted state |
| `SNEAKING_STATE` | `0x006F58B0` | VTable function | Actor sneaking state (mask `0x00000400`) |
| `SETPOS` | `0x006F2050` | `fn(unk, opcode, ref, axis, pos)` | Engine SetPos implementation |

**Field offsets (raw pointer math, FO3 1.7):**

| Struct | Offset | Type | Field |
|--------|--------|------|-------|
| TESObjectREFR | +0x0C | u32 | refID (Form ID) |
| TESObjectREFR | +0x20 | f32 | angle X (radians) |
| TESObjectREFR | +0x24 | f32 | angle Y (radians) |
| TESObjectREFR | +0x28 | f32 | angle Z (radians) |
| TESObjectREFR | +0x2C | f32 | pos X |
| TESObjectREFR | +0x30 | f32 | pos Y |
| TESObjectREFR | +0x34 | f32 | pos Z |
| Global | +0x24 | f32 | value |
| AnimData (via VTable+0x1E4) | +0x4E | u8 | moving |
| AnimData | +0x54 | u8 | weapon |
| AnimData | +0x118 | ptr → +0x2C → +0x0C | idle anim BaseForm |

---

## 3. FOSE/NVSE VTable Offsets (from xSE community)

Verified for FOSE. FNV equivalents differ — detect via game CRC.

| Class | Method | VTable Index (x86) | x86_64 Index | Known? |
|-------|--------|---------------------|--------------|--------|
| TESObjectREFR | GetPos | 0x30 (index 12) | 0x30 (index 6) | ✅ FOSE |
| TESObjectREFR | SetPos | 0x34 (index 13) | 0x38 (index 7) | ✅ FOSE |
| TESObjectREFR | GetAngle | 0x38 (index 14) | ??? | ⚠️ Estimated |
| TESObjectREFR | SetAngle | 0x3C (index 15) | ??? | ⚠️ Estimated |
| TESObjectREFR | GetBaseForm | 0x10 (index 4) | 0x10 (index 2) | ✅ |
| TESForm | GetFormID | 0x04 (index 1) | 0x08 (index 1) | ✅ |
| Actor | GetActorValue | 0x68 (index 26) | 0x68 (index 13) | ✅ FOSE |
| Actor | SetActorValue | 0x6C (index 27) | ??? | ⚠️ Estimated |
| Actor | GetActorBaseValue | 0x70 (index 28) | ??? | ⚠️ Estimated |
| PlayerCharacter | GetControl | 0x90 (index 36) | ??? | ✅ FOSE |
| PlayerCharacter | SetControl | 0x94 (index 37) | ??? | ⚠️ Estimated |

**Key:** x86 vtable entries = 4 bytes, indices = offset/4. x86_64 entries = 8 bytes, indices = offset/8.

---

## 4. Implementation Plan — 4 New Modules

### 4.1 `hooks/memory.rs` — Memory Patching System

Port from vaultmp.cpp lines 99-141 (SafeWrite8/16/32/Buf, WriteRelJump/Call).

```rust
// Minimal:
use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};

pub unsafe fn safe_write8(addr: usize, value: u8);
pub unsafe fn safe_write16(addr: usize, value: u16);
pub unsafe fn safe_write32(addr: usize, value: u32);
pub unsafe fn safe_write_buf(addr: usize, data: &[u8]);
pub unsafe fn write_rel_jump(from: usize, to: usize); // 0xE9 + rel32 offset
pub unsafe fn write_rel_call(from: usize, to: usize); // 0xE8 + rel32 offset
```

**What vaultmp patches at runtime** (22+ addresses — see §5). Ashfall needs same patches for FO3 1.7. FNV version needs separate address table.

### 4.2 `hooks/vtable.rs` — VTable Access Pattern

```rust
pub unsafe fn vtable_entry<T>(object: *mut u8, index: usize) -> Option<T>;
pub unsafe fn read_field<T: Copy>(obj: *mut u8, offset: usize) -> T;
pub unsafe fn write_field<T>(obj: *mut u8, offset: usize, value: T);

// Concrete hook implementations replacing stubs in hooks/mod.rs:
pub fn get_pos(ref_id: u32) -> [f32; 3] {
    let obj = LookupFormByID(ref_id);  // via FOSE global function
    // Call vtable index 12 (x86) or 6 (x86_64)
}
```

**Fallback:** If VTable patching fails (Wine build incompatibility), use raw field offsets from §2 as backup.

### 4.3 `hooks/detour.rs` — Trampoline Pattern

For functions that need **both** interception and original call-through (e.g., ScriptRunner::Execute, ConsoleManager::ExecuteCommand).

```rust
pub struct Detour {
    target: *mut u8,
    original: Vec<u8>,      // saved bytes
    trampoline: *mut u8,    // allocated gate: [org_bytes][jmp_back_to_target+5]
    installed: bool,
}
impl Detour {
    pub unsafe fn new(target: *mut u8, hook: *const u8) -> Option<Self>;
    pub unsafe fn install(&mut self);
    pub unsafe fn uninstall(&mut self);
    pub fn call_original<F>(&self) -> F; // type-erased trampoline
}
```

### 4.4 `hooks/opcode.rs` — Opcode Interception Engine

Port from vaultmp.cpp `ExecuteCommand()` + `BethesdaDelegator()`.

```rust
type OpcodeHandler = fn(opcode: u16, params: &[u32]) -> Option<Vec<u8>>;

static OPCODE_TABLE: OnceLock<Mutex<HashMap<u16, OpcodeHandler>>> = OnceLock::new();

pub fn register_handler(opcode: u16, handler: OpcodeHandler);
pub fn intercept(opcode: u16, params: &[u32]) -> Option<Vec<u8>>;
```

**Thread safety:** vaultmp's `BethesdaDelegator` pattern (execute on game thread via busy-wait). Ashfall simpler: queue writes, read-only hooks safe from TCP thread.

---

## 5. Engine Patches Required (from vaultmp.cpp PatchGame)

These are **mandatory** for multiplayer to function. VTable approach doesn't cover them — they're binary patches.

| Category | vaultmp Address (FO3 1.7) | Patch | Purpose |
|----------|---------------------------|-------|---------|
| **Delegator** | `0x006EEC86` → `0x006EDBD9` | RelCall | Command execution hook in game loop |
| **Delegator** | `0x006EDBDA` | PUSH/POP ECX | Preserve register |
| **Respawn** | `0x006D5965` | 2× NOP | Disable respawn check |
| **Respawn** | `0x0078B230` → `0x0078B2B9` | RelJump | Skip respawn code |
| **Anim** | `0x0045F704` | JMP SHORT (0xEB) | PlayGroup NULL crash |
| **Anim** | `0x0073BB20` → `AnimDetour` | RelJump | Idle animation hook |
| **Anim** | `0x00534D8D` → `PlayIdleDetour` | RelCall | PlayIdle detection |
| **Anim** | `0x0049DD6A/0x0049DD8E` | Complex | PlayGroup crash fix |
| **AI** | `0x0072051E` | 2× NOP | AI deadlock 1 |
| **AI** | `0x006FAEE8` | 0x30 (jump) | AI deadlock 2 |
| **AI** | `0x006FAF19` | 6-byte patch | AI deadlock 3 |
| **AI** | `0x0042FBDC` | 11× NOP | AI infinite loop |
| **Race** | `0x0052F4DD` | 10× NOP | MatchRace fix |
| **Race** | `0x0052F50F` | 3× NOP | MatchRace fix 2 |
| **Lock** | `0x00527F33` | 2× NOP | Lock/unlock crash |
| **AV** | `0x00473D35` → `AVFix` | RelJump | Actor value NULL guard |
| **Fire** | `0x0079236C/0x007923C5` | Jump/patch | Fire weapon crash |
| **Activate** | `0x0078A68D` → `GetActivate` | RelCall | Activate hook |
| **PlaceAtMe** | `0x00539785` → `PlaceAtMe` | RelJump | PlaceAtMe hook |
| **FireWeapon** | `0x0071F05F` → `FireWeapon` | RelJump | FireWeapon hook |
| **Plugins** | `0x00E10FF1` | ".vmp" string | Redirect plugins.txt |

**Risk:** All addresses are FO3 1.7.0.3 EN specific. FNV needs separate table. Pattern-finding (sig scanning) preferred over hardcoded addresses for production.

---

## 6. Command Opcode Extension Priority

vaultmp has ~117 total opcodes. Ashfall has 17. Extend `commands.rs` in priority order:

### Tier 1 — Unlock position + actor sync (5 opcodes)
| Opcode | vaultmp Source | Ashfall Priority |
|--------|---------------|-----------------|
| GetBaseActorValue | API.hpp 0x1115 | HIGH |
| GetDead | API.hpp 0x102E | HIGH |
| SetCurrentHealth | API.hpp 0x14BF (NVSE) | HIGH |
| IsMoving | API.hpp 0x1019 | MED |
| GetParentCell | API.hpp 0x1495 (NVSE) | HIGH |

### Tier 2 — Item/inventory sync (6 opcodes)
| Opcode | vaultmp Source | Ashfall Priority |
|--------|---------------|-----------------|
| EquipItem | API.hpp 0x10EE | HIGH |
| UnequipItem | API.hpp 0x10EF | HIGH |
| AddItem | API.hpp 0x1002 | HIGH |
| RemoveItem | API.hpp 0x1052 | HIGH |
| RemoveAllItems | API.hpp 0x10AD | MED |
| GetRefCount | API.hpp 0x14C3 (NVSE) | MED |

### Tier 3 — Combat + death (4 opcodes)
| Opcode | vaultmp Source | Ashfall Priority |
|--------|---------------|-----------------|
| Kill | API.hpp 0x108B | HIGH |
| DamageActorValue | API.hpp 0x1181 | HIGH |
| RestoreActorValue | API.hpp 0x1182 | HIGH |
| ForceActorValue | API.hpp 0x110E | HIGH |

### Tier 4 — AI + world (4 opcodes)
| Opcode | vaultmp Source | Ashfall Priority |
|--------|---------------|-----------------|
| GetCombatTarget | API.hpp 0x10E8 | HIGH |
| PlayGroup | API.hpp 0x1013 | MED |
| ForceWeather | API.hpp 0x112D | MED |
| SetRestrained | API.hpp 0x10F3 | MED |

**Total: 17 + 19 = 36 opcodes for MVP. Remaining 81 are GUI opcodes (not needed — egui handles GUI) or low-priority.**

---

## 7. Thread Safety Model

vaultmp's approach: `BethesdaDelegator` injected into game loop at `0x006EEC86`. Pipe thread sets `delegate = true`, spins `while (delegate) Sleep(1)`. Game thread picks up `delegated[8]` function pointer, calls it, sets `delegate = false`.

**Ashfall (simpler):** TCP server on spawned thread. Two rules:
1. **Read operations** (position, angle, actor values, cell) — call VTable directly from TCP thread. Gamebryo is single-threaded but reads are safe (no internal mutation from read-only getters).
2. **Write operations** (set position, set actor value, kill) — enqueue via `crossbeam::channel::bounded(256)`. Game thread processes queue via flag checked in injected game loop (same delegator address).

**No busy-wait.** TCP thread enqueues and returns immediately. Game thread picks up on next frame.

---

## 8. Files Not to Reuse

| File | Reason |
|------|--------|
| vaultgui.dll (HookedFunctions.cpp, myIDirect3D9.cpp, etc.) | CEGUI/GUI overlay — Ashfall uses egui in native Linux client. No in-game GUI overlay needed. |
| CEGUI headers | Replaced by egui. |
| DirectInputHook.cpp | Input goes through Proton → native client doesn't need DirectInput interception. |
| RakNet (lib/RakNet/) | Replaced by Ashfall's custom UDP + postcard. |
| PAWN/AMX (lib/amx/) | Replaced by wasmtime. |
| vaultscript/ | Pawn scripting replaced by WASM. |

---

## 9. What Ashfall Improved vs vaultmp

| vaultmp Issue | Ashfall Fix |
|---------------|-------------|
| Hardcoded FO3 1.7 addresses in `vaultmpdll/vaultmp.hpp` | VTable-based + fallback raw offsets, game version detection via CRC |
| FOSE-specific `LoadLibrary("fose_1_7_vmp.dll")` | NVSE plugin exports (`NVSEPlugin_Query/Load`) — works with both FOSE and NVSE |
| Windows named pipes (platform-locked) | TCP loopback (works in Wine/Proton) |
| CEGUI (heavy C++ GUI) | egui (native Rust, cross-platform) |
| No scripting engine | wasmtime WASM sandbox |
| No persistence | SQLite (17 tables) |
| No anti-cheat | AntiCheat validator (position, velocity, item, damage, sequence) |
| Pawn scripting (AMX, proprietary) | WASM (open, multi-language) |
| Single-threaded server | Async tokio UDP server |
| `shared_ptr` memory leaks (explicit `GameFactory::Free` needed) | Rust ownership — no leaks by construction |
| RakNet (heavy, C++ only) | Lightweight custom UDP reliability layer |
| Mutex + queue event system (C++ complexity) | `crossbeam::channel` or callback registry |

---

## 10. Summary — What to Build Next

```
Phase 10 completion requires 4 new files and extending 2 existing:

NEW:
  crates/ashfall-bridge/src/hooks/memory.rs    (~100 LOC) SafeWrite*, write_rel_jump/call
  crates/ashfall-bridge/src/hooks/vtable.rs    (~150 LOC) VTable entry lookups, field access
  crates/ashfall-bridge/src/hooks/detour.rs    (~80 LOC)  Trampoline pattern
  crates/ashfall-bridge/src/hooks/opcode.rs    (~120 LOC) OpcodeHandler table, dispatcher

EXTEND:
  crates/ashfall-bridge/src/hooks/mod.rs       Wire get_pos/set_pos/get_actor_value → vtable.rs
  crates/ashfall-bridge/src/commands.rs        Add 19 Tier 1-4 opcodes

DEPENDENCIES (already in Cargo.toml): windows-sys, crossbeam

SKIP (post-MVP):
  - Havok VTable (physics non-deterministic)
  - Quest alias system (months of RE)
  - Dialog/MenuMode sync (engine pause removal needed)
  - VATS (time freeze unsolvable)
  - Save/load (single-player format incompatible)
  - Leveled list seeding (server RNG needed)
  - 81 remaining GUI opcodes (egui replaces CEGUI)
```
