# Ashfall

**Rust recreation of [vaultmp-extended](https://github.com/massdivide/vaultmp-extended)** — a multiplayer mod for Bethesda's Fallout 3 / Fallout: New Vegas. Server-authoritative dedicated server with WASM scripting, UDP networking, SQLite persistence, and an egui client browser.

---

## Architecture

```
┌─────────────┐     UDP     ┌──────────────┐     UDP     ┌────────────┐
│ ashfall-     │◄──────────►│ ashfall-      │◄──────────►│ ashfall-   │
│ master       │            │ server        │            │ client     │
│ (registry)   │            │ (authority)   │            │ (egui)     │
└─────────────┘            └──────┬────────┘            └─────┬──────┘
                                  │                           │
                          ┌───────┴───────┐          ┌───────┴───────┐
                          │ wasmtime      │          │ TCP loopback  │
                          │ (scripts)     │          │ 127.0.0.1:1771│
                          ├───────────────┤          └───────┬───────┘
                          │ SQLite        │                  │
                          │ (persistence) │          ┌───────┴───────┐
                          └───────────────┘          │ Proton/Wine   │
                                                     │ ┌───────────┐ │
                        Native Linux (all)           │ │bridge.dll │ │
                                                     │ │(MinGW)    │ │
                                                     │ └─────┬─────┘ │
                                                     │ ┌─────┴─────┐ │
                                                     │ │Fallout3   │ │
                                                     │ │.exe       │ │
                                                     │ └───────────┘ │
                                                     └───────────────┘
```

- **Server-authoritative** — server owns all game state. Clients send input, server validates and broadcasts.
- **3 ordered channels** (System, Game, Chat) + 1 unordered channel for position updates.
- **30 Hz tick rate** with 9-cell grid context for visibility/interest management.
- **postcard** binary serialization over custom UDP reliability layer.

---

## Platform Support

| Binary | Linux (native) | Proton/Wine | Windows |
|--------|:---:|:---:|:---:|
| `ashfall-server` | ✅ | ✅ (native) | ❌ |
| `ashfall-master` | ✅ | ✅ (native) | ❌ |
| `ashfall-client` | ✅ | ✅ (native) | ❌ |
| `bridge.dll` | — | ✅ (injected) | ✅ (native) |

**Server + master** run natively. **Client** is a native Linux egui app that talks to the game via TCP loopback through bridge.dll inside Proton. For native Windows, bridge.dll loads as a standard DLL. [Proton setup guide →](./docs/proton-setup.md)

## Crates

| Crate | Purpose | Key Dependencies |
|-------|---------|-----------------|
| `ashfall-core` | Shared types, full `Packet` enum (120+ variants), `ObjectKind` bitmask hierarchy, `NetworkID`, math, constants | `serde`, `postcard` |
| `ashfall-server` | Dedicated authoritative server with object registry, cell sync, scripting, persistence | `tokio`, `wasmtime`, `rusqlite`, `dashmap` |
| `ashfall-client` | Player client with egui GUI, server browser, IPC bridge to game engine (TCP/Unix/Stub) | `tokio`, `egui`, `eframe` |
| `ashfall-master` | Lightweight server browser registry | `tokio` |
| `ashfall-script` | SDK for writing WASM game mode scripts | `ashfall-core` |
| `ashfall-bridge` | Cross-compiled DLL for Proton/Wine — hooks Gamebryo engine, exposes TCP server | `windows-sys` (MinGW target) |

---

## Installation & Setup

### Prerequisites

| Tool | Required For | Install |
|------|-------------|---------|
| Rust 1.75+ | All crates | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| MinGW-w64 | bridge.dll cross-compilation | `sudo apt install mingw-w64` (Debian/Ubuntu) or `sudo pacman -S mingw-w64-gcc` (Arch) |
| Fallout 3 / New Vegas | Game client | Steam or GOG, installed and playable under Proton |
| git | Clone repo | `sudo apt install git` |

### Clone & Build

```bash
# Clone
git clone https://github.com/YOUR_ORG/ashfall.git
cd ashfall

# Add cross-compilation target for Proton bridge (optional)
rustup target add x86_64-pc-windows-gnu

# Build everything
cargo build --release

# Bridge DLL (requires mingw-w64)
cargo build --release --target x86_64-pc-windows-gnu -p ashfall-bridge
```

### Quick Smoke Test

```bash
# Terminal 1 — master server
cargo run -p ashfall-master

# Terminal 2 — dedicated server
cargo run -p ashfall-server

# Terminal 3 — client (stub mode, no game needed)
cargo run -p ashfall-client
```

Client connects to server at `127.0.0.1:1770`. In stub mode it sends canned position/angle data — enough to verify the full auth→load→sync flow.

### Run with Proton

See [Proton Setup Guide](./docs/proton-setup.md) for full instructions. Quick version:

```bash
# 1. Copy bridge.dll to game directory
cp target/x86_64-pc-windows-gnu/release/bridge.dll \
   "$HOME/.steam/steam/steamapps/common/Fallout 3 goty/"

# 2. Launch Fallout 3 with DLL override
WINEDLLOVERRIDES="bridge=n,b" steam steam://rungameid/22370

# 3. Start Ashfall client
cargo run -p ashfall-client
```

### Configuration

Server (`~/.config/ashfall/server.ini`):
```ini
[server]
host = 0.0.0.0
port = 1770
connections = 4
announce = 127.0.0.1

[scripts]
path = ./scripts

[database]
path = ./data/fallout3.sqlite3
```

Client (`~/.config/ashfall/client.ini`):
```ini
[general]
name = Wanderer
master = 127.0.0.1

[ipc]
mode = proton       # proton | native | stub
port = 1771

[server]
address = 127.0.0.1
port = 1770
```

---

## Object Hierarchy

Bitmask type system matching the original C++ `ReferenceTypes.hpp`:

```
Reference                 0x0001
├── Object                0x0002
│   ├── Item              0x0008
│   └── Container         0x0010  (Object + ItemList)
│       └── Actor          0x0020
│           └── Player     0x0040
└── Window                0x0080
    ├── Button            0x0100
    ├── Text              0x0200
    ├── Edit              0x0400
    ├── Checkbox          0x0800
    ├── RadioButton       0x1000
    ├── ListItem          0x2000
    └── List              0x4000
```

Subtype checks use bitmask composition. `Actor` matches `ALL_OBJECTS | ALL_ITEMLISTS | ALL_CONTAINERS | ALL_ACTORS`.

---

## Protocol

All packets live in a single serde enum (`ashfall_core::protocol::Packet`). Categories:

| Category | Packets | Description |
|----------|---------|-------------|
| System | `GameAuth`, `GameLoad`, `GameStart`, `GameEnd`, `GameMod`, `GameChat`, `GameWeather`, `GameGlobal`, `GameBase`, `GameDeleted` | Authentication, lifecycle, chat, world globals |
| Object | `ObjectNew`, `VolatileNew`, `ObjectRemove`, `UpdatePos`, `UpdateAngle`, `UpdateCell`, `UpdateName`, `UpdateLock`, `UpdateOwner`, `UpdateActivate`, `UpdateSound` | 3D position, angle, cell, properties |
| Item | `ItemNew`, `UpdateItemCount`, `UpdateItemCondition`, `UpdateItemEquipped` | Inventory, stacks, condition, equipped state |
| Container | `ContainerNew`, `ItemListNew` | Chests, NPC inventories |
| Actor | `ActorNew`, `UpdateActorState`, `UpdateActorRace`, `UpdateActorSex`, `UpdateActorDead`, `UpdateActorValue`, `UpdateFireWeapon`, `UpdateActorIdle` | Actor values, animations, race, death, combat |
| Player | `PlayerNew`, `UpdateControl`, `UpdateInterior`, `UpdateExterior`, `UpdateContext`, `UpdateConsole` | Player controls, cell grid, console access |
| Window | `WindowNew`, `*New` for 9 widget types, `UpdateWindow*`, `Update*Selected`, `Update*Text`, etc. | Server-authoritative GUI overlay |
| Master | `MasterQuery`, `MasterAnnounce`, `MasterUpdate` | Server browser registry |

Wire format: `[length: u16][channel: u8][postcard(Packet)]`, max 1200 bytes.

---

## Connection Lifecycle

```
Client                    Server
  │                         │
  ├─── UDP connect ────────►│
  ├─── GameAuth(name,pwd) ─►│  → OnClientAuthenticate(name,pwd)
  │                         │  → Create session + Player object
  │◄── GameLoad ────────────┤
  │◄── GameWeather ─────────┤
  │◄── GameGlobal* ─────────┤  (all globals)
  │◄── PlayerNew* ──────────┤  (existing players)
  │◄── GameStart ───────────┤
  │                         │
  │◄── ObjectNew/ItemNew* ──┤  (cell context objects)
  │                         │
  │  steady state (30Hz) ───┤
  │  UpdatePos/Angle ──────►│  ← client polls engine
  │◄─────────── broadcasts ─┤  → cell neighbors
  │                         │
  │─── GameEnd(reason) ─────┤  (OR server sends)
  │◄── GameEnd(reason) ─────┤
```

---

## Cell System

9-cell grid around each player (`CellContext = [u32; 9]`). Center = player's current cell, 8 neighbors from exterior grid.

- Cell change → recompute enter/leave diff → spawn new objects, remove stale ones.
- Visibility: only objects in player's cell context are sent.
- Position broadcasts go to all players whose context contains the object's cell.
- `cell_refs: DashMap<u32, Vec<NetworkID>>` for O(1) cell→objects lookup.

---

## Scripting (WASM)

Server loads `.wasm` modules from the `scripts/` directory at startup. 31 callbacks and ~160 host functions exposed.

### 31 Callbacks

| Callback | Signature |
|----------|-----------|
| `OnServerInit` | `()` |
| `OnServerExit` | `(shutdown: bool)` |
| `OnClientAuthenticate` | `(name, password) → bool` |
| `OnPlayerDisconnect` | `(player_id, reason: u8)` |
| `OnPlayerRequestGame` | `(player_id) → base_id: u32` |
| `OnPlayerChat` | `(player_id, message) → bool` |
| `OnCreate` / `OnDestroy` | `(object_id)` |
| `OnSpawn` | `(player_id)` |
| `OnActivate` | `(ref_id, actor_id)` |
| `OnCellChange` | `(object_id, cell: u32)` |
| `OnLockChange` | `(object_id, actor_id, lock: u32)` |
| `OnItemCountChange` | `(item_id, count: u32)` |
| `OnItemConditionChange` | `(item_id, condition: f32)` |
| `OnItemEquippedChange` | `(item_id, equipped: bool)` |
| `OnActorValueChange` | `(actor_id, index: u8, value: f32)` |
| `OnActorBaseValueChange` | `(actor_id, index: u8, value: f32)` |
| `OnActorAlert` / `OnActorSneak` | `(actor_id, state: bool)` |
| `OnActorDeath` | `(actor_id, killer_id, limbs: u16, cause: i8)` |
| `OnActorPunch` | `(actor_id, power: bool)` |
| `OnActorFireWeapon` | `(actor_id, weapon_id: u32)` |
| `OnWindowMode` | `(player_id, enabled: bool)` |
| `OnWindowClick` / `OnWindowReturn` | `(player_id, window_id)` |
| `OnWindowTextChange` | `(player_id, window_id, text)` |
| `OnCheckboxSelect` | `(player_id, checkbox_id, selected: bool)` |
| `OnRadioButtonSelect` | `(player_id, radio_id, previous_id)` |
| `OnListItemSelect` | `(player_id, listitem_id, selected: bool)` |
| `OnGameTimeChange` | `(year, month, day, hour)` |

### ~160 Host Functions

Server management (`SetServerName`, `GetMaximumPlayers`, `timestamp`), object CRUD (`CreateObject`, `DestroyObject`, `GetPos`, `SetPos`, `GetCell`, `SetCell`, `SetLock`, `SetOwner`, `Activate`, `PlaySound`), item/container management (`CreateItem`, `AddItem`, `RemoveItem`, `EquipItem`, `GetItemCount`, `SetItemCondition`), actor management (`CreateActor`, `SetActorValue`, `GetActorValue`, `KillActor`, `SetActorRace`, `FireWeapon`, `PlayIdle`), player actions (`Kick`, `UIMessage`, `ChatMessage`, `ForceWindowMode`), GUI CRUD (`CreateWindow`, `DestroyWindow`, all widget create/destroy/get/set functions), timers (`CreateTimer`, `KillTimer`), world state (`SetGameWeather`, `SetGameTime`, `SetTimeScale`), utilities (`ValueToString`, `AxisToString`, `BaseToString`).

---

## Database (SQLite)

Ported schema from original `fallout3.sqlite3`:

```sql
records(baseID, name, description, type)
references(refID, baseID, cellID, objectID)
exteriors(worldID, x, y)
weapons(baseID, name, ...)
races(baseID, name, ...)
npcs(baseID, name, ...)
base_containers(baseID, name)
base_items(baseID, name)
terminals(baseID, name)
interiors(cellID, name)
ac_references(refID, baseID, cellID)
```

Loaded at server startup via `rusqlite`. Object persistence across restarts via `references` table.

---

## Server-Authoritative GUI

Server scripts create GUI elements (windows, buttons, text, edits, checkboxes, radio buttons, lists) as game objects with `NetworkID`s. Packets sent to attached players. Client renders with `egui`. Events (click, text change, select) flow back to server → script callbacks.

---

## C++ → Rust Mapping

| Original C++ | Rust Replacement |
|-------------|-----------------|
| RakNet (UDP, unreliable + ordered reliable) | Custom reliability layer over `tokio::net::UdpSocket` |
| CEGUI (in-game overlay) | `egui` / `eframe` |
| Pawn AMX VM | `wasmtime` (WASM) |
| Windows named pipes | TCP loopback (Proton) / Unix domain sockets (native) |
| `CriticalSection` / `Guarded<T>` | `Arc<RwLock<T>` / `tokio::sync::RwLock` |
| `GameFactory` singleton | `ObjectRegistry` (`DashMap<NetworkID, Arc<RwLock<dyn GameObject>>>`) |
| `boost::any` | `Box<dyn Any>` |
| `PF_MAKE` packet macros | `serde::Serialize`/`Deserialize` on `Packet` enum |
| SQLite3 via C API | `rusqlite` (bundled) |
| `Data.hpp` bitmask system | `ObjectKind` enum with composite masks |

---

## Sync Model

- **Server owns truth.** Client sends input — server validates, updates, broadcasts.
- **No client prediction** (matches original; ponytail: add when latency demands).
- **Interpolation** — client lerps between last two known positions for remote objects.
- **Reliable ordered** for chat, events, state changes. **Unreliable unordered** for position/angle (drop OK).
- **Delta compression** — deferred (ponytail: add when bandwidth problem proven).

---

## Implementation Plan

95 PRs across 9 phases. [Full plan →](./docs/impl-plan.md)

| Phase | PRs | Scope | Est LOC |
|-------|-----|-------|---------|
| 1: Core Protocol | 1–17 | Types, constants, Packet enum, wire format tests | 1,460 |
| 2: Server Foundation | 18–29 | UDP, sessions, registry, auth, connection flow, main loop | 1,260 |
| 3: World Sync | 30–39 | Cell grid, position/angle/actor/item sync, weather, globals | 1,010 |
| 4: Persistence | 40–47 | SQLite schema, all 11 tables, startup load | 640 |
| 5: Scripting | 48–59 | wasmtime engine, 31 callbacks, ~160 host functions, timers, example script | 1,790 |
| 6: GUI | 60–67 | Server-authoritative GUI handlers + egui rendering + events | 980 |
| 7: Client | 68–80 | Connection flow, client registry, handlers, egui server browser, chat, IPC (TCP/Unix/Stub) | 1,350 |
| 8: Master Server | 81–87 | Announce/query/cull + server integration | 420 |
| 9: Polish | 88–95 | Reliability tuning, interpolation, file transfer, tracing, graceful shutdown, stress test | 850 |
| 10: Proton Bridge | 96–102 | Bridge crate, TCP server, engine hooks, command dispatch, Proton setup docs, CI | 1,300 |
| **Total** | **102** | | **~11,100** |

Phases 3+4 can run in parallel. Phases 6+7 depend on 5.

---

## Source Tree

```
ashfall/
├── Cargo.toml
├── crates/
│   ├── ashfall-core/src/          # ✅ implemented
│   │   ├── lib.rs
│   │   ├── types.rs               # ObjectKind, GameObject trait, Reason
│   │   ├── constants.rs            # version, ports, limits, channel IDs
│   │   ├── id.rs                   # NetworkID
│   │   ├── math.rs                 # VaultVector, validation, distance
│   │   └── protocol/
│   │       ├── mod.rs              # Packet enum (120+ variants)
│   │       ├── channel.rs          # Channel::System/Game/Chat
│   │       └── header.rs           # PacketHeader
│   ├── ashfall-bridge/src/        # ✅ skeleton
│   │   ├── lib.rs                  # DllMain entry point
│   │   ├── network.rs              # TCP server on 127.0.0.1:1771
│   │   ├── commands.rs             # command dispatcher (opcodes)
│   │   └── hooks/mod.rs            # Gamebryo engine hook stubs
│   ├── ashfall-client/src/        # ✅ skeleton + ipc module
│   │   ├── main.rs
│   │   └── ipc/
│   │       ├── mod.rs              # IpcClient with execute/get_pos/get_angle
│   │       ├── transport.rs        # Tcp/Unix/Stub transport
│   │       └── commands.rs          # opcodes, Param, CommandResult
│   ├── ashfall-server/src/        # ✅ skeleton
│   │   └── main.rs
│   ├── ashfall-master/src/        # ✅ skeleton
│   │   └── main.rs
│   └── ashfall-script/src/        # ✅ SDK stub
│       └── lib.rs
└── docs/                           # 6 docs (analysis, requirements, arch, plan, proton, research)
```

Planned (created by later PRs):

```
ashfall/
├── crates/
│   ├── ashfall-server/src/
│   │   ├── dedicated.rs, network.rs, session.rs, dispatch.rs, config.rs, master.rs
│   │   ├── handlers/       # auth, game, object, actor, item, player, chat, gui
│   │   ├── world/           # registry, objects, cell, weather, globals, inventory
│   │   ├── db/              # schema + per-table CRUD
│   │   └── script/          # wasmtime engine, host fns, callbacks, timers
│   ├── ashfall-client/src/
│   │   ├── game.rs, network.rs, dispatch.rs, config.rs
│   │   ├── handlers/        # game, object, actor, item, player, chat, gui
│   │   ├── world/           # registry, cell, state
│   │   └── ui/              # app, server_browser, widgets, chat
│   ├── ashfall-master/src/  # server_list, announce, query, cull
│   ├── ashfall-bridge/src/  # expanded hooks + commands
├── scripts/                 # example WASM game modes
├── data/                    # SQLite databases, config templates
├── tests/                   # integration + stress tests
├── tools/esm-reader/        # ESM reader for populating DB
└── examples/
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `tokio` | 1 | Async runtime, UDP, Unix sockets |
| `serde` + `postcard` | 1 / 1 | Packet serialization |
| `wasmtime` | 22 | WASM scripting VM |
| `rusqlite` | 0.31 (bundled) | SQLite persistence |
| `dashmap` | 6 | Concurrent object registry |
| `egui` + `eframe` | 0.28 | Client GUI |
| `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Structured logging |
| `parking_lot` | 0.12 | Faster synchronization primitives |
| `anyhow` + `thiserror` | 1 / 2 | Error handling |
| `uuid` | 1 | Session GUIDs |

---

## Documents

- [Codebase Analysis](./docs/codebase-analysis.md) — Full C++ architecture breakdown of vaultmp-extended
- [Requirements](./docs/requirements.md) — Functional requirements extracted from the original
- [Architecture](./docs/architecture.md) — Rust design, Linux/Proton compatibility, type system, sync model
- [Implementation Plan](./docs/impl-plan.md) — 102 PRs across 10 phases with acceptance criteria
- [Proton Setup](./docs/proton-setup.md) — Steam Deck / Proton configuration guide
- [Research Brief](./docs/research-brief.md) — Tech landscape: Bethesda APIs, RakNet alternatives, Rust networking crates

---

## Status

**Phase 1 in progress.** Workspace compiles (all 6 crates). `ashfall-core` complete with types, constants, math, `NetworkID`, full 120-variant `Packet` enum. IPC module with TCP/Unix/Stub transport. Bridge crate skeleton for Proton. Next: wire format round-trip tests (PR 17).

---

## Contributing

**Vibe coding very welcome.** Seriously — AI-assisted code, LLM-generated PRs, cursor-driven refactors, prompt-engineering experiments — all fair game. The one hard rule:

> **It must pass tests.** No untested code lands on `main`. If you vibe-code a feature, vibe-code its test too. Stub mode means you can test the full client+server stack without the game running.

### Branch Convention

```
dashfall-{phase}-{number}-{short-description}
```

Example: `ashfall-phase1-pr17-wire-format-tests`

### Picking a Task

1. Read [Implementation Plan](./docs/impl-plan.md) — 102 PRs across 10 phases, each with estimated LOC and acceptance criteria.
2. Check open issues and PRs for conflicts.
3. Claim a task by opening a draft PR with the branch name.
4. Work in small increments — each PR should touch ~50-200 lines, be reviewable in under 10 minutes.

### Where to Start

| Skill Level | Good First Tasks |
|------------|-----------------|
| Rust beginner | Phase 1 PRs 3-6: constants, NetworkID, math, channel enums |
| Comfortable with Rust | Phase 1 PRs 7-16: packet variants, serde derives |
| Networking | Phase 2 PRs 20-21, 24-27: UDP sockets, sessions, dispatch, main loop |
| Database | Phase 4 PRs 40-47: SQLite schema, CRUD |
| WASM / compilers | Phase 5 PRs 48-59: wasmtime engine, host functions, callbacks |
| GUI / gamedev | Phase 6+7: egui widgets, server browser, chat UI |
| Reverse engineering | Phase 10 PRs 96-102: Gamebryo VTable hooks, Proton bridge |

### Quality Bar

- **Format**: `cargo fmt`
- **Lint**: `cargo clippy -- -D warnings`
- **Test**: `cargo test`
- **Build**: `cargo build --release` + `cargo build --release --target x86_64-pc-windows-gnu -p ashfall-bridge`
- **Commit**: conventional commits (`feat:`, `fix:`, `test:`, `docs:`, `chore:`)

### Development Flow

```bash
# Fork + clone
git checkout -b ashfall-phase1-pr42-sqlite-schema

# Work (vibe or otherwise)
$EDITOR crates/ashfall-server/src/db/schema.rs

# Test
cargo test -p ashfall-server

# Lint
cargo clippy -- -D warnings
cargo fmt -- --check

# Push + open PR
git push origin ashfall-phase1-pr42-sqlite-schema
```

### Questions?

- Architecture questions → [Architecture doc](./docs/architecture.md)
- Protocol details → [Codebase Analysis](./docs/codebase-analysis.md)
- Setup problems → [Proton Setup Guide](./docs/proton-setup.md)
- Stuck? Open a discussion or issue.

---

## License
