# GECK Script Surface — Host Function Roadmap

Sources (both MIT, copied into `docs/geck/`):

- `geck_function_index.txt` — 2,580 GECK/NVSE/JIP/kNVSE/JohnnyGuitar/ShowOff
  script functions with signatures, auto-generated from the GECK wiki
  (geckwiki.com Special:Export). Origin:
  [knork-fork/mojave-online](https://github.com/knork-fork/mojave-online) `geck_wiki_resource/`.
- `geck_anim_groups.txt` — 249 animation groups for `PlayGroup` (from the
  same wiki export).

Use `scripts/geck-search.sh <pattern>` to grep the index (handles the
`Name | Origin | Signature` format).

## Current Ashfall surface vs GECK

Ashfall exposes **56 host functions** (`crates/ashfall-server/src/script/host.rs`)
and **35 callbacks** (`crates/ashfall-server/src/script/callbacks.rs`). The
game script engine has 2,580 — the SDK is a working subset, not the surface.

| Ashfall host fn | GECK equivalent | Notes |
|---|---|---|
| `get_pos_*` / `set_pos` | `GetPos`, `SetPos` | per-axis host fns; `SetPos` = `MoveTo`-free teleport |
| `get/set_actor_value` | `GetActorValue` / `SetActorValue` | SetAV writes BASE value — matches vaultmp semantics |
| `kill_actor` | `Kill` | no `Resurrect` host fn yet |
| `create_object` / `destroy_object` | `PlaceAtMe` / `RemoveMe` | server-side entity creation (no game-side spawn yet) |
| `create_item` / `add/remove/equip_item` | `AddItem` / `RemoveItem` / `EquipItem` | |
| `get/set_quest_stage` | `GetStage` / `SetStage` | `GetStage` = highest completed stage |
| `get/set_dialogue_flag` | `SetDialogInProgress`-family | dialogue flags are the FO3/FNV dialog state bits |
| `get/set_game_weather` | `GetWeather` / `SetWeather` | no `ForceWeather` override mode |
| `get/set_game_time`, `set_time_scale` | `SetGameHour` / `SetGameTimeScale` | |
| `ui_message` | `ShowMessage` / `ShowNote` | |
| `chat_message` | — | MP-only, no GECK equivalent |
| `create_window/button/...` | — | server-authored GUI, MP-only |
| `get_damage_resistance/threshold` | `GetDamageResistance` / `GetDamageThreshold` | FNV DT is `AV_DamageThreshold` |
| `timestamp`, `get_config_int` | — | engine/meta, MP-only |

## Recommended additions (co-op game-mode value × server data available)

Priority order. Signatures verbatim from `geck_function_index.txt`.

### P1 — lock/door sync (server already tracks lock state in `refs`)

```
Lock       | [ObjectRefID].Lock(LockLevel:int)
Unlock     | [ObjectRefID].Unlock()
```

Host fns `lock_object(ref, level)` / `unlock_object(ref)` — writes the
server-side lock state and broadcasts it, closing the loop on the existing
`on_lock_change` callback.

### P1 — actor lifecycle (co-op revival, respawn gating)

```
Resurrect   | actor.Resurrect(flag:int?)        (NVSE/JIP variants add more)
Kill        | [ActorRefID].Kill(ActorRefID:ref, CauseofDeath:int, ...)
```

`kill_actor` exists; add `resurrect_actor(ref)`. With the bridge's respawn
patch (`hooks::vaultmp`) dead players stay dead until the server revives —
this host fn is the revive path.

### P1 — movement (teleport parties, cell grouping)

```
MoveTo   | [Object].MoveTo(MarkerID:ref, OffsetX:float?, OffsetY:float?, OffsetZ:float?)
```

`move_to(ref, target_ref, offset)` — ref-to-ref teleport, needed for co-op
"follow the leader" and cell-join warp. `set_pos` is absolute; `MoveTo` is
relative to another ref.

### P2 — faction & karma (server already has `factions`, `karma`, `reputation` tables)

```
AddToFaction      | [ActorRefID].AddToFaction(FactionID:ref, FactionRank:int)
RemoveFromFaction | [actor:ref].RemoveFromFaction(faction:baseform)
```

Host fns `add_to_faction(actor, faction, rank)` / `remove_from_faction(...)`
persist to the faction tables and broadcast.

### P2 — animation (bridge animation controller emits PlayGroup opcodes)

```
PlayGroup | Actor.PlayGroup(AnimGroup:string, InitFlag:int)
PlayIdle  | Actor.PlayIdle(IdleAnim:ref, ...)
```

The bridge (`hooks::animation`) already translates actor state → numeric
`PlayGroup`. A host fn lets game modes trigger one-shot animations on remote
actors (wave, point, sit). InitFlag 1 = immediate start.

### P2 — quest objectives (quest tables exist; objectives don't yet)

```
SetObjectiveDisplayed | SetObjectiveDisplayed(Quest:baseform, objectiveIndex:int, displayedFlag:int)
GetObjectiveCompleted | GetObjectiveCompleted(quest:baseform, objectiveIndex:int) -> bool
```

### P3 — weather override + sounds

```
ForceWeather | ForceWeather(WeatherID:ref, WeatherOverrideFlag:bool?)
PlaySound3D  | [Reference].PlaySound3D(soundID:baseform)
```

### P3 — ini + cells (server-side tuning knobs)

```
SetINISetting | SetINISetting(string:iniSetting, int:value)   (vaultmp used 0x0125)
CenterOnCell  | CenterOnCell(cell:baseform)                   (0x0123 in vaultmp)
CenterOnExterior | CenterOnExterior(gridX:int, gridY:int, ...)
```

## How to add one

1. Add the `func_wrap` in `crates/ashfall-server/src/script/host.rs` (`env` module).
2. Persist/broadcast via the packet that owns the concern (lock → `UpdateLock`,
   faction → new packet or reuse `UpdateActorState`-adjacent).
3. If it touches the game side, add the opcode to the bridge `commands.rs`
   table and the `hooks::animation` / `hooks::vaultmp` path.
4. Grep the index for the full signature + aliases first:
   `scripts/geck-search.sh Lock`

## Verify against the real engine

The `UpdateActorState { idle, moving, moving_xy, weapon, alerted, sneaking,
firing }` wire format matches vaultmp's actor-state encoding 1:1 (see
`crates/ashfall-bridge/src/hooks/animation.rs`), so anim-group numbers in
`geck_anim_groups.txt` map directly to `PlayGroup` numeric opcodes.
