# Steam Build Reverse-Engineering — Findings & Plan

Target: FO3 GOTY Steam (post-2023), exe md5 `8a3adab8...`, .text 11.6MB
(10MB classic). Toolchain: Python + objdump locally, radare2 6.0.5 on the
r2 analysis host. Image: `OP_DUMP_IMAGE` from the live process (PE-header based,
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
| **Respawn disable** (patch sites) | 0x6D5965 / 0x78B230 | **0x9C43A5 / 0x8C9CE0→0x8C9D5D / 0x8C9D52** | semantic anchor (death-UI string → death flow) + probe-verified live; applied + behavior-verified 2026-08-08 |
| **AI predicate** (actor-discovery detour) | 0x6FAE90 (`56 8B F1`) | **0x7D0A50** (`56 8B F1`) | Classic detour byte-guards `56 8B F1`. ⚠️ Two earlier re-derivations were **prologue false-positives**: 0x7F9B70 (`55 8B EC 51 57`) and **0x7DAF80** — both install byte-perfect but ~never fire live (3 NPC events / 120s combat). **CORRECTED 2026-08-17 (data/re lane A1):** the true 1:1 structural twin is **0x7D0A50** — same vtable slot order (+0x234/0x22C/0x3E0/0x230/0x214), death-state cmp 5/3, player singleton 0x123C674, 12 callers. 0x7DAF80 has a different order (vtable+0x22C first, player-compare early, helper 0x7DAF50) and NO death-state cmp. Verified: 0x7D0A50 = `56 8b f1 8b 06 8b 80 34` (classic = `56 8b f1 8b 06 8b 90 34`). Wired into `STEAM_AI_PREDICATE` 2026-08-17. |
| **PlayIdle stub** (anim_detour hook) | 0x73BB20 | **0x85E0A0** | byte-exact `c7 81 14 04 00 00 00 00 00 00 c3` (`mov dword [ecx+0x414],0; ret`), unique hit; same 11B method + padding shape. Re-derived 2026-08-14 |
| **Lock fix** (disable vanilla lock-bypass) | 0x527F33 | **0x798B65** | 8B prefix match `74 02 88 08 6a 01 8b c8` + identical tail; vcdiff EXACT cover agrees. Re-derived 2026-08-14 |
| **AI pause fix 1** (NOP 2B) | 0x72051E | **0x5E99E2** | vcdiff EXACT: `74 15 83 f8 03 74 10` → `74 1e 83 f8 03 74` (JE + cmp eax,3 + JE shape survived). **live-verified 2026-08-15**. 2026-08-14b |
| **GetActivate interception** | 0x78A68D | **0x8D3BC8** | vcdiff EXACT: `0f 84 dd 00 00 00 8b 06 8b ce 8b 80 00 01` — guard bytes identical, vtable slot shifted 0x224→0x100. **live-verified 2026-08-15**: jmp + ret **0x8D3CB8** (`8b 4d 08 83 f9 01`) both confirmed. 2026-08-14b |
| **Delegator stub spot** | 0x6EDBD9 | **0x405E69** | int3 padding after `ret` — vaultmp plants its PUSH-ECX stub here (stub bytes injected, only location translates). 2026-08-14b |
| **PlayGroup fix** (EB 27 into pad) | 0x49DD6A | **0x4350F9** | int3 padding + `cmp dword [0x13148c4],0; je`; vcdiff EXACT. 2026-08-14b |
| **FireWeapon relay** (call site / callee) | 0x71F05F / 0x4BE1A0 | **0x7DF3F7 / 0x770880** | E8 rel math verified (call → callee in both builds) + call-site shape + callee size/prologue. **live-verified 2026-08-15**: call `e8 84 14 f9 ff` → callee `53 8b dc` prologue. 2026-08-14 |
| **AV fix** (ActorValue formatter) | 0x473D35/3B/3E85 | **0x5B7AC7 / 0x5B7ACC / 0x5B7AE2** | vtable +0x130 call survived + "%s %s (%08X)" string; fn 0x5B79B3 SEH-prologue twin of classic 0x473C50. 2026-08-14 |
| **plugins.txt → .vmp** | 0xE10FF1 | **0xF9FDB1** | `.txt` at +9 of `\Plugins.txt` (0xF9FDA8), exact same layout. Re-derived 2026-08-14 |

Auto-detection: `vtable::fo3_lookup_addr()` reads 0x455190 (`51 8b 0d` = GOG)
vs 0x711EF0 (`55 8b ec 53` = Steam). Respawn patch is byte-guarded inside
`vaultmp::apply_steam_respawn()` (no-op unless the Steam bytes are present).

## Actor discovery (NPC sync) — GOG + Steam mapped

Session 2026-08-13 (r2 on battlecruiser.chaotic.lan, GOG Fallout3.exe):

**Findings (classic/GOG):**

| Item | Address | Notes |
|---|---|---|
| AI predicate `fcn.006FAE90` | **0x006FAE90** | `bool __thiscall(Actor*)` — the engine's per-actor AI processing gate. Entry `56 8B F1` (push esi; mov esi, ecx). 11 call sites: HighProcess + PlayerCharacter + combat/weather paths. Reads `[this+0xFC]` state (cmp 3/5), vtable `+0x214`, compares against PlayerCharacter singleton |
| HighProcess (ProcessLists high-actor processing) | **method @ 0x732BF0** | vtable slot 0x29C, 2994 bytes, per-actor. ai_fix2/ai_fix3 recipes live in the predicate; ai_fix1 (0x72051E) is a sibling state machine; ai_fix4 (0x42FBDC) is the object-creation path |
| PlayerCharacter singleton | **0x107A104** | hundreds of `mov ecx,[0x107A104]` sites (loaded as `this`); the predicate special-cases it. Bridge's `LOCAL_PLAYER_REF` is the ref id, this is the object |
| RefID offset | +0x0C | xFOSE-verified, already in the bridge |

**Dead ends (don't re-tread):** no direct `call [reg+0x29C]` and no E8-direct calls to HighProcess — dispatch is via a pointer field (the 12 `mov reg,[reg+0x29C]` sites are OTHER classes' member fields). xFOSE and STR (Skyrim/F4) do NOT document FO3's ProcessLists — the layout was never public.

**The shortcut that landed:** the AI predicate IS the active-actor list —
no ProcessLists layout needed. The bridge detours it (classic only,
byte-guarded `56 8B F1` prologue): a thunk preserves `this` (ecx), calls
the collector (reads formID at actor+0x0C), then runs the original through
the trampoline. `hooks::discovery` keeps the seen-set (STR VisitForms diff)
and a 10 Hz flush thread emits EVENT_NPC_SPAWN / EVENT_NPC_REMOVE frames
-> client -> ActorNew + OwnershipClaim / ObjectRemove. Installed from
`hooks::install()` (DllMain-safe: memory writes only; the flush thread
starts from the TCP server thread).

**Steam re-derivation: DONE (2026-08-17)** — Steam twin found at **0x7D0A50**
(1:1 structural twin of classic 0x6FAE90: same vtable slot order
+0x234/0x22C/0x3E0/0x230/0x214, death-state cmp 5/3, player singleton
0x123C674, 12 callers). Two earlier derivations were false positives:
0x7F9B70 (`55 8B EC 51 57 8B F9` prologue — structurally different)
and 0x7DAF80 (different slot order, no death-state cmp). See Session
2026-08-15b + data/re lane A1 for the correction. `ai_predicate_site()`
picks classic vs Steam by prologue signature (both `56 8B F1`). Details in
the Solved table above. The vaultmp AI-pause recipe twins (ai_fix1..3)
were derivable from it and are now wired in `apply_steam_vaultmp()`.
live-probed.

## Open: vaultmp behavior-patch sites

The 34 `hooks::vaultmp` recipes target classic addresses. The Steam recompile
changed code layout AND function boundaries — byte-pattern and seed matches
produce decoys (e.g. a JNZ+0x83 + global-load false positive at 0xB59356).
Per-site semantic identification is required. **Progress 2026-08-14:**
respawn + AI predicate + play_idle_call_src + lock_fix + fire_weapon +
plugins_vmp are SOLVED (see Solved table + session notes below); get_activate
candidates found (needs live probe). Remaining: AI pause (ai_fix1/4, ai_fix2/3
inside the known Steam AI predicate 0x7D0A50), fire_fix, match_race,
place_at_me, delegators.

**Respawn (highest value) — SOLVED for Steam (2026-08-08)** — GOG anatomy:
- Site A: 0x6D5965, 20-byte predicate (fcn.006D5960): `jne +3; xor al,al;
  ret; mov edx,[ecx+0x14]; push 1,0,edx,eax; call 0x6D5750`. vaultmp NOPs
  the JNE → always false.
- Site B: 0x78B230, `jne +0x83; mov ecx,[0x107A0D4]; mov byte[ecx+2],1`.
  vaultmp: NOP site A + WriteRelJump(0x78B230 → 0x78B2B9) — flag write never
  runs, players stay dead (server revives).
- **Steam twins (verified byte-level, side-by-side disasm):**

| GOG (classic) | Steam (post-2023) | role |
|---|---|---|---|
| 0x6D5965 `75 03` | **0x9C43A5** `75 03` | site-A JNE in predicate fn 0x9C43A0 (structurally byte-identical to fcn.006D5960; inner call 0x6D5750 → 0x9C3FE0) |
| 0x78B230 `0F 85 83 00 00 00` | **0x8C9CE0** `0F 85 77 00 00 00` | site-B JNE guarding first respawn-flag write; tests site-A predicate result (`test al,al` @ 0x8C9CDE, call @ 0x8C9CD9 → 0x9C43A0) |
| 0x78B2B9 | **0x8C9D5D** | skip destination (common continuation `mov eax,[edi]`; all three death-handler skip paths converge here) |
| 0x78B235 | **0x8C9CE5** | leftover byte after 5-byte WriteRelJump → NOP |
| flag struct +2 | 0x107A0D4 | **0x123C5D4** (`mov byte [eax+2],1` @ 0x8C9CEB) |
| death-handled flag | 0x107BA66 | **0x1228871** (written @ 0x8C9D56) |
| ==2-path flag write (vaultmp leaves it) | 0x78B2AE | **0x8C9D52** (dead-state path, leave for parity) |
| reload-save UI ptrs | 0xF65568/74/80 | **0x11017DC/E8/F4** ("Reloading the most recent save game" @ 0xF60DB8) |

  Steam death-flow fields that survived the recompile: 0xFC (death state,
  cmp 1/2), 0x5A8/0x5AA, 0x6E5, 0x184; vtable slots 0x214, 0x2A0 — same
  method (PlayerCharacter death handler), same structure. Patch = NOP
  0x9C43A5 (2B) + WriteRelJump 0x8C9CE0→0x8C9D5D + NOP 0x8C9CE5 (1B).
- Method: callee-graph matching was NOT usable here (aflj merges these
  methods into `method.*` nodes; no callees in JSON). Instead: string xref
  ("Reloading the most recent save game" → ptr cluster → death UI block)
  + `mov byte [reg+2],1` pattern scan (13 sites, 2 = respawn writes) +
  death-state/global-flag fingerprint. NOTE: the dump IS flat (offset =
  VA − 0x400000) — r2 `-m 0x400000` PE-parses the dump and shifts .text
  by +0xC00; subtract 0xC00 from r2-derived addresses (verified live via
  OP_PROBE_CODE).

### Respawn — residual risk (==2-path death menu)

Only the flag writes are NOP'd. In the death-state-2 path the SP death menu
UI call (0x8C9D43 `call 0x8FA990`, pushes of the Main-Menu/Reload strings)
is still live: a death that reaches state 2 (death-handled 0x1228871 == 0
and the 0x9C4970 check passing) can still show the menu. Observed deaths
park at state 1 (both flags stay 0), so it never fired. If it ever shows:
NOP the 5-byte call at 0x8C9D43 (the 9 pushes balance with `add esp,0x24`
at 0x8C9D48, stack stays aligned).

## Remaining patch-site groups — GOG anatomy + anchors

GOG addresses + recipe names live in `hooks/vaultmp.rs` (`FO3_STEAM_CLASSIC`
+ `recipes()`). vaultmp semantics source (fetched 2026-08-08):
`https://raw.githubusercontent.com/foxtacles/vaultmp/master/source/vaultmpdll/vaultmp.cpp`
(also `vaultmp.hpp`). For each group: disassemble the GOG site, extract the
structural signature, find the Steam twin semantically, probe-verify the VA
live (OP_PROBE_CODE) before patching. Group list:

| Group | vaultmp fn / what it does | GOG table fields |
|---|---|---|
| AI pause (4) | `aiFix1..4` — stop NPC AI processing in unloaded cells | ai_fix1 (NOP 2B), ai_fix2 (redirect), ai_fix3 (6B block), ai_fix4 (11B NOP) |
| Fire relay (2) | `FireWeapon` — relay fire calls so the server sees shots | fire_fix_jmp (3B jump), fire_fix_patch (9B block) |
| PlaceAtMe/activate (3) | `PlaceAtMe`, `GetActivate` — intercept spawn/activate | place_at_me_jmp/call/fix(+dest), get_activate_jmp/ret |
| Race match (2) | `matchRace` NOPs + param — body-type desync fix | match_race_nop1 (18B), nop2 (3B), patch, param |
| Lock fix (1) | `LockFix` — disable vanilla lock-bypass check | lock_fix (NOP) |
| Delegators (3) | `BethesdaDelegator`, `AnimDetour`, `PlayIdleDetour` — anim/idle forwarding | delegator_src/dest/call_src, play_idle_call_src/fix_src, play_group_fix(+src/dest) |
| PlayGroup/AV (extra) | `PlayGroup`, `AVFix` — anim group + actor-value fixes | play_group 0x45F704, av_fix_src/ret/term |

Anchors that survive recompiles (from the respawn work): player/actor field
offsets (0xFC death state, 0x5A8/0x5AA, 0x6E5, 0x184), vtable slots (0x214,
0x2A0), static UI strings (string → ptr-globals → code), and structural
signatures (early `jne +3; xor al,al; ret` predicates).

## Plan for the patch work (separate focused effort)

1. **r2 batch workflow** (verified working): `r2 -q -e scr.color=0 -i script.r2
   bin > out.txt`, python-parse. Analyze BOTH binaries (GOG PE + Steam raw
   @0x400000), dump function lists (addr/size). Callees are NOT in the
   aflj JSON — build callee sets per function with `s <addr>; axt` + `pdr`
   call extraction (see handoff below).
2. **Function-pair mapping**: for each GOG function containing a patch site,
   find the Steam function by size proximity + callee-graph similarity
   (recompile preserves function structure more than bytes).
3. **Verify each pair** by disassembling both sides side-by-side; only then
   patch. Status: respawn ✅, AI predicate ✅, play_idle_call_src ✅,
   lock_fix ✅, fire_weapon ✅, plugins_vmp ✅ (2026-08-14); get_activate
   candidates found (live probe pending); AI pause, fire_fix, PlaceAtMe,
   race match, delegators remain.
4. Live-verify each patch with a game restart (apply → observe behavior).

## Live probe infrastructure (in bridge, read-only where possible)

- OP_PROBE_CODE 0xFD — 16 bytes at table addresses (incl. the 4 respawn sites)
- OP_PROBE_FORM 0xFB — obj + 128 vtable entries + 64 obj fields
- OP_PROBE_PTR 0xFA — deref 16 dwords at an address (used for the respawn-
  flag struct: probe 0x123C5D4 → dword0 = struct ptr → probe struct →
  byte+2 = respawn flag)
- OP_VCALL_TEST / OP_VCALL_TEST0 — guarded thiscall calls (crash = wrong
  index, game restart needed)
- OP_DUMP_IMAGE 0xFC — full image (PE-header based, FLAT layout)
- scripts/re/bridge_probe.py + probe_baseform.py — python clients
- scripts/re/thiscall_test.rs — thiscall shim validation under wine

## Test hosts (session notes)

- **r2 analysis host** — r2 6.0.5; binaries in /tmp: `Fallout3.exe`
  (GOG PE), `steam-fo3.bin` (Steam dump).
- **game host** — live FO3 GOTY under Proton; bridge proxy deployed as
  `dinput8.dll` in the game dir; bridge TCP on 127.0.0.1:1771.

Full access/setup details (paths, launch incantation, X auth, poll
patterns) live in the untracked `hosts/` notes — kept out of git on
purpose. Respawn-flag check pattern: OP_PROBE_PTR chain — probe
0x123C5D4 → dword0 = struct ptr → probe struct → byte +2 = respawn flag;
0x1228871 = death-handled.

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

- r2 function lists (aflj) dumped for both binaries on the analysis host:
  `/tmp/gog_fns.json` (30,364 fns) + `/tmp/steam_fns.json` (17,724 fns);
  copies saved locally at `data/fallout3/` (gitignored).
- Parsing: use the `addr` field — fully populated (30,364 / 17,724),
  decimal VA. No `offset` field exists; names are `fcn.00ae92c0`-style
  (~20k) + `method.*` merged nodes (~10k) + 1 `entry0`.
- ⚠️ **aflj gaps**: vtable-only / indirectly-called code is NOT a function
  entry. The GOG respawn site 0x78B230 is inside
  `method.PlayerCharacter.5.virtual_760` (0x788350–0x78B8D3, 13,699B) —
  the "1699B respawn function" is a sub-range, not an aflj boundary.
  Same on Steam: LookupFormByID 0x711EF0 is absent from the list (nearest
  fns start 0x711F90). Extract real sub-function boundaries (prologue /
  ret scan from the site) or match on the containing method.
- Size-matching sanity check: Steam size==1699 gives **2** candidates
  (0x64D850, 0xAE92C0) — the earlier "147" figure came from a fuzzy pass
  and is not reproducible from the saved JSON.
- **Respawn mapped + LIVE-VERIFIED behaviorally (2026-08-08, game host)**: Steam
  sites found
  semantically (string → death-flow fingerprint) — see table above. Site A =
  0x9C43A5 (NOP), site B = 0x8C9CE0 → 0x8C9D5D (WriteRelJump, NOP @
  0x8C9CE5). **Trap: the dump is FLAT (offset = VA − 0x400000) — r2 PE-parse
  of the dump shifts .text by +0xC00.** Probe-verified live: address
  0x9C4FA5 (r2-derived) holds `2f 8b 0d` live; the true site-A bytes `75 03`
  are at 0x9C43A5. Verify candidate VAs live (OP_PROBE_CODE) before patching.
- **Live test results**: land death → player stays dead on ground 6+ min,
  position frozen, no auto-respawn, no SP death menu; respawn-flag byte
  (struct 0x123C5D4 + 2) stays 0, death-handled flag 0x1228871 stays 0;
  game stable. Water death crashed ~3 min post-death once (natural FO3
  water-death crash — land death is stable, so not the patch).
  The ==2-path flag write (0x8C9D52) is now ALSO NOP'd (4B, guarded) —
  no death path can set the respawn flag. Re-verified live: both flag
  writes blocked, flags stay clear while dead, game stable.
- Next session: map the remaining sites the same way (semantic anchors,
  probe-verify VAs): AI pause (4), fire relay (2), PlaceAtMe/activate (3),
  race match (2), lock fix (1), delegators (3).

## Session 2026-08-14 — remaining vaultmp sites, static pass (r2 on battlecruiser)

Dump refreshed on battlecruiser (`/tmp/Fallout3.exe` + `/tmp/steam-fo3.bin`,
scp'd from `data/fallout3/` after its reboot wiped /tmp). Tools added:
`scripts/re/gog_bytes.py` (PE VA→offset, fixed the image-base bug — the old
version missed 0x400000 and read garbage), `steam_twin_search.py`
(GOG site bytes → byte-search in the flat dump), `steam_pair_map.py`
(GOG containing fn → Steam size-proximity candidates).

**Confirmed (in `fo3_steam_17_vaultmp` table + byte-guarded):**

| Site | Classic | Steam | Method |
|---|---|---|---|
| play_idle_call_src (anim_detour) | 0x73BB20 | **0x85E0A0** | byte-exact, unique hit |
| lock_fix | 0x527F33 | **0x798B65** | 8B prefix + identical call tail |
| fire_weapon_jmp / fire_weapon_call | 0x71F05F / 0x4BE1A0 | **0x7DF3F7 / 0x770880** | call-site shape + callee size/prologue |
| plugins_vmp | 0xE10FF1 | **0xF9FDB1** | exact `.\Plugins.txt` layout |

**Candidates found, need live probe (OP_PROBE_CODE / restart) before wiring:**

- **get_activate** — SOLVED: jmp 0x8D3BC8 (vcdiff EXACT) + ret 0x8D3CB8
  (vaultmp-source + Steam flow analysis, see table below).
- **play_idle_fix** — 4 hits for the `cmp byte [reg+0x250],0` check
  (0x6EFDA9 / 0x6F00DA / 0x6F28DA / 0x6F45B3), all in one big fn — need
  disambiguation.
- **ai_fix1** (`cmp eax,3; je; mov [esp+0x18],0x19`) — 0 hits; the
  state-machine stack layout changed.

**Dead ends (don't re-tread):** av_fix's RTTI descriptor refs don't survive
(name strings exist at 0x1203A55/0x1204C95 but the dispatch code no longer
pushes them as imm32); vtable slots 0x224/0x218/0x2C0/0x278 all shifted
(note: +0x130 SURVIVED — av_fix was solved via it, see below); fire_fix's
0x7CB510 helper has no Steam twin; delegator stub (GetAsyncKeyState polling)
is vaultmp-injected code — only its int3-padding planting spot translates
(0x405E69).

**vcdiff method notes (see Session 2026-08-14b below):** only EXACT-cover
runs are trustworthy; gap-based translations and generic-prologue matches
are false positives. Scripts: `vcdiff_map5.py` (parse+verify), local copies
of the decoded files on battlecruiser `/tmp/steam-anniv.exe` +
`/tmp/classic_out.exe`.

Next session: live-probe the get_activate candidates on the game host, then
map AI pause / fire_fix / match_race / place_at_me / delegators the same way
(semantic anchors, probe-verify).

## Session 2026-08-14b — vcdiff breakthrough (online research + pristine exe)

The dead ends had a public solution all along: **FalloutAnniversaryPatcher's
`patch_steam.vcdiff` is a byte-level downgrade delta (Anniversary → classic
1.7.0.3)**. Decoding it with the pristine Steam exe maps every byte that
survived the recompile. Workflow:

1. Pristine Steam exe fetched from cyborg.wg (`~/.local/share/Steam/.../`),
   SHA1 `6d09781426a5c61aed59addec130a8009849e3c7` = f3_1704_steam (matches
   the patcher). Its .text is UNCOMPRESSED and byte-identical to our flat
   dump, just +0xC00 shifted (dump_off = file_off + 0xC00; VA = file_off +
   0x400C00).
2. `xdelta3 -d -s steam-anniv.exe patch_steam.vcdiff classic_out.exe` →
   SHA1 `2e57141a...` = f3_1703_mod (the patcher's exact target ≈ our GOG
   exe, 271 .text bytes differ). Decode VERIFIED by hash — ground truth.
3. `xdelta3 printdelta` dumps the instruction stream. **Trap: Offset and S@
   columns are DECIMAL, not hex.** CPY_0 = copy from Steam source; its
   Offset = classic target position. `scripts/re/vcdiff_map5.py` parses and
   verifies: 63,616 byte-identical runs (classic bytes == steam bytes).
4. Only runs that EXACTLY COVER a site are trustworthy. Gap-based
translations (nearest run + delta) are GARBAGE for rewritten code — proven:
   ai_predicate gap says 0x7DAF5E but the real site is 0x7D0A50
   (prologue-verified, corrected 2026-08-17; 0x7F9B70 was a false-positive).
   Generic SEH prologues (`6a ff 68`) also produce
   false EXACT covers (fire_weapon_call → 0x403E50 was a prologue
   coincidence, not the 4346B function).

**New EXACT-cover sites (byte-verified in both builds, in table):**

| Site | Classic | Steam | Verified bytes |
|---|---|---|---|
| ai_fix1 | 0x72051E | **0x5E99E2** | `74 1e 83 f8 03 74` (classic `74 15`) — JE + cmp eax,3 + JE shape survived |
| get_activate_jmp | 0x78A68D | **0x8D3BC8** | `0f 84 dd 00 00 00 8b 06 8b ce 8b 80 00 01` — EXACT guard, vtable slot shifted 0x224→0x100 (register alloc changed too: esi→edi). Ret target (0x78A995 twin) still needs live probe — the ret block was restructured |
| lock_fix | 0x527F33 | 0x798B65 | confirms byte-search (vcdiff + search agree) |
| play_idle_call_src | 0x73BB20 | 0x85E0A0 | confirms byte-search |
| delegator_dest / call_src | 0x6EDBD9 / 0x6EDBDA | **0x405E69 / 0x405E6A** | int3 padding after `ret` — vaultmp plants its PUSH-ECX stub here; only the padding location translates (stub bytes are injected) |
| play_group_fix | 0x49DD6A | **0x4350F9** | int3 padding + `cmp dword [0x13148c4],0; je` — vaultmp writes `EB 27` here |

**Still dead (need live probe or more RE):** fire_fix
(vtable +0x224 shifted, 0x7CB510 helper gone), match_race (+0x218/+0x110
shifted), place_at_me (+0x2A8 shifted), ai_fix2/3/4 (predicate restructured),
play_idle_fix, fire_weapon (call site 0x7DF3F7 + callee 0x770880 — E8 rel
math verified, structural only, probe before hooking).

**2026-08-14 static follow-up (no game):**

| Site | Classic | Steam | Evidence |
|---|---|---|---|
| **fire_weapon** (confirmed) | 0x71F05F → 0x4BE1A0 | 0x7DF3F7 → **0x770880** | E8 rel32 math: call at 0x7DF3F7 computes to 0x770880; classic 0x71F05F → 0x4BE1A0 — same call-site→callee relationship |
| **av_fix** (SOLVED, in table) | 0x473D35 / 0x473D3B / 0x473E85 | **0x5B7AC7 / 0x5B7ACC / 0x5B7AE2** | vtable +0x130 call SURVIVED + "%s %s (%08X)" sprintf string (0xF46628); fn 0x5B79B3 = SEH prologue twin of classic 0x473C50. Register alloc changed: `push [ecx+0xc]` (was `mov eax,[ecx+0xc]; push eax`), `call [eax+0x130]` (was `mov eax,[edx+0x130]; call eax`) |
| **get_activate_ret** (SOLVED, in table) | 0x78A995 | **0x8D3CB8** | Vaultmp source (GetActivate hook: captures EAX = the object at the jmp site, queues refID obj+0x0C, RelJump at jmp+5 → ret skips the loop body). Steam flow analysis: the jmp JE at 0x8D3BC8 targets 0x8D3CAB (loop continue); the loop exits to 0x8D3CB8 = `cmp [ebp+8],1` (activate-param/death-state check — the classic ret's `cmp byte [0x107ba64],0` twin). 0x8D3CB8 is the post-loop convergence |

Next session: live-probe fire_weapon + the remaining sites on the game host
(OP_PROBE_CODE), then use the same vcdiff workflow on any remaining sites.

## New Vegas (FNV 1.4.0.525) — static mapping session 2026-08-13

FNV needs its own addresses (different build/compiler than FO3), but there is
**no Steam/GOG split** — GOG 1.4.0.525(a) == Steam FNV, so one table covers
both. **Verified against a real Steam copy 2026-08-14:** Steam FNV
(`~/.local/share/Steam/steamapps/common/Fallout New Vegas/FalloutNV.exe`, md5
`516ed1c6...`) vs GOG (`0f374bae...`) — .text is SteamStub-encrypted on disk
but identical section layout (`.text` 0xbdd600 @ 0x401000), and `.data`/
`.rsrc`/`.reloc` are byte-identical. Only diffs: `.bind` section (0x71800,
the SteamStub loader) + 8 .rdata bytes — the import for `PI_IsSteamRunning`
(`steam_api.dll` vs GOG's `GalaxyWrp.dll`). The decrypted runtime .text is
the GOG code, so `fnv_14` + the anchors below hold on Steam unchanged. All
fnv_14 anchors re-verified in the GOG exe (prologues + `E8 25 11 BD FF`
frame-hook guard). The `fnv_14` table (xNVSE RUNTIME block) was already statically
verified; this session added the NPC-sync anchors:

| Item | FNV address | Source / method |
|---|---|---|
| PlayerCharacter singleton | **0x011DEA3C** | NVSE `g_thePlayer = (PlayerCharacter**)0x011DEA3C` (GameObjects.cpp) — double pointer |
| **Main-loop frame hook** | **0x0086B386** | NVSE `kMainLoopHookPatchAddr` — "7th call before first call to Sleep in oldWinMain" (Hooks_Gameplay.cpp). Loop confirmed: `call 0x43C4B0; ... Sleep(50)` per frame. `kMainLoopHookRetnAddr = 0x86B38B`. Wired as `vaultmp::apply_fnv_frame_hook()` (byte-guard `E8 25 11 BD FF`, hook calls the original getter + 10 Hz `report_player_state_due`, returns the result). FO3's equivalent per NVSE comment: 0x6EEC15 (mid-function dispatch `mov eax,[eax+0x288]` — NOT a call, deferred) |
| HighProcess (ProcessLists high-actor processing) | **0x008EEEC0** | vtable slot 16, 12,717 bytes — FNV's per-actor processor (the FO3 `+0x234` vtable fingerprint does NOT match FNV; slots differ) |
| Save/load hook anchors | 0x848C3C / 0x847C45 / 0x7D33CE | NVSE Hooks_SaveLoad — load/save/new-game events |
| FNV actor discovery | **SOLVED via AnhNVSE** — no AI predicate needed. FNV's ProcessLists is the **ActorProcessManager** (`nvse/nvse/GameProcess.h`, "straight from OBSE"): priority-tier `tList<Actor>` linked lists, object at **0x011E0E80** (`g_actorProcessManager = (ActorProcessManager*)0x011E0E80`), first tier `middleHighActors @ +0x00` (ListNode { data@+0, next@+4 }; the manager's own count method 0x977540 walks `[this+4]` — confirmed on the GOG binary). Wired: the FNV frame hook (0x86B386) enumerates the list each frame → `discovery::collect_ref_ids` → the 10 Hz flush diffs → EVENT_NPC_SPAWN/REMOVE. Other tiers (+0x0C/+0x18 low, +0x5C high — header marked "needs recalc") are host-verify candidates. |

**Dead ends:** mojave-online has NO patch-table recipes (only animation/
interpolation/entity_manager via PlaceAtMe expressions); NVSE source hooks
save/load/text/model-path only — no AI/process hook.

**Wired:** `apply_fnv_frame_hook()` (per-frame player-state, byte-guarded,
installed from `hooks::install()`). FNV discovery is DONE via the
ActorProcessManager (above) — the frame hook feeds the list walk. Remaining
FNV: live verification on the host (middleHighActors tier confirmed; the
+0x0C/+0x18 low and +0x5C high tiers are host-verify candidates).

## FO3 Anniversary/2023 build — community solution (online research 2026-08-13)

The FO3 "Anniversary" (post-2023) Steam recompile has **no public address
table** — the FOSE repo was deleted from ianpatt's account (the only xSE
missing), and no fork re-derives it. The community's answer is a **downgrade**:

- **FalloutAnniversaryPatcher** (c6-dev + lStewieAl, Nexus FO3 mod 24913,
  github.com/c6-dev/FalloutAnniversaryPatcher): SHA1-detects the build and
  xdelta-downgrades the exe back to classic 1.7.0.3. Recognized hashes:
  `f3_1704_steam` (Anniversary) = `6D09781426A5C61AED59ADDEC130A8009849E3C7`,
  `f3_1703_gog` = `FEB875F0EEC87D2D4854C56DD9CF1F75EC07A3B3` (our GOG exe
  matches this exactly — verified).
- **DECISION (2026-08-13): we will NOT downgrade.** The Steam/Anniversary
  re-derivation is the required path. The downgrade exists as a documented
  community option, but the project targets the live Steam build — every
  site gets re-derived (the classic table stays the GOG/comparison anchor).
- The post-2023 Steam dump we hold (SHA1 `92920737...`) is the unpacked
  SteamStub image — the downgrade targets the pristine exe, so re-derivation
  against the dump stays the fallback.

**FO3 classic frame hook now wired**: the main loop's frame body calls
0x6E3E40 (no-arg cdecl bool — menu/pause global check) at 0x6EEB2F, once per
frame (NVSE's FO3 anchor 0x6EEC15 is the same loop, dispatch shape).
`apply_fo3_frame_hook()` redirects the call (guard `e8 0c 53 ff ff`) → calls
the original + 10 Hz `report_player_state_due()`. Byte-guarded — no-op on
the Anniversary build (downgrade covers it).

**kFOSE** (lStewieAl/kFOSE) is the kNVSE animation fork for the classic
build — confirms the classic build is the modding baseline post-2023.

## FOSE source — provenance confirmed (online 2026-08-13)

The original FOSE source (deleted from GitHub) is archived at
**fose.silverlock.org** (`beta/fose_v1_3_beta2.7z` — full src tree + the
1.7/1.7ng DLLs). The `FALLOUT_VERSION_1_7` block in GameAPI.cpp matches
Ashfall's `fo3_17` table **exactly** (LookupFormByID 0x455190, ExtractArgs
0x517950, CreateFormInstance 0x43CDA0, ConsoleManager 0x62B5D0, DataHandler
0x106CDCC) — the table is now **quadruple-confirmed** (FOSE source + xFOSE +
live binary + the Anniversary-Patcher catalog from Project Crossroads,
2026-08-14 gh crawl — which also adds SET_POS 0x6F2050, QUEUE_UI_MESSAGE
0x61B850, alerted_state 0x6F6C70, sneaking_state 0x6F58B0 to `fo3_17`).
The `1_7ng` block is the 2010 NoGore variant (ConsoleManager
0x62B490), NOT the 2023 Anniversary build — no shortcut there. The version
blocks show FO3 address drift across releases (1.0→1.7: LookupFormByID
0x454CC0→0x455190 etc.) — per-site semantic re-derivation is the only way
for the Anniversary build.

## Session 2026-08-14c — Steam vtable re-derivation (static, r2 on battlecruiser)

The bridge's vtable-call ops (OP_GET_ACTOR_VALUE/STATE, OP_IS_MOVING) crash
on Steam because the recompile REORDERED the TESObjectREFR/PlayerCharacter
vtable. Static work this session (`scripts/re/vtable_steam.py`):

**Steam PC vtable base = 0xF938FC** (399 entries). Verified two ways:
- AI-predicate actor slot +0x22C → **0x8B8AF0** — matches `call [rax+0x22c]`
  in the Steam AI predicate (0x7F9C51); the predicate's actor is `esi`/`rsi`
  and `mov eax,[rsi]; ... call [rax+0x22c]` derefs the same vtable.
- Death-handler slot +0x23C → **0x8CA490** (in the 0x8C9xxx region).

**GOG PC vtable base = 0xE18110** (corrected 2026-08-18 Ghidra validation:
0xE16B10 is a different class's vtable — the true Actor vtable base, found
by scanning .rdata for the vtable whose +0x214/0x22C/0x230/0x234 hold the
AI-predicate dispatch targets 0x6F9EA0/0x787580/0x70C940/0x70C970; all
slots match incl. death-handler 0x788350 @ +0x2FC).

**Slot translation via byte-identical method matching**: 41 slots matched,
59% fit a **+0x58 shift** exactly (the recompile inserted 0x58/4 = 22 vtable
entries early). Confirmed translations (GOG → Steam):
- GOG +0x9C/0xA0 (lock-state getter `8a 41 0a 24 01 c3` = `mov al,[ecx+0xa];
  and al,1; ret`) → **Steam +0xF4/+0xFC** (0x57C770/0x57C780, byte-identical)
- GOG +0x1E8/0x1EC/0x1F0 (anim-data region) → Steam +0x240/+0x244/+0x248
- GOG +0x228 → Steam +0x280; GOG +0x2D4 → Steam +0x32C

**The early region (slots +0x00..+0x68) was REORDERED, not shifted** —
GetActorValue (+0x68) and GetBaseValue (+0x70) are NOT at +0x58-shifted
slots. Their Steam functions read the AV container field +0x1C4 (many
candidates found: 0x77C8C0, 0x784A7A, etc.) but none is a vtable entry in
the PC vtable — the Steam AV getter is not vtable-dispatched at +0x68, and
the AV container helper (GOG 0x6CF0B0) was rewritten (no byte twin). The
GOG 0x783900 "GetActorValue" is actually an AV set/get helper that calls
LookupFormByID + RTTI dispatch (same shape as get_activate). **GetActorValue
+ GetBaseValue + AnimData need live probing — no reliable static path.**

**Wired into the bridge**: `fo3_steam_vtable` module (BASE 0xF938FC,
GET_LOCKED +0xFC) + `steam_slot_for()` — `get_lock` now uses Steam slot
+0xFC when the running build is Steam (byte-guarded, GOG fallback).
Position/angle/cell/scale/refID stay FIELD-based (already Steam-safe).

Online research re-confirmed: **no public Anniversary vtable table exists**
(FOSE repo deleted; kFOSE/lStewieAl forks are classic-build only — checked
kFOSE's GameObjects.cpp: g_thePlayer 0x011DEA3C is the FNV build). The
community answer is the downgrade; we re-derive per-site.

Next: live-probe get_actor_value/state + is_moving on the game host
(OP_PROBE_FORM dumps the vtable — read the +0x68/+0x70/+0x1E4 entries),
then the remaining patch sites (fire_fix, match_race, place_at_me,
ai_fix2/3/4).

## Session 2026-08-14d-g — field-based getters + relay completion (no game)

The bridge had several stub getters/OP handlers that ARE field-writable
(no vtable call → Steam-safe). Completed them:

- **get_scale/set_scale** — were stubs (1.0/no-op); now field reads/writes
  (FO3 +0x38, FNV +0x3C — immediately after the pos triple, matching the
  documented layout).
- **is_dead** — was a TODO returning false; now reads the death-state
  field Actor+0xFC (survived the Steam recompile; the respawn handler does
  `cmp eax,2; je` there, AI predicate checks cmp 5/3).
- **set_lock** — lock byte +0xA bit 0, the same field the verified
  lock-state getter reads (`mov al,[ecx+0xa]; and al,1; ret`, GOG
  0x4017F0 / Steam 0x57C780). Wired OP_SET_LOCK + client UpdateLock mapping.
- **set_parent_cell** (FO3 +0x3C / FNV +0x40) → OP_SET_CELL.
- **set_enabled** (+0x50/0x54 bit 0x02, inverse of get_enabled) →
  OP_SET_ENABLED.
- **OP_MOVE_TO** — 20-byte params (ref + cell + xyz) → set_parent_cell +
  set_pos.
- **OP_SET_SCALE** (0x2B, new — moved off the 0x15 collision with
  OP_PLAY_SOUND) + client UpdateScale mapping.
- **OP_FIRE_WEAPON** — calls the engine fire routine via thiscall
  (`fire_routine_addr()`: classic 0x4BE1A0 / Steam 0x770880, byte-guarded).
- **Client relay** — packets_to_commands now maps UpdateActivate /
  UpdateFireWeapon / UpdateLock / UpdateScale / UpdateSound → the
  corresponding OPs (the server relayed these but receivers ignored them).

Remaining engine-bound OP stubs (no safe field path): OP_SET_NAME
(SetName vtable slot unmapped), OP_PLAY_SOUND's engine call, OP_PLACE_AT_ME
(engine spawn fn). These + the AV/anim vtable slots need the live host.

## Session 2026-08-14h — gh crawl: Project Crossroads + Anniversary catalog

Used `gh` (GitHub CLI, kurobeats auth, 5000-call budget) to crawl for
solutions to the live-bound tasks. Findings:

**Project Crossroads** (Brotaku-Vengeant, pushed 2026-08-14) — a VaultMP
lineage revival with working FO3 two-player movement. Confirms the
community direction: **Fallout Anniversary Patcher → classic 1.7.0.3 →
FOSE 1.2 beta 2** (downgrade + classic table). The FO3 update zip ships:
- `anniversary-patcher-1.7.0.3-patches.json` — the complete 31-patch site
  catalog with byte-verification (expected prefixes). Matches every vaultmp
  site we've been Steam-mapping exactly: respawn (0x6D5965/0x78B230),
  ai_fix1-4, match_race (0x52F4DD/0x52F50F/0xF51ADC), fire_fix
  (0x79236C/0x7923C5), get_activate (0x78A68D), place_at_me (0x539785/
  0x6F1CB6), play_group/delegator/actor_value_fix/plugins. **Independent
  validation that our classic table is the exact vaultmp lineage.**
- `anniversary-patcher-1.7.0.3-catalog.json` — the 31 downgrade patches.
- `fose-1.2.2-initialization-manifest.json` — 43 FOSE command-table writes
  (classic addresses, already known).
- `crossroads_fose_adapter.dll` — FOSE-API-based (no raw offsets; uses the
  extender, so it sidesteps the vtable problem entirely).

**New classic entry points from the catalog** (wired into `fo3_17`):
- `set_pos` 0x6F2050 (engine SetPos; bridge uses field writes instead)
- `alerted_state` 0x6F6C70 — `[this+0x60]` obj vtable +0x450 dispatch
- `sneaking_state` 0x6F58B0 — `[this+0x184]` obj vtable +0x20 dispatch
- `queue_ui_message` 0x61B850

`get_actor_state` alerted/sneaking (previously hardcoded false) now call
the classic getters (byte-guarded; both rewritten in Steam — the +0x60/
+0x184 field or +0x450/+0x20 vtable slots moved, confirmed no structural
twin in the flat dump).

**Crawl dead ends:** no public Anniversary vtable table exists (FOSE repo
deleted; kFOSE/VisualObjectives/ButcherPete are classic-only); vaultmp
forks all dead (2021 latest); the only active FO3 MP work is the downgrade
path. Our per-site re-derivation remains the correct approach for the
Steam/Anniversary build.

## Session 2026-08-14i — static RE exhausted + gh re-crawl (no game)

Systematic pass over every remaining site with semantic fingerprints
(reg/field/vtable-slot/singleton-xref), then a fresh gh crawl. Result:
**all remaining Steam sites are statically underivable** — the recompile
restructured every target function; only live probing (OP_PROBE_CODE /
OP_PROBE_FORM) can settle them. Two new confirmed globals/functions below.

**New confirmed (static, byte-verified against the flat dump):**

| Item | Classic | Steam | Evidence |
|---|---|---|---|
| `__security_cookie` | 0x106AB70 | **0x1202954** | `mov eax,[cookie]` refs: GOG 8,935× (all `xor eax,esp`) vs Steam 16,311×. Trap: the recompile switched the canary XOR — Steam dominates with `xor eax,ebp` (15,915×) over `xor eax,esp` (383×), so a classic-style `33 c4` scan undercounts 40×. Cookie value 0x24BB3820. Useful for /GS-function identification |
| Delegator fn | 0x6EDBE0 | **0x405E70** | `push ecx; push esi; mov esi,[0xd9b2a0]` (GOG, global-load) → `push ecx; push esi; mov esi,ecx` (Steam, thiscall — the singleton moved to a param). Stub span consistent: 7B before fn (GOG 0x6EDBD9/Steam 0x405E69, both already in `fo3_steam_17_vaultmp`) |

**Per-site dead ends (don't re-tread):**

| Site | Classic anchor | Why no static twin |
|---|---|---|
| match_race | +0x218 vtable ×2, +0x110 race field, +0x4c8 list | +0x218 slot gone (0 hits for `8b 82 18 02 00 00`); +0x4c8 field survives (4 hits) but the function was restructured; the vcdiff island 0x52F4E5→0x60E4F6 (`ff d0 84 c0 0f 84 a6`) is a cross-function coincidence |
| play_group | `cmp eax,1; je; mov edx,[esi+4]; cmp edx,[PC]` | `cmp edx,[0x123c674]` has 6 hits, none matches the structure; PC singleton has 1,424 `mov ecx,[…]` refs (no discriminator) |
| place_at_me internal (0x43DEF0) | SEH frame 0x30 + cookie + `mov edx,[esp+0x60]; xor esi; xor ebp` | body pattern 0 hits; `lea eax,[esp+0x44]; mov fs:[0]` (frame 0x30) 0 hits — frame size + body both changed |
| ai_fix2/3 | `cmp [esi+0xFC],5; je; cmp …,3; je` in predicate 0x6FAE90 | Steam predicate 0x7D0A50 (corrected 2026-08-17) — same slot order + death-state cmp 5/3 as classic; ai_fix2 = 0x7D0AA6 (write 0x2E), ai_fix3 = 0x7D0AD5 (85 FF 74 CE EB F6), both solved-static |
| fire_fix | vtable +0x224/+0x220 + helper 0x7CB510 | slots shifted, helper gone (re-confirmed) |
| frame hook target (0x6E3E40) | `mov eax,[0x10769b0]; … mov al,[eax+0x49]` | +0x49 field gone, menu global 0x10769B0 gone |
| delegator_src (0x6EEC86) | `mov ecx,[0x107a0d4]; call 0x6edbe0` | Steam `mov ecx,[0x123c5d4]; call` resolves to 0x9AE410/0x9B19E0 etc. — the delegator is now reached via those, not a direct E8 to 0x405E70 (only 1 direct call, local at 0x405F35) |

**gh re-crawl (kurobeats, 5000-call budget):** code search for
0x7F9B70 / 0x8D3BC8 / 0x123C674 → only `kurobeats/Ashfall` (no public
Anniversary table anywhere). `c6-dev/ButcherPeteFOSE` (pushed 2026-08-11)
is a full classic-FOSE source tree (GameObjects.h / GameForms.h /
GameRTTI.h) — confirms the classic vtable/field layouts (Actor::lifeState
0xFC, PlayerCharacter::firstPersonAnimData 0x5EC, disabledControlFlags
0x5DC) but is classic-only, no Anniversary slots. FalloutAnniversaryPatcher
ships only the vcdiff (already exhausted). Project Crossroads stays on the
downgrade + FOSE-API path. **Verdict unchanged: per-site live probe is the
only remaining path for fire_fix / match_race / place_at_me / ai_fix2-4 /
play_idle_fix / play_group / delegator_src and the AV/anim vtable slots.**

## Session 2026-08-14j — GECK RTTI investigation (other game files)

Question: do any OTHER files in either game help the remaining Steam RE?
Checked everything on cyborg.wg (the Steam install host):

| File | What it is | RE value |
|---|---|---|
| `Fallout3.exe` (Steam Anniv) | the recompiled engine | already exhausted |
| `Fallout3 - Garden of Eden Creation Kit.exe` | **installer stub** (68KB .text, zero `.?AV` strings) | none |
| `Fallout3Launcher.exe` | Steam DRM launcher | none |
| `FalloutNV.exe` | SteamStub-encrypted = GOG at runtime | already mapped |
| `FalloutNVLauncher.exe` | Steam DRM launcher | none |
| `GDFFalloutNV.dll` | Games-for-Windows-Live stub | none |
| `Fallout New Vegas/Geck.exe` | **the real GECK editor** (v1.4, md5 6ecfb21d…) | see below |

**GECK RTTI finding:** the editor builds RETAIN full MSVC RTTI (the game
exes strip it — FNV GECK has 2,322 `.?AV` type descriptors, named vtables
for Actor / TESObjectREFR / ActorValueOwner / TESFullName / TESForm).
`scripts/re/rtti_walk.py` walks TypeDescriptor → COL → vftable (the COL
signature is 0 in this build, not the usual 1) and enumerates named vtable
slots. **BUT the GECK cannot crack the Steam vtable problem:** (1) the
editor STUBS runtime-simulation methods — ActorValueOwner's AV getters are
`xor eax,eax; ret 4`; (2) the GECK vtable LAYOUT differs from the game's
(editor virtuals, stub overrides — FNV GECK Actor vtable +0x68 = a big SEH
Update method, not GetActorValue); (3) FNV (2011) ≠ FO3 (2008) — GECK
bodies don't byte-match FO3 classic; (4) the Steam recompile changed method
bodies regardless. Verdict: GECK RTTI is class-structure confirmation only;
the remaining Steam slots/patch sites still need the live host.

The FO3 GECK editor (2008, RTTI, would confirm classic vtable layout +
SetName slot) ships only inside the 7.6GB GOG FO3 GOTY installer
(`game-fallout.3.game.of.the.year.edition-12034` on archive.org) — the
Steam "GECK.exe" installer + `Fallout3_GECK_1.5_Update.exe` do NOT contain
the editor binary (the 8.9MB installer is an InstallShield web-downloader
bootstrap; the 1.3MB updater references GECK.exe but embeds no payload).
Downloading the 7.6GB RAR for classic-side confirmation only is low value;
skip unless SetName's classic slot is needed.

## Session 2026-08-14k — FO3 GECK acquired, RTTI verified, Steam AV still underivable

**The real FO3 GECK.exe (v1.5 era, 2008) was obtained** — the user's CDN
link (`cdn.bethsoft.com/fallout/3/geck/Fallout3_GECK.exe`) is the SAME
8.9MB InstallShield bootstrap as the Steam dir copy (md5 cabe531a...), but
the editor binary is EMBEDDED in its data1.cab. Extraction path that
worked: `unshield` couldn't parse the multi-volume IScab (v0x0095, data in
`data1.hdr`/`data1.cab` split), so ran the installer under Proton wine on
the game host (`wine Fallout3_GECK.exe` → installed to the prefix's
`Program Files (x86)\Bethesda Softworks\Fallout 3\`). Result:
`GECK.exe` 13.9MB (md5 bdc43722...) — the real FO3 editor.

**RTTI verified — 2,033 `.?AV` type descriptors** (the game exe strips
RTTI; the editor build retains it). `scripts/re/rtti_walk.py` enumerates
named vtables: Actor @ 0xD592DC (146 slots), ActorValueOwner @ 0xD3BD80
(11 slots), TESFullName @ 0xD23060, TESForm, TESObjectREFR, etc.

**Layout cross-validation (GECK ↔ game, both 2008):** the GECK Actor
vtable layout MATCHES the classic game's — shared byte-identical getters
sit at identical slots (lock getter `8a 41 0a 24 01 c3` = Actor-vtable
+0x7C in BOTH; GetAsForm = +0xA0 in both; game Actor-vtable base is in the
0xE1C8F4-family of .rdata vtables). **⚠️ doc correction: the lock getter
is at Actor-vtable +0x7C, not +0x9C/+0xA0** — the docs' +0x9C/+0xA0 (and
the Steam +0xF4/+0xFC mapping) were measured on the TESObjectREFR/base
vtable, not the Actor primary vtable. The bridge's `get_lock` targets
obj->vtbl[0] of whatever object it's called on; for pure TESObjectREFRs
the +0xA0 slot is right, for Actors the +0x7C slot is — worth a live
probe (OP_PROBE_FORM on both an actor and a plain ref) to confirm which
slot the bridge actually needs per object type.

**Steam AV getters: still statically underivable (now definitive).** The
GECK's ActorValueOwner methods are stubs (`xor eax,eax; ret 4` — the
editor stubs runtime AV simulation, same as FNV GECK). The Actor class
overrides them, but the game's real GetActorValue (Actor vtable +0x68 =
0x4F0E00, 2072-byte SEH method) has **no byte twin anywhere in the Steam
flat dump** — the recompile changed its body entirely. GECK RTTI does not
change the docs' verdict: GetActorValue/GetBaseValue/AnimData + the
remaining patch sites need live probing.

Net: the GECK RTTI is now a confirmed, reusable classic-layout oracle
(class-slot confirmation, future classic-side RE), but it cannot crack the
Steam re-derivation. The `rtti_walk.py` tool + this extraction path are
the durable artifacts.

## Session 2026-08-14l — vtable audit via GECK RTTI: get_lock/get_actor_value slot bugs

Cross-validating the GECK-RTTI vtable layouts against the classic game's
.rdata exposed that the DOCUMENTED vtable bases are run-start artifacts,
not class vtables. Two real bridge bugs found + fixed (byte-verified):

**get_lock — Steam ALWAYS returned 0.** The docs' "GOG +0xA0 → Steam
+0xFC" lock-getter mapping hit a MINORITY Steam vtable. The lock getter
`8a 41 0a 24 01 c3` sits at **+0xA0 in both builds** (GOG 0x4017F0 /
Steam 0x57C780, 66/71-vtable dominant TESObjectREFR family, NO shift);
Steam +0xFC in that family is `xor eax,eax; ret` (0x579C40). Fixed:
`get_lock_from_obj` uses +0xA0 and byte-guards the getter signature.

**get_actor_value/get_actor_base_value — removed (returned garbage /
corrupted flags).** The old "GOG PC vtable 0xE16B10" base was a vtable-
RUN-START artifact of a different class — CORRECTED 2026-08-18: the true
GOG Actor vtable is 0xE18110, which DOES contain the death handler
(0x788350 @ +0x2FC = 0xE1840C — the old "slot ~1599 of a contiguous run"
observation was this exact slot, mislabeled) and the lock/AV getters
(0x4017F0 @ +0xA0, 0x4017E0 @ +0x9C). Steam actor vtable 0xF938FC holds
the lock pair at +0xF8/+0xF4 (0x57C780/0x57C770 — the +0x58 region shift);
the REFR-family vtables carry them at +0x9C/+0xA0. At +0x68:
0xE16B10 has a flag-setter (`orb $8,[ecx+0x30]`), the dominant family has
a `ret 4` stub, the GECK-RTTI Actor vtable (0xD592DC ↔ game 0xE1C8F4
family) has a Process/Update method (0x4F0E00). **FNV is structurally
different too:** xNVSE + FNV GECK show ActorValueOwner is a MEMBER at
+0xA4 (composition), not a base like FO3 (inheritance) — so any
Actor-primary-vtable AV slot is wrong for FNV. The getters now return 0.0
with a ponytail note; the real GetActorValue needs a live OP_PROBE_FORM
(per object class: actor / PC / plain ref).

Method note: the GECK RTTI's Actor COL → 0xD592DC is a SECONDARY vtable
(lock getter at +0x7C there), while the object's vtbl[0] (dominant family,
lock at +0x9C/+0xA0) differs — RTTI class vtables must not be assumed to
be vtbl[0]. The `scripts/re/lock_scan.py` approach (scan .rdata vtable
runs for known getter pairs) is the reliable way to pin a slot.

**Concrete FNV path for the next live session (2026-08-14l follow-up):**
FNV `Actor::GetActorValue` = `((ActorValueOwner*)(actor+0xA4))->GetActorValueF(index)` —
the avOwner is an INLINE member at +0xA4 (xNVSE STATIC_ASSERT magicCaster
0x88 / magicTarget 0x94 / avOwner 0xA4; 16 `lea [reg+0xA4]` sites in the
GOG FNV exe), and GetActorValueF is its vtable slot 3 (+0x0C, float
return — FNV GECK ActorValueOwner vtable 0xD52AC4 confirms the 11-slot
order: GetBaseAVI/F, GetAVI/F, mods, GetPermAVI/F, GetAsForm,
GetActorLevel). Verify with OP_PROBE_FORM on a loaded FNV actor (read
[actor+0xA4], then its vtable +0x0C), then wire `get_actor_value`'s FNV
branch to that path.

**2026-08-14m follow-up — audit completed across ALL vtable ops:** the same
wrong-base problem hit `get_name` and `set_actor_value`:
- `get_name` called vtable +0x1C on the base form — but TESForm::GetFullName
  is a NON-virtual function (FOSE GameForms.h:517); +0x1C is TESForm slot 7
  (SaveAlt / ret-stub on the dominant family). Now returns "unnamed" safely;
  the real GetFullName address is a future re-derivation item (op is unused
  by server/client).
- `set_actor_value` called the "estimated" vtable +0x6C — a `ret 8` stub on
  the dominant family (silently no-op). Now a safe no-op; FNV's real
  SetActorValue is an Actor PRIMARY-vtable method (Get/SetAV live on the
  ActorValueOwner member at +0xA4 in FNV). Same live-probe list as
  get_actor_value.

Remaining vtable calls in the bridge are only: the AnimData (+0x1E4,
vaultmp's documented classic mechanism — runtime vtbl[0], keep) and the
byte-guarded get_lock (+0xA0). `get_actor_state`'s anim offsets (0x4E/0x54/
0x118) verified vaultmp-consistent against the FOSE AnimData struct
(animGroupIDs at +0x4C, u8 reads of UInt16 IDs fine for small IDs).

**2026-08-14n — FNV get_actor_value/get_actor_base_value WIRED via avOwner.**
The FO3 path resisted static derivation (GECK RTTI says ActorValueOwner at
Actor+0x7C, but the game binary shows ZERO lea/mov/add accesses at +0x7C —
the editor build's class layout differs from the game's; the game's AV
calls are not vtable-slot-2/3 patterns). FNV was certain, so it's wired:
`get_actor_value`/`get_actor_base_value` on FNV read the avOwner member at
+0xA4 (xNVSE STATIC_ASSERT + 16 `lea [reg+0xA4]` sites) and call its
vtable slot 3 (GetActorValueF, +0x0C) / slot 1 (GetBaseActorValueF, +0x04)
with a .rdata vtable-pointer guard. FO3 keeps returning 0.0 — the real
GetActorValue needs a live OP_PROBE_FORM (the game's AV access is likely a
direct function call, not a vtable call).

**2026-08-14o — command-table route: AV path triple-confirmed + more handlers.**
The static command tables exposed more:
- **FO3 GetActorValue handler** (entry 0xF54DA8, opcode 0x100E) → engine fn
  **0x50EF90**: `lea ecx,[actor+0x9C]` (avOwner; base-form path uses +0x100)
  → `call [avowner_vtable+0xC]` = GetActorValueF slot 3. Triple-confirms the
  wired path (ForceActorValue handler + GetActorValue handler + direct fn).
- **FNV SetActorValue** (ForceActorValue handler 0x5CD910 — corrected 2026-08-17
  data/re lane b1; 0x5BE190 was ModPCSkill): current via
  avOwner +0xA4 slot 3, delta applied via **Actor vtbl[0] slot +0x3A4**
  (FO3 uses +0x3A0). `set_actor_value` now wired for both builds.
- **Other handlers found** (classic): PlaySound 0x523590 (SoundManager
  0x11790C8 → 0xBCFBB0, complex — OP_PLAY_SOUND stays stubbed), PlaceAtMe
  0x53CA20 (internal 0x539280), PlayGroup 0x532690. OP_PLACE_AT_ME is
  unused by server/client — left stubbed.

**2026-08-14p — Steam/Anniversary per-frame hook re-derived + wired.**
The last "Steam per-frame" gap is closed: found the Steam main-loop frame
body via the respawn-struct write twin (`mov byte [0x123c5d4],1` at
0x9B3D92 — the classic 0x6EEB50 pattern). The frame body's per-frame call
is `call [0xF241E4]` at **0x9B3D77** — a SteamStub-relocated kernel32
timer import (IAT slot 0xF241E4, adjacent to Sleep's 0xF241E8; the
result is compared with the respawn-struct timestamp +0x10 at 0x9B3D83).
`apply_steam_frame_hook()` redirects it (6-byte guard `FF 15 E4 41 F2 00`,
E8 + NOP tail), and the hook calls the original through the IAT slot
(ASLR-safe — no hardcoded resolved address) + `report_player_state_due()`
per frame. Byte-guarded, no-op on classic/FNV. Main-loop structure mirrors
the classic: menu/pause check via global 0x123A93C `[+0x49]`, then the
frame work.

**2026-08-14q — kill_actor wired (classic); handler inventory complete.**
KillActor command handler (entry 0xF56130, opcode 0x108B) = engine Kill
0x71AC50(actor, killer, 0.0) + death processing 0x71C280(actor, cause,
limb, killer); KillActor signature (Killer, DismemberLimb, CauseOfDeath).
`kill_actor` wired for FO3 (FNV no-op — differs). Classic handler inventory
now complete: ForceActorValue 0x521F20, GetActorValue 0x521760 →
0x50EF90, KillActor 0x522030, PlaySound 0x523590 (SoundManager 0x11790C8
→ 0xBCFBB0 + 0xBD00C0 — intricate sound-instance flow, OP_PLAY_SOUND
stays stubbed), PlaceAtMe 0x53CA20 → 0x539280, PlayGroup 0x532690.
FNV: ForceActorValue 0x5CD910 (corrected 2026-08-17 — 0x5BE190 is
ModPCSkill 0x110F), PlaySound 0x5C4A70 (corrected — 0x5C21E0 is
GetDisease 0x1027).

## Session 2026-08-15 — live session (game host tetsuo.chaotic.lan)

First live-game session on the Steam build with the current code. Game
launched via Steam (Fallout3.exe), bridge live on 127.0.0.1:1771.

**Code fixes shipped (the i686 build had never actually linked — only
`cargo check`ed):**
1. `global_asm!` symbols lacked the C-ABI leading underscore on
   i686-pc-windows-gnu (`#[no_mangle]`/`extern "C"` get `_`, GNU as does
   not). Renamed all asm symbols (`_ashfall_*_thunk`, `_ashfall_hook_*`,
   `_ashfall_trampoline_addr`, `_ashfall_collect_actor_c`, `_ashfall_actor_
   collect_thunk`) + the `resolve()` extern decls/arms.
2. thiscall/vcall inline-asm shims (address.rs / vtable.rs) over-constrained
   registers ("requires more registers than available" on i686). Rewrote to
   `in("ecx") this` + `push {arg}` + `call {addr}` + `lateout("eax") ret`;
   disasm-verified the generated thiscall is correct.
3. TCP server dropped clients after 50ms idle — `set_read_timeout` returns
   `ErrorKind::TimedOut` on Linux, the old `Err(_) => break` closed the
   connection → every event frame was lost. Now matches
   `WouldBlock | TimedOut`.

**Live-verified (OP_PROBE_CODE / OP_PROBE_PTR, read-only):**
- respawn disable: all 5 sites patched, flag clear.
- fire_weapon: call 0x7DF3F7 `e8 84 14 f9 ff` → **0x770880** (callee prologue
  `53 8b dc` matches classic shape).
- get_activate: jmp **0x8D3BC8** `0f 84 dd 00 00 00 8b 06 8b ce 8b 80 00 01 00 00`
  EXACT + ret **0x8D3CB8** `8b 4d 08 83 f9 01` ✓.
- ai_fix1 **0x5E99E2** `74 1e 83 f8 03 74 19` EXACT ✓.
- play_idle_fix: all 4 candidates (`cmp byte [reg+0x250],0`) present —
  0x6EFDA9 / 0x6F00DA (`74 29`), 0x6F28DA (`74 4a`), 0x6F45B3 (`74 69`).
- play_group_fix **0x4350F9** (int3 pad + `83 3d c4 48 31 01 00 74`) ✓.
- delegator pad **0x405E69** (7× `cc` + `51 56 8b f1`) ✓.

**NOT firing (re-derivation needed):**
- **AI-predicate detour 0x7F9B70** — installed byte-perfect (live-verified the
  full chain: site `e9` → thunk `push ecx; push ecx; call collect_actor_c;
  add esp,4; pop ecx; jmp [trampoline]`; trampoline 0x001F0000 = original
  prologue `55 8b ec 51 57` + jmp back 0x7F9B75; call target =
  `_ashfall_collect_actor_c`). But it ~never executes: 3 NPC events over
  120s of combat + death (same ref 0x03008462). The `55 8B EC 51 57`
  prologue match is a false positive — not the per-actor gate.
- **Steam frame hook 0x9B3D77** — installed (`e8` + NOP; hook fn verified
  calling `[0xF241E4]` then `report_player_state_due`), but ZERO
  player-state events — alive or dead. The "frame body" twin is not a
  per-frame path. Re-derive a real per-frame site.

**Verified working live:** pipe round-trip + OP_GET_DEAD/GET_POS/PROBE_*,
respawn-patch behavior, discovery event pipeline (spawn/remove frames flow
when collection actually happens).

Next: re-derive the Steam AI-predicate + per-frame sites statically
(battlecruiser), then live-verify. Remaining patch groups still need Steam
twins: fire_fix, match_race, place_at_me, ai_fix2/3/4.

## Session 2026-08-15b — static re-derivation (battlecruiser, no game)

Follow-up to the live session's two "installs but never fires" findings.
Static work on `steam-fo3.bin` (flat, VA = file_off + 0x400000) + GOG
`Fallout3.exe`.

**AI predicate — RE-DERIVED (twice): 0x7DAF80 → CORRECTED 0x7D0A50 (2026-08-17).**
The 0x7F9B70 prologue match was a false positive (that function reads
`[edi+0xF8]`, references global 0x12399C8 — NOT the player singleton). The
classic predicate (0x6FAE90) is a 137-byte bool thiscall (`56 8B F1`,
11 callers incl. HighProcess). The 0x7DAF80 candidate kept the EXACT prologue
`56 8B F1` + the +0x22C(push 0) vtable call + `cmp esi,[0x123C674]`
singleton compare + [actor+0x60] sub-object — but its slot order differs and
it has NO death-state cmp 5/3 sequence; it ~never fired live (3 events/120s).
**2026-08-17 data/re lane A1 re-derived the true twin = 0x7D0A50** (1:1
structural: same slot order +0x234/0x22C/0x3E0/0x230/0x214, death-state
cmp 5/3, player singleton, 12 callers: 0x7e928f, 0x7ed315, 0x7efc0c,
0x7f18a6, 0x7f18f6, 0x86ac08, 0x88575c, 0x8a16ab, 0x8a7b1d, 0x8a9dae,
0x8c72fd, 0x8cdc3b). Verified `56 8b f1 8b 06 8b 80 34` (classic `56 8b f1
8b 06 8b 90 34`). Wired into `STEAM_AI_PREDICATE`
(byte-guard `56 8B F1`, now 3 bytes like classic). **Live fire-rate
unverified** — if the 2 surviving callers aren't per-actor, fall back to the
HighProcess twin.

**vtable slot map (classic 0xE18110 vs Steam 0xF938FC — base corrected
2026-08-18; slot values all verified):**
- The AI-predicate slots **did NOT shift**: +0x214/0x22C/0x230/0x234 hold
  recompiled twins at the SAME byte offsets (0x6F9EA0→0x75C6B0,
  0x787580→0x8B8AF0, 0x70C940→0x75DC50, 0x70C970→0x8992A0).
- The +0x58 shift is region-specific (anim 0x1E8/0x1EC/0x1F0 →
  0x240/0x244/0x248; 0x228→0x280; 0x2D4→0x32C; lock 0xA0→0xFC).
- classic +0x1E4 = 0x76FE20 (FPU math, NOT an anim pointer getter) — the
  bridge's `VTBL_ACTOR_ANIM_DATA` (vaultmp "index 121") is suspect. The
  real pointer-returning anim getters are 0x1EC (0x76CD00, returns
  [actor+0x5e8]/[actor+0x1a0]) and 0x1F0 (0x76FED0).

**Frame hook — site is CORRECT; the blocker was get_actor_state.**
0x9B3D77 sits inside 0x9B31A0 = the Steam `main()` (classic `main` =
0x6EE300, 3385B; Steam main called from CRT startup 0xD7A4DC). The frame
body is the classic twin (inlined pause check `cmp byte [eax+0x49],0` +
`call [0xF241E4]` + respawn-flag write `mov eax,[0x123C5D4]; mov byte[eax],1`
— that flag write appears ONCE in the whole dump, confirming this is the
main loop). So the hook fires every frame. The zero player-state events were
`get_actor_state` calling the wrong/uncertain anim-data slot (0x1E4) → the
wrong function returns a small int → the anim field reads fault → SEH
swallows it → no event. Added a result-pointer guard (`< 0x10000` → return
defaults) so `report_player_state` completes (pos/angle/health correct, anim
state zero) until the real anim-data slot is pinned. **The correct slot is
likely classic 0x1EC (Steam 0x244) — CONFIRMED 2026-08-18: Steam actor
vtable +0x244 = 0x8B8D40 / +0x248 = 0x8B8F60 (slot reads) and both fns are
32B BYTE-IDENTICAL to GOG 0x76CD00 (classic +0x1EC) / 0x76FED0 (+0x1F0).
The bridge's steam_slot_for() already maps them; no live probe needed.**

Next: live-verify 0x7D0A50 fire-rate (re-wired 2026-08-17). The anim-data
slot is resolved statically (see above) — no probe needed.

## Session 2026-08-17c — static follow-up: Steam command table + FNV signatures (no game)

Follow-up campaign (data/re lanes c1–c4, battlecruiser objdump + r2, all
static — no game sessions). See data/re/sessions/2026-08-17-c*.md.

**Steam FO3 command table FOUND** (the vaultmp-site unlock):
- Base **0x110B388**, 569 entries, stride 0x28, classic-FO3 format A
  ({name@+0, short@+4, opcode@+8, help@+0xC, flags@+0x10, params@+0x14,
  handler@+0x18, exec@+0x1C} + 8B tail) — NOT the FNV 0x1190950 layout.
- Handlers → engine fns: GetLocked 0x795450, GetActorValue 0x793080
  (avOwner path), PlayGroup 0x79EE20 (resolver 0x7D9E50 = classic 0x54B820
  twin), PlaceAtMe 0x79DDF0 → spawn 0x79DE90, PlaySound 0x79F9B0,
  Lock/UnLock 0x798AB0/0x7AF800 (vtable+0xC8), KillActor 0x798800 → engine
  Kill **0x7F3200** → death 0x7D4F40, ResurrectActor 0x7A1280 → 0x8C2B30,
  MoveToMarker 0x79BA90 → 0x79BC20, ForceActorValue 0x792770 → avOwner+0x9C
  slot 3 → SetAV 0x3A0 (confirms the bridge's FO3 path).

**FNV engine signatures pinned (c3, handler push-order + callee frame):**
- Kill 0x8B86E0(actor, killer, limb byte, isPlayer byte) — DIFFERS from
  FO3 0x71AC50(actor, killer, 0.0f); **wired into kill_actor** (was no-op).
- Resurrect 0x89D900(actor, killer, 0.0f), death-restore 0x8B51B0(3 args),
  GetActorValue 0x573170(actor, av_id, 0, 0, 1), MoveTo engine 0x79BC20.

**Wired 2026-08-17c:**
- `apply_steam_vaultmp()` — ai_fix2 0x7D0AA6, ai_fix3 0x7D0AD5, delegator
  chain (0x9B3EF6 relcall → stub 0x405E69), place_at_me (0x79E556
  reljumphook, 0x9CBCAF→0x9CBF97 fix), fire_weapon_jmp 0x7DF3F7 (E8
  rel32 → 0x770880 statically confirmed; RelJumpHook-over-E8 like classic).
- FNV `kill_actor` (0x8B86E0, 4-arg signature above).

**NOT wired (safe-guard):** Steam kill_actor (engine Kill 0x7F3200 mid-arg
[ebp+0xC] unmapped + death-processor arg order not handler-derived),
fire_fix relay stub (needs Steam register-alloc re-derivation), match_race
recipe bytes, play_idle twin choice (no E8 callers — vtable-only refs),
play_group (candidate dispatcher 0x580BD0 but entry-JE byte unpinnable),
play_group_fix_src (source fn inlined — 0 TLS-pattern hits), FNV
GetFullName + FNV lock getter (0x57B410 slot 0xB8 not fully confirmed).

**vtable map STATIC-EXHAUSTED (c4):** 89 byte-matches + 4 pinned groups;
call-graph/size/vector semantic fingerprints add 0 translations. Remaining
~310 Steam slots need OP_PROBE_FORM (confirmed statically underivable).
Re-confirmed: AV getters NOT vtable-dispatched, GetFullName non-virtual,
anim-state fields read raw off the +0x244 getter.

Next: live session (cyborg.wg) — verify 0x7D0A50 discovery fire-rate,
OP_PROBE_FORM the ~310 unmatched Steam vtable slots + 0x244/0x248 anim
pins, OP_PROBE_CODE the Steam kill death-path (0x7F3200 mid-arg +
0x7D4F40 arg order) to finish kill_actor, re-derive fire_fix relay stub +
match_race bytes from live disasm.

## Session 2026-08-18 — Ghidra headless validation of every address table (no game)

Ghidra 12.1.2 headless on battlecruiser (`/tmp/ghidra_12.1.2_PUBLIC`,
wrapper `/tmp/ghidra-headless.sh`). Tooling: `scripts/re/ghidra/VerifyRE.java`
(Ghidra 12 dropped Jython — Java post-script, Gson spec JSON) +
`scripts/re/ghidra/spec_{gog,steam,fnv}.json`. Imported GOG `Fallout3.exe`
(PE), Steam `steam-fo3.bin` (RawBinaryLoader, base 0x400000 — flat, no
+0xC00 shift), FNV `gog-fnv.exe` (PE). Steam raw-dump analysis is slow
(~1h for 17MB); the cmdtable/vtable/byte checks were re-verified directly
against the raw dump bytes (identical results, instant).

**Confirmed exact (all PASS, byte-level):**
- GOG: LookupFormByID 0x455190 (880 Ghidra refs vs doc 884 — count method
  differs, claim solid), AI predicate 0x6FAE90 (11 refs — EXACT), all
  engine fns + prologues, frame hook `e8 0c 53 ff ff`, respawn site
  0x6D5965 `75 03`, lock_fix `74 02 88 08`, ai_fix1/2/3, plugins ".txt"
  @ 0xE10FF1, anim getters (0x76CD00 slot 0x1EC, 0x76FED0 slot 0x1F0,
  0x76FE20 at 0x1E4 = FPU math as documented), avOwner 0x4017E0 (slot
  +0x9C), lock getter 0x4017F0 (slot +0xA0, `8a 41 0a 24 01 c3`), and
  every classic vaultmp recipe site (play_group je→jmp site, delegator
  call → 0x6EDBE0, place_at_me_jmp E8 → 0x43DEF0, fire_weapon_jmp E8 →
  0x4BE1A0, SEH prologues/epilogues, vtable +0x224 dispatch shapes).
- Steam: 13/13 command-table handlers at 0x110B388 (GetLocked 0x795450,
  GetActorValue 0x793080, PlayGroup 0x79EE20, PlaceAtMe 0x79DDF0,
  PlaySound 0x79F9B0, Lock 0x798AB0, UnLock 0x7AF800, KillActor 0x798800,
  ResurrectActor 0x7A1280, MoveToMarker 0x79BA90, ForceActorValue
  0x792770, SetRestrained 0x7A6670, SetOwnership 0x7A5C20 — the last two
  confirm opcode.rs); AI predicate 0x7D0A50 `56 8b f1` + **exactly 12 E8
  callers** (doc claim EXACT); LookupFormByID 0x711EF0 = 895 E8 calls
  (doc "880+" ✓); all vaultmp site guards (ai_fix2 `74 2a 83 f8 03 74`
  @0x7D0AA5, ai_fix3 cc×6, delegator `8b 0d d4 c5 23 01` + 0x405E69 pad,
  place_at_me `e8 25 5f f6 ff` → 0x704480 + fix `0f 84 e2 02 00 00`,
  fire_weapon `e8 84 14 f9 ff` → 0x770880, get_activate
  `0f 84 dd 00 00 00 8b 06 8b ce`, play_idle call `c7 81 14 04 00 00 00
  00 00 00 c3` (mov dword[ecx+0x414],0; ret), play_idle_fix twins
  `68 80 00 00 00 8b 01 ff 90 0c`, lock_fix, ai_fix1 `74 1e` (Steam form
  of the classic `74 15`), play_group pad, frame hook `ff 15 e4 41 f2 00`,
  respawn A `75 03` / B `0f 85 77 00 00 00` / B2 `c6 40 02 01`, kill guard
  `55 8b ec 51`); all 13 command handlers + engine fns + steam main
  0x9B31A0 + MoveTo engine 0x79BC20 (`53 8b dc` prologue — REAL fn on
  Steam).
- FNV: Kill 0x8B86E0 `55 8b ec 83 ec 08`, Resurrect 0x89D900, death-
  restore 0x8B51B0, frame hook `e8 25 11 bd ff` → 0x43C4B0 (3 refs),
  EXTRACT 0x5ACCB0 (called by ForceActorValue handler — seen in disasm),
  CREATE 0x465110, CONSOLE 0x71B160, GET_FORM 0x483A00, APM count
  0x977540, lock-getter candidate 0x57B410, ActorProcessManager
  0x11E0E80; **FNV command table 0x1190950 fully mapped (569 entries,
  saved to data/re/fnv_cmdtable_0x1190950.txt)** — KillActor idx 137 →
  0x5C7F10 ✓, ForceActorValue idx 268 → 0x5CD910 ✓, ModPCSkill idx 269 →
  0x5BE190 ✓ (both doc corrections confirmed), GetActorValue idx 12 →
  0x5B59F0, GetLocked idx 3 → 0x5C03D0, SetActorValue idx 13 → 0x5BD8A0,
  PlaceAtMe idx 35 → 0x5C4240, MoveToMarker idx 156 → 0x5CCAF0.
- FO3 SetAV path verified END-TO-END: ForceAV handler 0x521F20 gets the
  current value via avOwner (Actor+0x9C) vtable slot 3, then dispatches
  **Actor vtable +0x3A0 → 0x76E350** as thiscall(actor, index, delta, 0).
  The bridge's `set_actor_value` mirrors this exactly. (The doc label
  "SetAV = 0x521F20" is the command handler, not the setter.)

**Errors found + corrected (docs-only; bridge code is byte-guarded and
already safe):**
1. **GOG Actor vtable base is 0xE18110, NOT 0xE16B10** (0x1600 apart —
   0xE16B10 is a different class's vtable; no constructor references it).
   Correct base found by scanning .rdata for the vtable whose
   +0x214/0x22C/0x230/0x234 hold the AI-predicate dispatch targets
   (0x6F9EA0/0x787580/0x70C940/0x70C970) — all 4 + anim + AV + lock slots
   match on 0xE18110. All slot VALUES in steam-re.md were correct; only
   the base was wrong.
2. **Steam respawn "flag write @ 0x8CA8EB" is wrong** — that address is
   SSE math (`0f 5c 4d fc` movss sub), not `c6 40 02 01`. The real Steam
   flag writes are 0x8C9D52 (wired, verified) + 0x8C9D52's twin; GOG
   twin is 0x78B2B2 `c6 41 02 01` (docs said 0x78B2AE, off-by-4, and
   ecx-form not eax-form).
3. **FNV "MoveTo engine 0x79BC20" is wrong** (that's Steam's — on FNV
   0x79BC20 is int3 padding after a fn ending 0x79BC1D; candidate real
   fn = 0x79BC50 `55 8b ec 51`, pushed by the caller). FNV MoveToMarker
   handler = 0x5CCAF0 (table idx 156), not 0x79BA90 (Steam's). Bridge
   doesn't call FNV MoveTo engine (OP_MOVE_TO is a field write) — no code
   impact.
4. **Command-table name fields are POINTERS, not inline strings, in all
   three builds** (Steam/GOG classic-FO3 format A: name_ptr@+0, handler@
   +0x18; FNV format B: name_ptr@+0x10, 3 handler slots @+0/+4/+8). The
   steam-re.md "name@+0" wording implied inline.
5. **Steam "+0x9C/+0xA0 = 0x57C770/0x57C780 in both builds"** — TRUE for
   the TESObjectREFR family (hundreds of subclass vtables share them:
   0x57C770 = `8b 41 08 c1 e8 06`, 0x57C780 = `8a 41 0a 24 01 c3`), but
   the **Actor vtable 0xF938FC overrides both** (+0x9C → 0x759580,
   +0xA0 → 0x765910). The bridge's get_locked byte-guards the slot
   signature before calling (returns 0 on the Actor vtable instead of
   calling garbage) — safe, but lock reads on Steam actors return 0.
6. ai_fix1 Steam guard is `74 1e` (classic twin `74 15`) — matches the
   vcdiff note; the bridge's classic ai_fix1 (0x72051E `74 15`) is
   classic-only, Steam ai_fix1 is documented but NOT patched (no
   apply_steam site) — consistent.

**Bridge impact: NONE** — every byte-guard in apply_steam_respawn /
apply_steam_vaultmp / apply_classic_vaultmp matched the verified bytes;
the FO3 SetAV path is engine-identical. The doc errors above are now
corrected here; the vtable-base fix (0xE18110) matters for future Steam
vtable slot work (the +0x58 region-shift map in steam-re.md stays valid —
slot VALUES were all confirmed).

Next: live session (cyborg.wg) — unchanged from 2026-08-17c, plus
OP_PROBE_FORM the corrected GOG actor vtable 0xE18110 slots.

## Session 2026-08-18b — Ghidra decompile pass: Steam kill + match_race WIRED (no game)

Same Ghidra setup (see Session 2026-08-18). Decompiled the Steam kill
trio, match_race fn, play_idle twins, play_group dispatcher, AI predicate,
and the GOG counterparts (artifacts: scripts/re/ghidra/decomp_*.txt).

**Steam kill_actor WIRED (was no-op):** decompile proved 0x7F3200 is the
structural twin of GOG **0x71C280**, not of 0x71AC50 — bodies match
param-for-param ([this+0x1D0] killer lookup → [baseForm+0x180] → fn(limb)
→ +99 limb bit → vtable +0x3A8 damage → death processor). The former
"unmapped [ebp+0xC] arg" = **the LIMB** (feeds the +99 limb-bit read,
death-processor arg3, and the param_3==0 isPlayer test). Death processor
0x7D4F40 = 8-arg (0, limb_bit, limb, 0, cause, 0, 1000, isPlayer) — same
shape as GOG 0x71BFE0. Steam KillActor handler 0x798800 calls ONLY
0x7F3200 (no separate big-Kill) — the wrapper is the complete Steam kill.
Bridge: `call_thiscall_3(0x7F3200, actor, cause, limb, killer)`, guarded
`55 8b ec`, replacing the old no-op branch.

**Steam match_race WIRED (was pending-live):** decompiled fn 0x6F71E0 —
the +0x218 vtable helper call is gone; the race check is now the inline
+0x110 race-id compare `mov 0x110(%edx); cmp 0x110(%ecx); jne +0xc` @
0x6F7220/0x6F7226, with the SameRace scale (1.0) set on match. Bridge NOPs
the `75 0c` jne @ 0x6F722C (14-byte guard) → always same-race, the
vaultmp match_race semantics (prevents body-type desync).

**play_idle twin choice RESOLVED (decompile, still not wired):**
0x79DA88 site1 sits in FUN_0079D9C0 = a 3-arg thin wrapper; 0x79F2BB
site2 sits in FUN_0079F160 = the full 8-arg PlayIdle command handler
(EXTRACT_ARGS 0x787530, SEH cookie 0x1202954, form resolve via 0x711E90).
The handler site is the canonical script/console entry — hook 0x79F2BB
when the delegator relay lands.

**fire_fix derivation (decompile, still deferred):** Steam site 0x8DA397
is a 6-byte `mov eax,[eax+0x224]` + 2-byte `call eax` inside the
fire-loop fn 0x8DA370 (list @ +8, flags test 0x200820, then +0x220/
+0x314/0x83E3C0 fallbacks). No 3-byte fit (classic's EB-rel8 doesn't
transfer); a 5-byte E9 needs the mov+call relocated into a stub, and the
int3 pad at 0x8DA3CE is only 2 bytes — the stub needs a code cave.
Relay wiring stays pending-live (enhancement, not core sync).

**play_group:** 0x580BD0 confirmed as the anim-group dispatcher
(thiscall; group-id mapping 0x14→1/0x15→4, anim table @ +0xE0, timing
reads) — the vaultmp play_group fix rewrites its caller flow through the
delegator; entry byte still unpinnable — pending-live as before.

**AI predicate decompiler-confirmed 1:1 twin:** 0x7D0A50 vs GOG 0x6FAE90
decompile to the same structure (slots +0x234/+0x22C/+0x3E0/+0x230/+0x214,
death-state [actor+0xFC] cmp 5/3, player singleton 0x123C674 vs
0x107A104) — the identification is no longer just byte-pattern.

**FNV lock + naming (decompile/byte):** FNV GetLocked handler 0x5C03D0
dispatches ref vtable +0x1D0 → lock object; 0x57B410 sits at slot +0xB8
of ~19 FNV lock-object vtables (docs claim confirmed). FNV SetActorFullName
handler 0x5D1890 → engine name-setter **0x489100** (thiscall, writes via
0x4037F0) — the OP_SET_NAME engine hook for FNV. No FNV GetFullName
command exists (table has SetCellFullName 0x5D1870, SetActorFullName
0x5D1890, GetPlayerName 0x5DA060).

**Command-table bases now all known + dumped (scripts/re/tables/):**
GOG 810 entries @ 0xF525D0 (opcodes 0x0000+, events first), Steam 569 @
0x110B388 (opcodes 0x1000+), FNV 569 @ 0x1190950. All format name@+0 is a
POINTER; handlers @+0x18 (FO3/Steam) or three handlers @+0/+4/+8 with
name @+0x10 (FNV).

**Steam Ghidra analysis notes:** full raw-dump analysis takes ~50-60 min
(-process needs the saved program; the first run was killed before save).
xrefs on the analyzed project: LookupFormByID 894 refs, AI predicate
exactly 12 — matching the python E8-count (895/12).

## Session 2026-08-18c — vtable slots: NOT underivable after all (Ghidra decompile-twinning)

The "~310 unmatched Steam vtable slots need live OP_PROBE_FORM" claim is
**obsolete**. Ghidra recovered the map statically (scripts/re/ghidra/
VTableSlots.java + vtable_{gog,steam}.txt[.lines] + vtable_twins.txt):

- Every slot's fn was decompiled; each decompile normalized (addresses,
  fn/label names, numeric literals, local names stripped → structure kept)
  and fingerprinted (SHA1 of the sorted code-line set + line-set Jaccard).
- **147/260 Steam real fns have a decompile-matched GOG twin** (70 with
  Jaccard ≥ 0.7, incl. 50 at J=1.00; 77 moderate 0.4–0.7). Byte-matching
  found only 89 — decompile-similarity survives the recompile register
  re-alloc that byte-identity doesn't.
- **The remaining 113 all have a GOG fn at the SAME vtable offset** —
  restructured methods, position-identified (the AI-predicate slots
  +0x214/+0x22C/+0x230/+0x234 are exactly this: same slot + same caller
  context in both builds, different code). **0 true unknowns.**
- The +0x58 region shift confirmed by hash-identity on the tiny-getter
  pairs: +0x9C→+0xF4, +0xA0→+0xF8, +0x1E8→+0x240 (all IDENTICAL
  normalized decompiles); the AI-pred slots did NOT shift (same offset).
  The docs' "59% of 41 translated slots" was byte-matching's floor, not
  the real recovery rate.
- Dispatch-site xrefs are partial on the raw dump (only constructor
  vtable-copy sites get refs; `call [reg+0xNNN]` sites don't) — for the
  restructured methods, method identity = slot position + the documented
  caller map (AI predicate, kill chain, frame hook, command handlers).

Live OP_PROBE_FORM is now needed only to CONFIRM semantics of the 113
restructured methods (not to identify them), and even that can be deferred
until a specific slot is needed by the bridge.

## Session 2026-08-18d — FNV lock WIRED + naming/fire paths resolved (Ghidra)

**FNV lock relay WIRED (was no-op since 2026-08-17):** decompiled the FNV
Lock/UnLock command handlers (cmdtable idx 112/113 → 0x5C7280/0x5CBF80) →
both call the state setter **0x60CA30** (thiscall: ref, bool). Asm-verified:
Lock = `or byte [ecx+0x3C], 2`, UnLock = `and byte [ecx+0x3C], ~2` (+ a
vtable+0x48 refresh call). So **FNV lock state = [obj+0x3C] bit 1 (0x02)** —
not the FO3/Steam +0xA-bit-0 layout (the old docs' "FNV lock = +0x20 →
TESObjectLOCK vtable slot 0xB8" was the GetLocked command's internal path:
0x57B410 at +0xB8 of the REFR-family vtables = `[this+0x20] → vtable 0xB8`
wrapper, and `0x7AF430` = `[this+0x20]`). Bridge `set_lock_flags` +
`get_lock_from_obj` now branch on FNV to read/write `[obj+0x3C]` bit 1
(field ops, no vtable — mirrors the engine setter's raw write; the +0x48
refresh call is skipped, same trade-off as FO3/Steam).

**OP_SET_NAME (FNV) fully resolved (plumbing deferred):** SetActorFullName
handler 0x5D1890 asm: `ecx = form; add ecx,0x18; call 0x408DA0` (get) then
`push eax; ecx = form2; add ecx,0x18; call 0x489100` (set). So
**SetFullName = 0x489100(thiscall: form+0x18, name)** → wraps
**0x4037F0(form+0x18+0x4, name, 0)** = the game-heap string assigner
(FORM_HEAP_ALLOCATE 0x401000 / FREE 0x401030) writing the name char* into
TESFullName+4 — exactly the field the bridge's field-based get_name reads
(base_form+0x18+0x04). GetFullName = 0x408DA0(form+0x18). Wiring OP_SET_NAME
needs a game-heap name buffer in the bridge (plumbing), deferred until the
naming-sync feature is exercised.

**FNV AV getter path re-confirmed:** SetActorValue handler 0x5BD8A0 →
0x59C4F0 → reads via `[actor+0xA4] vtable slot 3` (the "GetActorValue: %s
>> %0.2f" print helper) — the avOwner +0xA4 / slot-3 claim holds.

**fire_fix clarified (deprioritized further):** the Steam site 0x8DA397's
enclosing fn 0x8DA370 is a per-actor fire-dispatch loop (list @ +8, flags
test 0x200820, vtable +0x224 fire dispatch, +0x220/+0x314 fallbacks) — the
NPC/projectile path. The player weapon-fire EVENT is already relayed by the
wired fire_weapon hook (0x7DF3F7 → 0x770880). fire_fix wiring would relay
the NPC-side dispatch — not core sync (owned-NPC simulation handles it) —
kept pending-live, lowest priority.

## Session 2026-08-18e — OP_PLAY_SOUND wired (last engine-blocked OP stub)

PlaySound command handlers decompiled (Steam 0x79F9B0 / GOG 0x523590,
cmdtable op 0x1026): both EXTRACT_ARGS then call a **cdecl**
`sound_play(sound_form, refID, flags)` engine entry — Steam **0x9CC980**
(SEH prologue `55 8b ec 6a ff`, plays via 0x9D2D70/0x9CF370), GOG
**0xBCFBB0** (frameless SEH `6a ff 68`). Flags mirror the handler logic:
0x40000101 default, 0x121 when the loop-count arg >= 1. Bridge
`play_sound()` (hooks/mod.rs) resolves ref + sound form, byte-guards per
build, calls the engine entry; OP_PLAY_SOUND (commands.rs) is wired —
remote sound relay complete. FNV PlaySound path not derived (no-op).
OP_PLACE_AT_ME remains an unused stub (documented); OP_SET_NAME's FNV
engine path is resolved (0x489100, this=form+0x18) but the game-heap name
buffer plumbing is deferred until naming sync has a caller.

## Session 2026-08-18f — FNV play_sound wired; fire_fix recipe complete; ai_fix4 shape

**FNV play_sound WIRED** (was no-op): FNV PlaySound handler 0x5C4A70 →
engine entry **0x5C4B30** = cdecl `(target_obj, sound_form, loop,
pos_override, type, volume)` — arg order differs from FO3/Steam's
`(sound_form, refID, flags)`. Bridge `play_sound` now picks per build by
prologue (Steam 0x9CC980 `55 8b ec 6a` / FNV 0x5C4B30 `53 8b dc` / GOG
0xBCFBB0 `6a ff 68`) and calls with the right layout (FNV: 0,0,0,1.0).
OP_PLAY_SOUND now works on all three builds.

**fire_fix — COMPLETE ready-to-wire recipe (wiring gated on live overlap
check):** the site 0x8DA397 = `mov eax,[eax+0x224]; call eax` (8B) inside
the pending-fire-action loop 0x8DA370 (list @ +8, flags test 0x200820).
The +0x224 slot = 0x8BC750 (the weapon-fire executor: SEH, weapon resolve
0x622290, 0x61B270 calls) — a DIFFERENT layer from the wired fire_weapon
hook's fn 0x7DF210 (the per-frame fire executor, its 0x770880 call at
0x7DF3F7). The other direct 0x770880 callers: 0x79266D (the FireWeapon
command handler 0x792600) + 0x9CB367 (fire-cancel cleanup 0x9CB270).
Whether the 0x8DA370 path's shots ALSO pass the wired hook is only
observable live (indirect-call gap: 0x8BC750 → 0x61B270 does not reach
0x770880 directly) — wiring fire_fix risks double EVENT_FIRE. Recipe when
wiring:
  - cave: 48B int3 pad @ 0x9DCB50 (after `ret 8` @ 0x9DCB4E)
  - E9 rel32 @ 0x8DA397 → cave (guard `8b 80 24 02 00 00 ff d0`)
  - stub (24B): `push esi; call ashfall_hook_fire; add esp,4; mov eax,
    [esi]; mov eax,[eax+0x224]; call eax; jmp 0x8DA39F`
  - re-entry is clean: 0x8DA39F reloads edx/ecx (`mov edx,[esi]; mov
    ecx,esi`) and reads only eax (the dispatch result → test al,al).

**ai_fix4 — GOG shape documented, Steam twin unpinned:** the NOP'd 11B
@ 0x42FBDC = `push 0; push edi; push edx; mov ecx,ebx; call 0x559740`
inside the object-creation fn FUN_0042FB60 (TLS 0x298 save, [obj+0x28]
vtable +0x1E8 dispatch, create helpers 0x556050/0x559740, construction
0x4715F0/0x429410). 0x559740 = the actor **spawn-init** (AI/combat-target
setup, [actor+0x60] sub-object +0x464 call, player-singleton checks) —
vaultmp disabled it on creation. Steam twin search via the +0x1E8
dispatch fingerprint (61 sites) maxes at J=0.11 — the creation fn was
restructured past recognition (as documented). Pinning the Steam site
needs the vaultmp SEMANTIC (why the spawn-init call was disabled) first —
source archaeology, not a Ghidra question. Classic bridge keeps applying
the 11B NOP (recipe fidelity).

## Session 2026-08-18g — play_group flow mapped; anim-sync redundancy conclusion

**Steam PlayGroup handler 0x79EE20 decompiled:** command handler (EXTRACT
_ARGS 0x787530, SEH cookie) → [actor vtable +0x48] state-check → +0x1E4
anim-data → +0x100 → +0x1D0 → FUN_0058E170 (NiControllerManager resolve
via string 0x132720C) → play. The classic GOG implementation is a
DIFFERENT fn (FUN_0045F400, thiscall(actor, group, seq, flags), anim-slot
dedup at +0x118/+0x11C/+0x120/+0x124). vaultmp's play_group fix (the
15-byte block @ 0x49DCF1 + je→jmp @ 0x45F704 + reljump 0x49DD8E) is
AUTHORED relay logic routing PlayGroup through the bethesda_delegator —
the Steam flow differs structurally, so the fix doesn't transfer 1:1.

**Conclusion — play_group fix is redundant with Ashfall's state-based anim
sync:** the bridge already relays anim state via ActorStateDelta's idle
field (sampled per-frame, applied remotely), and the delegator
(bethesda_delegator hook + EVENT relay) exists for script-command
forwarding. The vaultmp play_group fix was event-based anim forwarding —
superseded by state sampling. Deferred: play_group wiring (needs re-
authored relay + live validation), same bucket as fire_fix. All remaining
RE items are now either resolved, documented as redundant/vaultmp-legacy,
or gated on the live session.
