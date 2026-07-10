# vaultmp-extended Codebase Analysis

## 1. Project Overview

**vaultmp** (Vault-Tec Multiplayer Mod) is a multiplayer mod for Bethesda's Fallout 3 (PC). C++ codebase. Architecture: client DLL injection into game process, dedicated server, master server browser, Pawn scripting engine.

**Key technologies:** RakNet (networking), CEGUI (GUI overlay), SQLite3 (server persistence), Pawn/AMX (scripting).

---

## 2. Complete Class Hierarchy

### 2.1 Core Type System (`ReferenceTypes.hpp`)

Bitmask type hierarchy:
```
ID_REFERENCE    = 0x00000001
ID_OBJECT       = 0x00000002
ID_ITEMLIST     = 0x00000004
ID_ITEM         = 0x00000008
ID_CONTAINER    = 0x00000010
ID_ACTOR        = 0x00000020
ID_PLAYER       = 0x00000040
ID_WINDOW       = 0x00000080
ID_BUTTON       = 0x00000100
ID_TEXT         = 0x00000200
ID_EDIT         = 0x00000400
ID_CHECKBOX     = 0x00000800
ID_RADIOBUTTON  = 0x00001000
ID_LISTITEM     = 0x00002000
ID_LIST         = 0x00004000
```

Composite tokens (subclass checks use mask & token):
```
ALL_REFERENCES  = REFERENCE|OBJECT|ITEM|CONTAINER|ACTOR|PLAYER
ALL_OBJECTS     = OBJECT|ITEM|CONTAINER|ACTOR|PLAYER
ALL_ITEMLISTS   = ITEMLIST|CONTAINER|ACTOR|PLAYER
ALL_CONTAINERS  = CONTAINER|ACTOR|PLAYER
ALL_ACTORS      = ACTOR|PLAYER
ALL_WINDOWS     = WINDOW|BUTTON|TEXT|EDIT|CHECKBOX|RADIOBUTTON|LIST
```

### 2.2 Base Class (`Base.hpp`)

`Base` — root of all types. Inherits from `CriticalSection` (recursive mutex) and `RakNet::NetworkIDObject` (network-synced identity).

**Responsibilities:** Session counting (shared_ptr ref tracking), `StartSession()`/`EndSession()` for factory-lock protocol, virtual `toPacket()` for serialization, virtual `initializers()`/`freecontents()` hooks.

### 2.3 In-Game Object Hierarchy

```
Base
├── Reference          — refID + baseID, static refID→NetworkID map
│   ├── Object         — name, pos(game+network), angle, cell(game+network), enabled, lock, owner
│   │   ├── Item       — container, count, condition, equipped, silent, stick
│   │   └── ItemList (virtual, via Base)  — list of NetworkIDs for inventory
│   │       └── Container (Object + ItemList) 
│   │           └── Actor — actor values, base values, race, age, animations, sex, states
│   │               └── Player — controls, respawn, cell context, console, attached windows
│   └── [end in-game]
└── Window            — parent, label, pos, size, locked, visible, text
    ├── Button        — close button
    ├── Text          — static text label
    ├── Edit          — text input, maxlength, validation
    ├── Checkbox      — selected state
    ├── RadioButton   — selected state, group
    └── ListItem (separate)
    └── List          — multiselect, child ListItems
```

### 2.4 Class Responsibilities

| Class | Key Responsibility | Key Methods |
|-------|-------------------|-------------|
| **Reference** | Game entity identity. refID/baseID mapping. | `GetReference()`, `GetBase()`, `SetBase()` |
| **Object** | 3D position, angle, cell, name, lock, owner. Game coords vs network coords. | `GetGamePos()`, `SetNetworkPos()`, `IsNearPoint()` |
| **Item** | Inventory item. Stacks, condition, equipped state. | `GetItemContainer()`, `SetItemCount()`, `SetItemEquipped()` |
| **ItemList** | Inventory container. Add/remove items, stacking logic. | `AddItem()`, `RemoveItem()`, `EquipItem()` |
| **Container** | Union of Object + ItemList. Chests, NPC inventories. | — (combines Object + ItemList) |
| **Actor** | NPC/creature. Actor values (health, skills), animations, race, sex, death. | `GetActorValue()`, `SetActorRace()`, `SetActorDead()` |
| **Player** | Human player. Controls, cell context (9-cell grid), respawn, console, GUI windows. | `GetPlayerControl()`, `SetNetworkCell()`, `AttachWindow()` |
| **Window** | GUI frame. Parent/child hierarchy, position, visibility. | `SetWindowPos()`, `SetWindowVisible()` |
| **Button/Text/Edit/Checkbox/RadioButton** | GUI widgets extending Window. | Specialized per widget |
| **List** | GUI listbox with children ListItems. | `SetListMultiSelect()` |
| **ListItem** | GUI list entry. Text, selected state. | `SetListItemText()` |

