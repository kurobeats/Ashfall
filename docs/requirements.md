# Ashfall — Functional Requirements
Extracted from vaultmp-extended v0.1a.

---

## 1. Networking

### 1.1 Client-Server Architecture
- **Transport**: RakNet peer-to-peer reliable/unreliable UDP messaging.
- **Channels**: 3 ordered channels — `CHANNEL_SYSTEM`, `CHANNEL_GAME`, `CHANNEL_CHAT`.
- **Packet factory**: Typed packet system (`PacketFactory`) with `PF_MAKE` macros, each packet type has a unique ID, auto-serialization of fields.
- **Packet priority/reliability**: `PacketPriority` + `PacketReliability` per message.
- **Network queue**: Deferred dispatch via `Network::Queue()` / `Network::Dispatch()`.
- **NetworkID**: Every game object gets a `RakNet::NetworkID` managed by `NetworkIDManager`.
- **Max connections**: `RAKNET_STANDARD_CONNECTIONS` = 4 per server.
- **Ports**:
  - Game server: `1770` (standard)
  - Master server: `1660` (standard)
  - File server: `1550`
- **Connection flow**: Client connects → version check → authenticate (name + password) → load game world → player spawn.

### 1.2 Master Server
- Maintains list of active dedicated servers.
- Servers announce themselves with `ID_MASTER_ANNOUNCE`.
- Clients query with `ID_MASTER_QUERY`.
- Server entries carry: name, map, player count (current/max), rules (key-value), mod file list.
- Update per-server with `ID_MASTER_UPDATE`.
- Ping measurement via `ID_UNCONNECTED_PONG`.

### 1.3 Server Browser (Client)
- Win32 GUI with server list (name, players, ping, map).
- Sortable columns.
- Server detail pane showing rules.
- "Get servers", "Update server", "Join server" buttons.
- Connect/disconnect with error handling.

### 1.4 Authentication
- Client sends `Authenticate(name, password)` → `ID_GAME_AUTH` packet.
- Server validates against configured credentials (script callback `OnClientAuthenticate`).
- Max name length: `MAX_PLAYER_NAME` = 16 chars.
- Max password length: `MAX_PASSWORD_SIZE` = 16 chars.
- Rejection reasons: `ID_REASON_KICK`, `ID_REASON_BAN`, `ID_REASON_ERROR`, `ID_REASON_DENIED`.

### 1.5 File Transfer
- RakNet `FileListTransfer` over `PacketizedTCP`.
- Server can serve mod files (`.esp`, `.esm`, archives).
- Client downloads before joining; CRC32 checked against `VAULTMP_F3`.
- Progress indication.
- Overwrite confirmation for existing files.
- File transfer slots configurable (`RAKNET_FILE_SERVER`).

---

## 2. World Sync

### 2.1 Object Hierarchy
```
Base
└── Reference (refID, baseID)
    └── Object (name, pos, angle, cell, enabled, lock, owner)
        ├── Item (container, count, condition, equipped, silent, stick)
        ├── Container (Object + ItemList)
        │   └── Actor (actor values, race, age, animations, states, dead)
        │       └── Player (controls, respawn, cell context, console, windows)
        └── Window (GUI root)
            ├── Button
            ├── Text
            ├── Edit
            ├── Checkbox
            ├── RadioButton
            ├── ListItem
            └── List
```

### 2.2 Object State Synchronization
- **Position**: `(X, Y, Z)` — game pos (actual) and network pos (authoritative). Validated for NaN/inf bounds.
- **Angle**: `(X, Y, Z)` euler angles.
- **Cell**: `unsigned int` cell ID. Network cell vs game cell; `SetCell` event moves object between cells.
- **Name**: string name.
- **Enabled state**: toggle visibility.
- **Lock level**: `unsigned int`.
- **Owner**: `unsigned int` (reference ID of owner).
- **Volatile objects**: Named temporary objects placed without persistence.

