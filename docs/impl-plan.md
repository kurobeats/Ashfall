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

## Phase 8: Master Server

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 81 | `ashfall-phase8-pr81-master-crate` | ashfall-master crate skeleton | 30 | `crates/ashfall-master/Cargo.toml`, `src/main.rs` | `cargo build` |
| 82 | `ashfall-phase8-pr82-server-list` | ServerEntry + HashMap registry | 40 | `crates/ashfall-master/src/server_list.rs` | Insert/update/get_all |
| 83 | `ashfall-phase8-pr83-announce-handler` | MasterAnnounce handler | 50 | `crates/ashfall-master/src/announce.rs` | Deserialize announce; upsert server entry |
| 84 | `ashfall-phase8-pr84-query-handler` | MasterQuery handler | 40 | `crates/ashfall-master/src/query.rs` | Serialize all entries; send response |
| 85 | `ashfall-phase8-pr85-cull-loop` | Stale entry culling + main loop | 50 | `crates/ashfall-master/src/cull.rs` | Every 60s remove entries >120s stale |
| 86 | `ashfall-phase8-pr86-server-announce` | Dedicated server master announce heartbeat | 60 | `crates/ashfall-server/src/master.rs` | Every 60s send MasterAnnounce to master addr |
| 87 | `ashfall-phase8-pr87-master-integration-test` | Integration test: master + server + client query | 150 | `tests/master_integration.rs` | Server announces→master lists→client queries→sees server |

**Phase 8 total: ~420 LOC** (depends on PR80)

---

## Phase 9: Polish

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 88 | `ashfall-phase9-pr88-reliability-tune` | ACK retransmit + RTT estimation + ordering fix | 150 | `crates/ashfall-server/src/network.rs`, `crates/ashfall-client/src/network.rs` | Packet loss simulation: 10% loss→reorder→correct |
| 89 | `ashfall-phase9-pr89-pos-interpolation` | Client-side position interpolation (lerp between last two) | 60 | `crates/ashfall-client/src/world/state.rs` | Remote objects move smoothly between ticks |
| 90 | `ashfall-phase9-pr90-file-transfer` | Mod file transfer (TCP channel, CRC32 verify) | 200 | `crates/ashfall-server/src/file_transfer.rs` | File list→CRC check→download→verify |
| 91 | `ashfall-phase9-pr91-bandwidth-monitor` | Per-session byte counters, log stats | 50 | `crates/ashfall-server/src/network.rs` (extend) | `tracing::info!` bytes/sec per session |
| 92 | `ashfall-phase9-pr92-tracing` | Structured tracing throughout: spans for dispatch, handlers, sync | 80 | All `src/` files | `RUST_LOG=debug` shows request flow; spans named |
| 93 | `ashfall-phase9-pr93-graceful-shutdown` | SIGINT→drain sessions→notify master→close DB→exit | 60 | `crates/ashfall-server/src/dedicated.rs` (extend) | No data loss; sessions get GameEnd; master delisted |
| 94 | `ashfall-phase9-pr94-stress-test` | Multi-client stress test harness | 150 | `tests/stress.rs` | 10 clients connect, move, chat for 60s; no crashes, no memory leak |
| 95 | `ashfall-phase9-pr95-docs-readme` | README + developer guide + API docs | 100 | `README.md`, `docs/developing.md` | Build instructions; arch overview; how to write scripts |

**Phase 9 total: ~850 LOC** (depends on PR87)

---

## Phase 10: Proton Bridge

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 96 | `ashfall-phase10-pr96-bridge-crate` | ashfall-bridge crate skeleton + Cargo.toml (target `x86_64-pc-windows-gnu`) | 30 | `crates/ashfall-bridge/Cargo.toml`, `src/lib.rs` | `cargo build --target x86_64-pc-windows-gnu` produces bridge.dll |
| 97 | `ashfall-phase10-pr97-bridge-tcp` | TCP server on 127.0.0.1:1771; accept Ashfall client connection | 80 | `crates/ashfall-bridge/src/network.rs` | Listens on loopback; receives/decodes pipe-protocol commands |
| 98 | `ashfall-phase10-pr98-bridge-hooks` | Gamebryo engine hooks: GetPos, GetAngle, GetActorState, SetPos, SetAngle, etc. | 300 | `crates/ashfall-bridge/src/hooks/` | VTable patches; calls Original→Ashfall; commands modify game state |
| 99 | `ashfall-phase10-pr99-bridge-commands` | Full command dispatcher: all ~80 API opcodes (matching original Interface/API) | 250 | `crates/ashfall-bridge/src/commands.rs` | Every opcode (GetPos, GetAngle, SetPos, GetActorValue, etc.) handled |
| 100 | `ashfall-phase10-pr100-client-tcp-ipc` | IPC client connects via TCP to bridge (replaces stub in pr79) | 80 | `crates/ashfall-client/src/ipc/mod.rs` (extend) | TCP connect to 127.0.0.1:1771; send/recv works; stub fallback on failure |
| 101 | `ashfall-phase10-pr101-proton-integration-test` | End-to-end Proton test: start server + bridge stub + client | 150 | `tests/proton_integration.rs` | Mock bridge responds to TCP commands; client polls position→sends to server |
| 102 | `ashfall-phase10-pr102-proton-docs` | Proton setup guide + CI cross-compile workflow | 60 | `docs/proton-setup.md`, `.github/workflows/bridge.yml` | Step-by-step for Steam Deck + desktop; CI produces bridge.dll artifact |

**Phase 10 total: ~950 LOC** (depends on PR79, PR80)

---

## Summary

| Phase | PRs | Est LOC | Depends On | Key Additions |
|-------|-----|---------|------------|---------------|
| Phase 1: Core Protocol | 1–17h | 2,170 | — | ✅ DONE. Physics, combat, quest, AI, FNV packets + FormID |
| Phase 2: Server Foundation | 18–29 | ~2,030 | PR17h | ✅ DONE. NPC AI manager, combat resolver, physics validator, quest manager |
| Phase 3: World Sync | 30–39 | ~1,690 | PR29 | ✅ DONE. Physics, combat, projectile, NPC AI, door/terminal handlers |
| Phase 4: Persistence | 40–47 | ~800 | PR29 | ✅ DONE. Quest stages, dialogue flags, karma, reputation, hardcore, faction tables |
| Phase 5: Scripting | 48–59 | ~2,420 | PR47 | ✅ DONE. 50+ host fns, WASM engine, timers, SDK crate |
| Phase 6: GUI | 60–67 | ~1,120 | PR59 | eframe app, server browser, chat overlay, widgets, IPC wired | ✅ |
| Phase 7: Client | 68–80 | ~1,770 | PR67 | UDP networking, connection flow, client registry, handlers, background poll | ✅ |
| Phase 8: Master Server | 81–87 | 420 | PR80 | (no changes) |
| Phase 9: Security + Testing | 88–97 | ~1,610 | PR87 | Anti-cheat, movement+combat+quest+cell tests, stress tests |
| Phase 10: Proton Bridge | 98–107 | ~2,650 | PR79, PR80 | Physics/combat/AI/dialogue/quest/FNV hooks, NVSE integration, event sinks |
| **Total** | **~102** | **~16,680** | | |

P3+P4 can run in parallel (both depend on P2). P6+P7 can run in parallel after P5+P7 foundation ready. P10 can start after P7 IPC module (PR79).

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