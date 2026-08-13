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

Auto-detection: `vtable::fo3_lookup_addr()` reads 0x455190 (`51 8b 0d` = GOG)
vs 0x711EF0 (`55 8b ec 53` = Steam). Respawn patch is byte-guarded inside
`vaultmp::apply_steam_respawn()` (no-op unless the Steam bytes are present).

## Actor discovery (NPC sync) — GOG mapped, Steam TBD

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

**Steam re-derivation task:** find the Steam twin of fcn.006FAE90 (the AI
predicate). Method: the AI-pause recipe sites are already on the remaining-
patches list; on Steam, locate by the same semantic anchors (predicate reads
`[this+0xFC]` cmp 3/5 + PlayerCharacter compare — probe candidates live via
OP_PROBE_CODE before patching, remember the dump is FLAT/+0xC00). Once found:
same detour, same collector — no other changes needed. The Steam AI-pause
patch sites in the vaultmp table (ai_fix1..4) are the natural anchor: find
those first, the predicate is the function containing ai_fix2/ai_fix3.

## Open: vaultmp behavior-patch sites

The 34 `hooks::vaultmp` recipes target classic addresses. The Steam recompile
changed code layout AND function boundaries — byte-pattern and seed matches
produce decoys (e.g. a JNZ+0x83 + global-load false positive at 0xB59356).
Per-site semantic identification is required.

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
   patch. Status: respawn (2 sites) ✅ done, AI pause (4), fire relay (2),
   PlaceAtMe/activate (3), race match (2), lock fix (1), delegators (3).
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

## New Vegas (FNV 1.4.0.525) — static mapping session 2026-08-13

FNV needs its own addresses (different build/compiler than FO3), but there is
**no Steam/GOG split** — GOG 1.4.0.525(a) == Steam FNV, so one table covers
both. The `fnv_14` table (xNVSE RUNTIME block) was already statically
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
installed from `hooks::install()`). Remaining FNV: the AI-predicate address
for the discovery detour + live verification on the host.

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
- Implication for Ashfall: **downgrading the host's game to 1.7.0.3 makes
  the ENTIRE classic table apply** — vaultmp recipes, the AI-predicate
  discovery detour (0x6FAE90), GetBaseForm, the address library. The Steam
  per-site re-derivation becomes optional (only needed if the host refuses
  the downgrade). This is the recommended host deployment path.
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
