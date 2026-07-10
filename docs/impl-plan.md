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

## Phase 1: Core Protocol

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 1 | `ashfall-phase1-pr1-workspace-core` | Workspace + ashfall-core skeleton | 50 | `Cargo.toml`, `crates/ashfall-core/Cargo.toml`, `crates/ashfall-core/src/lib.rs` | `cargo build` passes, re-exports correct |
| 2 | `ashfall-phase1-pr2-types` | ObjectKind enum + composite masks + GameObject trait | 80 | `crates/ashfall-core/src/types.rs` | All bitmask values match architecture doc; `is_kind()` works |
| 3 | `ashfall-phase1-pr3-constants` | Constants: MAX_PLAYER_NAME, ports, channels, version | 40 | `crates/ashfall-core/src/constants.rs` | Values match requirements doc |
| 4 | `ashfall-phase1-pr4-id` | NetworkID newtype over u64 | 30 | `crates/ashfall-core/src/id.rs` | `From<u64>`, `Display`, `Serialize`, `Deserialize`; Eq+Hash |
| 5 | `ashfall-phase1-pr5-math` | VaultVector, IsValidCoordinate, IsValidAngle | 60 | `crates/ashfall-core/src/math.rs` | NaN/inf rejected; bounds enforced |
| 6 | `ashfall-phase1-pr6-channel-header` | Channel enum + PacketHeader struct | 30 | `crates/ashfall-core/src/protocol/channel.rs`, `header.rs` | Correct discriminant values (0/1/2) |
| 7 | `ashfall-phase1-pr7-system-packets` | System packet variants (Auth, Load, Start, End, Mod, Chat, Weather, Global, Base, Deleted) | 120 | `crates/ashfall-core/src/protocol/system.rs` | Each variant matches arch doc; serde derives |
| 8 | `ashfall-phase1-pr8-reference-packets` | Reference/Base create/update packet variants | 60 | `crates/ashfall-core/src/protocol/reference.rs` | Matches ObjectNew fields |
| 9 | `ashfall-phase1-pr9-object-packets` | Object packet variants (pos/angle/cell/name/lock/owner/activate/sound) | 90 | `crates/ashfall-core/src/protocol/object.rs` | Each update variant present |
| 10 | `ashfall-phase1-pr10-item-packets` | Item packet variants (new/count/condition/equip) | 70 | `crates/ashfall-core/src/protocol/item.rs` | Container ID, count, condition, equipped fields |
| 11 | `ashfall-phase1-pr11-container-packets` | Container + ItemList packet variants | 50 | `crates/ashfall-core/src/protocol/container.rs` | ContainerNew, ItemListNew |
| 12 | `ashfall-phase1-pr12-actor-packets` | Actor packet variants (new/values/race/anim/state/dead/fire) | 130 | `crates/ashfall-core/src/protocol/actor.rs` | All 7 actor update variants; HashMap values |
| 13 | `ashfall-phase1-pr13-player-packets` | Player packet variants (new/controls/interior/exterior/context/console) | 80 | `crates/ashfall-core/src/protocol/player.rs` | Controls HashMap, cell context array |
| 14 | `ashfall-phase1-pr14-window-packets` | Window + widget packet variants (all 9 widget types + updates) | 200 | `crates/ashfall-core/src/protocol/window.rs` | Every widget create+update variant from arch doc |
| 15 | `ashfall-phase1-pr15-master-packets` | Master server packet variants | 40 | `crates/ashfall-core/src/protocol/master.rs` | Query/Announce/Update |
| 16 | `ashfall-phase1-pr16-packet-enum` | Unified Packet enum + protocol mod | 60 | `crates/ashfall-core/src/protocol/mod.rs` | All sub-module variants re-exported; Channel→packet mapping |
| 17 | `ashfall-phase1-pr17-wire-test` | Round-trip serialization test for every Packet variant | 300 | `crates/ashfall-core/tests/wire_format.rs` | Every variant: serialize→deserialize→assert_eq; test max size ≤1200 |

**Phase 1 total: ~1,460 LOC**

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

## Phase 4: Persistence

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 40 | `ashfall-phase4-pr40-db-module` | Database struct + open/close + schema migration | 80 | `crates/ashfall-server/src/db/mod.rs`, `schema.rs` | `open(path)` creates tables if not exist |
| 41 | `ashfall-phase4-pr41-record-table` | Records table: insert/get/get_by_type | 60 | `crates/ashfall-server/src/db/record.rs` | CRUD for baseID→name/type/desc |
| 42 | `ashfall-phase4-pr42-reference-table` | References table: refID→baseID/cellID/objectID | 60 | `crates/ashfall-server/src/db/reference.rs` | Load references on startup |
| 43 | `ashfall-phase4-pr43-exterior-table` | Exteriors table: worldID+x+y primary key | 40 | `crates/ashfall-server/src/db/exterior.rs` | Load exterior cell data |
| 44 | `ashfall-phase4-pr44-weapon-race-npc` | Weapon, Race, NPC tables | 100 | `crates/ashfall-server/src/db/weapon.rs`, `race.rs`, `npc.rs` | Each table has load/get pattern |
| 45 | `ashfall-phase4-pr45-container-item-terminal` | BaseContainer, Item, Terminal tables | 80 | `crates/ashfall-server/src/db/container.rs`, `item.rs`, `terminal.rs` | Each table has load/get pattern |
| 46 | `ashfall-phase4-pr46-db-startup-load` | Load all tables at server startup into memory | 100 | `crates/ashfall-server/src/dedicated.rs` (extend) | Records, references, NPCs, weapons loaded; available to handlers |
| 47 | `ashfall-phase4-pr47-db-integration-test` | Persistence test: create records, restart, verify | 120 | `crates/ashfall-server/tests/persistence.rs` | Insert→commit→reopen→verify data persisted |