### 2.5 Non-Hierarchy Classes

| Class | Responsibility |
|-------|---------------|
| **Game** | Client-side orchestrator. Creates objects from network packets, sends player state to server, manages spawn/weather/UI. All static. |
| **Network** | Static packet queue/dispatch. Wraps `RakNet::RakPeerInterface`. `NetworkResponse` = vector of `SingleResponse` (packet + descriptor + targets). |
| **NetworkClient** | Client-side packet handler. `ProcessEvent()` + `ProcessPacket()`. |
| **NetworkServer** | Server-side packet handler. Same interface. |
| **GameFactory** | Singleton object registry. Map `NetworkID→shared_ptr<Base>`. CRUD with session locking. Template-heavy. `Operate()` for atomic reads. |
| **Interface** | Command pipe to injected DLL. Queues commands with priority, sends via named pipes, receives results asynchronously. |
| **Pipe** | Windows named pipes. `PipeServer`/`PipeClient`. IPC between vaultmp.exe and vaultmpdll.dll. |
| **Value\<T\>** | Thread-safe value wrapper inheriting `Shared\<T\>` (promise/future support). `Lockable*` returned on set for network propagation. |
| **Guarded\<T\>** | RAII critical-section guard for arbitrary type. `Operate(lambda)` pattern. |
| **CriticalSection** | Recursive mutex wrapper. |
| **VaultFunctor** | Functor base used by Interface for lazy parameter evaluation. |
| **VaultVector** | 3D math utility. |
| **Expected\<T\>** | Result or exception type. |
| **VaultException** | Exception with stacktrace. |

---

## 3. Packet Protocol

### 3.1 Packet Factory Macros

All packets defined via `PF_MAKE` macros using RakNet BitStream. Two generators:
- `pGeneratorDefault` — no reference, global packet
- `pGeneratorReference` — includes `RakNet::NetworkID` reference
- `pGeneratorReferenceExtend` — extends `pGeneratorReference` (used by subclass factories)
- `_E` suffix = empty packet (0 payload beyond header)

### 3.2 Complete Packet Catalog

#### System / Game packets (`pGeneratorDefault`, global scope)

| Packet | Fields |
|--------|--------|
| `ID_GAME_AUTH` | `string name, string password` |
| `ID_GAME_LOAD` | *(empty)* |
| `ID_GAME_MOD` | `string filename, uint32 crc` |
| `ID_GAME_START` | *(empty)* |
| `ID_GAME_END` | `Reason reason` |
| `ID_GAME_MESSAGE` | `string message, uint8 emoticon` |
| `ID_GAME_CHAT` | `string message` |
| `ID_GAME_GLOBAL` | `uint32 global, int32 value` |
| `ID_GAME_WEATHER` | `uint32 weather` |
| `ID_GAME_BASE` | `uint32 playerBase` |
| `ID_GAME_DELETED` | `DeletedObjects (map<uint32,vector<uint32>>)` |
| `ID_UPDATE_WMODE` | `bool enabled` |

#### Reference packets (`pGeneratorReference` = NetworkID prefix)

| Packet | Fields |
|--------|--------|
| `ID_BASE_NEW` | *(empty, reference-only)* |
| `ID_REFERENCE_NEW` | `uint32 refID, uint32 baseID` |

#### Object packets

