# Ashfall — Implementation Plan

## Branch Convention
```
ashfall-{phase}-{pr-number}-{short-desc}
```
Example: `ashfall-phase1-pr1-workspace-core`

## Dependency Graph
```
PR1 ─► PR2 ─► PR3 ─► PR4 ─► ... (phases sequential within phase, phases chain)
PRs within a phase often parallelizable unless noted.
```

---

## Phase 1: Core Protocol ✅ DONE

**Implemented:**
- Workspace + 6 crates, ObjectKind bitmask hierarchy, GameObject trait
- Constants: version, CRC32 checksums (FO3 + FNV + FOSE + NVSE), size limits, ports, anti-cheat bounds
- NetworkID newtype, VaultVector math (coordinate/angle validation, distance)
- PacketHeader + Channel enum with is_unreliable() routing
- 140+ Packet variants: system, object, item, container, actor, player, window, master
- Extended packets: physics (UpdateVelocity), combat (ActorHit, ActorDamaged, ActorDeathExt, ProjectileNew/Remove, ExplosionNew), NPC AI (ActorCombatTarget, ActorAIPackage, ActorFaction), world state (DoorState, TerminalState), quest/dialogue (QuestStage, DialogueFlag, DialogueChoice), FO3 globals (KarmaUpdate), FNV globals (ReputationUpdate, HardcoreStats), cell snapshot (CellSnapshot + FormIDSync)
- Scale field added to ObjectNew, ItemNew, ActorNew, PlayerNew
- FormID type with mod_index/object_id helpers
- Game type field on MasterAnnounce (fo3/fnv)
- Bridge hooks: 40+ stubs (physics, combat DR/DT, AI, faction, door/terminal, quest/dialogue, FNV reputation/hardcore, NVSE event sinks, console hooks, opcode interception)
- 71 wire format round-trip tests, all variants under 1200 bytes

**Phase 1 total: ~2,170 LOC** ✅

---

## Phase 2: Server Foundation ✅ DONE

**Implemented:**
- Config parsing (ini + TOML) with CLI overrides
- UDP socket + custom reliability layer (3 ordered + 1 unordered channel)
- Session state machine (Connecting → Auth → Loading → InGame → Disconnecting)
- ObjectRegistry: concurrent DashMap, cell_refs, type_counts, deleted tombstones
- Full object hierarchy: Reference → Object, Item, Container, Actor, Player
- Packet dispatch routing all 140+ variants to handlers
- Auth handler: GameAuth → GameLoad flow with session creation
- Connection flow: weather/globals/deleted → PlayerNew → GameStart
- Main loop: 30Hz tick + UDP recv select + session cull
- CLI with --config, --port, --game-type flags, graceful SIGINT shutdown
- Combat, AI, quest, physics sub-systems with full validation

**Phase 2 total: ~1,260 LOC** ✅

---

## Phase 3: World Sync ✅ DONE

**Implemented:**
- CellGrid: 9-cell neighbor computation, interior/exterior cell encoding
- CellContext: enter/leave diff, visibility management
- Cell registry: O(1) cell→objects lookup, get_by_cells batch query
- Object handlers: UpdatePos/UpdateAngle/UpdateCell/UpdateName with validation
- Physics handler: UpdateVelocity with bounds checking
- Actor handlers: state/value/race/sex/dead/fire weapon sync
- Item handlers: count/condition/equipped with container linkage
- Container handlers: create, ItemList management
- Player handlers: controls, cell context with enter/leave ObjectNew/ObjectRemove
- Weather + globals: set/get with broadcast on change
- Combat resolution: Fallout damage formula, projectile/explosion relay
- NPC AI sync: combat target, AI package, faction broadcast
- Quest/Dialogue: stage updates, flag changes, choice relay
- Cell snapshot: FormID-based full cell dump on entry

**Phase 3 total: ~1,010 LOC** ✅

---

## Phase 4: Persistence ✅ DONE

**Implemented:**
- `crates/ashfall-server/src/db/mod.rs` — Database struct, open/close, schema migration
- `crates/ashfall-server/src/db/schema.rs` — 17 SQLite tables (records, refs, exteriors, weapons, races, npcs, containers, items, terminals, interiors, ac_references, quest_stages, dialogue_flags, karma, reputation, hardcore_stats, factions)
- `crates/ashfall-server/src/db/` — 15 files with full CRUD for all tables
- `startup_load()` wired into `DedicatedServer::new()` — loads all data at boot
- 10 persistent tests (round-trip + persistence)

