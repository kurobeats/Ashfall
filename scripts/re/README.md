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