| Packet | Fields |
|--------|--------|
| `ID_OBJECT_NEW` | `string name, tuple<float,float,float> gamePos, tuple<float,float,float> networkPos, uint32 cell, bool enabled, uint32 lock, uint32 owner` |
| `ID_VOLATILE_NEW` | `uint32 baseID, float aX, float aY, float aZ` |
| `ID_OBJECT_REMOVE` | `bool silent` |
| `ID_UPDATE_NAME` | `string name` |
| `ID_UPDATE_POS` | `float X, float Y, float Z` |
| `ID_UPDATE_ANGLE` | `float X, float Y` |
| `ID_UPDATE_CELL` | `uint32 cell, float X, float Y, float Z` |
| `ID_UPDATE_LOCK` | `uint32 lock` |
| `ID_UPDATE_OWNER` | `uint32 owner` |
| `ID_UPDATE_ACTIVATE` | `NetworkID actor` |
| `ID_UPDATE_SOUND` | `uint32 sound` |

#### Item packets

| Packet | Fields |
|--------|--------|
| `ID_ITEM_NEW` | `NetworkID container, uint32 count, float condition, bool equipped, bool silent, bool stick` |
| `ID_UPDATE_COUNT` | `uint32 count, bool silent` |
| `ID_UPDATE_CONDITION` | `float condition, uint32 health` |
| `ID_UPDATE_EQUIPPED` | `bool equipped, bool silent, bool stick` |
| `ID_UPDATE_VALUE` | `bool base, uint8 index, float value` |

#### ItemList / Container packets

| Packet | Fields |
|--------|--------|
| `ID_ITEMLIST_NEW` | `vector<pPacket> items` |
| `ID_CONTAINER_NEW` | `pPacket itemList` |

#### Actor packets

| Packet | Fields |
|--------|--------|
| `ID_ACTOR_NEW` | `map<uint8,float> values, map<uint8,float> baseValues, uint32 race, int32 age, uint32 idle, uint8 moving, uint8 movingXY, uint8 weapon, bool female, bool alerted, bool sneaking, bool dead, uint16 limbs, int8 cause` |
| `ID_UPDATE_STATE` | `uint32 idle, uint8 moving, uint8 movingXY, uint8 weapon, bool alerted, bool sneaking, bool firing` |
| `ID_UPDATE_RACE` | `uint32 race, int32 age, int32 deltaAge` |
| `ID_UPDATE_SEX` | `bool female` |
| `ID_UPDATE_DEAD` | `bool dead, uint16 limbs, int8 cause` |
| `ID_UPDATE_FIREWEAPON` | `uint32 weapon` |
| `ID_UPDATE_IDLE` | `uint32 idle, string name` |

#### Player packets

| Packet | Fields |
|--------|--------|
| `ID_PLAYER_NEW` | `map<uint8, pair<uint8,bool>> controls` |
| `ID_UPDATE_CONTROL` | `uint8 control, uint8 key` |
| `ID_UPDATE_INTERIOR` | `string cell, bool spawn` |
| `ID_UPDATE_EXTERIOR` | `uint32 worldID, int32 x, int32 y, bool spawn` |
| `ID_UPDATE_CONTEXT` | `array<uint32,9> cells, bool spawn` |
| `ID_UPDATE_CONSOLE` | `bool enabled` |

#### Window/UI packets

| Packet | Fields |
|--------|--------|
| `ID_WINDOW_NEW` | `NetworkID parent, string label, tuple<float,float,float,float> pos, tuple<float,float,float,float> size, bool locked, bool visible, string text` |
| `ID_WINDOW_REMOVE` | *(empty)* |
| `ID_BUTTON_NEW` | *(empty, inherits window)* |
| `ID_TEXT_NEW` | *(empty)* |
| `ID_EDIT_NEW` | `uint32 maxLength, string validation` |
| `ID_CHECKBOX_NEW` | `bool selected` |
| `ID_RADIOBUTTON_NEW` | `bool selected, uint32 group` |
| `ID_LIST_NEW` | `vector<pPacket> items, bool multiselect` |
| `ID_LISTITEM_NEW` | `NetworkID container, string text, bool selected` |
| `ID_LISTITEM_REMOVE` | *(empty)* |
| `ID_UPDATE_WPOS` | `tuple<float,float,float,float> pos` |
| `ID_UPDATE_WSIZE` | `tuple<float,float,float,float> size` |
| `ID_UPDATE_WLOCKED` | `bool locked` |
| `ID_UPDATE_WVISIBLE` | `bool visible` |
| `ID_UPDATE_WTEXT` | `string text` |
| `ID_UPDATE_WMAXLEN` | `uint32 maxLength` |
| `ID_UPDATE_WVALID` | `string validation` |
| `ID_UPDATE_WSELECTED` | `bool selected` (checkbox) |
| `ID_UPDATE_WRSELECTED` | `NetworkID previous, bool selected` (radio) |
| `ID_UPDATE_WGROUP` | `uint32 group` (radio) |
| `ID_UPDATE_WLMULTI` | `bool multiselect` (list) |
| `ID_UPDATE_WLTEXT` | `string text` (listitem) |
| `ID_UPDATE_WLSELECTED` | `bool selected` (listitem) |
| `ID_UPDATE_WCLICK` | *(empty, GUI click)* |
| `ID_UPDATE_WRETURN` | *(empty, GUI return)* |
| `ID_UPDATE_WMODE` | `bool enabled` (window mode toggle) |

