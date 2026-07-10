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

## Phase 5: Scripting ✅ DONE

**Implemented:**
- wasmtime v22 Engine + ScriptState + module loader + instance lifecycle
- 35 callback stubs (OnHit, OnEquip, OnQuestStage, OnDialogueChoice + 31 original)
- 51 host function stubs (server, object, item, actor, player, container, world, utility, timers, quest, combat, GUI widgets)
- TimerManager with create_timer/kill_timer/tick, wired into dedicated loop
- ashfall-script SDK crate with host_fn!/callback! macros and type aliases
- Example freeroam WASM game mode (scripts/freeroam/)
- Auth callback stub, 14 integration tests (engine creation, module loading, callback stubs, timer lifecycle)
- Integrated into DedicatedServer::new() — scripts loaded at startup

**Phase 5 total: ~1,500 LOC** ✅

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

## Phase 10: Proton Bridge ⚠️ DEFERRED

**Implemented:**
- `crates/ashfall-bridge/src/lib.rs` — DllMain entry point
- `crates/ashfall-bridge/src/network.rs` — TCP server on 127.0.0.1:1771
- `crates/ashfall-bridge/src/commands.rs` — 5 opcodes (GetPos/SetPos/GetAngle/SetAngle)
- `crates/ashfall-bridge/src/hooks/mod.rs` — 40+ hook stubs for all categories

**Deferred (requires Gamebryo RE):**
- VTable offset discovery for Fallout 3 1.7 / FNV 1.4
- Actual VTable patching inside Wine/Proton process
- NVSE CommandTable registration
- Event sink callbacks (OnHit, OnActivate, OnDeath)
- Console command interception
- Havok bhkRigidBody hooking

**Known offsets documented in hooks/mod.rs.** See `https://github.com/ianpatt/fose/blob/master/common/GameAPI.cpp`

**Phase 10 total: stubs complete; VTable patches deferred to post-MVP**

---

## Summary

| Phase | PRs | Est LOC | Key Additions |
|-------|-----|---------|---------------|
| Phase 1: Core Protocol | 1–17h | 2,170 | ✅ DONE. 140+ packets, FormID, physics, combat, quest, AI, FNV, bridge hooks |
| Phase 2: Server Foundation | 18–29 | ~2,030 | ✅ DONE. Config, UDP + reliability, sessions, registry, dispatch, combat resolver, AI, physics |
| Phase 3: World Sync | 30–39 | ~1,690 | ✅ DONE. Cell grid, position/angle/actor/item sync, combat, projectile, NPC AI, cell snapshot |
| Phase 4: Persistence | 40–47 | ~800 | ✅ DONE. 17 SQLite tables, CRUD, startup load, quest/karma/reputation/hardcore/factions |
| Phase 5: Scripting | 48–59 | ~1,500 | ✅ DONE. wasmtime v22, 35 callbacks, 51 host fns, timers, example script, 14 tests |
| Phase 6: GUI | 60–67 | ~1,120 | ✅ DONE. eframe/egui app, server browser, chat overlay, widget manager |
| Phase 7: Client | 68–80 | ~1,770 | ✅ DONE. UDP networking, connection flow, object cache, handlers, 30Hz poll loop |
| Phase 8: Master Server | 81–87 | 420 | ✅ DONE. Announce/query/cull, server heartbeat, client query, 6 integration tests |
| Phase 9: Security + Testing | 88–97 | ~1,610 | ✅ DONE. AntiCheat validator, 48 tests (AC, combat, stress, world_sync) |
| Phase 10: Proton Bridge | 98–107 | ~2,650 | ⚠️ DEFERRED. TCP server + hook stubs done. VTable patches need Gamebryo RE. |
| **Total** | **~102** | **~16,680** | |

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