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

### Phase 11: SkyrimTogetherReborn reuse (items 1–10) ✅

| Item | What shipped | Files |
|------|-------------|-------|
| 1. Address library / thiscall | `AutoPtr` (lazy cached address), `select_candidate` (prologue-signature build selection), `call_thiscall_0..3` (x86 thiscall at explicit addr, edx preserved; x64 `extern "system"` fallback). `fo3_lookup_addr` refactored onto it. | `crates/ashfall-bridge/src/hooks/address.rs`, `hooks/vtable.rs` |
| 2. Differential state | `ActorStateDelta` — presence-optional actor state (STR Differential.h pattern): one packet per state burst, receiver merges. Server applies + relays owner-gated; client merges into `ClientObject::Actor`. | `protocol/mod.rs`, `handlers/actor.rs`, client `world/registry.rs` |
| 3. StringCache | `StringTable` + `CachedString` (Plain/Inline{id,value}/Id). Server assigns ids per-session, `Packet::finalize_strings` binds in the send path (dedicated.rs `send()`), repeats go out as 2-byte ids. Wired into ObjectNew/UpdateName/UpdateActorIdle/GameChat/GameMessage/UpdateInterior. | `ashfall-core/src/string_cache.rs`, `protocol/mod.rs`, `dedicated.rs`, client registry/dispatch |
| 4. Ownership transfer | `OwnershipClaim/Granted/Released` packets + registry owner map. ActorNew grants sender ownership (dedup by ref_id); mutations gated to own player OR sim owner; disconnect releases all owned actors + broadcasts release. Client tracks `owned_actors`; bridge hooks: `Game::claim_ownership()` / `Game::owns()`. | `protocol/mod.rs`, `world/registry.rs`, `handlers/actor.rs`, `handlers/object.rs`, `dedicated.rs`, client `game.rs`/`registry.rs` |

483 tests, 0 warnings. Ownership/delta rules proven in `tests/ownership.rs` (5 handler-level tests); string-cache wire semantics in `string_cache.rs` unit tests + `wire_format.rs`. Next: bridge `events.rs` NPC-spawn reporting → `claim_ownership()` wiring.

| 5. Time/settings/spell/version | `GameTime` (authoritative clock, advances server-side at time_scale, 30-day-month rollover, join-send + change-broadcast — STR CalendarService), `ServerSettings { pvp_enabled }` (config → join broadcast), `SpellCast` (owner-gated relay, STR NotifySpellCast), `GameAuth.version` (reject mismatch, STR AuthenticationRequest). | `protocol/mod.rs`, `dedicated.rs`, `config.rs`, `handlers/auth.rs`, `handlers/actor.rs`, client `game.rs`/`dispatch.rs` |

| 6. PvP enforcement | `pvp_enabled` (config) now enforced in combat: player-on-player hits rejected when off (was broadcast-only/decorative). | `dispatch.rs`, `handlers/combat.rs`, `dedicated.rs` |
| 7. Game clock display | Client stores `GameClock` from GameTime packets, egui top bar shows date/time + scale + ⚔ PvP badge. | client `game.rs`, `dispatch.rs`, `ui/app.rs` |
| 8. Entity streaming | Actors register in their owner's cell; `UpdateContext` enter/leave now streams all entity kinds (Actor/Player/Container/Item/Object) — New on enter, Remove on leave. | `handlers/actor.rs`, `handlers/player.rs` |
| 9. Mod policy | `GameModList` (client load order: filename+crc) verified against server config `mod` entries (STR ModPolicy); mismatch → GameEnd Denied + disconnect. Off when list empty. CRCs are IEEE CRC-32 of the raw file bytes — shared `ashfall_core::crc32` (zlib-verified), and `ashfall-server --list-mod-crc <dir>` prints ready-to-paste config lines (base master first, verified against the real data/ files: Fallout3.esm = C092218B). | `protocol/mod.rs`, `handlers/game.rs`, `config.rs`, `crc32.rs`, `main.rs`, client `config.rs`/`game.rs` |
| 10. Bridge event pipeline (coop loop) | Length-prefixed pipe framing (`ashfall_core::event` — responses + events share the TCP stream unambiguously), bridge event queue + push in the connection loop, `OP_REPORT_PLAYER_STATE` debug reporter (samples the local player via the vtable getters → EVENT_PLAYER_STATE), client `IpcClient` event buffering + `poll_events`, client `sync.rs` (events→packets: UpdatePos/Angle/ActorStateDelta/health + NPC-spawn→ActorNew+claim; packets→commands: remote UpdatePos/Angle→OP_SET_POS/ANGLE), wired into the client poll loop. Real-TCP tests on both ends. | `ashfall-core/event.rs`, bridge `network.rs`/`commands.rs`, client `ipc/*`, `sync.rs`, `game.rs`, `main.rs` |

