# Ashfall — soul

Multiplayer mod for Fallout 3 / New Vegas (Rust): server-authoritative
dedicated server, WASM scripting, UDP reliability layer, SQLite, egui
client, and a cross-compiled bridge DLL injected into the game under
Proton/Wine.

## Documentation locations

### Orientation
- `README.md` — project overview, status (phases 1–10 done, 396+ tests), quick start, What's Left
- `data/README.md` — data dir layout, what's tracked, DB regeneration, verified import counts

### Design & plans
- `docs/architecture.md` — full architecture: crates, types, protocol, server/client/bridge design, phases, design decisions
- `docs/impl-plan.md` — phase-by-phase implementation record (1–10)
- `docs/external-ingestion-plan.md` — 34-item external-repo ingestion plan (all done), NVSE/event/network/ESM-import fixes
- `docs/nvmp-ingestion.md` — NVMP lineage sources (vaultmp, mojave-online), what was ingested where, ESPs, dump verification

### Proton / bridge / reverse engineering
- `docs/proton-setup.md` — build + inject bridge (dinput8 proxy), config, save locations, troubleshooting
- `docs/proton-testing.md` — live runtime test matrix, safe vs crashing commands, Steam build re-derivation progress
- `docs/steam-re.md` — **the RE working doc**: Steam (post-2023) address table, vaultmp patch-site mapping (respawn solved + live-verified), remaining site groups, flat-dump +0xC00 trap, probe infra, next-session handoff
- `docs/geck/host-function-roadmap.md` — 56 WASM host fns vs GECK surface, prioritized additions with verbatim signatures
- `docs/geck/geck_function_index.txt` — 2,580 GECK/NVSE/JIP/kNVSE functions (grep with `scripts/geck-search.sh`)
- `docs/geck/geck_anim_groups.txt` — 249 PlayGroup animation groups

### Scripts (analysis tooling)
- `scripts/re/README.md` — RE verification scripts: two-tool constant verification, Steam re-derivation pipeline, probe clients, results matrix
- `scripts/re/bridge_probe.py` — live-game probe client (probe/dump/dead/pos/ptr/respawn; OP_PROBE_CODE 0xFD, OP_PROBE_PTR 0xFA, OP_PROBE_FORM 0xFB, OP_DUMP_IMAGE 0xFC)
- `scripts/re/steam_map.py` — VA↔file-offset mapper for the Steam dump (FLAT: offset = VA − 0x400000)
- `scripts/re/probe_baseform.py`, `scripts/re/thiscall_test.rs` — field probe + thiscall shim validation
- `scripts/verify-esm-dumps.py` — ESM import ↔ vaultmp dump corpus cross-check
- `scripts/geck-search.sh` — grep the GECK function index

### Bridge internals (code = docs for the patch tables)
- `crates/ashfall-bridge/src/hooks/vaultmp.rs` — classic FO3 table + 34 recipes; `apply_steam_respawn()` (byte-guarded Steam respawn-disable: 0x9C43A5, 0x8C9CE0→0x8C9D5D, 0x8C9D52)
- `crates/ashfall-bridge/src/hooks/mod.rs` — address tables (fo3_17, fo3_steam_17, fnv_14), install/uninstall, read_bytes
- `crates/ashfall-bridge/src/commands.rs` — pipe opcodes (36 + debug probes)

### Host access (untracked — not in git)
- `hosts/README.md` — machine roles, paths, launch incantations, X auth, poll patterns

## Session state (latest, 2026-08-08)
- Steam respawn-disable patch: mapped, applied, live-verified on the game host (dead stays dead, flags clear, stable)
- Next: vtable-call getters (Steam GetBaseForm slot; baseForm field +0x1C found) + remaining patch-site groups (AI pause, fire relay, PlaceAtMe/activate, race match, lock fix, delegators) — see `docs/steam-re.md` handoff
- Image dump is FLAT; r2 PE-parse shifts .text +0xC00 — verify VAs live via OP_PROBE_CODE before patching