**Phase 4 total: ~640 LOC** (depends on PR29)

---

## Phase 5: Scripting

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 48 | `ashfall-phase5-pr48-script-crate` | ashfall-script SDK crate | 40 | `crates/ashfall-script/Cargo.toml`, `src/lib.rs` | Helper macros for WASM imports; ID type aliases |
| 49 | `ashfall-phase5-pr49-wasm-engine` | wasmtime Engine+Store init, module loading | 100 | `crates/ashfall-server/src/script/engine.rs` | Load .wasm module from scripts/ dir |
| 50 | `ashfall-phase5-pr50-callback-dispatch` | 31 callback dispatch functions (stubs) | 150 | `crates/ashfall-server/src/script/callbacks.rs` | Each callback invokes all loaded instances; bool returns OR-ed |
| 51 | `ashfall-phase5-pr51-host-object-fns` | ~40 host functions: CreateObject–DestroyObject, GetPos–SetPos, GetCell–SetCell | 250 | `crates/ashfall-server/src/script/host.rs` | WASM can create objects in registry; positions update |
| 52 | `ashfall-phase5-pr52-host-actor-fns` | ~30 host functions: CreateActor, actor value get/set, animations, death | 180 | `crates/ashfall-server/src/script/host.rs` (extend) | WASM can create actors, set values, kill |
| 53 | `ashfall-phase5-pr53-host-item-fns` | ~20 host functions: CreateItem, container ops, equip, count, condition | 130 | `crates/ashfall-server/src/script/host.rs` (extend) | WASM can create items, add to containers, equip |
| 54 | `ashfall-phase5-pr54-host-gui-fns` | ~40 host functions: window/widget create/destroy/set/get | 250 | `crates/ashfall-server/src/script/host.rs` (extend) | WASM can create full GUI hierarchy |
| 55 | `ashfall-phase5-pr55-host-misc-fns` | ~30 host functions: weather, time, chat, kick, timers, utility | 200 | `crates/ashfall-server/src/script/host.rs` (extend) | WASM can set weather, game time, send chat, kick players |
| 56 | `ashfall-phase5-pr56-timer-system` | Script timer: CreateTimer, KillTimer, tick dispatch | 100 | `crates/ashfall-server/src/script/timer.rs` | Timers fire at interval; invoke WASM callback |
| 57 | `ashfall-phase5-pr57-auth-callback` | Wire OnClientAuthenticate into auth handler | 40 | `crates/ashfall-server/src/handlers/auth.rs` (modify) | Script can reject auth; rejection sent to client |
| 58 | `ashfall-phase5-pr58-freeroam-script` | Example freeroam WASM script | 150 | `scripts/freeroam/Cargo.toml`, `src/lib.rs` | Server init, onPlayerRequestGame returns spawn cell, onPlayerChat echoes |
| 59 | `ashfall-phase5-pr59-script-integration-test` | Integration test: load freeroam, auth, spawn, chat | 200 | `crates/ashfall-server/tests/scripting.rs` | Full script lifecycle test |

**Phase 5 total: ~1,790 LOC** (depends on PR47)

---

## Phase 6: GUI (Server-Authoritative)

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 60 | `ashfall-phase6-pr60-gui-types` | GuiWindow, GuiWidgetKind enums, GuiState struct | 80 | `crates/ashfall-client/src/ui/widgets.rs` | All 9 widget kinds represented |
| 61 | `ashfall-phase6-pr61-window-handlers` | Window create/update/remove packet handlers | 120 | `crates/ashfall-server/src/handlers/gui.rs` | Server-side: store window state, relay to client |
| 62 | `ashfall-phase6-pr62-widget-handlers` | Button/Text/Edit/Checkbox/RadioButton/List/ListItem handlers | 150 | `crates/ashfall-server/src/handlers/gui.rs` (extend) | Each widget create+update handled |
| 63 | `ashfall-phase6-pr63-gui-events` | GUI event dispatch: click, return, text change, checkbox, radio, list select | 120 | `crates/ashfall-server/src/handlers/gui.rs` (extend) | Events routed to script callbacks |
| 64 | `ashfall-phase6-pr64-egui-app` | egui app skeleton + AshfallApp struct | 60 | `crates/ashfall-client/src/ui/app.rs` | eframe window opens; empty canvas |
| 65 | `ashfall-phase6-pr65-egui-server-gui` | Render server-authored GUI widgets with egui | 200 | `crates/ashfall-client/src/ui/widgets.rs` (extend) | Window/Button/Text/Edit/Checkbox/RadioButton/List render correctly |
| 66 | `ashfall-phase6-pr66-egui-events` | egui events → network packets back to server | 100 | `crates/ashfall-client/src/handlers/gui.rs` | Button click→packet; edit enter→packet; checkbox toggle→packet |
| 67 | `ashfall-phase6-pr67-gui-integration-test` | End-to-end GUI test | 150 | `crates/ashfall-server/tests/gui.rs` | Script creates window→client renders→click→server callback fires |

