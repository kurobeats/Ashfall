# Ashfall

**Rust multiplayer mod for Fallout 3 / Fallout: New Vegas.** Server-authoritative dedicated server with WASM scripting, UDP networking, SQLite persistence, and an egui client browser. Started as a recreation of [vaultmp-extended](https://github.com/massdivide/vaultmp-extended), got bigger, fast.

[![Status](https://img.shields.io/badge/phases-1%E2%80%9310%20complete-brightgreen)](#status)
[![Tests](https://img.shields.io/badge/tests-382%20passed-brightgreen)](https://github.com/YOUR_ORG/ashfall/actions)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![Work in Progress](https://img.shields.io/badge/status-work%20in%20progress-orange)](#whats-left)

> **All 34 items of the external ingestion plan complete** ([docs/external-ingestion-plan.md](./docs/external-ingestion-plan.md)): NVSE plugin interface fixes, real VTable getters, a fully functional reliability layer (ACK/NACK, RTO retransmit, send window, rate limiting, varint framing), an ESM/ESP → SQLite import tool, and a zero-warning build.
>
> **Bridge:** full memory patching system (SafeWrite*, VTable access, trampoline detours), GECK opcode interception engine with 11 default handlers, 36 pipe command opcodes, 7 event sink types, 96 tests. Needs Proton runtime testing — see [What's Left](#whats-left).
>
> **NVMP lineage ingestion** ([vaultmp](https://github.com/foxtacles/vaultmp) + [mojave-online](https://github.com/knork-fork/mojave-online), both MIT): full classic-Steam FO3 multiplayer patch table + 34 byte-exact detour recipes (`hooks/vaultmp`), remote-actor animation state machine (`hooks/animation`, vaultmp `net_SetActorState` semantics), render-behind interpolation buffer with extrapolation (`ashfall-client` `world::state`, mojave-online semantics), 2,580-function GECK script index + host-function roadmap (`docs/geck/`), NVMP server-mod ESPs (`data/plugins/`), and a vaultmp-dump ↔ SQLite cross-checker (`scripts/verify-esm-dumps.py`). 382 tests, zero warnings.

---

## Quick Start

Three terminals, no game needed:

```bash
# Terminal 1 — master server
cargo run -p ashfall-master

# Terminal 2 — dedicated server
cargo run -p ashfall-server

# Terminal 3 — client (stub mode)
cargo run -p ashfall-client
```

Client connects to `127.0.0.1:1770`. Stub mode sends canned data — enough to verify the full auth→load→sync flow without Fallout running.

**With Proton/Wine:** see the [Proton Setup Guide](./docs/proton-setup.md).

---

## Status

**Phases 1–10 complete. 382 tests, 0 failures. Zero-warning build** (lib + test targets).

| Phase | What's built |
|-------|-------------|
| 1. Protocol | 140+ packet variants — physics, combat damage, NPC AI, quests, dialogue, FO3/FNV globals, cell snapshots. FormID type. 71 wire format tests. |
| 2. Server | UDP networking with a real reliability layer — ACK/NACK control frames, Jacobson/Karels RTO with exponential-backoff retransmission, 32-packet send window, per-channel priority queues (System > Game > Chat), per-address token-bucket rate limiting, varint sequence framing. Session state machine. Object registry. Packet dispatch routing all 140+ variants. Verified by real-UDP loss simulation (50/50 packets in order under 25% loss). |
| 3. Sync | 9-cell visibility grid with enter/leave diff. Position, angle, velocity, actor state, item, container sync. Combat resolution (Fallout damage formula). NPC AI packages + faction hostility. |
| 4. Persistence | SQLite — 17 tables. Records, weapons, NPCs, quest stages, dialogue flags, karma, reputation (FNV), hardcore stats, factions. Startup load at boot. **ESM/ESP import tool** (`ashfall-server --import-esm Fallout3.esm --import-game fo3 --import-db data/fallout3.sqlite3`) populates all tables from plugin files via a native TES4 parser. |
| 5. Scripting | wasmtime v22 engine. **All 35 callbacks dispatched** — auth/chat/spawn/hit-gate/equip/activate/item/death/quest/time/cell/window/dialogue/lock + actor sub-events; **all 51 host functions real** (world/quest/chat/items/combat/GUI/meta). Timers, script effect queue (chat/kick/packet broadcast), **real Rust-compiled freeroam WASM game mode builds and runs end-to-end** (wasm32 via rustup; SDK macros emit `#[link(wasm_import_module = "env")]`). 19 WAT runtime tests + 5 e2e wire tests. SDK crate. |
| 6. GUI | eframe/egui app. Server browser with direct connect. Chat overlay. Server-authored widget manager (windows, buttons, edits, checkboxes, lists). |
| 7. Client | UDP networking. Connection flow (auth→load→ingame). Client object registry. Handlers for all packet categories. Background 30Hz poll loop. |
| 8. Master | UDP server registry. Announce/query/cull lifecycle. Client integration for server browser population. |
| 9. Security | Anti-cheat validator — position bounds, velocity caps, teleport detection, item count limits, damage bounds, sequence nonces, FormID whitelist. **Handler ownership enforcement**: clients may only mutate their own player object (movement, actor state/value/death, controls, combat hits) and their own inventory items (item→container→player chain); world/NPC objects are server-authoritative. 22 ownership/PvP/item tests. |
| 10. Bridge | Memory patching system (SafeWrite*, VTable access, trampoline detours). 36 command opcodes (Tier 1-4). GECK opcode interception engine (11 default handlers, direct-indexed static table). Real VTable getters (cell, enabled, name, lock, parent cell, combat target — FO3/FNV aware). 7 event sink types + pipe event frames. 96 tests. **Every hardcoded constant verified against the real GOG 1.7.0.3 binaries with two independent tools** (r2 + python/objdump/headers — see scripts/re/) — 4 wrong opcodes and 2 wrong field offsets were found and fixed; FOSE/NVSE plugin ABI corrected to the real interface layout. Bridge cross-compiles to i686 Windows and runs under wine (TCP pipe protocol round-trip verified, no game needed). |

## What's Left

- **Co-op game mode content** — the full script stack now works (host functions, callbacks, real WASM builds); next is writing actual game modes: co-op quest logic, NPC AI behaviors, custom rules on top of the freeroam example.
- **Proton runtime testing** — injection + pipe protocol verified in real FO3 GOTY under Proton; **the GOG-verified address table does NOT match the Steam build** (in-process probe: 0x455190 is garbage — vtable commands crash the game). Next: dump the unpacked image via bridge `OP_DUMP_IMAGE`, re-derive constants, re-test — see [docs/proton-testing.md](./docs/proton-testing.md).
- **Proton integration testing** — end-to-end test with real Fallout running under Proton/Wine.
- **Windows native client** — currently Linux-only. Bridge DLL already cross-compiles for Windows.
- **Client world renderer** — the GUI layer renders server-authored windows, but there is no 3D/2D world view yet; remote objects are listed with render-behind interpolated positions (67ms delay + 500ms extrapolation, `world::state::InterpBuffer`). `on_actor_punch` has no wire source (no Punch packet).
- **Bridge animation executor** — `hooks::animation` (remote-actor PlayGroup state machine) is tested and ready; wiring it into the game needs the engine-side PlayGroup dispatcher (classic Steam `0x45F704`, or re-derived per build) and a client→bridge `OP_PLAY_GROUP` pipe command.
- **Item ownership chain** — item count/condition/equipped handlers cap values but lack the item→container→player authorization chain (client could inflate its own item counts).

> ✅ ESM reader tool: **done** — `ashfall-server --import-esm Fallout3.esm --import-game fo3 --import-db data/fallout3.sqlite3` populates all 17 tables from plugin files. **Verified against the real game**: base game + all 5 DLC esms (GOG 1.7.0.3, 276MB master) import in ~15s each — 301 weapons, 3,642 NPCs, 747k world references total. Requires real-binary fixes: 24-byte record headers, TES4 16-byte tail, and zlib decompression for compressed records (flag 0x00040000, data = [u32 size][zlib]). Counts cross-verified with an independent python record walker (weapons 160, NPCs 2,180 = NPC_+CREA, refs 573,610 = REFR+ACHR+ACRE, items 1,601, factions 326 — all exact) and against the vaultmp `other/data3` dumps (`scripts/verify-esm-dumps.py`: 301 weapons, 2736 NPC_, 747,790 REFR+ACHR+ACRE, 30 races, 485 terminals — exact). Quests boot at stage 0 (the import's stage lists are known-stages, not progress).
>
> ✅ ESP import path: **verified** — the four NVMP server-mod ESPs (`data/plugins/`) import cleanly without masters (285/41/54/62 records; overrides only, base stats come from the master ESM).

---

## Build

```bash
git clone https://github.com/YOUR_ORG/ashfall.git
cd ashfall

cargo build --release
cargo test --workspace   # 382 tests
```

Optional: cross-compile bridge DLL for Proton (`sudo apt install mingw-w64`):

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu -p ashfall-bridge
```

> ⚠️ FO3/FNV are **32-bit** executables — real Proton injection needs
> `i686-pc-windows-gnu` (see [docs/proton-setup.md](./docs/proton-setup.md)).

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

- **Server-authoritative** — server owns all game state. Clients send input; server validates and broadcasts.
- **3 ordered channels** (System, Game, Chat) + 1 unordered for position/physics updates.
- **30 Hz tick rate** with 9-cell grid for visibility management.
- **postcard** binary serialization over custom UDP reliability layer.
- **wasmtime** v22 for sandboxed WASM game mode scripts.

Full architecture: [architecture.md](./docs/architecture.md) | Implementation plan: [impl-plan.md](./docs/impl-plan.md)

---

## Contributing

**Vibe coding very welcome.** AI-assisted code, LLM-generated PRs, prompt-engineering — all fair game. One hard rule:

> **It must pass tests.** No untested code lands on `main`. Stub mode means you can test the full client+server stack without the game running.

### Quick flow

```bash
git checkout -b ashfall-phase{phase}-pr{number}-{desc}
# Work
$EDITOR ...
# Verify
cargo test -p ashfall-server
cargo clippy -- -D warnings
cargo fmt -- --check
# Push
git push origin ...
```

### Where to start

| Skill | Good task |
|-------|-----------|
| Rust beginner | constants, math, wire format tests |
| Networking | UDP sockets, reliability layer, session management |
| Database | SQLite schema, CRUD, startup load |
| WASM / compilers | wasmtime engine, host functions, callbacks |
| GUI / gamedev | egui widgets, server browser, chat UI |
| Reverse engineering | Gamebryo VTable hooks, Proton bridge (Phase 10) |

For details: [Implementation Plan](./docs/impl-plan.md). Stuck? Open a discussion or issue.

---

## License

GPL-3.0.
