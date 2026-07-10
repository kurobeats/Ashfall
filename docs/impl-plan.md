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

| PR | Branch | Task | Est LOC | Status |
|----|--------|------|---------|--------|
| 1-6 | (merged) | Workspace, types, constants, NetworkID, math, channel/header | 290 | ✅ |
| 7-16 | (merged) | All Packet variants: system, object, item, container, actor, player, window, master | 820 | ✅ |
| 17 | (merged) | Wire format round-trip tests (71 tests) | 400 | ✅ |
| 17a | `phase1-pr17a-physics` | Physics packets: UpdateVelocity | 50 | ✅ |
| 17b | `phase1-pr17b-combat` | Combat packets: ActorHit, ActorDamaged, ActorDeathExt, Projectile, Explosion | 100 | ✅ |
| 17c | `phase1-pr17c-quest` | Quest+dialogue packets: QuestStage, DialogueFlag, DialogueChoice | 80 | ✅ |
| 17d | `phase1-pr17d-ai-world` | NPC AI + world state: ActorCombatTarget, ActorAIPackage, ActorFaction, DoorState, TerminalState | 80 | ✅ |
| 17e | `phase1-pr17e-scale-globals` | Scale field on ObjectNew/ItemNew/ActorNew/PlayerNew, FO3/FNV globals: KarmaUpdate, ReputationUpdate, HardcoreStats | 70 | ✅ |
| 17f | `phase1-pr17f-formid` | FormID type + FormIDSync + CellSnapshot packet | 100 | ✅ |
| 17g | `phase1-pr17g-bridge-hooks` | Bridge hooks expanded: 40+ stubs for physics, combat, AI, quest, FNV, NVSE | 150 | ✅ |
| 17h | `phase1-pr17h-channel-routing` | Channel routing for new packets + is_unreliable() helper + anti-cheat constants | 30 | ✅ |

**Phase 1 total: ~2,170 LOC** ✅

---

## Phase 2: Server Foundation

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 18 | `ashfall-phase2-pr18-server-crate` | ashfall-server crate skeleton + Cargo.toml | 30 | `crates/ashfall-server/Cargo.toml`, `src/main.rs` | `cargo build` works; depends on ashfall-core |
| 19 | `ashfall-phase2-pr19-config` | Server config parsing (ini-style) | 80 | `crates/ashfall-server/src/config.rs` | Parse port, host, announce addr, script path, db path |
| 20 | `ashfall-phase2-pr20-udp-socket` | UDP socket bind + send/recv helpers | 80 | `crates/ashfall-server/src/network.rs` | Bind to configured port; tokio UdpSocket |
| 21 | `ashfall-phase2-pr21-session` | Session struct: guid, addr, player_id, state enum | 60 | `crates/ashfall-server/src/session.rs` | State machine: Connecting→Auth→Loading→InGame→Disconnecting |
| 22 | `ashfall-phase2-pr22-object-registry` | ObjectRegistry: DashMap insert/get/remove/is_deleted | 120 | `crates/ashfall-server/src/world/registry.rs` | Concurrent insert+get+remove; type_counts; deleted tombstone |
| 23 | `ashfall-phase2-pr23-object-structs` | Server-side Object/Item/Container/Actor/Player structs | 150 | `crates/ashfall-server/src/world/objects.rs` | All data structs match arch doc; Clone+Debug |
| 24 | `ashfall-phase2-pr24-dispatch` | Packet dispatch: match packet → handler function | 100 | `crates/ashfall-server/src/dispatch.rs` | Match all Packet variants; route to handler stubs |
| 25 | `ashfall-phase2-pr25-auth-handler` | Auth handler: validate, call script callback stub, create session | 80 | `crates/ashfall-server/src/handlers/auth.rs` | Name/pwd validated; GameAuth→GameLoad flow |
| 26 | `ashfall-phase2-pr26-connection-flow` | Full connect→auth→load→ingame lifecycle | 200 | `crates/ashfall-server/src/handlers/game.rs`, `src/handlers/player.rs` | Server sends weather/globals/time/deleted; creates player; sends PlayerNew to existing |
| 27 | `ashfall-phase2-pr27-main-loop` | Main server loop: tick + recv select | 100 | `crates/ashfall-server/src/dedicated.rs` | 30Hz tick; UDP recv; dispatch; session cull |
| 28 | `ashfall-phase2-pr28-cli` | CLI entry: parse args, load config, start server | 60 | `crates/ashfall-server/src/main.rs` | `--config` flag; `--port` override; graceful shutdown on SIGINT |
| 29 | `ashfall-phase2-pr29-server-integration-test` | Integration test: start server, connect mock client, auth flow | 200 | `crates/ashfall-server/tests/auth_flow.rs` | Mock client sends GameAuth, receives GameLoad; timeout-based |