**Phase 4 total: ~800 LOC** ✅

---

## Phase 5: Scripting ✅ DONE (Part B: real host functions + callback dispatch)

**Implemented (Part A — foundation):**
- wasmtime v22 Engine + ScriptState + module loader + instance lifecycle
- 35 callback stubs (OnHit, OnEquip, OnQuestStage, OnDialogueChoice + 31 original)
- 56 host function stubs (server, object, item, actor, player, container, world, utility, timers, quest, combat, GUI widgets)
- TimerManager with create_timer/kill_timer/tick, wired into dedicated loop
- ashfall-script SDK crate with host_fn!/callback! macros and type aliases
- Example freeroam WASM game mode (scripts/freeroam/)
- 14 integration tests (engine creation, module loading, callback stubs, timer lifecycle)
- Integrated into DedicatedServer::new() — scripts loaded at startup

**Implemented (Part B — real execution, matching the freeroam ABI: u64 ids as i64, strings as ptr/len):**
- Real host functions: set/get_game_weather, set_game_time, get/set_quest_stage,
  get/set_dialogue_flag, chat_message/ui_message/kick (effect queue), timestamp,
  get_current_players/get_max_players, host_log/debug_log, create_timer (reads
  callback name from module memory)
- Callback dispatch into WASM: on_client_authenticate (any 0 vote denies),
  on_player_chat (blocks), on_player_request_game (spawn cell), on_spawn,
  on_player_disconnect, on_actor_death, on_game_time_change; timer callbacks
  routed to exported functions by name
- Server wiring: auth/chat gates in dedicated.rs, script-chosen spawn cell,
  tick drains ScriptEffect queue (private/broadcast chat, kick), tick syncs
  script-authored weather/quest deltas to clients, live player count
- Sharing fix: WeatherState/GlobalState/QuestManager Clone now shares via Arc
  (script mutations were previously invisible to the dispatcher)
- 11 new WAT-based runtime integration tests (tests/script_runtime.rs) — real
  WASM execution without a wasm32 toolchain (wat crate parses WAT at test time)
- 3 full end-to-end tests (tests/script_e2e.rs) — real server + WASM game mode
  + raw UDP clients over the wire: script auth gate, script-set weather,
  on_spawn chat effect, and two-client chat relay. These caught two critical
  pre-existing bugs: first-contact auth dropped by the reliability layer, and
  a DashMap write-guard deadlock on every broadcast with 2+ players.

**Implemented (Part C — remaining host-function stubs now real):**
- Item ops: create_item/add_item/remove_item/equip_item/get_item_count (registry
  item lists, Container/Actor/Player links, equip flag)
- Combat: get_damage_resistance/get_damage_threshold (actor values 0x29/0x2A)
- Server meta: set_server_name (shared), get_config_int (max_players),
  set_time_scale (GameTimeState scale)
- GUI widgets: create_* / set_window_* / list ops emit real packets via
  ScriptEffect::BroadcastPacket; ButtonNew/TextNew protocol variants added
- 16 WAT runtime tests total

**Phase 5 total: ~2,300 LOC (362 workspace tests)** ✅
- All 35 callbacks dispatched (hit gate, equip, activate, item count/condition/
  equipped, cell change, window select/text/mode, dialogue, lock, actor
  sub-events) — WASM bool/u32 args are i32; typed notify helpers
