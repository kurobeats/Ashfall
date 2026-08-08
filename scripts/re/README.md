# Reverse-engineering verification scripts

Run against the real game binaries (Fallout3.exe / GECK.exe, v1.7.0.3 GOG) on
a Linux host with radare2 — see a Linux analysis host (`~/Downloads/fallout3/app`).

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
| ESM import | FalloutNV.esm + 5 DLCs: 488 weapons (all unique; 14 formIDs shared across masters), 6,452 NPCs, 380,497 refs, 772 factions — cross-verified vs independent python walker. 1 corrupt LAND record (of 33,179 compressed) skipped via `stats.skipped_compressed`. **Re-imported 2026-08-07 with `--import-index`: 496 weapons / 6,455 NPCs / 427,089 refs (collision recovery)** |

## Proton runtime (FO3 GOTY, Steam, Fedora 44)

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

## Steam FO3 GOTY — address table MISMATCH, 2026-08-06

The GOG 1.7.0.3-verified address table does **not** apply to the Steam build:

| Constant (GOG-verified) | Steam build (in-process probe, 16B at addr) | Verdict |
|---|---|---|
| `LOOKUP_FORM_BY_ID` 0x455190 | `d9 cc d9 cd d9 cb d9 cc 83 c2 01...` (FPU ops — garbage) | ❌ wrong for Steam |
| `EXTRACT_ARGS` 0x517950 | `1e 10 01 8b c8 8b 10 ff 52 04 68` (plausible code, unverified) | ⚠️ unverified |
| `CREATE_FORM_INSTANCE` 0x43CDA0 | `24 28 03 f0 e8 97 5f 64 00` (plausible code) | ⚠️ unverified |
| `CONSOLE_MANAGER_GET_SINGLETON` 0x62B5D0 | code tail, not a prologue | ⚠️ unverified |
| `FORM_HEAP_ALLOCATE` 0x401000 | `56 68 50 ce 23 01 8b f1 ff 15...` (vtable call pattern) | ⚠️ unverified |
| `DATA_HANDLER` 0x106CDCC | data bytes | ⚠️ unverified |

**Consequence**: any vtable-path pipe command (`OP_GET_POS`, `OP_GET_ACTOR_VALUE`,
`OP_IS_MOVING`, ...) crashes Fallout3.exe even with a valid loaded save —
`get_actor_value` on refID 0x14 killed the game; probe evidence shows the
underlying addresses are wrong for this binary.

Steam Fallout3.exe is SteamStub-packed (no import table); the unpacked image
is only in process memory. `/proc/<pid>/mem` is ptrace-blocked (yama), so the
dump must come from inside the process: bridge debug opcodes (see below).

### Bridge debug opcodes (temporary, in `commands.rs`/`hooks/mod.rs`)

