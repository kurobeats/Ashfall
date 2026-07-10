# Ashfall

**Rust multiplayer mod for Fallout 3 / Fallout: New Vegas.** Server-authoritative dedicated server with WASM scripting, UDP networking, SQLite persistence, and an egui client browser. Started as a recreation of [vaultmp-extended](https://github.com/massdivide/vaultmp-extended), got bigger, fast.

[![Status](https://img.shields.io/badge/phases-1%E2%80%939%20complete-brightgreen)](#status)
[![Tests](https://img.shields.io/badge/tests-169%20passed-brightgreen)](https://github.com/YOUR_ORG/ashfall/actions)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![Work in Progress](https://img.shields.io/badge/status-work%20in%20progress-orange)](#whats-left)

> **Not yet playable.** Core engine is functional but the bridge to the actual game (VTable hooks) is not yet implemented. See [What's Left](#whats-left).

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

**Phases 1–9 complete. 169 tests, 0 failures.**

| Phase | What's built |
|-------|-------------|
| 1. Protocol | 140+ packet variants — physics, combat damage, NPC AI, quests, dialogue, FO3/FNV globals, cell snapshots. FormID type. 71 wire format tests. |
| 2. Server | UDP networking with custom reliability layer (3 ordered + 1 unordered channel). Session state machine. Object registry. Packet dispatch routing all 140+ variants. |
| 3. Sync | 9-cell visibility grid with enter/leave diff. Position, angle, velocity, actor state, item, container sync. Combat resolution (Fallout damage formula). NPC AI packages + faction hostility. |
| 4. Persistence | SQLite — 17 tables. Records, weapons, NPCs, quest stages, dialogue flags, karma, reputation (FNV), hardcore stats, factions. Startup load at boot. |
| 5. Scripting | wasmtime v22 engine. 35 callbacks (OnHit, OnEquip, OnQuestStage + original 31). 51 host functions. Timer system. Example freeroam WASM script. SDK crate. |
| 6. GUI | eframe/egui app. Server browser with direct connect. Chat overlay. Server-authored widget manager (windows, buttons, edits, checkboxes, lists). |
| 7. Client | UDP networking. Connection flow (auth→load→ingame). Client object registry. Handlers for all packet categories. Background 30Hz poll loop. |
| 8. Master | UDP server registry. Announce/query/cull lifecycle. Client integration for server browser population. |
| 9. Security | Anti-cheat validator — position bounds, velocity caps, teleport detection, item count limits, damage bounds, sequence nonces, FormID whitelist. 48 security tests. |

---

## What's Left

- **Bridge VTable hooks** — the bridge DLL can already accept TCP connections and dispatch commands, but the actual Gamebryo engine hooks (GetPos, SetPos, GetActorValue, etc.) are stubs. Requires reverse engineering Fallout3.exe / FalloutNV.exe VTable layouts.
- **Proton integration testing** — end-to-end test with real Fallout running under Proton/Wine.
- **ESM reader tool** — populate the SQLite database from `.esm` / `.esp` plugin files.
- **Full WASM game mode scripts** — co-op quest logic, NPC AI behaviors, custom game modes.
- **Real network testing** — packet loss simulation, latency compensation, bandwidth tuning.
- **Windows native client** — currently Linux-only. Bridge DLL already cross-compiles for Windows.

---

## Build

```bash
git clone https://github.com/YOUR_ORG/ashfall.git
cd ashfall

cargo build --release
cargo test --workspace   # 169 tests
```

Optional: cross-compile bridge DLL for Proton (`sudo apt install mingw-w64`):

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu -p ashfall-bridge
```

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