- Real Rust-compiled freeroam WASM game mode builds and runs end-to-end
  (rustup wasm32 target; SDK macros emit #[link(wasm_import_module = "env")])

---

## Phase 6: GUI ✅ DONE

**Implemented:**
- `ashfall-client/src/ui/app.rs` — eframe::App with server browser + chat + game view
- `ashfall-client/src/ui/server_browser.rs` — Direct connect input + server list
- `ashfall-client/src/ui/chat.rs` — Chat panel with input and history
- `ashfall-client/src/ui/widgets.rs` — Server-authored GUI widget manager (9 widget types)
- `ashfall-client/src/main.rs` — eframe::run_native with AshfallApp, tokio background poll task

**Phase 6 total: ~1,120 LOC** ✅

---

## Phase 7: Client ✅ DONE

**Implemented:**
- `ashfall-client/src/config.rs` — ClientConfig with vaultmp.ini-style defaults
- `ashfall-client/src/network.rs` — UDP socket + reliability layer (3 channels + 1 unordered)
- `ashfall-client/src/game.rs` — Client state machine (Disconnected→Connecting→Auth→Loading→InGame), connect/auth/poll/chat
- `ashfall-client/src/dispatch.rs` — Client packet dispatch (apply to registry + UI events)
- `ashfall-client/src/world/registry.rs` — Client object cache (Object/Actor/Item/Player variants)
- `ashfall-client/src/world/state.rs` — Interpolation state + last positions
- `ashfall-client/src/world/cell.rs` — Client cell tracking
- Background tokio task for 30Hz network poll
- egui: server browser with direct connect, chat panel, object list, player stats

**Phase 7 total: ~1,770 LOC** ✅

---

## Phase 8: Master Server ✅ DONE

**Implemented:**
- `crates/ashfall-master/src/main.rs` — UDP listener, MasterAnnounce/MasterQuery handler, cull stale entries
- `crates/ashfall-master/src/server_list.rs` — HashMap registry with 120s cull
- `crates/ashfall-server/src/master.rs` — MasterAnnouncer with 60s heartbeat, shared UdpSocket
- Wired into `DedicatedServer::tick()` — auto-announces player count to master
- `crates/ashfall-client/src/ui/server_browser.rs` — Refresh button, server list display, Join button
- Client sends MasterQuery via background thread, collects responses with 2s timeout
- 6 integration tests (encode/decode, announce, update, query, FNV, cull)

**Phase 8 total: ~420 LOC** ✅

---

## Phase 9: Security ✅ DONE

**Anti-cheat module:**
- `anti_cheat.rs` — AntiCheat validator: position (speed+teleport), velocity, item count, scale, damage, sequence (anti-replay), FormID spoofing — with 18 unit tests
- Wired into handlers: object.rs (position, scale), physics.rs (velocity), item.rs (count)
- Session: `last_seq` field for anti-replay sequence tracking

**Comprehensive tests added:**
- `tests/anti_cheat.rs` — 25 integration tests (teleport, speed hack, NaN, item count, damage bounds, sequence replay, FormID spoof, scale, velocity)
- `tests/world_sync.rs` — 4 tests (cell context enter/leave, object create/move, packet serialization)
- `tests/combat_tests.rs` — 14 tests (damage formula: basic, headshot, limb, DR, DT, crit, full pipeline, limb indices, headshot fatal)
- `tests/stress.rs` — 5 tests (1000 objects, 256 cells, 20 sessions, concurrent reads, type counts)

**Phase 9 total: 48 new test assertions, 169 total tests** ✅

---

## Phase 10: Proton Bridge ✅ DONE

**Background:** Bridge DLL injects into Fallout3.exe/FalloutNV.exe under Proton/Wine, hooks Gamebryo engine via VTable patching, exposes TCP server on 127.0.0.1:1771 for the native Linux client.

**Implemented:**
- DllMain entry point + Wine DLL override loading via `WINEDLLOVERRIDES`
- NVSE/FOSE plugin exports: `NVSEPlugin_Query`/`NVSEPlugin_Load`, `PluginInfo` struct (260 bytes, `#[repr(C)]`)
- TCP server (accepts single client, pipe protocol: `PIPE_OP_COMMAND`/`PIPE_OP_RETURN`)
- 36 command opcodes (Tier 1-4): position, angle, cell, actor state, actor values, controls, items, inventory, combat, death, AI, weather
- Memory patching system: `SafeWrite8/16/32/Buf`, `WriteRelJump/Call`, `MemoryProtect` RAII, `Patch` with apply/restore, trampoline `Detour` pattern
- VTable access: entry lookup (x86 + x86_64), raw field read/write, `vcall_0`/`vcall_1` virtual method dispatch, `LookupFormByID`, angle rad→deg conversion
- Concrete hook implementations: `get_pos`/`set_pos`, `get_angle`/`set_angle`, `get_actor_state`, `get/set_actor_value`, `get_actor_base_value`, `get_base` (VTable → FormID chain)
- GECK opcode interception engine: `OpcodeHandler` registry, 11 default handlers (PlaceAtMe, AddItem, EquipItem, RemoveItem, SetActorValue, ForceActorValue, SetCurrentHealth, Kill, SetStage, Lock, Unlock)
- EventSink infrastructure: 5 event types with `#[repr(C)]` structs, callback registry
- Console command interception framework: `/kick`, `/players`, etc.
- 73 bridge tests (pipe protocol 8, command dispatch 7, events 7, plugin info 7, memory 7, detour 3, vtable 9, opcode 8, unit 17)

**Reuse from vaultmp-extended:** Extended vaultmp analysis produced `docs/phase10-reuse-plan.md` — complete engine address reference (FO3 1.7), 22+ patch addresses, 36 VAULTFUNCTION opcode table, VTable offset cross-reference (FOSE/NVSE), thread safety model, and Rust adaptation guide.

### What Still Needs Runtime Testing
- [ ] **VTable patch verification** — inject bridge.dll into actual FO3/FNV under Proton, verify hook fires
      (vtable INDEX offsets — GET_POS idx 12, GET_LOCKED idx 40, GET_BASE_FORM idx 4, ACTOR_ANIM_DATA
      idx 121 — and the anim-struct field offsets are only verifiable at runtime; statically verifiable
      constants are all r2-checked: see below)
- [x] **Opcode table verification — DONE (r2 on GECK.exe command table, 2026-08-06)** — 4 of the 15
      delegated opcodes were wrong (PlaceAtMe 0x1007→0x1025, SetStage 0x101B→0x1039, SetAlert
      0x101E→0x105A, Activate 0x100C→0x100D — each would have hijacked a different command); 11
      verified correct. Cross-checked with xNVSE SetReturnType (0x1025 = PlaceAtMe).
- [x] **Field offsets verification — DONE (xFOSE/xNVSE STATIC_ASSERT layouts, 2026-08-06)** —
      get_parent_cell was reading rotZ (+0x28/+0x2C); now parentCell FO3 0x3C / FNV 0x40.
      get_cell read cell refID at +0x14; now TESForm::refID +0x0C. Pos/angle now FNV-aware.
- [x] **Wine runtime verification — DONE (2026-08-06, wine 11, no game)** — the
      bridge now cross-compiles to i686-pc-windows-gnu (first time ever; needed
      Win32_Foundation + Win32_System_LibraryLoader features) and runs under
      wine: DLL loads, all exports resolve, Query/Load return true, the TCP
      server binds 127.0.0.1:1771 on the Linux loopback, and the pipe protocol
      round-trips (heartbeat + PIPE_OP_COMMAND → PIPE_OP_RETURN). Game-address
      getters safely return zeros outside the game via `is_game_process()`.
- [ ] **Proton integration test** — end-to-end: bridge.dll → TCP → client → server
      (needs the game running; protocol side already verified)
- [x] **CRC validation — RESOLVED (real-binary analysis, 2026-08-06)** — the
      `FALLOUT3_EN_VER17 = 0x00E59528` / `FNV_EN_VER14 = 0x0206FEC7` constants
      match NO computable hash of the real GOG Fallout3.exe 1.7.0.3
      (whole-file CRC32 = 0x425A8C16). FOSE/NVSE never use CRC detection
      (they compile per-version). detect_engine was de-fabricated; real
      scheme (VS_VERSION_INFO) deferred to runtime testing.
- [x] **Address table — VERIFIED (xFOSE fose.h FALLOUT_VERSION_1_7, checked
      against the real binary)** — LookupFormByID = 0x00455190 (884 xrefs),
      ExtractArgs = 0x00517950, CreateFormInstance = 0x0043CDA0,
      ConsoleManager_GetSingleton = 0x0062B5D0, FormHeap 0x00401000/0x401010,
      DataHandler = 0x0106CDCC. Now in `ashfall-bridge/src/hooks/mod.rs`.
- [ ] **NVSE CommandTable registration** — actual `NVSEPlugin_Load` integration with NVSE SDK
- [ ] **Engine AI suppression patches** — FO3/FNV addresses for 4 AI fixes (different per game version)
- [ ] **Wine VTable layout** — verify Wine mirrors Windows VTable exactly

### Engine Quirks (Known from vaultmp)
- Havok ragdolls non-deterministic → accept per-client variance or freeze corpses
- VATS freezes time per-client → disable in MP
- Dialog MenuMode pause breaks sync → skip dialog camera or server-only dialog
- Leveled lists per-client RNG → seed from server
- FormID mapping with different load orders → require load order match
- Save/load never worked in any FO3/FNV MP mod → won't support

**Phase 10 total: ~2,360 LOC, 96 tests** ✅

---

## Summary

| Phase | PRs | Est LOC | Key Additions |
|-------|-----|---------|---------------|
| Phase 1: Core Protocol | 1–17h | 2,170 | ✅ DONE. 140+ packets, FormID, physics, combat, quest, AI, FNV, bridge hooks |
| Phase 2: Server Foundation | 18–29 | ~2,030 | ✅ DONE. Config, UDP + reliability, sessions, registry, dispatch, combat resolver, AI, physics |
| Phase 3: World Sync | 30–39 | ~1,690 | ✅ DONE. Cell grid, position/angle/actor/item sync, combat, projectile, NPC AI, cell snapshot |
| Phase 4: Persistence | 40–47 | ~800 | ✅ DONE. 17 SQLite tables, CRUD, startup load, quest/karma/reputation/hardcore/factions |
| Phase 5: Scripting | 48–59 | ~2,300 | ✅ DONE. wasmtime v22, all 35 callbacks dispatched, all 56 host fns real, real WASM game mode builds + runs e2e, timers, effects queue |
| Phase 6: GUI | 60–67 | ~1,120 | ✅ DONE. eframe/egui app, server browser, chat overlay, widget manager |
| Phase 7: Client | 68–80 | ~1,770 | ✅ DONE. UDP networking, connection flow, object cache, handlers, 30Hz poll loop |
| Phase 8: Master Server | 81–87 | 420 | ✅ DONE. Announce/query/cull, server heartbeat, client query, 6 integration tests |
| Phase 9: Security + Testing | 88–97 | ~1,610 | ✅ DONE. AntiCheat validator + handler ownership (own-player + own-inventory-only mutation) + item container-chain authz |
| Phase 10: Proton Bridge | 98–107 | ~2,360 | ✅ DONE. 36 commands, memory/VTable/detour/opcode hooks, 11 default opcode interceptors (all 15 opcodes two-tool verified — 4 corrected), real VTable getters (2 field offsets corrected), FOSE/NVSE ABI corrected, i686 cross-build + wine protocol round-trip, 96 tests |
| **Total** | **~102** | **~18,430** | |

### Post-Phase-10 Follow-up (external-ingestion-plan.md, all 34 items ✅)

| Batch | Items | Result |
|-------|-------|--------|
| P0 bridge fixes | #1–3 | NVSEInterface snapshot, PluginInfo dedupe, event-sink consolidation + pipe event frames |
| P1 bridge hooks | #4–18 | Direct-indexed opcode table, real VTable getters (cell/enabled/name/lock/parent-cell/combat-target), `find_pattern`, `write_rel_jump_padded`, 2 new event types, forward-compat version guard |
| P2 networking | #19–26 | Working reliability: ACK/NACK frames, Jacobson RTO, backoff retransmit, send window, priority queues, rate limiter, varint seqs — verified by real-UDP loss simulation |
| P3 ESM import | #27–30 | `ashfall-server --import-esm` native TES4 parser → all 17 tables |
| P4 cleanup | #31–34 | Opcode range docs, import-pipeline tests, dead-code annotations, zero-warning build |

**362 tests, 0 warnings** (lib + test targets). See `docs/external-ingestion-plan.md` for per-item status.

### Phase 11: SkyrimTogetherReborn reuse (items 1–4) ✅

| Item | What shipped | Files |
|------|-------------|-------|
| 1. Address library / thiscall | `AutoPtr` (lazy cached address), `select_candidate` (prologue-signature build selection), `call_thiscall_0..3` (x86 thiscall at explicit addr, edx preserved; x64 `extern "system"` fallback). `fo3_lookup_addr` refactored onto it. | `crates/ashfall-bridge/src/hooks/address.rs`, `hooks/vtable.rs` |
| 2. Differential state | `ActorStateDelta` — presence-optional actor state (STR Differential.h pattern): one packet per state burst, receiver merges. Server applies + relays owner-gated; client merges into `ClientObject::Actor`. | `protocol/mod.rs`, `handlers/actor.rs`, client `world/registry.rs` |
| 3. StringCache | `StringTable` + `CachedString` (Plain/Inline{id,value}/Id). Server assigns ids per-session, `Packet::finalize_strings` binds in the send path (dedicated.rs `send()`), repeats go out as 2-byte ids. Wired into ObjectNew/UpdateName/UpdateActorIdle/GameChat/GameMessage/UpdateInterior. | `ashfall-core/src/string_cache.rs`, `protocol/mod.rs`, `dedicated.rs`, client registry/dispatch |
| 4. Ownership transfer | `OwnershipClaim/Granted/Released` packets + registry owner map. ActorNew grants sender ownership (dedup by ref_id); mutations gated to own player OR sim owner; disconnect releases all owned actors + broadcasts release. Client tracks `owned_actors`; bridge hooks: `Game::claim_ownership()` / `Game::owns()`. | `protocol/mod.rs`, `world/registry.rs`, `handlers/actor.rs`, `handlers/object.rs`, `dedicated.rs`, client `game.rs`/`registry.rs` |

390 tests, 0 warnings. Ownership/delta rules proven in `tests/ownership.rs` (5 handler-level tests); string-cache wire semantics in `string_cache.rs` unit tests + `wire_format.rs`. Next: bridge `events.rs` NPC-spawn reporting → `claim_ownership()` wiring.

| 5. Time/settings/spell/version | `GameTime` (authoritative clock, advances server-side at time_scale, 30-day-month rollover, join-send + change-broadcast — STR CalendarService), `ServerSettings { pvp_enabled }` (config → join broadcast), `SpellCast` (owner-gated relay, STR NotifySpellCast), `GameAuth.version` (reject mismatch, STR AuthenticationRequest). | `protocol/mod.rs`, `dedicated.rs`, `config.rs`, `handlers/auth.rs`, `handlers/actor.rs`, client `game.rs`/`dispatch.rs` |

| 6. PvP enforcement | `pvp_enabled` (config) now enforced in combat: player-on-player hits rejected when off (was broadcast-only/decorative). | `dispatch.rs`, `handlers/combat.rs`, `dedicated.rs` |
| 7. Game clock display | Client stores `GameClock` from GameTime packets, egui top bar shows date/time + scale + ⚔ PvP badge. | client `game.rs`, `dispatch.rs`, `ui/app.rs` |
| 8. Entity streaming | Actors register in their owner's cell; `UpdateContext` enter/leave now streams all entity kinds (Actor/Player/Container/Item/Object) — New on enter, Remove on leave. | `handlers/actor.rs`, `handlers/player.rs` |
| 9. Mod policy | `GameModList` (client load order: filename+crc) verified against server config `mod` entries (STR ModPolicy); mismatch → GameEnd Denied + disconnect. Off when list empty. CRCs are IEEE CRC-32 of the raw file bytes — shared `ashfall_core::crc32` (zlib-verified), and `ashfall-server --list-mod-crc <dir>` prints ready-to-paste config lines (base master first, verified against the real data/ files: Fallout3.esm = C092218B). | `protocol/mod.rs`, `handlers/game.rs`, `config.rs`, `crc32.rs`, `main.rs`, client `config.rs`/`game.rs` |

Admin/ban system: explicitly skipped per project direction.

396 tests, 0 warnings.

P3+P4 can run in parallel (both depend on P2). P6+P7 can run in parallel after P5+P7 foundation ready. P10 can start after P7 IPC module (PR79).

---

## Risks

| Risk | Mitigation |
|------|------------|
| Custom UDP reliability layer is bug-prone | Start with toy ACK; add loss-simulation tests in post-MVP |
| 51 WASM host functions is large surface | Stub all first; fill in by category as needed |
| Client IPC depends on game engine that doesn't exist yet | Stub mode allows full client testing without engine |
| postcard varint may exceed 1200-byte limit for large packets | Wire format tests verify max size for every variant |
| Proton bridge.dll injection fails on some Wine versions | WINEDLLOVERRIDES tested on Proton 9+ / Wine 9+ |
| Cross-compilation of bridge.dll requires MinGW toolchain | CI provides prebuilt DLL; local dev uses stub mode |
| Havok physics VTable hooking untested on Proton/Wine | Start with velocity relay only; add rigid body hooks after basic position sync works |
| Fallout damage formula replication may diverge from game | Integration test against known weapon/actor combos; expose DR/DT as configurable |
| FNV reputation/karma sync not backwards compatible with FO3 | Protocol fields are optional; FO3 clients ignore FNV packets |
| CellSnapshot >1200 bytes for large cells | Split into multi-packet batches post-MVP; MAX_CELL_SNAPSHOT_OBJECTS safety cap |
| NVSE CommandTable registration requires exact offset matching | Detect NVSE version at bridge init; fallback to basic DLL injection |
| Server-authoritative NPC AI latency may cause visible lag | AI package state changes are infrequent; use dead reckoning on client between updates |