# Steam Build Reverse-Engineering — Findings & Plan

Target: FO3 GOTY Steam (post-2023), exe md5 `8a3adab8...`, .text 11.6MB
(10MB classic). Toolchain: Python + objdump on malum, radare2 6.0.5 on
battlecruiser. Image: `OP_DUMP_IMAGE` from the live process (PE-header based,
pressure-vessel safe), stable across runs.

## Solved (live-verified, in the bridge)

| Item | Classic/GOG | Steam | Method |
|---|---|---|---|
| LookupFormByID | 0x455190 | **0x711EF0** | call-count + prologue byte-match (`55 8b ec 53 56 57 8b 3d 84 4b 22 01`); form-map global 0x1224B84 |
| ExtractArgs | 0x517950 | 0x787530 | call-count (~431) + structure |
| ConsoleManager | 0x62B5D0 | 0x788B30 | call-count (33) |
| Calling convention | stdcall (WRONG) | **cdecl** | epilogue check: plain `ret` |
| vtable methods | stdcall (WRONG) | **thiscall** | entries read ECX (`mov (%ecx),%eax`); inline-asm shims `vcall_0..3` |
| baseForm field | vtable 0x10 (WRONG) | **field +0x1C** | obj-field probe: +0x1C → form object with ID 0x7 (Player base) |
| pos/angle/cell/scale/refID | fields | fields (+0x2C/0x30/0x34, +0x38, +0x3C, +0x0C) | live battery, game stable |

Auto-detection: `vtable::fo3_lookup_addr()` reads 0x455190 (`51 8b 0d` = GOG)
vs 0x711EF0 (`55 8b ec 53` = Steam).

## Open: vaultmp behavior-patch sites

The 34 `hooks::vaultmp` recipes target classic addresses. The Steam recompile
changed code layout AND function boundaries — byte-pattern and seed matches
produce decoys (e.g. a JNZ+0x83 + global-load false positive at 0xB59356).
Per-site semantic identification is required.

**Respawn (highest value)** — GOG anatomy:
- Site A: 0x6D5965, 20-byte predicate: `jne +3; xor al,al; ret; mov edx,[ecx+0x14]; push 1,0,edx,eax; call 0x6D5750`. vaultmp NOPs the JNE → always false.
- Site B: 0x78B230, respawn-function entry (1699 bytes): `jne +0x83; mov ecx,[0x107A0D4]; mov byte[ecx+2],1`. vaultmp redirects the JNE via RespawnDetour.
- Steam: neither byte pattern exists; the respawn function must be located
  semantically (find where the death flow sets the respawn flag/position).

## Plan for the patch work (separate focused effort)

1. **r2 batch workflow** (verified working): `r2 -q -e scr.color=0 -i script.r2
   bin > out.txt`, python-parse. Analyze BOTH binaries (GOG PE + Steam raw
   @0x400000), dump function lists (addr/size/callees).
2. **Function-pair mapping**: for each GOG function containing a patch site,
   find the Steam function by size proximity + callee-graph similarity
   (recompile preserves function structure more than bytes).
3. **Verify each pair** by disassembling both sides side-by-side; only then
   patch. Expected: respawn (2 sites), AI pause (4), fire relay (2),
   PlaceAtMe/activate (3), race match (2), lock fix (1), delegators (3).
4. Live-verify each patch with a game restart (apply → observe behavior).

## Live probe infrastructure (in bridge, read-only where possible)

- OP_PROBE_CODE — 16 bytes at table addresses
- OP_PROBE_FORM — obj + 128 vtable entries + 64 obj fields
- OP_PROBE_PTR — deref 16 dwords at an address
- OP_VCALL_TEST / OP_VCALL_TEST0 — guarded thiscall calls (crash = wrong
  index, game restart needed)
- OP_DUMP_IMAGE — full image (PE-header based)
- scripts/re/bridge_probe.py + probe_baseform.py — python clients
- scripts/re/thiscall_test.rs — thiscall shim validation under wine

## Known traps

- Probing unloaded formIDs via the Steam lookup crashes the game (miss path
  not null-safe under the cdecl call) — avoid.
- Steam vtable indices differ from xFOSE: index 4 = destructor (`ret $4`,
  delete-flag), 20-29 = not the AV virtuals, 60-72 = bool stubs, 121 (anim
  data per vaultmp) = null entry.
- No RTTI in the image (vtable[-1] is not a COL).
- FO3 command table is runtime-built (no static name→opcode array in the
  exe) — string-xref identification doesn't work.

## Session handoff (2026-08-08)

- r2 function lists (aflj) dumped for both binaries on battlecruiser:
  `/tmp/gog_fns.json` (30,364 fns) + `/tmp/steam_fns.json` (17,724 fns);
  copies saved locally at `data/fallout3/` (gitignored).
- `aflj` quirk: the JSON `offset` field is 0 — addresses are encoded in the
  `name` field (`fcn.00ae92c0`); parse the name when mapping.
- Size-matching for the respawn function (GOG 1699B) yields 147 Steam
  candidates — needs callee-graph comparison (compare each candidate's
  called-functions set against the GOG respawn fn's callees) to converge.
- Next session: load both function lists, build the GOG-respawn callee set
  (r2: `s 0x78b230; axt` + `pdr` call extraction), filter Steam candidates
  by callee overlap, verify by side-by-side disassembly, then patch.
