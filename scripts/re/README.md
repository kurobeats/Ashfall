# Reverse-engineering verification scripts

Run against the real game binaries (Fallout3.exe / GECK.exe, v1.7.0.3 GOG) on
a Linux host with radare2 — see analysis host (`~/Downloads/fallout3/app`).

Every hardcoded constant in `ashfall-bridge` was verified with **two
independent tools**:

| Check | Tool 1 | Tool 2 |
|-------|--------|--------|
| 15 GECK interception opcodes | python command-table walker (`CommandInfo+0x08`) | `r2_opcode_check.py` — radare2 `/x` string+pointer search, `p8` entry dump, `ps` name read |
| Exe identity (crc32/md5) | python zlib/hashlib | radare2 `ph crc32` / `ph md5` (raw `-nn` mode) + `md5sum` |
| 7 hardcoded function addresses | objdump disassembly | `verify.r2` — radare2 `aaa`/`axt` xrefs + `pd` disassembly |
| pos offsets 0x2C/0x30/0x34 | xFOSE `GameObjects.h` (STATIC_ASSERT) | vaultmp-extended `vaultmp.cpp` GetPosAngle |
| refID offset 0x0C | xFOSE `GameForms.h` | vaultmp-extended (object+0x0C) |
| parentCell 0x3C/0x40 | xFOSE/xNVSE headers | binary: 3,877 `[reg+0x3C]` read sites (objdump + r2) |
| ESM import counts | `ashfall-server --import-esm` + sqlite | independent python record walker (exact for all mapped types) |

Usage (on the host with the binaries):

```bash
# Function addresses (Fallout3.exe)
r2 -q -i verify.r2 Fallout3.exe

# Opcodes (GECK.exe — the script compiler's command table)
python3 r2_opcode_check.py
```

Expected results:

```text
PlaceAtMe 0x1025  AddItem 0x1002  RemoveItem 0x1052  EquipItem 0x10EE
UnequipItem 0x10EF  ForceActorValue 0x110E  KillActor 0x108B  SetRestrained 0x10F3
PlayGroup 0x1013  Lock 0x1072  UnLock 0x1073  SetOwnership 0x1117  Activate 0x100D
SetStage 0x1039  SetAlert 0x105A
Fallout3.exe crc32 = 425A8C16, md5 = 7691d7180f225ee8e876358d170ecc93
```

The bridge's constants live in `crates/ashfall-bridge/src/hooks/` (opcode.rs,
vtable.rs, mod.rs) and were corrected to match these verified values.

## New Vegas (FalloutNV.exe / Geck.exe, 1.4.0.525 GOG)

Extracted with `innoextract` (`setup_fallout_new_vegas_1.4.0.525(a)_(55068).exe`),
analyzed identically:

| Check | Result |
|-------|--------|
| Exe identity (two tools) | crc32 `881FDAF8` (python + `r2 -nn ph crc32`), md5 `0f374bae0d6c34b754d3a487d49486ba` (r2 + md5sum); the bridge's fabricated FNV CRC `0x0206FEC7` matches nothing |
| FNV address table (xNVSE RUNTIME block, r2-verified) | ExtractArgs `0x5ACCB0`, CreateFormInstance `0x465110`, ConsoleManager `0x71B160`, FormHeap `0x401000/0x401030`, GetFormByID `0x483A00`, bEchoConsole `0x11F158C` — in `bridge::mod::fnv_14` |
| FNV LookupFormByID | no direct function — xNVSE wraps the form-map global `0x11C54C0` (xref-confirmed) |
| parentCell offset | 0x40 — 8,924 `[reg+0x40]` reads in the binary (tool 2) + xNVSE header (tool 1) |
| Opcodes | FNV's GECK/game command tables are RUNTIME-BUILT (no static name→opcode array — the FO3-style scan finds nothing). Values verified via FO3 GECK binary + xNVSE `SetReturnType` (shared gamebryo VM opcodes) |
| ESM import | FalloutNV.esm + 5 DLCs: 488 weapons (all unique; 14 formIDs shared across masters), 6,452 NPCs, 380,497 refs, 772 factions — cross-verified vs independent python walker. 1 corrupt LAND record (of 33,179 compressed) skipped via `stats.skipped_compressed` |