| Opcode | What it does |
|---|---|
| `OP_PROBE_CODE` 0xFD | returns 16 bytes at each hardcoded engine address (no execution) — used to prove the mismatch above |
| `OP_PROBE_SAVES` 0xFE | resolves the save dir via `SHGetFolderPath` + counts `.fos` in-game |
| `OP_DUMP_IMAGE` 0xFC | streams the unpacked image (mapping containing 0x400000 from wine's `/proc/self/maps`) as `[0x04][size:4][bytes]` |

### Save location on Linux (verified)

Steam puts the prefix under the **game library**'s compatdata, not
`~/.local/share/Steam`:

```
$HOME/.local/share/.games/SteamLibrary/steamapps/compatdata/22370/
  pfx/drive_c/users/steamuser/Documents/My Games/Fallout3/Saves/   <- .fos files
```

(`SLocalSavePath=Saves\` in FALLOUT.INI; FO3 saves live in the `Saves/`
subdir, not the Fallout3 root. A hand-made prefix at
`~/.local/share/Steam/steamapps/compatdata/22370` from a direct `proton run`
is a decoy — the game never uses it.)

### Next step

Dump the unpacked image via `OP_DUMP_IMAGE`, disassemble locally
(`i686-w64-mingw32-objdump -b binary -m i386 -D` on the raw dump), locate the
real `LookupFormByID` (884-call-site function), re-derive the address table
for the Steam build, then re-run the vtable round trip with a loaded save.

## GOG downloads verified (2026-08-07) — classic build resolved

Both games re-downloaded from GOG (innoextract on a separate host) and the
address tables re-verified **statically on the real executables** — no game
process needed:

| Check | FO3 1.7.0.3 GOG | FNV 1.4.0.525(a) GOG |
|---|---|---|
| exe md5 | `7691d7180f225ee8e876358d170ecc93` (documented ✓) | `0f374bae0d6c34b754d3a487d49486ba` (documented ✓) |
| `LOOKUP_FORM_BY_ID` 0x455190 | **883 direct call sites** (doc: 884) | — (FNV: form-map global 0x11C54C0, 0 calls as expected) |
| `EXTRACT_ARGS` | 0x517950 = 434 calls | 0x5ACCB0 = 480 calls |
| `CREATE_FORM_INSTANCE` | 0x43CDA0 = 7 calls | 0x465110 = 7 calls |
| `CONSOLE_MANAGER` | 0x62B5D0 = 33 calls | 0x71B160 = 32 calls |
| `GET_FORM_BY_ID` | — | 0x483A00 = 43 calls |

**Key finding: the GOG FO3 exe IS the classic Steam-era build.** The vaultmp
patch sites match byte-for-byte: 0x6D5965 already holds vaultmp's restored
`75 03` (respawn guard), 0xE10FF1 holds the `.txt` tail of `Plugins.txt`
(vaultmp's `.vmp` patch target), 0x45F704 = `74 2a` (the byte vaultmp
overwrote with `EB`). The Steam mismatch reported earlier is the
**post-2023 Steam update**, not GOG. The whole bridge table set + all 34
vaultmp recipes apply to the GOG exe as-is.

ESM import updated with the real files: FO3 = 124,540 records / 299 weapons /
3,613 NPCs / 747k refs; FNV = 141,502 records / 496 weapons / 6,455 NPCs /
427,089 refs / 772 factions. FNV counts now exceed the original run (488 /
6,452 / 380,497) because `--import-index` assigns distinct load-order bytes,
recovering the 14-formID cross-master collisions. One GRA quirk: 95 refs
authored at hi=0 are genuine base overrides (correct); 1 ref authored at
hi=2 collides with HonestHearts (1 row in 427k).

## Steam build runtime re-derivation (2026-08-07, live on the game host)

Full pipeline exercised against the real Steam FO3 GOTY (post-2023 build,
exe md5 `8a3adab8...`) under Proton Experimental:

1. **Deploy**: `ashfall_bridge_proxy.dll` → game dir as `dinput8.dll`;
   launch via `steam steam://rungameid/22370`, xdotool Return on the
   launcher; bridge binds `127.0.0.1:1771` (retry loop added — the launcher
   holds the port until it spawns the game).
2. **Probe**: `python3 scripts/re/bridge_probe.py --action probe` — 16 bytes
   at each classic address. Confirmed 0x455190 = FPU garbage in Steam
   (build mismatch, as documented).
3. **Dump**: `--action dump` — new PE-header-based `dump_image()` (reads
   e_lfanew → SizeOfImage directly; `/proc/self/maps` fails inside
   pressure-vessel). Two runs byte-identical in code — stable.
4. **Derive**: call-count histogram + structural disassembly vs the GOG
   binary. ⚠️ The first histogram pass had a +0xC00 offset bug (dump offset
   = VA − 0x400000, NOT PE roff math) — caught by direct byte-pattern
   search for the prologue (`55 8b ec 53 56 57 8b 3d 84 4b 22 01`).

**Steam table** (in `hooks::mod.rs::fo3_steam_17`):

| Function | Steam addr | Confidence |
|---|---|---|
| `LookupFormByID` | 0x711EF0 (form map global 0x1224B84) | high — prologue byte-match, 880+ call sites, structure identical to GOG |
| `ExtractArgs` | 0x787530 | medium — ~431 call sites (GOG 434) |
| `ConsoleManager::GetSingleton` | 0x788B30 | medium — 33 call sites |

Auto-selected at runtime: `vtable::fo3_lookup_addr()` reads each candidate's
prologue (`51 8b 0d` = GOG/classic, `55 8b ec 53` = Steam).

**Calling conventions — two live-found bugs:**
- `LookupFormByID` is **cdecl** (plain `ret` epilogue in both builds) — the
  bridge called it stdcall → 4-byte stack imbalance → crash. Fixed.
- Gamebryo **vtable methods are thiscall** (this in ECX, callee cleans) —
  the bridge used `extern "system"` (stdcall). Fixed with inline-asm thiscall
  shims (`vcall_0..vcall_3`, `scripts/re/thiscall_test.rs` validates them
  under wine).

**Live results (loaded save, Steam build):**
- `OP_GET_POS` → real coords `(42878.5, -72844.4, 11118.6)` ✓ field reads
- `OP_GET_ANGLE`, `OP_GET_PARENT_CELL` ✓ field reads
- `OP_GET_DEAD` stub ✓, form probe: lookup returns a real object with a real
  vtable (all entries .text pointers) ✓
- **Blocked**: vtable-call getters (`get_base`, `get_actor_value`, ...).
  The Steam TESObjectREFR vtable layout differs from the xFOSE assumption
  at index 4 (holds a destructor with `ret $0x4` + delete-flag arg, not a
  no-arg GetBaseForm) — calling it corrupts the stack. Next: locate the
  real GetBaseForm slot (or the baseForm field) via the object-field probe
  (`OP_PROBE_FORM` dumps obj fields; player base form = 0x707).
