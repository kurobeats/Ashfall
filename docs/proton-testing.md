# Proton runtime testing — Fallout 3

Status of the bridge inside the real game (FO3 GOTY, Steam, Proton
Experimental, verified 2026-08-06 against the real game — see
[scripts/re/README.md](../scripts/re/README.md) for the result matrix).

## What works

- **Injection**: `ashfall-bridge-proxy` shipped as `dinput8.dll` loads in
  the game process. `WINEDLLOVERRIDES="bridge=n,b"` does NOT work (nothing
  imports `bridge`). Full instructions: [proton-setup.md](./proton-setup.md).
- **Bridge TCP server**: `127.0.0.1:1771` LISTENING inside Fallout3.exe.
- **Pipe protocol**: wakeup (`01` → `01`) and stub-path commands
  (`OP_GET_DEAD`, `OP_GET_ACTOR_VALUE` — no engine deref) round-trip.
- **Game stability**: stable at main menu; crashes only on vtable-path commands.
- **Save discovery**: real saves dir = game-library compatdata, `Saves/` subdir
  (see below).

## Why vtable commands crash (verified 2026-08-06, loaded save)

**The classic address table (xFOSE/vaultmp-era) does NOT match the user's
Steam build.** In-process probe: `LOOKUP_FORM_BY_ID` at 0x455190
holds FPU-op garbage, not the form lookup. Calling any of these addresses
(or vtable calls through them) crashes the game — reproduced with a valid
loaded save (player ref 0x14): `OP_GET_ACTOR_VALUE` killed Fallout3.exe;
`OP_GET_POS` never ran after that. Probe evidence + constant table:
[scripts/re/README.md](../scripts/re/README.md) (Steam FO3 GOTY section).

**Update (2026-08-07, GOG download analyzed):** the classic table is NOT
wrong for GOG. The GOG 1.7.0.3 exe (md5 `7691d7180f225ee8e876358d170ecc93`)
matches the vaultmp-era patch sites byte-for-byte (0x455190 = 883 call sites;
0x6D5965 holds vaultmp's restored `75 03`; 0xE10FF1 holds the `Plugins.txt`
tail; 0x45F704 = `74 2a` as vaultmp patched over). So the Steam build the
user ran is the post-2023 updated exe, and the GOG exe is the classic build
this project's whole table set was verified against. If the user's Steam
build stays updated, re-derive for it; if they run the GOG exe (DRM-free,
works under Proton), the existing table + `hooks::vaultmp` recipes apply
as-is.

Until the Steam-build table is re-derived: **do not send vtable-path commands
(OP_GET_POS/SET_POS/GET_ANGLE/GET_CELL/GET_PARENT_CELL/IS_MOVING/
GET_ACTOR_STATE/GET_ACTOR_VALUE/GET_BASE) — every one crashes the game.
Only stub-path commands are safe (`OP_GET_DEAD`).**

## What crashes the game (verified)

Any command reaching a real vtable call — see the mismatch note above.
A loaded save does NOT make them safe: the GOG-verified offsets are wrong
for the Steam build, so the crash is in the lookup, not the missing player.

## In-game verification plan

1. Launch — GOG build (recommended, DRM-free, table-correct): extract with
   `innoextract`, drop `ashfall_bridge_proxy.dll` as `dinput8.dll` in the
   game dir, `WINEDLLOVERRIDES="dinput8=n,b"` or direct run.
   Steam build: Steam → Fallout 3 → launcher → Enter (SteamStub DRM: must
   go through Steam; launcher is the default target and needs the Play
   click — post-2023 build needs table re-derivation first).
2. Load any save (player ref 0x14 now valid) — GOG saves live in the
   extracted game's `Documents/My Games/Fallout3/Saves/`; Steam saves in
   the game library's compatdata:
   `~/.local/share/.games/SteamLibrary/steamapps/compatdata/22370/pfx/
   drive_c/users/steamuser/Documents/My Games/Fallout3/Saves/`.
3. Pipe round trip from the host (python, no tools needed):

```python
import socket, struct
s = socket.create_connection(("127.0.0.1", 1771), timeout=10)
def cmd(op, refid=0x14):
    s.sendall(b"\x02" + struct.pack("<I", op) + struct.pack("<I", op)
              + bytes([4]) + struct.pack("<I", refid))
    return s.recv(64).hex()
print("get_dead:", cmd(0x19))          # stub path — safe everywhere
print("probe_code:", ...)              # 0xFD — safe (no execution)
print("get_pos:", cmd(0x0001))         # vtable — CRASHES until table re-derived
s.close()
```

4. **First re-verify the constants** (classic build — verified 2026-08-07
   for GOG): the GOG exe matches the table statically (883 call sites at
   0x455190, vaultmp patch sites byte-identical). If running the **Steam
   post-2023 build**, dump the unpacked image via `OP_DUMP_IMAGE` (0xFC),
   disassemble the raw dump with `i686-w64-mingw32-objdump -b binary -m i386 -D`,
   find the real `LookupFormByID` (884-call-site fn), then update
   `hooks/mod.rs::fo3_17`. Only then test getters with a loaded save.

## Constants — classic build verified for GOG; Steam (post-2023) differs

All in `crates/ashfall-bridge/src/hooks/vtable.rs` / `mod.rs`. The values
below were verified statically against the GOG 1.7.0.3 binary (xFOSE
headers + vaultmp-extended + r2 + objdump). **2026-08-07: re-verified on the
actual GOG download** — call-site counts and patch-site bytes match, so this
table is correct for the GOG exe (and it is the same build vaultmp used).
The in-process probe showed the values do NOT hold in the user's **Steam
build** (post-2023 update) — re-derive from a dump of that image before
trusting any there (see [scripts/re/README.md](../scripts/re/README.md)).
GOG is the recommended runtime: DRM-free and table-correct.

