# Ashfall

**Rust multiplayer mod for Fallout 3 / Fallout: New Vegas.** Server-authoritative dedicated server with WASM scripting, UDP networking, SQLite persistence, and an egui client browser. Started as a recreation of [vaultmp-extended](https://github.com/massdivide/vaultmp-extended), got bigger, fast.

[![Status](https://img.shields.io/badge/phases-1%E2%80%9310%20complete-brightgreen)](#status)
[![Tests](https://img.shields.io/badge/tests-318%20passed-brightgreen)](https://github.com/YOUR_ORG/ashfall/actions)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![Work in Progress](https://img.shields.io/badge/status-work%20in%20progress-orange)](#whats-left)

> **All 34 items of the external ingestion plan complete** ([docs/external-ingestion-plan.md](./docs/external-ingestion-plan.md)): NVSE plugin interface fixes, real VTable getters, a fully functional reliability layer (ACK/NACK, RTO retransmit, send window, rate limiting, varint framing), an ESM/ESP → SQLite import tool, and a zero-warning build.
>
> **Bridge:** full memory patching system (SafeWrite*, VTable access, trampoline detours), GECK opcode interception engine with 11 default handlers, 36 pipe command opcodes, 7 event sink types, 96 tests. Needs Proton runtime testing — see [What's Left](#whats-left).

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

**Phases 1–10 complete. 318 tests, 0 failures. Zero-warning build** (lib + test targets).

| Phase | What's built |
|-------|-------------|
| 1. Protocol | 140+ packet variants — physics, combat damage, NPC AI, quests, dialogue, FO3/FNV globals, cell snapshots. FormID type. 71 wire format tests. |
| 2. Server | UDP networking with a real reliability layer — ACK/NACK control frames, Jacobson/Karels RTO with exponential-backoff retransmission, 32-packet send window, per-channel priority queues (System > Game > Chat), per-address token-bucket rate limiting, varint sequence framing. Session state machine. Object registry. Packet dispatch routing all 140+ variants. Verified by real-UDP loss simulation (50/50 packets in order under 25% loss). |
| 3. Sync | 9-cell visibility grid with enter/leave diff. Position, angle, velocity, actor state, item, container sync. Combat resolution (Fallout damage formula). NPC AI packages + faction hostility. |
| 4. Persistence | SQLite — 17 tables. Records, weapons, NPCs, quest stages, dialogue flags, karma, reputation (FNV), hardcore stats, factions. Startup load at boot. **ESM/ESP import tool** (`ashfall-server --import-esm Fallout3.esm --import-game fo3 --import-db data/fallout3.sqlite3`) populates all tables from plugin files via a native TES4 parser. |
| 5. Scripting | wasmtime v22 engine. 35 callbacks (OnHit, OnEquip, OnQuestStage + original 31). 51 host functions — world/quest/chat/clock/player-count/object-actor CRUD now real (Phase 5 Part B). Timer system with WASM callback routing. Script effect queue (chat/kick) drained per tick. Auth/chat/spawn-cell/death/quest-stage/time callback dispatch into WASM. 11 WAT-based runtime tests + 3 full end-to-end tests (server + WASM + raw UDP clients: auth gate, weather sync, spawn chat, two-client chat relay). Example freeroam WASM script. SDK crate. |
| 6. GUI | eframe/egui app. Server browser with direct connect. Chat overlay. Server-authored widget manager (windows, buttons, edits, checkboxes, lists). |
| 7. Client | UDP networking. Connection flow (auth→load→ingame). Client object registry. Handlers for all packet categories. Background 30Hz poll loop. |
| 8. Master | UDP server registry. Announce/query/cull lifecycle. Client integration for server browser population. |
| 9. Security | Anti-cheat validator — position bounds, velocity caps, teleport detection, item count limits, damage bounds, sequence nonces, FormID whitelist. 48 security tests. |
| 10. Bridge | Memory patching system (SafeWrite*, VTable access, trampoline detours). 36 command opcodes (Tier 1-4). GECK opcode interception engine (11 default handlers, direct-indexed static table). Real VTable getters (cell, enabled, name, lock, parent cell, combat target — FO3/FNV aware). 7 event sink types + pipe event frames. 96 tests. |

## What's Left

- **Full WASM game mode scripts** — host functions and callback dispatch now work end-to-end (auth, chat, spawn, quests, weather, timers, effects, object/actor CRUD); remaining callbacks (on_hit, on_equip, on_activate, GUI) and item stack ops are still stubs. Next: co-op quest logic, NPC AI, custom game modes on top.
- **Proton runtime testing** — inject bridge.dll into actual FO3/FNV under Proton, verify VTable hooks fire correctly
- **Proton integration testing** — end-to-end test with real Fallout running under Proton/Wine.
- **Network testing** — latency compensation, bandwidth tuning still open; reliability layer has ACK/NACK, RTO retransmit, send window, rate limiting, and a real-UDP loss-simulation suite (25% loss, 50/50 packets delivered in order — see tests/reliability.rs, tests/loss_simulation.rs). The auth flow and two-client chat relay are now verified end-to-end over real UDP (tests/script_e2e.rs).
- **Windows native client** — currently Linux-only. Bridge DLL already cross-compiles for Windows.

> ✅ ESM reader tool: **done** — `ashfall-server --import-esm Fallout3.esm --import-game fo3 --import-db data/fallout3.sqlite3` populates all 17 tables from plugin files. **Verified against the real game**: base game + all 5 DLC esms (GOG 1.7.0.3, 276MB master) import in ~15s each — 301 weapons, 3,642 NPCs, 747k world references total. Requires real-binary fixes: 24-byte record headers, TES4 16-byte tail, and zlib decompression for compressed records (flag 0x00040000, data = [u32 size][zlib]).

---

## Build

```bash
git clone https://github.com/YOUR_ORG/ashfall.git
cd ashfall

cargo build --release
cargo test --workspace   # 318 tests
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