### 3.3 Packet Subtyping System

Packet types use `template<> Cast_<>` specializations to enable polymorphic deserialization. Example: `ID_OBJECT_NEW` also matches `ID_ITEM_NEW`, `ID_CONTAINER_NEW`, `ID_ACTOR_NEW`, `ID_PLAYER_NEW` — the factory creates the right subclass based on packet type tag, then reads base-class fields through the cast.

### 3.4 Packet Channels

```cpp
CHANNEL_SYSTEM = 0   // authentication, disconnect, sync setup
CHANNEL_GAME   = 1   // object state, position, animation, items
CHANNEL_CHAT   = 2   // chat messages
```

---

## 4. Client-Server Sync Flow

### 4.1 Connection Lifecycle

```
1. Client connects to Dedicated server (RakNet)
2. Server sends GAME_START (empty)
3. Client sends GAME_AUTH (name, password)
4. Server creates Player via GameFactory, calls Script callbacks (OnClientAuthenticate, OnPlayerRequestGame)
5. Server sends full game state:
   - GAME_GLOBAL, GAME_WEATHER, GAME_BASE
   - GAME_DELETED (static refs already removed by scripts)
   - For each existing Player: PLAYER_NEW → PLAYER_NEW dispatched to all clients
   - GAME_LOAD (empty) signals state ready
6. Client loads game, sends environment (cell context, interior/exterior)
7. Server sends OBJECT_NEW/ACTOR_NEW/etc for all objects in player's cell context
8. Steady state: position/animation/state updates at ~30Hz
```

### 4.2 Position Sync

**Client→Server:**
- Client polls `GetPos`, `GetAngle` via Interface→DLL→game engine
- Results trigger `ID_UPDATE_POS` (X,Y,Z) and `ID_UPDATE_ANGLE` (X,Y) to server
- Client also sends `ID_UPDATE_CELL` when cell changes

**Server→Clients:**
- Server stores network pos/angle on Object
- On update, server broadcasts to all clients in relevant cell context
- Server validates coordinates (`IsValidCoordinate`)

**Cell Context System (9-cell grid):**
- Player has `CellContext` = `array<uint32, 9>` (current cell + 8 neighbors)
- Cell context changes trigger `ID_UPDATE_CONTEXT`
- Objects in new cells spawn; objects leaving context removed
- Server maintains `CellRefs` map: cell→base→set<refID>

### 4.3 Animation/Actor State Sync

```
Client→Server: ID_UPDATE_STATE (idle, moving animation, weapon animation, alerted, sneaking, firing)
Server→All: rebroadcast (with server-side validation), triggers Script callbacks
```

Actor death: `ID_UPDATE_DEAD` with limbs bitmask and death cause.

### 4.4 Item/Inventory Sync

**Items:** `ID_ITEM_NEW` creates item (container, count, condition, equipped/silent/stick flags).
**Updates:** `ID_UPDATE_COUNT`, `ID_UPDATE_CONDITION`, `ID_UPDATE_EQUIPPED`.
**Equip/Unequip:** Server calls `EquipItem()` on ItemList, finds matching item by baseID, sends equipped state.

### 4.5 Actor Value Sync

`ID_UPDATE_VALUE` (base bool, index uint8, value float) — syncs health, skills, SPECIAL, etc.

### 4.6 Global State

- Weather: `ID_GAME_WEATHER`
- Global values: `ID_GAME_GLOBAL`
- Player base: `ID_GAME_BASE` (base race)
- Deleted static objects: `ID_GAME_DELETED`

### 4.7 GUI Sync