**Phase 6 total: ~980 LOC** (depends on PR59, PR83)

---

## Phase 7: Client

| PR | Branch | Task | Est LOC | Files | Acceptance |
|----|--------|------|---------|-------|------------|
| 68 | `ashfall-phase7-pr68-client-crate` | ashfall-client crate skeleton | 30 | `crates/ashfall-client/Cargo.toml`, `src/main.rs` | `cargo build` passes |
| 69 | `ashfall-phase7-pr69-client-config` | Client config: vaultmp.ini parsing | 60 | `crates/ashfall-client/src/config.rs` | name, master addr, inittime parsed |
| 70 | `ashfall-phase7-pr70-client-network` | UDP socket + connect + reliability layer (lite) | 150 | `crates/ashfall-client/src/network.rs` | Connect to server; recv/send packets; seq-based ordering |
| 71 | `ashfall-phase7-pr71-connection-flow` | Client connect→auth→load→ingame flow | 120 | `crates/ashfall-client/src/game.rs` | GameAuth sent; GameLoad received; state transitions |
| 72 | `ashfall-phase7-pr72-client-dispatch` | Client packet dispatch | 100 | `crates/ashfall-client/src/dispatch.rs` | Match incoming Packet, route to handler |
| 73 | `ashfall-phase7-pr73-client-registry` | Client-side object cache (ClientRegistry, ClientObject enum) | 100 | `crates/ashfall-client/src/world/registry.rs` | Objects inserted from packets; position/angle updated |
| 74 | `ashfall-phase7-pr74-handlers-obj-actor` | Client handlers: Object, Item, Actor, Container packets | 120 | `crates/ashfall-client/src/handlers/object.rs`, `actor.rs`, `item.rs` | ObjectNew→cache insert; UpdatePos→update cache |
| 75 | `ashfall-phase7-pr75-handlers-game-player` | Client handlers: Game, Player, Chat packets | 100 | `crates/ashfall-client/src/handlers/game.rs`, `player.rs`, `chat.rs` | Weather/global/time updates; player spawn; chat display |
| 76 | `ashfall-phase7-pr76-client-loop` | Client main loop: tick + recv select | 80 | `crates/ashfall-client/src/game.rs` (extend) | 30Hz tick; UDP recv; dispatch; flush outgoing |
| 77 | `ashfall-phase7-pr77-egui-server-browser` | Server browser: master query, list display, connect | 150 | `crates/ashfall-client/src/ui/server_browser.rs` | Query master→display servers→click join→connect flow |
| 78 | `ashfall-phase7-pr78-chat-ui` | Chat input/output in egui overlay | 80 | `crates/ashfall-client/src/ui/chat.rs` | Type message→send GameChat; receive GameChat→display |
| 79 | `ashfall-phase7-pr79-ipc-core` | IPC module: IpcTransport enum (Tcp/Unix/Stub), IpcMode, connect + execute | 150 | `crates/ashfall-client/src/ipc/mod.rs`, `transport.rs`, `commands.rs` | TCP connects to 127.0.0.1:1771; Unix connects to socket; Stub returns canned; wire format matches pipe protocol |
| 80 | `ashfall-phase7-pr80-client-server-test` | End-to-end client-server integration test | 200 | `tests/integration.rs` (workspace root) | Spin up server→client connects→auth→weather sync→chat round-trip |

**Phase 7 total: ~1,350 LOC** (depends on PR67)

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

| Phase | PRs | Est LOC | Depends On |
|-------|-----|---------|------------|
| Phase 1: Core Protocol | 1–17 | 1,460 | — |
| Phase 2: Server Foundation | 18–29 | 1,260 | PR17 |
| Phase 3: World Sync | 30–39 | 1,010 | PR29 |
| Phase 4: Persistence | 40–47 | 640 | PR29 |
| Phase 5: Scripting | 48–59 | 1,790 | PR47 |
| Phase 6: GUI | 60–67 | 980 | PR59, PR83 |
| Phase 7: Client | 68–80 | 1,350 | PR67 |
| Phase 8: Master Server | 81–87 | 420 | PR80 |
| Phase 9: Polish | 88–95 | 850 | PR87 |
| Phase 10: Proton Bridge | 96–102 | 1,300 | PR79, PR80 |
| **Total** | **102** | **~11,100** | |

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