**Phase 2 total: ~1,260 LOC** (depends on PR17)

---

## Phase 3: World Sync

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 30 | `ashfall-phase3-pr30-cell-system` | Cell struct, cell neighbors, cell context (9-grid) | 100 | `crates/ashfall-server/src/world/cell.rs` | neighbors() returns 8; context update diff correct |
| 31 | `ashfall-phase3-pr31-cell-registry` | cell_refs DashMap, get_by_cell, get_by_kind | 70 | `crates/ashfall-server/src/world/registry.rs` (extend) | O(1) cell→objects; insert auto-registers cell |
| 32 | `ashfall-phase3-pr32-object-handler` | Handle ObjectNew, ObjectRemove, UpdatePos, UpdateAngle | 120 | `crates/ashfall-server/src/handlers/object.rs` | Auth state updated; fanout to cell context players |
| 33 | `ashfall-phase3-pr33-cell-sync` | UpdateCell + UpdateName + UpdateLock + UpdateOwner handlers | 80 | `crates/ashfall-server/src/handlers/object.rs` (extend) | Cell change → recompute enter/leave; send diff to affected players |
| 34 | `ashfall-phase3-pr34-actor-handler` | ActorNew, UpdateActorState, UpdateActorValue, UpdateActorDead | 150 | `crates/ashfall-server/src/handlers/actor.rs` | Values validated; state broadcasts to cell; death callback stub |
| 35 | `ashfall-phase3-pr35-actor-race-sex` | UpdateActorRace, UpdateActorSex handlers | 60 | `crates/ashfall-server/src/handlers/actor.rs` (extend) | Race/age/sex changes broadcast |
| 36 | `ashfall-phase3-pr36-item-handler` | ItemNew, UpdateItemCount, UpdateItemCondition, UpdateItemEquipped | 100 | `crates/ashfall-server/src/handlers/item.rs` | Container linkage validated; silent flag respected |
| 37 | `ashfall-phase3-pr37-container-handler` | ContainerNew, ItemListNew handlers | 70 | `crates/ashfall-server/src/handlers/object.rs` (extend or new `container.rs`) | ItemList items validated against registry |
| 38 | `ashfall-phase3-pr38-weather-globals` | Weather + Globals handlers | 60 | `crates/ashfall-server/src/world/weather.rs`, `src/world/globals.rs` | Set/get weather; global map read/write; broadcast on change |
| 39 | `ashfall-phase3-pr39-sync-integration-test` | World sync integration test | 200 | `crates/ashfall-server/tests/world_sync.rs` | Create objects, move, verify position broadcasts to cell neighbors |

**Phase 3 total: ~1,010 LOC** (depends on PR29)

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
- `crates/ashfall-server/src/script/engine.rs` — wasmtime Engine, ScriptState, module loader, instance lifecycle
- `crates/ashfall-server/src/script/host.rs` — 50+ host functions (server, object, item, actor, player, container, world, utility, timers, quest, combat, GUI widgets)
- `crates/ashfall-server/src/script/timer.rs` — TimerManager with create_timer/kill_timer/tick
- `crates/ashfall-script/src/lib.rs` — SDK crate with host_fn!/callback! macros, type aliases
- Integrated into `DedicatedServer::new()` — scripts loaded at startup, `OnServerInit` called

**PRs:**