### 2.3 Cell System
- Interior cells: named cells loaded via `CenterOnCell`.
- Exterior cells: loaded via `CenterOnExterior(x, y)` or `CenterOnWorld(baseID, x, y)`.
- Cell context: 3×3 grid of cells around a player (`CellContext` — array of 9 cell IDs).
- Context updates: `net_UpdateContext` syncs the cell grid to clients.
- Cell reference tracking: `CellRefs` — map of `worldID → cellID → set<refID>`.
- `IsInContext(cell)` checks if cell is in player's context grid.
- `GetAnchor(cell)` returns first reference in cell.
- Deleted objects tracked per cell (`DeletedObjects`).

### 2.4 Actor Synchronization
- **Actor values**: Per-value (indexed by hex code), both current value and base value. Default value map exists.
- **States**: idle animation, moving animation, moving XY direction, weapon animation, alerted, sneaking, dead.
- **Race**: base race ID + age delta.
- **Sex**: male/female.
- **Death**: dead flag, limb state bitmask, cause code.
- **Firing**: weapon fire event with weapon baseID.
- **Attacking**: jumping, punching, power-punching detection.
- **Equipped weapon**: server tracks currently equipped weapon baseID.

### 2.5 Item Synchronization
- **Container parent**: which container holds this item.
- **Count**: stack count.
- **Condition**: float 0.0–1.0 + health value.
- **Equipped state**: equipped + silent + stick flags.
- Operations: `AddItem`, `RemoveItem`, `EquipItem`, `UnequipItem`, `SetItemCount`, `SetItemCondition`.

### 2.6 Container Synchronization
- `ItemList` interface: list of contained items.
- Operations: add/remove items, query item count by baseID, get item list.

### 2.7 Weather
- Single global weather state (`Weather` = `unsigned int`).
- `SetWeather` / `GetWeather` for all clients.

### 2.8 Globals
- Global variable map (`Globals`): `baseID → value`.
- Server sets, all clients receive.

### 2.9 Game Time
- Server manages game time: year, month, day, hour.
- Time scale configurable.
- `OnGameTimeChange` callback.

---

## 3. Player Management

### 3.1 Connection Lifecycle
1. Client connects to server with version match.
2. `Authenticate(name, pwd)` sent → server calls `OnClientAuthenticate`.
3. On success: server sends `LoadGame` — full world state.
4. Server creates `NewPlayer` — spawns player entity.
5. Client runs `Startup()` — enters game loop.
6. Disconnect: `ID_GAME_END` packet with `Reason` code.
7. `OnPlayerDisconnect` callback on server.

### 3.2 Spawn & Respawn
- Default spawn cell: configurable per-server, per-player override.
- Default respawn time: `DEFAULT_PLAYER_RESPAWN` = 8000 ms.
- `OnPlayerRequestGame` callback returns cell ID for spawn.
- `RespawnTime` per-player override.
- `ForceRespawn()` forces player back to menu.

### 3.3 Player Controls
- `EnablePlayerControls` / `DisablePlayerControls` with per-control flags:
  - movement, pipboy, fighting, POV, looking, rollover, sneaking.
- `ToggleKey(enabled, scancode)` — enable/disable specific keys.
- `ToggleControl(enabled, control)` — enable/disable game controls.
- Control state synced: each control has key binding + enabled state.
- `GetPlayerControl(control)` returns key; `GetPlayerControlEnabled(control)` returns state.
- Default console enabled state per-server.

### 3.4 Chat
- `ChatMessage` broadcast or targeted.
- In-game UI display via CEGUI overlay.
- `OnPlayerChat` callback for filtering/moderation.
- Max chat length: `MAX_CHAT_LENGTH` = 128 chars.
- Chat channel separate from game/sync.

### 3.5 Player State (Server-Side)
- Each player has `AttachedWindows` list for GUI association.
- `BaseIDTracker` — set of all base IDs used by players.
- `WindowTracker` — which players have which windows attached.
- `CellContext` per-player for visibility/interest management.

