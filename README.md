# ☢️ Ashfall

**Play Fallout 3 / Fallout: New Vegas with your friends.** A co-op multiplayer
mod for the classic Bethesda games — host a server, connect with your crew,
and explore the wasteland together. Built from scratch in Rust, inspired by
the old vaultmp project and its successors.

[![Tests](https://img.shields.io/badge/tests-600%20passed-brightgreen)](#status)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![Status](https://img.shields.io/badge/status-co-op%20MVP%20in%20progress-orange)](#status)

---

## What is this?

Ashfall is a **server-authoritative multiplayer mod** — a dedicated server
owns the world, and players connect to it. Your game still runs the real
Fallout 3/NV on your machine (through Steam/GOG); Ashfall syncs players,
position, combat, chat, and world state between them. Think
Skyrim Together, but for the Capital Wasteland and the Mojave.

**The stack, in one line:** Rust server (UDP + SQLite + WASM scripting),
a native Linux client with a server browser, and a small DLL injected into
the game that lets it talk to the client.

---

## What works right now

This is an honest status. The plumbing is done; the polish is coming.

| Area | Status |
|------|--------|
| **Connect & play together** | ✅ Two players can connect, authenticate, see each other, and sync position/state over the network (real, tested) |
| **Dedicated server** | ✅ Run your own — UDP reliability layer, sessions, anti-cheat validation, persistence, master-server browser listing |
| **Chat** | ✅ In-game chat relayed by the server |
| **Server-side rules** | ✅ PvP on/off, game clock (time of day syncs to players), load-order check, player limits |
| **Owned NPC simulation** | ✅ Ownership protocol — whoever's near an NPC simulates it, handoff on disconnect. The bridge's NPC discovery (detours the engine's actor-processing gate, GOG-verified) + owned-NPC state reporting are built and tested |
| **Mod support** | ✅ Full ESM/ESP import → server database (both games + all DLC verified); optional load-order verification |
| **Scripted game modes** | ✅ WASM scripting — servers can run custom game modes written in Rust/WASM |
| **Combat** | ✅ Server-authoritative damage with Fallout's DR/DT formula |
| **GUI** | ✅ Client has a server browser + chat + top-down world view (health bars, player names, server GUI). The game window is the 3D view — a separate 3D renderer is out of scope. |
| **NPC sync in-game** | 🚧 Fully wired client/server/bridge — discovery (classic + Steam re-derived + FNV ActorProcessManager), ownership, state sampling, remote application all built + tested. Needs live verification on the game host (see [What's Left](#whats-left)) |

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
  rate limiting (RakNet semantics, no RakNet). 3 ordered channels + 1
  fire-and-forget for positions.
- **SQLite persistence** — the full game world (records, NPCs, weapons,
  quests, factions) imported natively from your ESM/ESP files.
- **WASM game modes** — sandboxed scripts drive server rules (auth gates,
  spawn logic, chat commands, custom quests).
- **Real Fallout engine hooks** — a small DLL (injected via dinput8 proxy)
  reads/writes the live game: positions, actor state, combat, respawn
  behavior. The Steam-build reverse engineering lives in
  [docs/steam-re.md](./docs/steam-re.md).

Full architecture: [docs/architecture.md](./docs/architecture.md)

---

## Status & roadmap

**Phases 1–10 complete, 600 tests, zero warnings.** The phase-by-phase record
lives in [docs/impl-plan.md](./docs/impl-plan.md). Recent highlights:

- Ownership transfer, string compression, and differential state sync
  (ported from Skyrim Together Reborn)
- Game clock sync, PvP enforcement, entity streaming, mod policy
- A working bridge→client event pipeline (the co-op loop's transport)
- NPC discovery via the engine's actor-processing gate + owned-NPC state
  reporting + remote-NPC application (the full sync loop, GOG-mapped)
- Actor-value getters/setters wired for both games (2026-08-14) — the
  engine's command-table handlers revealed the real AV access (FO3
  ActorValueOwner at +0x9C, FNV at +0xA4, vtable slot 3 GetActorValueF;
  SetActorValue delta via Actor vtable +0x3A0/+0x3A4), so health/DR/DT
  now read and set for real; the client world view shows health bars +
  player names from that data
- Kill relay wired (FO3 engine Kill 0x71AC50 + death processing
  0x71C280) — remote deaths apply locally
- Verified ESM import for both games + all DLC (real GOG binaries)
- Live Proton testing: Steam respawn-disable patch applied and verified on
  the game host

### What's left

- **NPC sync live** — the full loop is built and tested (discovery detours
  on both the classic and Steam builds — the Steam AI predicate was
  re-derived from the dump; ownership, state sampling, remote application
  all wired); remaining is live verification on the game host
  (see [docs/steam-re.md](./docs/steam-re.md))
- **Remaining Steam patch sites** — 2026-08-14: 6 more vaultmp behavior
  sites re-derived without the game — via the FalloutAnniversaryPatcher
  vcdiff (ai_fix1 0x5E99E2, get_activate_jmp 0x8D3BC8, delegator stub spot
  0x405E69, play_group_fix 0x4350F9) + static analysis (av_fix 0x5B7AC7,
  fire_weapon confirmed 0x7DF3F7/0x770880) — live-probe before hooking.
  Steam PC vtable base found (0xF938FC); GET_LOCKED slot re-derived
  (GOG +0xA0 → Steam +0xFC) and wired. get_activate fully solved
  (jmp 0x8D3BC8 + ret 0x8D3CB8). The 8 vaultmp hooks are implemented
  (EVENT_ACTIVATE/EVENT_FIRE relay) and the activate/fire/cell/enabled/
  move/scale/lock/sound relay paths are complete end-to-end (field writes,
  Steam-safe); get_scale/set_scale + is_dead are field-based (no vtable);
  alerted/sneaking call the classic engine getters (byte-guarded). A gh
  crawl found Project Crossroads' Anniversary-Patcher catalog — the full
  vaultmp site table byte-verified, independently confirming our classic
  table (see docs/steam-re.md Session 2026-08-14h). **2026-08-14i:** a
  full static pass (semantic fingerprints + fresh gh re-crawl) confirmed
  the remaining sites — fire_fix, match_race, place_at_me, ai_fix2/3/4,
  play_idle_fix, play_group, delegator_src — plus the AV/anim vtable
  slots (GetActorValue/State/is_moving) are all statically underivable
  (recompile restructured every target function); they need live probe
  (OP_PROBE_CODE/OP_PROBE_FORM). New confirmed: Steam `__security_cookie`
  0x1202954 (canary XOR now `ebp`, not `esp`) + delegator fn 0x405E70.
  Remaining engine-bound OP stubs: SET_NAME, PLAY_SOUND's engine call,
  PLACE_AT_ME.
- **Per-frame player hook** — wired for all three builds: FNV
  (0x86B386), FO3 classic (0x6EEB2F), and Steam/Anniversary (0x9B3D77,
  re-derived 2026-08-14 via the respawn-struct frame-body twin; the hook
  derefs the SteamStub IAT slot so it's ASLR-safe). Live verification on
  the game host remains
- ~~**3D client view**~~ — explicitly out of scope: the game window IS the
  view (the client's top-down projection + health bars/names + server GUI
  are the companion HUD)
- **Windows-native client** — currently Linux-only (the bridge DLL already
  cross-compiles)
- **Co-op game modes** — the script stack works; actual game modes (shared
  quests, custom rules) are content work on top of it

---

## Contributing

**Vibe coding very welcome** — AI-assisted code, LLM-generated PRs, prompt
engineering, all fair game. One hard rule:

> **It must pass tests.** No untested code lands on `main`. Stub mode means
> you can exercise the whole client+server stack without the game running.

```bash
cargo test --workspace   # 600 tests
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
| [steam-re.md](./docs/steam-re.md) | Steam-build reverse-engineering notes + next-session handoff |
| [geck/](./docs/geck/) | 2,580-function GECK/NVSE script index |

## License

GPL-3.0. Not affiliated with Bethesda Softworks or Obsidian Entertainment.
Fallout is a trademark of Bethesda Softworks.