| PR | Task | Status |
|----|------|--------|
| 48 | ashfall-script SDK crate + macros | ✅ |
| 49 | wasmtime Engine + module loading | ✅ |
| 50–55 | Host functions: object, item, actor, container, player, world, quest, combat, GUI, utility | ✅ |
| 56 | Timer system | ✅ |
| 57–59 | Auth callback, example script, integration test | Deferred (scripts dir + callback wiring) |

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

| Phase | PRs | Est LOC | Depends On | Key Additions |
|-------|-----|---------|------------|---------------|
| Phase 1: Core Protocol | 1–17h | 2,170 | — | ✅ DONE. Physics, combat, quest, AI, FNV packets + FormID |
| Phase 2: Server Foundation | 18–29 | ~2,030 | PR17h | ✅ DONE. NPC AI manager, combat resolver, physics validator, quest manager |
| Phase 3: World Sync | 30–39 | ~1,690 | PR29 | ✅ DONE. Physics, combat, projectile, NPC AI, door/terminal handlers |
| Phase 4: Persistence | 40–47 | ~800 | PR29 | ✅ DONE. Quest stages, dialogue flags, karma, reputation, hardcore, faction tables |
| Phase 5: Scripting | 48–59 | ~2,420 | PR47 | ✅ DONE. 50+ host fns, WASM engine, timers, SDK crate |
| Phase 6: GUI | 60–67 | ~1,120 | PR59 | ✅ DONE. eframe app, server browser, chat overlay, widgets, IPC wired | ✅ |
| Phase 7: Client | 68–80 | ~1,770 | PR67 | ✅ DONE. UDP networking, connection flow, client registry, handlers, background poll | ✅ |
| Phase 8: Master Server | 81–87 | 420 | PR80 | (no changes) | ✅ DONE. |
| Phase 9: Security + Testing | 88–97 | ~1,610 | PR87 | AntiCheat module, 48 new tests (anti_cheat, combat, stress, world_sync) | ✅ DONE. |
| Phase 10: Proton Bridge | 98–107 | ~2,650 | PR79, PR80 | Hook stubs complete. VTable patches deferred. | ⚠️ Deferred |
| **Total** | **~102** | **~16,680** | | |

P3+P4 can run in parallel (both depend on P2). P6+P7 can run in parallel after P5+P7 foundation ready. P10 can start after P7 IPC module (PR79).

---

## Risks

| Risk | Mitigation |
|------|------------|
| Custom UDP reliability layer is bug-prone | Start with toy ACK; add loss-simulation tests early (PR88) |
| ~160 WASM host functions is large surface | Code-gen from YAML spec file; phase PR50–55 break into groups |
| ObjectRegistry transmute in `get<T>()` is unsafe | Lock down with integration tests; consider `AnyMap` alternative if crashes |
| Client IPC depends on game engine that doesn't exist yet | Stub mode (PR79) allows full client testing without engine |
| postcard varint may exceed 1200-byte limit for large packets | PR17 wire test verifies max size for every variant |
| Phase ordering: GUI (P6) needs client crate (P7) for egui rendering | P6 handlers are server-side; P7 client crate created before P6 egui rendering PRs |
| Proton bridge.dll injection fails on some Wine versions | `WINEDLLOVERRIDES` tested on Proton 9+ / Wine 9+; VTable hooking same on Wine as Windows |
| Cross-compilation of bridge.dll requires MinGW toolchain | CI provides prebuilt DLL; local dev uses stub mode (no MinGW required) |
| Havok physics VTable hooking untested on Proton/Wine | Start with velocity relay only; add rigid body hooks after basic position sync works |
| Fallout damage formula replication may diverge from game | Integration test against known weapon/actor combos; expose DR/DT as configurable |
| FNV reputation/karma sync not backwards compatible with FO3 | Protocol fields are optional; FO3 clients ignore FNV-specific packets; game type detected at bridge init |
| CellSnapshot >1200 bytes for large cells | Split into multi-packet batches in Phase 9; MAX_CELL_SNAPSHOT_OBJECTS constant as safety cap |
| NVSE CommandTable registration requires exact offset matching | Detect NVSE version at bridge init; fallback to basic DLL injection without NVSE features |
| Server-authoritative NPC AI latency may cause visible lag | AI package state changes are infrequent; use dead reckoning on client between updates |