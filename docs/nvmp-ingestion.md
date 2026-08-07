# NVMP Lineage Ingestion — Sources, Assets, How to Use

What was taken from the New Vegas Multiplayer project lineage (the
[github.com/NVMP](https://github.com/NVMP) org itself holds only the
launcher + balance ESPs; the actual codebase lives in its continuation
forks) and where it landed in Ashfall. All sources MIT (Apache-2.0 +
Commons Clause for ClientLauncher — not ingested beyond the ESPs).

| Source repo | What it is | License |
|---|---|---|
| [foxtacles/vaultmp](https://github.com/foxtacles/vaultmp) | Original full MP mod (FO3), 104k lines C++ | MIT |
| [knork-fork/mojave-online](https://github.com/knork-fork/mojave-online) | NVMP renamed — FNV client + full NVSE source + GECK wiki dump | MIT |
| [NVMP/PublicServerMods](https://github.com/NVMP/PublicServerMods) | 4 server balance ESPs | — |
| [NVMP/ClientLauncher](https://github.com/NVMP/ClientLauncher) | C# patcher/launcher, exe checksums, EOS auth | Apache-2.0 + Commons Clause |

## Ingests

### 1. `crates/ashfall-bridge/src/hooks/vaultmp.rs`
Full classic-Steam FO3 1.7 address table (~35 addresses, `FO3_STEAM_CLASSIC`)
+ the complete `PatchGame()` sequence from `vaultmpdll/vaultmp.cpp` as 34
byte-exact `Recipe`s: respawn disable, AI pause in unloaded cells, race
matching on spawn, lock fix, PlayGroup/idle delegator, actor-value fix,
fire/activate/PlaceAtMe interception, Plugins.txt redirect.

⚠️ **Classic Steam build only.** Ashfall's verified table is GOG 1.7.0.3;
the Steam build differs (docs/proton-testing.md). Use `apply()` only after
re-deriving addresses per build (`OP_DUMP_IMAGE` + `memory::find_pattern`).
Hooks (`REQUIRED_HOOKS`) are Rust functions the caller supplies at install.

### 2. `crates/ashfall-bridge/src/hooks/animation.rs`
Remote-actor animation state machine — port of vaultmp `Game::net_SetActorState`
(anim-group → `PlayGroup` opcode dispatch, Aim↔AimIS sequence, firing
suppression, strafe yaw adjust) + mojave-online locomotion names. Pure,
tested, no unsafe. Wire-in: engine-side PlayGroup executor + client→bridge
`OP_PLAY_GROUP` pipe command (see README "What's Left").

### 3. `crates/ashfall-client/src/world/state.rs` + `world/registry.rs`
Render-behind interpolation buffer from mojave-online `fnvmp/game/interpolation.cpp`:
`INTERP_DELAY` 67ms (2 packet intervals), velocity extrapolation, 500ms
`EXTRAP_TIMEOUT` freeze. Replaces the old last-two-sample lerp.

### 4. `docs/geck/`
- `geck_function_index.txt` — 2,580 GECK/NVSE/JIP/kNVSE functions w/ signatures.
- `geck_anim_groups.txt` — 249 `PlayGroup` animation groups.
- `host-function-roadmap.md` — Ashfall's 56 host fns vs the GECK surface,
  prioritized additions (lock/unlock, resurrect, MoveTo, factions, quest
  objectives, ForceWeather, SetINISetting) with verbatim signatures.
- `scripts/geck-search.sh` — grep helper.

### 5. `data/plugins/` — NVMP server-mod ESPs
`NVMP-Q.esp`, `NVMP_-_Pres.esp`, `NVMP_-_Rebalance.esp`, `NVMP_-_Wachter.esp`
import cleanly without masters (285/41/54/62 records; overrides only).
Import: `ashfall-server --import-esm data/plugins/NVMP-Q.esp --import-game fnv
--import-db out.sqlite3`.

### 6. `scripts/verify-esm-dumps.py`
Cross-checks an imported DB against the vaultmp `other/data3` dumps (FO3 +
DLC record counts). Handles dump quirks: zone files overlap (dedupe) and
placeholder mod indices (compare low-24 bits; DLCs collide at 0x01 in the
dumps, the DB keeps them distinct via `--import-index`). **Green on the
real GOG import**: 0 forms missing across weapons (299), refs (639,633),
races (30), terminals (484); NPC_ dump count fully present + 886 CREA.

## Not ingested (deliberately)
- vaultmp `research/formulas` — simplified damage calc; Ashfall's is richer.
- ClientLauncher EOS auth — Commons Clause, future scope.
- Mojave-online's protocol (9 messages) — regression vs Ashfall's 140+.
- `other/data3` raw dumps (72MB) — scripted comparison instead of vendoring.

## Update — real game data imported (2026-08-07)

Both games fully ingested from GOG downloads (innoextract on
battlecruiser, files in `data/fallout3/` + `data/falloutnv/` — gitignored):

| | FO3 1.7.0.3 GOG | FNV 1.4.0.525(a) GOG |
|---|---|---|
| exe md5 | `7691d718...` (documented build ✓) | `0f374bae...` (documented build ✓) |
| DB | `data/fallout3/fallout3.sqlite3` | `data/falloutnv/falloutnv.sqlite3` |
| records | 124,540 | 141,502 |
| weapons | 299 | 496 |
| NPCs | 3,613 | 6,455 |
| factions | 451 | 772 (matches old docs exactly) |
| refs | 747k | 427,089 |
| dump corpus | ✓ 0 missing (verify-esm-dumps.py) | n/a (no FNV dumps) |

Server boots on both (config `[database] path`, `[server] game_type`).
Static exe verification: FO3 0x455190 = 883 call sites; FNV fnv_14
functions all hold (480/7/32/43 call sites). The FO3 GOG exe IS the
classic build the whole address table set was made against — the
post-2023 Steam update is the only mismatched build.

Known quirk: GRA.esm authors 95 refs at hi=0 (genuine overrides of base
forms — imported as overrides, correct) and 1 ref at hi=2 (collides with
HonestHearts' 0x02000801; GRA imported last wins). 1 row in 427k.