---

## 4. GUI System (Server-Authoritative Overlay)

### 4.1 GUI Hierarchy
```
Window (root)
├── Button
├── Text (label)
├── Edit (input field)
├── Checkbox
├── RadioButton (grouped)
├── ListItem
└── List (container for ListItems)
```

All GUI elements are game objects with NetworkIDs, created/destroyed/modified from server scripts.

### 4.2 Window
- **Properties**: position `(X, Y, offsetX, offsetY)`, size `(X, Y, offsetX, offsetY)`, visible, locked.
- **Text**: display text.
- **Parent/child**: hierarchy management (`AddChildWindow` / `RemoveChildWindow`).
- **Mode**: window mode toggle (enables/disables full GUI).
- Script API: `CreateWindow`, `DestroyWindow`, `SetWindowPos`, `SetWindowSize`, `SetWindowVisible`, `SetWindowLocked`, `SetWindowText`, `GetWindowParent`, `GetWindowRoot`, `GetWindowChildCount`, `GetWindowChildList`.

### 4.3 Button
- Creates with position, size, visible, locked, text.
- Click callback: `OnWindowClick(windowID, playerID)`.

### 4.4 Text (Label)
- Static text display.
- Can be updated via script (`SetWindowText`).

### 4.5 Edit (Input)
- Text input with max length constraint (`SetEditMaxLength`).
- Input validation regex (`SetEditValidation`).
- Return callback: `OnWindowReturn(windowID, playerID)`.
- Text change callback: `OnWindowTextChange(windowID, playerID, text)`.

### 4.6 Checkbox
- Creates with position, size, visible, locked, text.
- Selected state: `SetCheckboxSelected` / `GetCheckboxSelected`.
- Select callback: `OnCheckboxSelect(windowID, playerID, selected)`.

### 4.7 RadioButton
- Creates with position, size, visible, locked, text.
- Selected state + group ID.
- `SetRadioButtonSelected` / `SetRadioButtonGroup`.
- Select callback: `OnRadioButtonSelect(windowID, playerID, previousID)`.

### 4.8 List / ListItem
- List creates with position, size, visible, locked, text, multiselect flag.
- `AddListItem` / `RemoveListItem` child management.
- Each ListItem has text, selected state, container relationship.
- `SetListItemSelected` / `SetListItemText`.
- Select callback: `OnListItemSelect(windowID, playerID, selected)`.
- Query: `GetListItemCount`, `GetListItemList`, `GetListSelectedItemCount`, `GetListSelectedItemList`.
- Chatbox: special window type with `IsChatbox` / `GetPlayerChatboxWindow`.

### 4.9 Client-Side GUI Events
- Window mode toggle → `GetWindowMode` packet.
- Window click → `GetWindowClick` packet.
- Window return (enter) → `GetWindowReturn` packet.
- Window text change → `GetWindowText` packet.
- Checkbox select → `GetCheckboxSelected` packet.
- RadioButton select → `GetRadioButtonSelected` packet.
- ListItem select → `GetListboxSelections` packet.

---

## 5. Scripting

### 5.1 Script Engine
- **Pawn**: AMX virtual machine embedded in server. `.amx` compiled scripts loaded at startup.
- **C++ scripts**: Native shared libraries loaded via `dlopen`/`LoadLibrary`.
- Both script types coexist; callbacks fire on all loaded scripts.

### 5.2 Script Loading
- Config path: `SCRIPTS_PATH` = `"scripts"`.
- Pawn file path: `PWNFILES_PATH` = `"AMXFILE=files"`.
- `LoadScripts(scripts, base)` initializes both C++ and Pawn scripts.
- `Initialize()` called after load.
- `UnloadScripts()` on shutdown.

