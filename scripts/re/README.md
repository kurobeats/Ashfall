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

## Proton runtime (FO3 GOTY, Steam, Fedora 44, test host)

Real-game injection test of `ashfall-bridge` + the new `ashfall-bridge-proxy`
(dinput8.dll proxy) under Proton Experimental (11.0-100), RX 6700 XT, DXVK 3.

| Check | Result |
|-------|--------|
| Fallout3.exe identity | PE32 i386, **SteamStub-packed** (no import table, 5 sections) — cannot run outside Steam; `proton run` directly exits 0 silently |
| Injection via `WINEDLLOVERRIDES="bridge=n,b"` | **does not work** — overrides only load DLLs something imports; nothing imports `bridge`. README's old fallback path was broken-by-design |
| Injection via `dinput8.dll` proxy (game imports dinput8, app-dir native wins over builtin) | **works** — bridge DllMain runs in the real game process |
| Bridge TCP server | `127.0.0.1:1771` LISTENING inside Fallout3.exe (verified in launcher and game processes) |
| Pipe protocol round trip | wakeup `0x01→0x01` OK; `OP_GET_DEAD` (stub path) → `[03][key][00]` OK — full request/response against the live game process |
| Real vtable commands at main menu (`OP_IS_MOVING`/`OP_GET_ACTOR_STATE` via `vtable::get_actor_state`) | **crash the game** — no player ref exists at menu (refID 0x14 is garbage) and anim-struct offsets are the still-unverified constants. Repro: broken pipe + process exit on 2nd command |
| Game stability | launcher auto-runs (FO3 GOTY default target = `Fallout3Launcher.exe`); game stable 70s+ at menu without bridge commands |

Stack for future runtime tests: `cargo build --release --target i686-pc-windows-gnu -p ashfall-bridge-proxy`
→ copy to game dir as `dinput8.dll` → launch via Steam (DRM) → Enter in launcher.
Vtable commands need a loaded save (player ref valid) + offset re-verification.