Remaining (needs game host — steam-re.md): live verification of the discovery detours (GOG 0x6FAE90 + Steam 0x7F9B70 — probe-verify before patching, FLAT/+0xC00 caveat), the per-frame game-loop hooks, the vtable-call ops on Steam (GetActorValue/State/AnimData — early vtable region reordered, OP_PROBE_FORM will read the slots), the remaining Steam patch-site groups (fire_fix, match_race, place_at_me, ai_fix2/3/4, play_idle_fix — all need live-probe), and behavior-verifying the mapped sites + hooks in-game. get_activate fully solved (jmp 0x8D3BC8 + ret 0x8D3CB8). Remaining bridge OP stubs (engine-bound, not field-writable): OP_SET_NAME (SetName vtable slot unmapped), OP_PLAY_SOUND engine call, OP_PLACE_AT_ME (engine spawn fn). Load-order bridge reporting explicitly deprioritized — MVP targets vanilla games.

| 17. Steam vtable re-derivation (2026-08-14c) | Steam PC vtable base found (0xF938FC, verified via AI-pred +0x22C and death-handler +0x23C); GOG base 0xE16B10. Slot translation by byte-identical matching: 41 slots, 59% fit a +0x58 shift (recompile inserted 22 early entries). Early region (+0x00..0x68) REORDERED — GetActorValue/BaseValue/AnimData need live probe. GET_LOCKED confirmed: GOG +0xA0 -> Steam +0xFC (byte-identical `8a 41 0a 24 01 c3`). Wired `fo3_steam_vtable` + `steam_slot_for()` into `get_lock` (byte-guarded, GOG fallback). Online: no public Anniversary vtable exists (kFOSE checked — classic only). | bridge `vtable.rs`, scripts/re/vtable_steam.py, docs/steam-re.md |
| 18. Vaultmp hook framework (2026-08-14d) | The 8 REQUIRED_HOOKS (respawn_detour, bethesda_delegator, play_idle_detour, anim_detour, av_fix, get_activate, place_at_me, fire_weapon) were referenced by the recipe table but never implemented — the full vaultmp behavior-patch pipeline was dead code. Implemented the hook registry (name → thunk addr) + 8 register-preserving x86 thunks (PUSHAD/POPAD, collector call) + Rust collectors. get_activate/fire_weapon collectors read the object refID (+0x0C) and emit new EVENT_ACTIVATE (11) / EVENT_FIRE (12) pipe events; the client relays them as UpdateActivate / UpdateFireWeapon packets. `apply_classic_vaultmp()` wires the full 34-recipe set into `install()` (byte-guarded, no-op on Steam). Fixed pre-existing x86-build blocker: `select_candidate`'s unsafe prologue read lacked an unsafe block. 483 tests, x86 cross-check clean. | bridge `vaultmp.rs`/`mod.rs`/`address.rs`, core `event.rs`, client `sync.rs` |