### 5.3 Pawn Integration
- `PAWN::LoadProgram` — load `.amx` from file or memory.
- `PAWN::Init` — initialize VM.
- `PAWN::Exec` — execute entry point.
- `PAWN::Call` — call named function with format string and args.
- `PAWN::IsCallbackPresent` — check if callback defined.
- Timers: `CreateTimer` / `CreateTimerEx` for Pawn scripts.

### 5.4 Server Script API (All functions available to scripts)
**Server management**:
- `SetServerName`, `SetServerMap`, `SetServerRule`
- `GetMaximumPlayers`, `GetCurrentPlayers`
- `timestamp`

**Timers**:
- `CreateTimer`, `CreateTimerEx`, `KillTimer`
- `MakePublic`, `CallPublic`

**Player actions**:
- `Kick`, `UIMessage`, `ChatMessage`
- `SetRespawnTime`, `SetSpawnCell`, `SetConsoleEnabled`
- `ForceWindowMode`

**World state**:
- `SetGameWeather`, `GetGameWeather`
- `SetGameTime`, `GetGameTime`
- `SetTimeScale`, `GetTimeScale`
- `SetGameYear/Month/Day/Hour`, `GetGameYear/Month/Day/Hour`

**Type queries**:
- `IsValid`, `IsReference`, `IsObject`, `IsItem`, `IsContainer`, `IsActor`, `IsPlayer`
- `IsCell`, `IsInterior`, `IsItemList`
- `IsWindow`, `IsButton`, `IsText`, `IsEdit`, `IsCheckbox`, `IsRadioButton`, `IsListItem`, `IsList`, `IsChatbox`
- `GetType`, `GetConnection`, `GetCount`, `GetList`

**Object operations**:
- `CreateObject`, `CreateVolatile`, `DestroyObject`
- `GetID`, `GetReference`, `GetBase`, `GetBaseName`
- `GetPos`, `SetPos`, `GetAngle`, `SetAngle`
- `GetCell`, `SetCell`
- `GetLock`, `SetLock`, `GetOwner`, `SetOwner`
- `Activate`, `PlaySound`, `IsNearPoint`, `SetBaseName`

**Item operations**:
- `CreateItem`, `SetItemContainer`, `GetItemContainer`
- `GetItemCount`, `SetItemCount`
- `GetItemCondition`, `SetItemCondition`
- `GetItemEquipped`, `SetItemEquipped`
- `GetItemSilent`, `GetItemStick`

**Container operations**:
- `CreateContainer`, `CreateItemList`, `DestroyItemList`
- `GetContainerItemCount`, `GetContainerItemList`
- `AddItem`, `RemoveItem`, `RemoveAllItems`, `AddItemList`

**Actor operations**:
- `CreateActor`
- `GetActorValue`, `SetActorValue`
- `GetActorBaseValue`, `SetActorBaseValue`
- `GetActorIdleAnimation`, `PlayIdle`
- `GetActorMovingAnimation`, `SetActorMovingAnimation`
- `GetActorWeaponAnimation`, `SetActorWeaponAnimation`
- `GetActorAlerted`, `SetActorAlerted`
- `GetActorSneaking`, `SetActorSneaking`
- `GetActorDead`, `KillActor`
- `GetActorBaseRace`, `SetActorBaseRace`, `AgeActorBaseRace`
- `GetActorBaseSex`, `SetActorBaseSex`
- `IsActorJumping`
- `EquipItem`, `UnequipItem`, `FireWeapon`

**Player queries**:
- `GetPlayerRespawnTime`, `GetPlayerSpawnCell`
- `GetPlayerConsoleEnabled`
- `GetPlayerWindowCount`, `GetPlayerWindowList`
- `GetPlayerChatboxWindow`

