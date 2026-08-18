# ☢️ Ashfall

**Play Fallout 3 / Fallout: New Vegas with your friends.** A co-op
multiplayer mod for the classic Bethesda games — host a server, connect
with your crew, and explore the wasteland together. Built from scratch in
Rust, inspired by the old vaultmp project and its successors.

[![Tests](https://img.shields.io/badge/tests-605%20passed-brightgreen)](#status)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![Status](https://img.shields.io/badge/status-co-op%20MVP%20in%20progress-orange)](#status)

---

## What is this?

Ashfall is a **server-authoritative multiplayer mod**. A dedicated server
owns the world; players connect to it and play the real game together —
your Fallout 3/NV still runs on your machine, Ashfall keeps everyone's
world in sync. Think Skyrim Together, but for the Capital Wasteland and
the Mojave.

**The pieces:** a Rust server (with SQLite storage and WASM scripting for
custom game modes), a small client app with a server browser, and a tiny
DLL injected into the game that lets it talk to the client.

---

## What works right now

An honest status. The plumbing is done; the polish is coming.

| Area | Status |
|------|--------|
| **Play together** | ✅ Connect, authenticate, see each other, sync position and state |
| **Dedicated server** | ✅ Run your own — sessions, persistence, anti-cheat, server browser listing |
| **Chat** | ✅ In-game chat relayed by the server |
| **Server-side rules** | ✅ PvP on/off, game clock, load-order check, player limits |
| **Owned NPC simulation** | ✅ Whoever's near an NPC simulates it; ownership hands off cleanly on disconnect |
| **Mod support** | ✅ Import your ESM/ESP mods into the server database (both games + all DLC verified) |
| **Scripted game modes** | ✅ Custom game modes in Rust/WASM — see [scripts/](./scripts/README.md) |
| **Combat** | ✅ Server-authoritative damage with Fallout's DR/DT formula |
| **GUI** | ✅ Server browser, chat, top-down world view with health bars and player names. (The game window is the 3D view — a separate 3D renderer is out of scope.) |
| **NPC sync in-game** | 🚧 Fully built and tested end-to-end; needs live verification on a real game host (see [What's left](#whats-left)) |

**The goal:** vanilla co-op — your group playing the actual game together,
no mods required, on either Fallout 3 or New Vegas.

---

## Quick start (no game needed)

Three terminals. This runs the full stack with a fake "game" — enough to see
the auth → world-load → sync flow work.

```bash
# 1. Master server (lists servers for the browser)
cargo run -p ashfall-master

# 2. Dedicated server
cargo run -p ashfall-server

# 3. Client (stub mode — no Fallout required)
cargo run -p ashfall-client
```

The client connects to `127.0.0.1:1770`. Stub mode feeds canned data so the
whole loop runs without the game installed.

---

## Running a server for real

Point the server at your game's data, import it once, then run:

```bash
# One-time import (Fallout 3, base + DLCs)
cargo run -p ashfall-server -- --import-esm data/fallout3/Fallout3.esm \
    --import-game fo3 --import-db data/fallout3/fallout3.sqlite3 --import-index 0
# ...repeat with --import-index 1..5 for each DLC

# Then run the server
cargo run -p ashfall-server
```

**Server config** (`~/.config/ashfall/server.ini`, ini or TOML):

```ini
[server]
host = 0.0.0.0
port = 1770
connections = 8
game_type = fo3        ; fo3 or fnv
pvp_enabled = true     ; allow players to fight each other
time_scale = 30        ; how fast game time flows
mod = "Fallout3.esm:C092218B"   ; optional: require this load order
```

`ashfall-server --list-mod-crc <game-data-dir>` prints the exact `mod =`
lines for your files, so you don't hand-type CRCs.

**Playing with a real game under Proton/Wine:** see the
[Proton Setup Guide](./docs/proton-setup.md) — it covers the injected bridge
DLL and where saves live.

---

## The tech (for the curious)

```
┌──────────┐   UDP    ┌───────────┐   UDP    ┌─────────┐   TCP    ┌──────────────┐
│ master   │◄────────►│ server    │◄────────►│ client  │◄────────►│ bridge.dll   │
│ (browser │          │ (authority│          │ (egui)  │ 1771     │ (in game,    │
│  listing)│          │  + WASM)  │          │         │          │  Proton/Wine)│
└──────────┘          └───────────┘          └─────────┘          └──────────────┘
```

- **Server-authoritative** — the server owns all game state; clients send
  input, the server validates and broadcasts. No trust-the-client.
- **Custom UDP reliability layer** — ACK/NACK, retransmission, send window,
  rate limiting (RakNet semantics, no RakNet). Ordered channels for
  gameplay state + a fire-and-forget lane for positions.
- **SQLite persistence** — the full game world (records, NPCs, weapons,
  quests, factions) imported natively from your ESM/ESP files.
- **WASM game modes** — sandboxed scripts drive server rules (auth gates,
  spawn logic, chat commands, custom quests).
- **Real Fallout engine hooks** — a small DLL (injected via dinput8 proxy)
  reads and writes the live game: positions, actor state, combat, respawn
  behavior. All three builds are supported: Fallout 3 GOG/classic, Fallout
  3 Steam (post-2023), and New Vegas.

Full architecture: [docs/architecture.md](./docs/architecture.md)

---

## Status & roadmap

**Phases 1–10 complete, 605 tests, zero warnings.** The phase-by-phase
record lives in [docs/impl-plan.md](./docs/impl-plan.md); the reverse-
engineering deep-dive lives in [docs/steam-re.md](./docs/steam-re.md).

### Where the project is

The full co-op pipeline is **built and tested**: players connect, the
server owns the world, NPCs are simulated by whoever's near them, and
state (position, health, combat, chat, clock) syncs between clients. The
engine-side work covers both Fallout 3 builds and New Vegas — position
sync, actor values (health/DR/DT), death/kill relay, locks, sounds, names,
and respawn behavior. Every one of the engine addresses is byte-verified
against the real binaries; a Ghidra-based campaign recovered the full
vtable maps and validated every hook site without running the game.

Recently completed:

- **Two-player co-op loop end-to-end** — engine events flow through the
  bridge to the server and back to other clients
- **NPC sync fully wired** — discovery, ownership, state sampling, remote
  application (GOG + Steam + FNV)
- **Shared-quest demo game mode** — see [scripts/shared-quest](./scripts/)
- **Full reverse-engineering validation** — every address table verified
  against real binaries, vtable slot maps recovered, several previously
  "stuck" hooks solved statically

### What's left

- **Live verification on a real game host** — the one remaining
  bottleneck. The bridge, hooks, and a verification tool are ready; the
  next step is a single session on the game machine to confirm the wired
  patches behave in the running game (the plan is pre-written in
  [docs/steam-re.md](./docs/steam-re.md)).
- **Windows-native client** — cross-compiles and has a CI job; the runtime
  still needs a Windows host to verify.
- **Content work** — more game modes, client UI polish, and shared quests
  on top of the working script stack.
- **One suspected bug** — the engine's name field for FO3/Steam may differ
  from what the client reads; noted in [docs/steam-re.md](./docs/steam-re.md).

---

## Contributing

**Vibe coding very welcome** — AI-assisted code, LLM-generated PRs, prompt
engineering, all fair game. One hard rule:

> **It must pass tests.** No untested code lands on `main`. Stub mode means
> you can exercise the whole client+server stack without the game running.

```bash
cargo test --workspace   # 605 tests
cargo clippy -- -D warnings
cargo fmt -- --check
```

Good first tasks: wire-format tests, client UI, WASM game-mode content,
networking edge cases, and (if you enjoy reverse engineering) the Steam-build
hook work — see [docs/steam-re.md](./docs/steam-re.md) for the handoff notes.

---

## Docs

| Doc | What it covers |
|-----|----------------|
| [architecture.md](./docs/architecture.md) | Full design: crates, protocol, reliability, server/client/bridge |
| [impl-plan.md](./docs/impl-plan.md) | Phase-by-phase record + what's left |
| [proton-setup.md](./docs/proton-setup.md) | Building + injecting the bridge, running under Proton |
| [steam-re.md](./docs/steam-re.md) | Engine reverse-engineering notes + next-session handoff |
| [scripts/](./scripts/README.md) | WASM game modes: build, deploy, write your own |
| [geck/](./docs/geck/) | 2,580-function GECK/NVSE script index |

## License

GPL-3.0. Not affiliated with Bethesda Softworks or Obsidian Entertainment.
Fallout is a trademark of Bethesda Softworks.