| 19. Relay completion — no more stub OP handlers (2026-08-14e) | The activate/fire/cell/enabled/move/scale/lock/sound relay paths completed end-to-end. Client `packets_to_commands` now maps remote UpdateActivate → OP_GET_ACTIVATE, UpdateFireWeapon → OP_FIRE_WEAPON, UpdateLock → OP_SET_LOCK, UpdateScale → OP_SET_SCALE, UpdateSound → OP_PLAY_SOUND (the server relayed these but receivers ignored them). OP_FIRE_WEAPON calls the engine fire routine via thiscall (per-build byte-guarded: classic 0x4BE1A0 / Steam 0x770880); OP_GET_ACTIVATE confirms the ref resolves (safe field lookup); OP_SET_CELL (parent cell +0x3C/0x40), OP_SET_ENABLED (+0x50/0x54 bit 0x02), OP_SET_LOCK (byte +0xA bit 0 — the verified lock getter's field), OP_SET_SCALE (+0x38/0x3C), OP_MOVE_TO (20-byte params) are all raw field writes (Steam-safe). New opcodes: OP_SET_SCALE 0x2B (moved off the 0x15 collision with OP_PLAY_SOUND — caught by a new opcode-uniqueness test). | bridge `commands.rs`/`vtable.rs`/`mod.rs`, client `sync.rs`/`ipc` |
| 20. Field-based getters replace stubs (2026-08-14f/h) | get_scale/set_scale were stubs (returned 1.0) — now field reads/writes (FO3 +0x38, FNV +0x3C, immediately after the pos triple). is_dead was a TODO returning false — now reads the death-state field Actor+0xFC (survived the Steam recompile; the respawn handler does `cmp eax,2` there, AI predicate checks cmp 5/3). Field reads, no vtable calls — Steam-safe. **2026-08-14h (gh crawl):** get_actor_state's alerted/sneaking (previously hardcoded false) now call the classic getters (alerted 0x6F6C70 = [this+0x60] vtable +0x450; sneaking 0x6F58B0 = [this+0x184] vtable +0x20 — from the Anniversary-Patcher catalog); byte-guarded, Steam no-ops. Added SET_POS 0x6F2050 / QUEUE_UI_MESSAGE 0x61B850 to fo3_17. | bridge `vtable.rs`/`mod.rs` |
| 21. Wire-format audit (2026-08-14g) | Deep audit: all defined opcodes have handlers; all ~60 packet variants referenced; GameMessage/UpdateInterior/UpdateActorIdle relayed-but-unproduced (idle sync flows via ActorStateDelta's idle field, chat via GameChat — consistent). Server combat self-contained (stored actor values, not bridge stubs). Client owns()/claim_ownership()/send_spell_cast() are documented future hooks (server enforces ownership via can_mutate; NPC spawn → OwnershipClaim already wired through sync.rs). Closed the last wire-format test gap: UpdateName + UpdateActorIdle round-trip tests. 483 tests. | ashfall-core/tests/wire_format.rs |

| 16. vcdiff breakthrough (2026-08-14) | Dead-end Steam patch sites solved via **FalloutAnniversaryPatcher's `patch_steam.vcdiff`** (online find): the downgrade delta encodes every byte that survived the recompile as a CPY_0 instruction (classic target ↔ Steam source). Decoded with the pristine Steam exe (f3_1704_steam, SHA1-verified; its .text is byte-identical to our flat dump, +0xC00) → classic_out.exe (f3_1703_mod, SHA1-verified). `printdelta` trap: Offset/S@ columns are DECIMAL. **63,616 verified byte-identical runs** (`vcdiff_map5.py`). New EXACT-cover sites wired: **ai_fix1 → 0x5E99E2**, **get_activate_jmp → 0x8D3BC8** (vtable slot shifted 0x224→0x100), **delegator stub spot → 0x405E69/0x405E6A**, **play_group_fix → 0x4350F9**; lock_fix + play_idle_call_src confirmed. Gap-based translations and generic SEH-prologue matches are false positives (proven: ai_predicate gap ≠ real 0x7F9B70). | bridge `vaultmp.rs`, scripts/re/vcdiff_map5.py, docs/steam-re.md |
| 15. Steam AI predicate re-derived | The actor-discovery detour now covers the Steam/Anniversary build: `cmp [reg+0xFC],5/3` fingerprint scan on the flat dump found the AI predicate at **0x7F9B70** (entry `55 8B EC 51 57 8B F9`, structurally identical to classic — same `[edi+0xF8]`/`[edi+0xFC]` checks, player compare vs Steam PlayerCharacter singleton 0x123C674, shared vtable slot +0x22C). `ai_predicate_site()` picks classic vs Steam by prologue signature. The vaultmp ai_fix recipe twins live inside it (derivable once live-probed). | bridge `vaultmp.rs`, docs/steam-re.md |
| 14. Coop-loop integration test + client fixes | `coop_loop.rs` (client): mock bridge → client A → real server → client B → mock bridge, proving the whole vanilla-coop pipeline in CI (A's engine event becomes server packets, relays to B, B applies them as engine commands — OP_SET_POS/ANGLE/ACTOR_VALUE captured). Found + fixed two real client bugs: (1) `send_seq` was bumped for unreliable sends (no seq on the wire) → holes in the reliable sequence space → server reassembly stalled; (2) `IpcClient::poll_events` never read the transport — events only arrived inside `execute()`; now try_reads + ingests. | client `network.rs`/`ipc`/`coop_loop.rs` |
| 13. FO3 frame hook + downgrade path | FO3 classic frame hook wired (0x6EEB2F → `call 0x6E3E40`, no-arg cdecl, per-frame; guard e8 0c 53 ff ff) → 10 Hz `report_player_state_due`. Online research: **no public Anniversary/2023 FO3 address table exists** (FOSE repo deleted); the community solution is FalloutAnniversaryPatcher (c6-dev + lStewieAl) — downgrade the exe to 1.7.0.3 (our GOG exe SHA1 matches `f3_1703_gog` exactly). **Decision: we do NOT downgrade** — the Steam/Anniversary build gets per-site re-derivation instead (AI predicate done in item 15). See steam-re.md. | bridge `vaultmp.rs`, docs/steam-re.md |
| 12. FNV mapping + frame hook | FNV static session: PlayerCharacter singleton 0x011DEA3C (NVSE), main-loop frame hook 0x86B386 (NVSE kMainLoopHookPatchAddr — wired as `apply_fnv_frame_hook()`: byte-guarded redirect → original getter + 10 Hz `report_player_state_due` + FNV actor discovery), HighProcess 0x8EEEC0 identified. **FNV NPC discovery solved via AnhNVSE's `ActorProcessManager` docs** (tList tiers at 0x011E0E80, first tier @ +0x00 confirmed on-binary): the frame hook enumerates the list → collector → diff → spawn/remove events. No Steam/GOG split (single 1.4.0.525 build). See steam-re.md. | bridge `vaultmp.rs`/`discovery.rs`, docs/steam-re.md |
| 11. Owned-NPC sync halves | **Owned-NPC reporting**: `OP_TRACK_ACTOR`/`OP_UNTRACK_ACTOR` (0x00F6/0x00F5) + bridge tracked set; the 10 Hz flush samples tracked refs → `EVENT_NPC_STATE` (same layout as player-state, new tag) → client → `UpdatePos/Angle/ActorStateDelta/UpdateActorValue` for the ref-derived entity id. Client queues TRACK on `OwnershipGranted`, UNTRACK on `OwnershipReleased`. **Remote application**: `packets_to_commands` now also maps `UpdateActorValue` → `OP_SET_ACTOR_VALUE` (with 1-byte index, new `Param::U8`) and `UpdateActorDead` → `OP_KILL`. | bridge `network.rs`/`commands.rs`, core `event.rs`, client `sync.rs`/`game.rs`/`ipc` |

Admin/ban system: explicitly skipped per project direction.

**483 tests, 0 warnings** (2026-08-13).

2026-08-14: live-collector tests serialized (shared CURRENT/LAST statics raced in
parallel); repo-wide clippy/lint cleanup (map_or → is_none_or/is_some_and,
deadlock-safe connect threads, send-without-read-lock, Default impls). No new
tests. Infra note: game-host OOM (4.8GB probe-side python) killed the tmux/pi
session — no work lost, checkpoint committed.

2026-08-14 (second pass): Steam patch-site re-derivation via the
FalloutAnniversaryPatcher vcdiff — 4 new EXACT-cover sites wired (ai_fix1,
get_activate_jmp, delegator stub spot, play_group_fix; see item 16), 1 new
test. Also verified the **Steam FNV download** against GOG: same binary at
runtime (sections/layout identical, .data/.rsrc/.reloc byte-identical; only
.8diff: .text SteamStub-encrypted on disk + steam_api.dll vs GalaxyWrp.dll
import + .bind unpacker) → **fnv_14 table applies to Steam unchanged, no
re-derivation needed**. 483 tests, 0 warnings.

2026-08-14 (d): vaultmp hook framework — the 8 REQUIRED_HOOKS implemented
(resolver + x86 thunks + collectors), EVENT_ACTIVATE/EVENT_FIRE pipe events
+ client relay (UpdateActivate/UpdateFireWeapon), apply_classic_vaultmp()
wired into install() (byte-guarded, no-op on Steam). Fixed the pre-existing
x86-build blocker in select_candidate (unsafe block). +4 tests (472 total),
x86 cross-check clean. Answer to "is it purely live testing now": no — the
whole vaultmp behavior-patch pipeline was dead code; the hooks + wiring were
implementable without the game (only the per-site live probe + Steam AV/anim
vtable slots remain game-host work).

2026-08-14 (e): relay completion — client packets_to_commands maps remote
UpdateActivate/UpdateFireWeapon/UpdateLock/UpdateScale/UpdateSound →
OP_GET_ACTIVATE/OP_FIRE_WEAPON/OP_SET_LOCK/OP_SET_SCALE/OP_PLAY_SOUND;
OP_FIRE_WEAPON calls the engine fire routine (per-build byte-guarded);
OP_SET_CELL/OP_SET_ENABLED/OP_SET_LOCK/OP_SET_SCALE/OP_MOVE_TO are raw
field writes (Steam-safe). Caught + fixed an opcode collision (SET_SCALE
was 0x15 = PLAY_SOUND; moved to 0x2B) with a new uniqueness test.

2026-08-14 (f): field-based getters — get_scale/set_scale (FO3 +0x38 /
FNV +0x3C) and is_dead (Actor+0xFC death-state field) replaced stubs;
both Steam-safe raw field reads/writes, no vtable calls.

2026-08-14 (g): wire-format audit — closed the last test gap (UpdateName +
UpdateActorIdle round-trip); confirmed all opcodes handled, all packets
referenced, server combat self-contained, dead client methods are
documented future hooks. 483 tests, 0 warnings.

2026-08-14 (h): gh crawl — found Project Crossroads' Anniversary-Patcher
catalog (the full vaultmp site table + 8 engine entry points, byte-
verified), independently confirming the fo3_17 classic table. Wired
alerted/sneaking getters (0x6F6C70/0x6F58B0) into get_actor_state
(classic, byte-guarded); added SET_POS 0x6F2050 + QUEUE_UI_MESSAGE
0x61B850 to fo3_17. Community dead ends reconfirmed (no public Anniversary
vtable; downgrade + classic table is the only public path).

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