**GUI management** (full CRUD):
- Window: `CreateWindow`, `DestroyWindow`, `AddChildWindow`, `RemoveChildWindow`, `SetWindow*`, `GetWindow*`
- Button: `CreateButton`
- Text: `CreateText`
- Edit: `CreateEdit`, `SetEditMaxLength`, `SetEditValidation`, `GetEdit*`
- Checkbox: `CreateCheckbox`, `SetCheckboxSelected`, `GetCheckboxSelected`
- RadioButton: `CreateRadioButton`, `SetRadioButtonSelected`, `SetRadioButtonGroup`, `GetRadioButton*`
- List: `CreateList`, `SetListMultiSelect`, `GetListMultiSelect`
- ListItem: `AddListItem`, `RemoveListItem`, `SetListItemContainer`, `SetListItemSelected`, `SetListItemText`, `GetListItem*`

**Utility**:
- `ValueToString`, `AxisToString`, `AnimToString`, `BaseToString`, `BaseToType`

### 5.5 Script Callbacks (Server → Script)
- `OnServerInit()` — server startup.
- `OnServerExit(bool shutdown)` — server shutdown.
- `OnClientAuthenticate(name, pwd) -> bool` — auth filter.
- `OnPlayerDisconnect(id, reason)` — player left.
- `OnPlayerRequestGame(id) -> cell` — spawn cell selection.
- `OnPlayerChat(id, message) -> bool` — chat filter.
- `OnCreate(id)` — object created.
- `OnDestroy(id)` — object destroyed.
- `OnSpawn(id)` — player spawned.
- `OnActivate(refID, actorID)` — object activated.
- `OnCellChange(id, cell)` — object moved between cells.
- `OnLockChange(id, actorID, lock)` — lock state changed.
- `OnItemCountChange(id, count)` — item stack changed.
- `OnItemConditionChange(id, condition)` — item durability changed.
- `OnItemEquippedChange(id, equipped)` — equip state changed.
- `OnActorValueChange(id, index, value)` — actor value changed.
- `OnActorBaseValueChange(id, index, value)` — base value changed.
- `OnActorAlert(id, alerted)` — alert state changed.
- `OnActorSneak(id, sneaking)` — sneak state changed.
- `OnActorDeath(id, killerID, limbs, cause)` — actor died.
- `OnActorPunch(id, power)` — actor punched.
- `OnActorFireWeapon(id, weaponID)` — actor fired weapon.
- `OnWindowMode(id, enabled)` — GUI mode toggle.
- `OnWindowClick(windowID, playerID)` — button clicked.
- `OnWindowReturn(windowID, playerID)` — edit submitted.
- `OnWindowTextChange(windowID, playerID, text)` — text changed.
- `OnCheckboxSelect(windowID, playerID, selected)` — checkbox toggled.
- `OnRadioButtonSelect(windowID, playerID, previousID)` — radio selection.
- `OnListItemSelect(windowID, playerID, selected)` — list item selection.
- `OnGameTimeChange(year, month, day, hour)` — time advanced.

---

## 6. Persistence

### 6.1 SQLite Database
- Database file: `fallout3.sqlite3` in data directory.
- Template class `Database<T>` manages typed rows.
- Tables per type: `Record`, `Reference`, `Exterior`, `Weapon`, `Race`, `NPC`, `BaseContainer`, `Item`, `Terminal`, `Interior`, `AcReference`.
- `initialize(file, tables)` loads from DB at server startup.
- Schema defined by `DB::Record`, `DB::Reference`, etc. structs (each has `Create`, `SQL` queries, `Load`).
- Records map Fallout 3 base IDs to in-game asset data.
- References store persistent game object state.
- Exteriors map worldspace/cell data.

### 6.2 Persistent State
- All server-side objects (`Object`, `Item`, `Container`, `Actor`) track mutable state in memory.
- `GameFactory` owns all instances; lifetime managed via reference counting and deletion tracking.
- `BaseDeleted` set tracks destroyed NetworkIDs to prevent reuse.
- `BaseCount` per-type for object counts.

---

## 7. DLL Injection & IPC

### 7.1 Injected DLL (vaultmpdll)
- DLL injected into `Fallout3.exe` process.
- Hooks game functions for state interrogation and manipulation.
- Communicates with main client process via named pipes.