- Server creates Windows/Buttons/etc via Script API → fanout to attached players
- Client sends clicks (`ID_UPDATE_WCLICK`), text changes (`ID_UPDATE_WTEXT`), checkbox/radio toggles
- Server validates, calls script callbacks, broadcasts results

---

## 5. Server Architecture

### 5.1 Deployment Layout

```
vaultserver (dedicated server)
├── Dedicated        — Main loop, RakNet peer, master server announce
├── NetworkServer    — Packet dispatch (ProcessPacket/ProcessEvent)
├── Server           — Game logic: auth, spawn, state handlers, chat
├── Client           — Per-connection state (guid↔id↔player mapping)
├── Database\<T\>    — Template SQLite3 persistence layer
│   ├── Record       — Game ESM records (baseID→name, type, desc)
│   ├── Reference    — Persisted reference IDs
│   ├── Exterior     — Exterior cell data
│   ├── Weapon       — Weapon records
│   ├── Race         — Race records
│   ├── NPC          — NPC records
│   ├── BaseContainer— Container base records
│   ├── Item         — Item base records
│   ├── Terminal     — Terminal records
│   ├── Interior     — Interior cell data
│   └── AcReference  — Activator references
├── Script           — C++/PAWN script bridge
│   ├── ScriptFunction — Timer callback wrapper
│   ├── PAWN          — AMX interpreter interface
│   └── functions[]/callbacks[] — ~160 script APIs + 31 callbacks
├── Timer            — Timer system (ScriptFunc + interval)
├── Public           — Public function registry
└── ServerEntry      — Master server listing data
```

### 5.2 Dedicated Server (Main Loop)

```cpp
Dedicated::InitializeServer(port, host, connections, announce, query, fileserve, fileslots)
  → spawns DedicatedThread()
    → RakPeer startup + master announce
    → Loop:
      - Process RakNet packets (NetworkServer::ProcessPacket)
      - Dispatch queued NetworkResponses
      - Timer::GlobalTick() (script timers)
```

### 5.3 Script System

Two backends:
- **C++ scripts** — `.so`/`.dll` loaded at runtime with `dlopen`/`LoadLibrary`
- **PAWN scripts** — `.amx` compiled bytecode, AMX VM

**31 script callbacks:**
| Callback | Signature |
|----------|-----------|
| `OnCreate` | `(NetworkID object)` |
| `OnDestroy` | `(NetworkID object)` |
| `OnSpawn` | `(NetworkID player)` |
| `OnActivate` | `(NetworkID object, NetworkID actor)` |
| `OnCellChange` | `(NetworkID object, uint32 cell)` |
| `OnLockChange` | `(NetworkID object, NetworkID actor, uint32 lock)` |
| `OnItemCountChange` | `(NetworkID item, uint32 count)` |
| `OnItemConditionChange` | `(NetworkID item, float condition)` |
| `OnItemEquippedChange` | `(NetworkID item, bool equipped)` |
| `OnActorValueChange` | `(NetworkID actor, uint8 index, float value)` |
| `OnActorBaseValueChange` | `(NetworkID actor, uint8 index, float value)` |
| `OnActorAlert` | `(NetworkID actor, bool alerted)` |
| `OnActorSneak` | `(NetworkID actor, bool sneaking)` |
| `OnActorDeath` | `(NetworkID actor, NetworkID killer, uint16 limbs, int8 cause)` |
| `OnActorPunch` | `(NetworkID actor, bool powerPunch)` |
| `OnActorFireWeapon` | `(NetworkID actor, uint32 weapon)` |
| `OnPlayerDisconnect` | `(NetworkID player, Reason reason)` |
| `OnPlayerRequestGame` | `(NetworkID player) → uint32 (baseID)` |
| `OnPlayerChat` | `(NetworkID player, string message) → bool` |
| `OnWindowMode` | `(NetworkID player, bool enabled)` |
| `OnWindowClick` | `(NetworkID player, NetworkID window)` |
| `OnWindowReturn` | `(NetworkID player, NetworkID window)` |
| `OnWindowTextChange` | `(NetworkID player, NetworkID window, string text)` |
| `OnCheckboxSelect` | `(NetworkID player, NetworkID checkbox, bool selected)` |
| `OnRadioButtonSelect` | `(NetworkID player, NetworkID radio, NetworkID previous)` |
| `OnListItemSelect` | `(NetworkID player, NetworkID listitem, bool selected)` |
| `OnClientAuthenticate` | `(string name, string password) → bool` |
| `OnGameTimeChange` | `(uint32 year, uint32 month, uint32 day, uint32 hour)` |
| `OnServerInit` | `()` |
| `OnServerExit` | `(bool restart)` |