| Constant | Value (FO3 1.7) | Source | Verify against |
|----------|-----------------|--------|----------------|
| `VTBL_REF_GET_POS` | VTable+0x30 | xFOSE GameObjects.h | loaded save, `get_pos` = player coords |
| `VTBL_REF_GET_BASE_FORM` | VTable+0x10 | xFOSE | `get_base` = player base formID |
| `VTBL_ACTOR_GET_VALUE` | VTable+0x68 | xFOSE | `get_actor_value(0x14, 0x14)` = health |
| `VTBL_ACTOR_GET_BASE_VALUE` | VTable+0x70 (estimated) | vaultmp | base value |
| `VTBL_ACTOR_ANIM_DATA` | VTable+0x01E4 | vaultmp.cpp GetActorState | `is_moving` |
| `OFFSET_ANIM_MOVING` | 0x4E | vaultmp.cpp | `is_moving` |
| `OFFSET_ANIM_WEAPON` | 0x54 | vaultmp.cpp | weapon state |
| `OFFSET_ANIM_IDLE_PTR` | 0x118 → 0x2C → 0x0C | vaultmp.cpp | idle anim formID |
| pos/rot/parentCell offsets | 0x2C/0x30/0x34, 0x20/0x24/0x28, 0x3C | xFOSE + 3,877 `[reg+0x3C]` reads in binary | loaded save |

Re-verification method options (in order of laziness):

1. **Runtime**: load save → call each getter → compare against expected
   values (player position from the map, health from pip-boy). The
   reference implementation is vaultmp-extended, which works, so wrong
   offsets here are transcription bugs, not ground truth.
2. **Static**: r2/objdump cross-check of the VTable layout against the
   xFOSE `STATIC_ASSERT`s (already done for addresses; vtables are the
   remaining gap) — see [scripts/re](./scripts/re).

## Safe command set (stub path — usable at any time)

`OP_GET_DEAD` (0x19) is the only fully safe vtable-independent command.
`OP_PROBE_CODE` (0xFD), `OP_PROBE_SAVES` (0xFE), `OP_DUMP_IMAGE` (0xFC) are
debug-only and safe (reads, no execution). **Everything else in `commands.rs`
either stubs (returns zeros) or hits a vtable call — check the
`crate::hooks::` call in the dispatch arm before sending it. vtable-path
commands crash the Steam build regardless of save state.**

## Steam build — live re-derivation progress (2026-08-07, tetsuo)

Full runtime pipeline exercised against the real Steam FO3 GOTY (post-2023
build, exe md5 `8a3adab8...`) under Proton Experimental:

- **Injection** ✓ — dinput8 proxy loads, bridge TCP on 127.0.0.1:1771
  (added a bind retry: the launcher holds the port until it spawns the game).
- **Dump** ✓ — `dump_image()` now reads the PE header directly (e_lfanew →
  SizeOfImage); `/proc/self/maps` fails inside pressure-vessel. Two runs
  byte-identical in code.
- **Steam table derived** ✓ — `LookupFormByID` = 0x711EF0, form-map global
  0x1224B84 (prologue byte-match `55 8b ec 53 56 57 8b 3d 84 4b 22 01`,
  880+ call sites, structure identical to GOG). Auto-selected at runtime by
  `vtable::fo3_lookup_addr()` (reads 0x455190: `51 8b 0d` = GOG, else
  checks 0x711EF0: `55 8b ec 53` = Steam).
- **Convention bugs found + fixed**:
  1. `LookupFormByID` is **cdecl** (plain `ret` in both builds' epilogues) —
     was called stdcall → 4-byte stack imbalance → crash on return. Fixed.
  2. Gamebryo vtable methods are **thiscall** (this in ECX, callee cleans) —
     `extern "system"` (stdcall) was used → garbage ECX. Fixed with
     inline-asm thiscall shims `vcall_0..vcall_3` (i686), validated by
     `scripts/re/thiscall_test.rs` under wine (PASS).
- **Live with a loaded save**: `OP_GET_POS` → (42878.5, -72844.4, 11118.6),
  `OP_GET_ANGLE` → (6.3, 0.0, 186.9), parent cell read, form probe shows a
  real object (0x0303A6F4) with a real vtable (0xF93958, all entries .text).
- **Blocked**: vtable-call getters (`get_base`, `get_actor_value`, `is_moving`).
  The Steam TESObjectREFR vtable layout differs from the xFOSE assumption:
  index 4 holds a destructor (`ret $0x4`, delete-flag arg) not a no-arg
  GetBaseForm; calling it corrupts the stack (game dies). The object-field
  probe (`OP_PROBE_FORM` dumps obj fields) is in place — next: scan the
  object fields for a pointer to the player base form (FO3 base form
  ID 0x707) to find the baseForm field, and map the real GetBaseForm slot
  in the Steam vtable.

Commands that work on the Steam build now: OP_GET_DEAD (stub), OP_GET_POS,
OP_GET_ANGLE, OP_GET_PARENT_CELL, OP_PROBE_CODE, OP_PROBE_FORM, OP_DUMP_IMAGE.
Do NOT send vtable-call commands (OP_GET_BASE, OP_GET_ACTOR_STATE,
OP_GET_ACTOR_VALUE, OP_IS_MOVING) until the GetBaseForm slot is re-derived —
they crash the game (reproduced twice).