### 7.2 Pipe IPC
- **Pipe protocol**:
  - Buffer size: `PIPE_LENGTH` = 2048 bytes.
  - Opcodes: `PIPE_SYS_WAKEUP`, `PIPE_OP_COMMAND`, `PIPE_OP_RETURN`, `PIPE_OP_RETURN_BIG`, `PIPE_OP_RETURN_RAW`, `PIPE_ERROR_CLOSE`.
- **PipeServer**: creates named pipe, accepts connections, `Send`/`Receive` raw byte streams.
- **PipeClient**: connects to existing pipe server.
- **Interface class**: sits between game logic and pipe layer, translates API commands to pipe messages and routes responses via `ResultHandler`.

### 7.3 Command Interface
- `API` maps opcodes (`Values::Func`) to engine commands.
- Command system with priority-based scheduling:
  - `SetupCommand`: persistent scheduled commands (run every frame).
  - `ExecuteCommand`: one-shot commands.
  - `PushJob`: time-delayed jobs.
- 3 threads: `CommandThreadReceive`, `CommandThreadSend`, `CommandThreadJob`.
- `wakeup` atomic flag for signaling.
- `endThread` / `shutdown` for lifecycle.
- `ResultHandler` callback for async command results.

### 7.4 Game Boot Sequence
1. Check Fallout3.exe version (CRC32 = `FALLOUT3_EN_VER17`).
2. Check FOSE version (`FOSE_VER0122`).
3. Check xlive.dll patch (`XLIVE_PATCH`).
4. Check vaultmp.esp CRC32 (`VAULTMP_F3`).
5. Check vaultmp.dll CRC32 (`VAULTMP_DLL`).
6. Read `vaultmp.ini` config.
7. Initialize RakNet.
8. Launch Fallout3.exe with DLL injection.
9. Wait for pipe connection from DLL.
10. Execute init commands with configurable delay (`inittime`, default 9000ms).
11. `Interface::Initialize(ResultHandler)` → start command threads.
12. `Game::Startup()` → begin game loop.

### 7.5 Cleanup
- On game completion: `Game::LoadGame(savegame)` or `ForceRespawn()`.
- Disconnect: `peer->CloseConnection`.
- Shutdown: `Interface::SignalEnd()` → wait for `HasShutdown()`.
- Terminate pipe connections.
- Clean RakNet, destroy mutex, free config.

---

## 8. Configuration

### 8.1 INI Files
- **Client**: `vaultmp.ini` — `general:name`, `general:master`, `general:inittime`, `general:multiinst`, `general:servers`.
- **Server**: `vaultserver.ini` — port, connections, host, announce address, query, fileserve, fileslots.

### 8.2 Dependencies (C++ codebase)
- RakNet (networking, bundled).
- Pawn/AMX (scripting, bundled).
- SQLite3 (database, bundled).
- CEGUI (client GUI, bundled in vaultgui).
- iniparser (config parsing, bundled).
- FOSE (Fallout Script Extender, external).
- uFMOD (chiptune playback, bundled).
- exchndl.dll (crash handler, debug only).

---

## 9. Safety & Validation

### 9.1 Crash Handling
- `VaultException` with stack trace on all errors.
- `Expected<T>` type for fallible operations.
- Debug build with `exchndl.dll` crash handler.

### 9.2 Input Validation
- Position coordinates validated (`IsValidCoordinate`, `IsValidAngle`).
- Name/password length limits enforced.
- Version checks (CRC32) on all binaries and data files.
- Mod file integrity via CRC32.

### 9.3 Concurrency
- `CriticalSection` / `Lockable` / `Guarded<T>` for thread-safe access.
- `Value<T>` with lock-before-write semantics.
- `Shared<T>` for promise/future pattern.
- Atomic flags for thread signaling.

---

*Extracted from vaultmp-extended source at `/home/ants/dev/vaultmp-extended`.*
*Date: 2026-07-10*