### 5.4 Database Schema

SQLite3 databases stored as separate files:
- `fallout3.sqlite3` — main game data (Records, Weapons, Races, NPCs, Containers, Items, Terminals, Interiors)
- `vaultserver/data/` — server-specific data (References, Exteriors, AcReferences)

**Record fields:** `baseID, name, description, type`

### 5.5 Persistence Logic

`GameFactory` holds static `Database<T>` instances. On server startup, databases initialized from SQLite files. When objects created/modified/deleted, database updated. References persisted across restarts via `DB::Reference`.

---

## 6. Client Architecture

### 6.1 Process Layout

```
vaultmp.exe (client GUI + network)
  └── Game              — orchestrator, net handlers, object lifecycle
  └── NetworkClient     — RakNet packet processor
  └── Interface         — command pipe to DLL
  └── vaultgui (CEGUI)  — in-game overlay

vaultmpdll.dll (injected into Fallout3.exe)
  └── Hooks game functions
  └── Pipe server — receives commands, returns results
```

### 6.2 Interface / API / Pipe

The Interface class schedules game commands with priority ordering:
- Static commands — run every frame (poll positions, angles, actor states)
- Dynamic commands — run once (spawn object, set value)
- Job commands — delayed execution

Commands serialized as `(opcode, parameters...)` via named pipe to DLL, results returned asynchronously to `Game::CommandHandler`.

---

## 7. Master Server

- `vaultmaster` — separate binary, server browser
- Servers announce themselves via `ID_MASTER_ANNOUNCE` to master
- Master responds to client queries with `ID_MASTER_QUERY`
- Server entry includes name, map, rules, player count
- Default master port: 1660

---

## 8. Key Dependencies

| Library | Usage |
|---------|-------|
| **RakNet** | UDP reliable/unreliable networking, file transfer, NAT punchthrough, ReplicaManager |
| **CEGUI** | In-game GUI overlay (vaultgui) |
| **SQLite3** | Server-side data persistence |
| **Pawn/AMX** | Embedded scripting VM |
| **Boost** | `boost::any` for script argument passing |
| **pthread** (Unix) / **Win32 threads** | Threading |

---

## 9. Constants

```cpp
MAX_PLAYER_NAME    = 16
MAX_PASSWORD_SIZE  = 16
MAX_MASTER_SERVER  = 32
MAX_MOD_FILE       = 64
MAX_CELL_NAME      = 36
MAX_MESSAGE_LENGTH = 64
MAX_CHAT_LENGTH    = 128
PIPE_LENGTH        = 2048
RAKNET_STANDARD_PORT        = 1770
RAKNET_MASTER_STANDARD_PORT = 1660
RAKNET_STANDARD_CONNECTIONS = 4
RAKNET_FILE_SERVER          = 1550
```

---

## 10. Operational Notes for Rust Recreation

### What to Preserve

1. **Bitmask type system** — Rust enums with `#[repr(u32)]` + bit patterns
2. **Packet factory pattern** — Use a trait-based serializer; no runtime casting needed in Rust
3. **Cell context sync (9-cell grid)** — Same approach; rayon for parallel cell loading
4. **Server-authoritative state** — Server owns truth for position, items, actor values
5. **Script callback model** — 31 callbacks as trait or enum; Wasmtime for scripting
6. **SQLite persistence** — `rusqlite` with same schema

### What to Replace

| C++ | Rust |
|-----|------|
| RakNet | `tokio::net::UdpSocket` + custom reliability layer or QUIC |
| CEGUI | `egui` or `iced` for GUI |
| Pawn AMX VM | `wasmtime` (WASM scripting) |
| Named pipes (Windows) | Unix domain sockets / `tokio::net::UnixStream` |
| CriticalSection/Guarded | `Arc<RwLock<T>>` / `tokio::sync::RwLock` |
| GameFactory singleton | `Arc<DashMap<NetworkID, Arc<dyn GameObject>>>` |
| boost::any | `Box<dyn Any>` or serde-based tagged values |
