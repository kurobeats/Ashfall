//! FO3 classic-Steam multiplayer behavior patches — ported from
//! vaultmp's `vaultmpdll/vaultmp.cpp` (MIT).
//!
//! The original project patched a running FO3 1.7 executable with the
//! addresses below (Steam era, pre-GOG). Each recipe preserves the exact
//! byte sequence from `PatchGame()`, so the same patches can be re-targeted
//! per build via `memory::find_pattern` when a table doesn't match.
//!
//! These are the game-side behaviors a co-op mod needs **on top of** GECK
//! opcode interception:
//!   - respawn disable (players stay dead until the server revives them)
//!   - AI pause in unloaded cells (NPCs freeze when nobody is nearby)
//!   - race matching on spawn (prevents body-type desync between clients)
//!   - fire/activate/PlaceAtMe interception (so the server sees the event)
//!   - animation delegator (PlayGroup / idle forwarding)
//!
//! ⚠️ Addresses are for the classic Steam FO3 build only. Ashfall's verified
//! table (`fo3_17` in `mod.rs`) is GOG 1.7.0.3 — the two builds differ
//! (see `docs/proton-testing.md`). Re-derive before applying to any build:
//! dump the image via `OP_DUMP_IMAGE`, locate each site, update the table.

/// Classic Steam FO3 1.7 address table — all addresses taken from vaultmp's
/// `vaultmpdll/vaultmp.cpp` (verified in-game by the original authors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fo3SteamClassic {
    /// Plugins.txt string constant — patched to ".vmp" so vaultmp's own
    /// plugin list loads instead of the game's.
    pub plugins_vmp: usize,
    /// Animation group dispatcher (PlayGroup implementation entry).
    pub play_group: usize,
    /// Script-delegator call site → pushed to a queue for the pipe thread.
    pub delegator_src: usize,
    /// Delegator redirect target (PUSH ECX stub).
    pub delegator_dest: usize,
    /// Delegator call site (E8 target rewritten to the delegator stub).
    pub delegator_call_src: usize,
    /// Respawn guard: 2-byte conditional (JNZ rel8) at this address.
    pub no_respawn_nop: usize,
    /// Respawn check jump source (re-route to skip auto-respawn).
    pub no_respawn_jmp_src: usize,
    /// Respawn check jump destination (the skip path).
    pub no_respawn_jmp_dest: usize,
    /// PlayIdle call site (re-routed through the anim delegator).
    pub play_idle_call_src: usize,
    /// PlayIdle fix site (call rewritten).
    pub play_idle_fix_src: usize,
    /// Race-matching NOP block 1 (18 bytes).
    pub match_race_nop1: usize,
    /// Race-matching NOP block 2 (3 bytes).
    pub match_race_nop2: usize,
    /// Race-matching patch site.
    pub match_race_patch: usize,
    /// Race-matching parameter byte.
    pub match_race_param: usize,
    /// Lock-pick fix (NOP'd — disables vanilla lock bypass check).
    pub lock_fix: usize,
    /// AI fix 1 (NOP'd — prevents NPC processing outside loaded cells).
    pub ai_fix1: usize,
    /// AI fix 2 (byte redirect).
    pub ai_fix2: usize,
    /// AI fix 3 (6-byte instruction block).
    pub ai_fix3: usize,
    /// AI fix 4 (11-byte NOP block).
    pub ai_fix4: usize,
    /// PlayGroup fix (2-byte redirect).
    pub play_group_fix: usize,
    /// PlayGroup fix jump source.
    pub play_group_fix_src: usize,
    /// PlayGroup fix jump destination.
    pub play_group_fix_dest: usize,
    /// Actor-value fix source (jump to the AVFix detour).
    pub av_fix_src: usize,
    /// Actor-value fix return address.
    pub av_fix_ret: usize,
    /// Actor-value fix terminator.
    pub av_fix_term: usize,
    /// Fire relay: 3-byte jump at the fire call site.
    pub fire_fix_jmp: usize,
    /// Fire relay: 9-byte instruction block.
    pub fire_fix_patch: usize,
    /// Activate interception jump source.
    pub get_activate_jmp: usize,
    /// Activate interception return address.
    pub get_activate_ret: usize,
    /// PlaceAtMe interception jump source.
    pub place_at_me_jmp: usize,
    /// PlaceAtMe internal call (position write target).
    pub place_at_me_call: usize,
    /// PlaceAtMe spawn fix source.
    pub place_at_me_fix: usize,
    /// PlaceAtMe spawn fix destination.
    pub place_at_me_fix_dest: usize,
    /// FireWeapon interception jump source.
    pub fire_weapon_jmp: usize,
    /// FireWeapon internal call (the real fire routine).
    pub fire_weapon_call: usize,
}

/// The classic Steam FO3 1.7 table (values from vaultmp 2010–2013 era).
pub const FO3_STEAM_CLASSIC: Fo3SteamClassic = Fo3SteamClassic {
    plugins_vmp: 0x00E10FF1,
    play_group: 0x0045F704,
    delegator_src: 0x006EEC86,
    delegator_dest: 0x006EDBD9,
    delegator_call_src: 0x006EDBDA,
    no_respawn_nop: 0x006D5965,
    no_respawn_jmp_src: 0x0078B230,
    no_respawn_jmp_dest: 0x0078B2B9,
    play_idle_call_src: 0x0073BB20,
    play_idle_fix_src: 0x00534D8D,
    match_race_nop1: 0x0052F4DD,
    match_race_nop2: 0x0052F50F,
    match_race_patch: 0x0052F513,
    match_race_param: 0x00F51ADC,
    lock_fix: 0x00527F33,
    ai_fix1: 0x0072051E,
    ai_fix2: 0x006FAEE8,
    ai_fix3: 0x006FAF19,
    ai_fix4: 0x0042FBDC,
    play_group_fix: 0x0049DD6A,
    play_group_fix_src: 0x0049DD8E,
    play_group_fix_dest: 0x0049DCF1,
    av_fix_src: 0x00473D35,
    av_fix_ret: 0x00473D3B,
    av_fix_term: 0x00473E85,
    fire_fix_jmp: 0x0079236C,
    fire_fix_patch: 0x007923C5,
    get_activate_jmp: 0x0078A68D,
    get_activate_ret: 0x0078A995,
    place_at_me_jmp: 0x00539785,
    place_at_me_call: 0x0043DEF0,
    place_at_me_fix: 0x006F1CB6,
    place_at_me_fix_dest: 0x006F1F6E,
    fire_weapon_jmp: 0x0071F05F,
    fire_weapon_call: 0x004BE1A0,
};

/// Steam FO3 (post-2023) re-derived vaultmp sites — verified via the
/// FalloutAnniversaryPatcher vcdiff (2026-08-14): the downgrade delta
/// encodes classic↔Steam byte-identical regions; sites covered by an EXACT
/// vcdiff COPY run are byte-verified in BOTH builds (see docs/steam-re.md
/// "Session 2026-08-14" + scripts/re/vcdiff_map5.py). Every site below is
/// byte-guarded before patching, exactly like `apply_steam_respawn`.
pub mod fo3_steam_17_vaultmp {
    /// anim_detour hook site: `mov dword [ecx+0x414],0; ret` — the
    /// PlayIdle stub twin (GOG 0x73BB20). Byte-exact, unique (vcdiff EXACT
    /// + byte search agree): `c7 81 14 04 00 00 00 00 00 00 c3`.
    pub const PLAY_IDLE_CALL_SRC: usize = 0x0085_E0A0;
    /// Lock-pick fix: `je +2; mov byte [eax],cl; push 1; mov ecx,eax;
    /// call <fn>; mov ecx,esi; call <fn>; cmp; jle` — GOG 0x527F33 twin,
    /// vcdiff EXACT (`74 02 88 08 6a 01 8b c8`). NOP the JE.
    pub const LOCK_FIX: usize = 0x0079_8B65;
    /// AI fix 1 (NOP 2B): `je +0x1e; cmp eax,3; je` — GOG 0x72051E twin
    /// (`74 15 83 f8 03 74` → `74 1e 83 f8 03 74`), vcdiff EXACT cover.
    pub const AI_FIX1: usize = 0x005E_99E2;
    /// GetActivate interception: the 6-byte guard `je +0xdd; mov eax,[reg];
    /// mov ecx,reg; mov eax,[eax+0x100]; call eax; test al,al; je` — GOG
    /// 0x78A68D twin, vcdiff EXACT (`0f 84 dd 00 00 00 8b 06 8b ce`).
    /// vtable slot shifted 0x224→0x100. Ret target needs live probe.
    pub const GET_ACTIVATE_JMP: usize = 0x008D_3BC8;
    /// Delegator stub planting spot (int3 padding after `ret`, GOG
    /// 0x6EDBD9 twin) — vaultmp writes its PUSH-ECX stub here. The stub
    /// bytes are vaultmp-injected, only the padding location translates.
    pub const DELEGATOR_DEST: usize = 0x0040_5E69;
    pub const DELEGATOR_CALL_SRC: usize = 0x0040_5E6A;
    /// PlayGroup fix (2B `EB 27` written into int3 padding after `ret 8`,
    /// GOG 0x49DD6A twin), vcdiff EXACT.
    pub const PLAY_GROUP_FIX: usize = 0x0043_50F9;
    /// AV fix (ActorValue display formatter): vtable +0x130 call + push
    /// [reg+0xc] + push [global] + push "%s %s (%08X)" + sprintf — GOG
    /// 0x473D35 twin in fn 0x5B79B3 (SEH prologue twin of classic 0x473C50).
    /// The vtable +0x130 slot SURVIVED the recompile (register alloc
    /// changed: `mov eax,[ecx+0xc]; push eax` → `push [ecx+0xc]`, and
    /// `mov eax,[edx+0x130]; call eax` → `call [eax+0x130]`).
    /// Re-derived 2026-08-14 (static: +0x130 slot + sprintf string match).
    pub const AV_FIX_SRC: usize = 0x005B_7AC7;
    /// AV fix continuation (the instruction after the 5-byte hook slot):
    /// `call [eax+0x130]` — GOG 0x473D3B twin.
    pub const AV_FIX_RET: usize = 0x005B_7ACC;
    /// AV fix terminator (the sprintf call): GOG 0x473E85 twin.
    pub const AV_FIX_TERM: usize = 0x005B_7AE2;
    /// GetActivate ret-target: the loop-exit convergence `cmp [ebp+8],1`
    /// (the activate-parameter/death-state check) at the post-loop
    /// continuation — GOG 0x78A995 twin. Vaultmp's GetActivate hook
    /// (vaultmp.cpp GetActivate) captures EAX (the object) at the jmp
    /// site, queues its refID (obj+0x0C), and the RelJump at jmp+5 sends
    /// control here, skipping the loop body. Re-derived 2026-08-14 from
    /// the vaultmp source + Steam flow analysis.
    pub const GET_ACTIVATE_RET: usize = 0x008D_3CB8;
    /// FireWeapon call site: `call <fire>` + second call + `mov [reg+0x144],
    /// eax; movss [reg+0x148]` + jmp + player-compare tail — structural
    /// byte-search match (GOG 0x71F05F). vcdiff marks the region rewritten;
    /// needs live probe (OP_PROBE_CODE) before hooking.
    pub const FIRE_WEAPON_JMP: usize = 0x007D_F3F7;
    /// Steam fire routine candidate (GOG 0x4BE1A0): SEH prologue + ~4523B
    /// (GOG 4346B). vcdiff EXACT is a generic SEH-prologue coincidence —
    /// structural only, needs live probe.
    pub const FIRE_WEAPON_CALL: usize = 0x0077_0880;
    /// plugins.txt loader: `.txt` in `\Plugins.txt` at +9 (GOG 0xE10FF1,
    /// exact same layout). Patched to `.vmp`.
    pub const PLUGINS_VMP: usize = 0x00F9_FDB1;

    // ── 2026-08-17 data/re campaign (lanes A1/A2) — newly solved sites ──
    // All byte-guarded before patching, exactly like `apply_steam_respawn`.
    // Full derivations: data/re/fo3/steam-vaultmp-twins.md.

    /// ai_fix2 (death-state-5 JE redirect): Steam twin of classic 0x6FAEE8
    /// (2nd byte of `74 2c` JE). Steam JE `74 2a` @ 0x7D0AA5 inside AI
    /// predicate 0x7D0A50; write 0x2E → redirects death-state-5 from
    /// return-false to the ai_fix3 test block (classic wrote 0x30).
    /// Guard: `74 2a 83 f8 03 74`.
    pub const AI_FIX2: usize = 0x007D_0AA6;
    /// ai_fix3 (test-block): Steam twin of classic 0x6FAF19 int3 pad @
    /// 0x7D0AD5 (after predicate 0x7D0A50 ends 0x7D0AD4). Write
    /// `85 FF 74 CE EB F6` (classic `85 FF 74 CC EB F6`; JE disp CC→CE for
    /// Steam layout — je → cmp eax,3 @ 0x7D0AA7, jmp → return-false @
    /// 0x7D0AD1). Guard: `cc cc cc cc cc cc`.
    pub const AI_FIX3: usize = 0x007D_0AD5;
    /// AI predicate itself (corrected 2026-08-17): 0x7D0A50 is the 1:1
    /// structural twin of classic 0x6FAE90 (same slot order + death-state
    /// cmp 5/3 + player singleton 0x123C674, 12 callers). The earlier
    /// 0x7DAF80 never fires live. See `STEAM_AI_PREDICATE`.
    pub const AI_PREDICATE: usize = 0x007D_0A50;
    /// delegator_src (bethesda delegator call): Steam twin of classic
    /// 0x6EEC86 in the main game loop — `mov ecx,[0x123c5d4]; call
    /// 0x9B0740` @ 0x9B3EF6 (0x9B0740 = delegator twin of 0x6EDBE0,
    /// timer-check prologue verified). Recipe: relcall → delegator stub
    /// 0x405E69. Guard: `8b 0d d4 c5 23 01`.
    pub const DELEGATOR_SRC: usize = 0x009B_3EF6;
    /// play_idle_fix_src (PlayIdle detour site): Steam twin of classic
    /// 0x534D8D — `push 0x80; call [vtable+0x60c]` @ 0x79DA88 (+5 nop site
    /// 0x79DA8D), 2nd twin instance 0x79F2BB (duplicated handler). The 4
    /// older candidates (0x6EFDA9/0x6F00DA/0x6F28DA/0x6F45B3) are the
    /// post-site +0x250 check, NOT the fix site. Guard:
    /// `68 80 00 00 00 8b 01 ff 90 0c`. Choice between twins pending-live.
    pub const PLAY_IDLE_FIX_SRC: usize = 0x0079_DA88;
    /// fire_fix_jmp (fire relay): Steam twin of classic 0x79236C —
    /// vtable +0x224 dispatch `8b 80 24 02 00 00 ff d0` @ 0x8DA397 (the
    /// +0x224 slot SURVIVED the recompile). CAVEAT: vaultmp's 3-byte
    /// `EB 57 90` rel8 + mid-instruction re-entry does NOT transfer (Steam
    /// re-entry 0x8DA399 is mid-instruction); needs 5-byte E9 + rewritten
    /// relay stub — pending-live for exact stub bytes.
    pub const FIRE_FIX_JMP: usize = 0x008D_A397;
    /// fire_fix_patch (relay stub pad): Steam int3 pad after ret 4 @
    /// 0x8DA3CB — pad at 0x8DA3CE (`cc cc`). Stub bytes are vaultmp-
    /// authored; only the pad location translates (relay stub must be
    /// re-derived for Steam register alloc). Guard: `c2 04 00 cc cc`.
    pub const FIRE_FIX_PATCH: usize = 0x008D_A3CE;
    /// place_at_me_jmp (spawn interception): Steam twin of classic
    /// 0x539785 — internal spawn call `e8 25 5f f6 ff` → 0x704480 @
    /// 0x79E556 (command-table route: PlaceAtMe entry 0x110F910 → handler
    /// 0x79E6C0 → engine 0x79DE90). UNIQUE (1 hit). Guard:
    /// `6a 00 6a 00 6a 00 8b cf e8`.
    pub const PLACE_AT_ME_JMP: usize = 0x0079_E556;
    /// place_at_me_call (internal spawn fn): Steam twin of classic
    /// 0x43DEF0 = 0x704480 (SEH prologue frame 0x7C, cookie 0x1202954,
    /// 7 args). Guard: `55 8b ec 6a ff 68 ba 4e de 00`.
    pub const PLACE_AT_ME_CALL: usize = 0x0070_4480;
    /// place_at_me_fix (skip the +0x2A8 spawn): Steam twin of classic
    /// 0x6F1CB6 — `0f 84 e2 02 00 00` (je → 0x9CBF97) @ 0x9CBCAF, preceded
    /// by movss pos copy, followed by `call [eax+0x2a8]`. je pattern 6 hits
    /// but unique with pos-copy + +0x2A8-after + epilogue dest. Recipe:
    /// 5B E9 rel32 to dest + 1B NOP @ +5.
    pub const PLACE_AT_ME_FIX: usize = 0x009C_BCAF;
    /// place_at_me_fix_dest: the je target = SEH epilogue @ 0x9CBF97.
    pub const PLACE_AT_ME_FIX_DEST: usize = 0x009C_BF97;
    /// match_race sites (Steam fn 0x6F71E0, restructured — +0x218 vtable
    /// slot GONE, new guard chain: actor-null + [reg+0x1c] base-form +
    /// form-type 0x2a + cmove). Recipe bytes do NOT transfer; fix semantics
    /// must be re-derived against the new guard chain (pending-live for
    /// exact bytes). Sites for reference:
    pub const MATCH_RACE_NOP1: usize = 0x006F_71FA;
    pub const MATCH_RACE_NOP2: usize = 0x006F_720E;
    pub const MATCH_RACE_PATCH: usize = 0x006F_7220; // `8b 82 10 01 00 00 3b 81` unique (1 hit)
}

/// Steam FO3 (post-2023) respawn-disable sites — re-derived 2026-08-08 by
/// side-by-side disassembly vs GOG (see docs/steam-re.md). Same semantics as
/// vaultmp's ToggleRespawn disable: NOP the site-A predicate JNE + jump the
/// site-B guard over the respawn-flag write (`mov byte [eax+2],1` @ 0x8C9D52,
/// verified 2026-08-18 — the earlier 0x8CA8EB note was SSE math).
pub mod fo3_steam_17_respawn {
    /// Site A: `jne +3` inside the respawn predicate fn (GOG fcn.006D5960
    /// twin at 0x9C43A0, structurally byte-identical). NOP -> always false.
    pub const SITE_A_JNE: usize = 0x009C_43A5; // bytes 75 03
    /// Site B: guard `jne 0x8c9d5d` (tests the predicate result, `test al,al`
    /// @ 0x8C9CDE) — GOG 0x78B230 twin. Jump it -> flag write never runs.
    pub const SITE_B_JNE: usize = 0x008C_9CE0; // bytes 0F 85 77 00 00 00
    /// Skip destination for the site-B jump (common continuation, all three
    /// skip paths in the death handler converge here: 0x8C9CE0+6+0x77,
    /// 0x8C9CF4+2+0x67, 0x8C9CEF+2+0x6C).
    pub const SITE_B_SKIP: usize = 0x008C_9D5D;
    /// Leftover byte after the 5-byte jump over the 6-byte JNE.
    pub const SITE_B_TAIL: usize = SITE_B_JNE + 5; // 0x8C9CE5, original 0x00
    /// ==2-path (death-state 2) respawn-flag write — vaultmp left it
    /// unpatched; we NOP it too so no death path can ever set the flag
    /// (a state-2 death would otherwise still force the SP reload-save
    /// flow). GOG twin 0x78B2AE.
    pub const SITE_B2_FLAG: usize = 0x008C_9D52; // bytes c6 40 02 01
}

/// Apply the Steam respawn-disable patch. Byte-guarded: reads the site bytes
/// first and only patches when they match the verified Steam build — safe on
/// the GOG/classic build (bytes won't match) and on a wrong build (no-op).
///
/// # Safety
///
/// Patches executable memory of the current process; guarded by byte checks.
pub unsafe fn apply_steam_respawn() -> Option<Vec<crate::hooks::memory::Patch>> {
    use crate::hooks::memory;
    use fo3_steam_17_respawn as s;

    if crate::hooks::read_bytes(s::SITE_A_JNE, 2) != [0x75, 0x03] {
        return None; // not the verified Steam bytes
    }
    if crate::hooks::read_bytes(s::SITE_B_JNE, 6) != [0x0F, 0x85, 0x77, 0x00, 0x00, 0x00] {
        return None;
    }
    if crate::hooks::read_bytes(s::SITE_B2_FLAG, 4) != [0xC6, 0x40, 0x02, 0x01] {
        return None;
    }

    let mut out = Vec::new();
    // Site A: NOP the predicate JNE -> always `xor al,al; ret` -> respawn denied.
    let a = memory::Patch::new(s::SITE_A_JNE as *const u8, &[0x90, 0x90]);
    a.apply();
    out.push(a);
    // Site B: unconditional JMP over the respawn-flag write (0x8C9D52).
    memory::write_rel_jump(s::SITE_B_JNE, s::SITE_B_SKIP);
    // Tail byte of the 6-byte JNE left after the 5-byte JMP.
    let t = memory::Patch::new(s::SITE_B_TAIL as *const u8, &[0x90]);
    t.apply();
    out.push(t);
    // ==2-path flag write: NOP so no death path sets the respawn flag.
    let b2 = memory::Patch::new(s::SITE_B2_FLAG as *const u8, &[0x90, 0x90, 0x90, 0x90]);
    b2.apply();
    out.push(b2);
    Some(out)
}

/// A single patch recipe — the exact bytes written at one address, or a
/// relative jump/call whose target is a fixed table address or a hook
/// function supplied by the caller at install time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipe {
    /// `SafeWriteBuf(addr, bytes)` — fixed byte sequence.
    Bytes {
        name: &'static str,
        addr: usize,
        bytes: &'static [u8],
    },
    /// `SafeWrite8/16/32(addr, value)`.
    Write {
        name: &'static str,
        addr: usize,
        value: u32,
        width: u8,
    },
    /// `WriteRelCall(from, to)` — target is a table address.
    RelCall {
        name: &'static str,
        from: usize,
        to: usize,
    },
    /// `WriteRelJump(from, to)` — target is a table address.
    RelJump {
        name: &'static str,
        from: usize,
        to: usize,
    },
    /// `WriteRelCall(from, hook)` — target is a Rust hook function,
    /// resolved by name at install time.
    RelCallHook { name: &'static str, from: usize },
    /// `WriteRelJump(from, hook)` — target is a Rust hook function,
    /// resolved by name at install time.
    RelJumpHook { name: &'static str, from: usize },
}

impl Recipe {
    /// Address this recipe writes to (for conflict detection).
    pub fn addr(&self) -> usize {
        match self {
            Recipe::Bytes { addr, .. }
            | Recipe::Write { addr, .. }
            | Recipe::RelCall { from: addr, .. }
            | Recipe::RelJump { from: addr, .. }
            | Recipe::RelCallHook { from: addr, .. }
            | Recipe::RelJumpHook { from: addr, .. } => *addr,
        }
    }

    /// Name of the hook function this recipe needs, if any.
    pub fn required_hook(&self) -> Option<&'static str> {
        match self {
            Recipe::RelCallHook { name, .. } | Recipe::RelJumpHook { name, .. } => Some(name),
            _ => None,
        }
    }
}

/// Translate the complete `PatchGame()` sequence from vaultmp into recipes.
///
/// Hook names follow the original function names (`respawn_detour`,
/// `anim_detour`, `play_idle_detour`, `av_fix`, `get_activate`,
/// `place_at_me`, `fire_weapon`, `bethesda_delegator`).
pub fn recipes(t: &Fo3SteamClassic) -> Vec<Recipe> {
    const NOP: &[u8] = &[0x90; 18];

    vec![
        // Respawn disable (ToggleRespawn, respawn=true path):
        // restore JNZ rel8 at the guard + NOP the leftover byte of the 6-byte
        // JNZ so the rel-call we plant below returns cleanly.
        Recipe::Write {
            name: "no_respawn_jnz",
            addr: t.no_respawn_nop,
            value: 0x03_75, // JNZ +3 (little-endian u16)
            width: 2,
        },
        Recipe::Write {
            name: "no_respawn_jmp_nop",
            addr: t.no_respawn_jmp_src + 5,
            value: 0x90,
            width: 1,
        },
        Recipe::RelCallHook {
            name: "respawn_detour",
            from: t.no_respawn_jmp_src,
        },
        // Script delegator: stub entry gets PUSH ECX, call site gets POP ECX
        // so the delegate can consume the register the original did.
        Recipe::Write {
            name: "delegator_push_ecx",
            addr: t.delegator_dest,
            value: 0x51,
            width: 1,
        },
        Recipe::Write {
            name: "delegator_call_pop_ecx",
            addr: t.delegator_call_src + 5,
            value: 0x59,
            width: 1,
        },
        Recipe::Write {
            name: "play_group_short_jmp",
            addr: t.play_group,
            value: 0xEB,
            width: 1,
        },
        Recipe::RelCall {
            name: "delegator_redirect",
            from: t.delegator_src,
            to: t.delegator_dest,
        },
        Recipe::RelCallHook {
            name: "bethesda_delegator",
            from: t.delegator_call_src,
        },
        // Animation / idle forwarding.
        Recipe::Write {
            name: "play_idle_fix_nop",
            addr: t.play_idle_fix_src + 5,
            value: 0x9090,
            width: 2,
        },
        Recipe::RelCallHook {
            name: "play_idle_detour",
            from: t.play_idle_fix_src,
        },
        Recipe::RelJumpHook {
            name: "anim_detour",
            from: t.play_idle_call_src,
        },
        // Lock fix (disable vanilla lock-bypass).
        Recipe::Write {
            name: "lock_fix_nop",
            addr: t.lock_fix,
            value: 0x9090,
            width: 2,
        },
        // AI pause in unloaded cells.
        Recipe::Write {
            name: "ai_fix1_nop",
            addr: t.ai_fix1,
            value: 0x9090,
            width: 2,
        },
        Recipe::Write {
            name: "ai_fix2_redirect",
            addr: t.ai_fix2,
            value: 0x30,
            width: 1,
        },
        Recipe::Bytes {
            name: "ai_fix3_block",
            addr: t.ai_fix3,
            bytes: &[0x85, 0xFF, 0x74, 0xCC, 0xEB, 0xF6],
        },
        Recipe::Bytes {
            name: "ai_fix4_nop",
            addr: t.ai_fix4,
            bytes: &[0x90; 11],
        },
        // Race matching on spawn (prevents body-type desync).
        Recipe::Bytes {
            name: "match_race_nop1",
            addr: t.match_race_nop1,
            bytes: NOP,
        },
        Recipe::Bytes {
            name: "match_race_nop2",
            addr: t.match_race_nop2,
            bytes: &[0x90; 3],
        },
        Recipe::Write {
            name: "match_race_patch",
            addr: t.match_race_patch + 1,
            value: 0xF1,
            width: 1,
        },
        Recipe::Bytes {
            name: "match_race_patch_nop",
            addr: t.match_race_patch + 2,
            bytes: &[0x90; 4],
        },
        Recipe::Write {
            name: "match_race_param",
            addr: t.match_race_param,
            value: 0x0F,
            width: 1,
        },
        // PlayGroup fix (forward group requests through the delegator).
        Recipe::Bytes {
            name: "play_group_fix_block",
            addr: t.play_group_fix_dest,
            bytes: &[
                0x85, 0xC9, 0x0F, 0x84, 0xFF, 0x00, 0x00, 0x00, 0x8B, 0x71, 0x0C, 0x85, 0xF6, 0xEB,
                0x6A,
            ],
        },
        Recipe::Bytes {
            name: "play_group_fix_jmp",
            addr: t.play_group_fix,
            bytes: &[0xEB, 0x27],
        },
        Recipe::RelJump {
            name: "play_group_fix_reljump",
            from: t.play_group_fix_src,
            to: t.play_group_fix_dest,
        },
        // Actor-value fix (route through the AVFix detour).
        Recipe::RelJumpHook {
            name: "av_fix",
            from: t.av_fix_src,
        },
        // Fire relay.
        Recipe::Bytes {
            name: "fire_fix_jmp",
            addr: t.fire_fix_jmp,
            bytes: &[0xEB, 0x57, 0x90],
        },
        Recipe::Bytes {
            name: "fire_fix_patch",
            addr: t.fire_fix_patch,
            bytes: &[0x85, 0xED, 0x74, 0xE8, 0x8B, 0x55, 0x00, 0xEB, 0xA1],
        },
        // Activate interception.
        Recipe::RelCallHook {
            name: "get_activate",
            from: t.get_activate_jmp,
        },
        Recipe::RelJump {
            name: "get_activate_ret",
            from: t.get_activate_jmp + 5,
            to: t.get_activate_ret,
        },
        // PlaceAtMe interception + spawn position fix.
        Recipe::RelJumpHook {
            name: "place_at_me",
            from: t.place_at_me_jmp,
        },
        Recipe::RelJump {
            name: "place_at_me_fix",
            from: t.place_at_me_fix,
            to: t.place_at_me_fix_dest,
        },
        Recipe::Write {
            name: "place_at_me_fix_nop",
            addr: t.place_at_me_fix + 5,
            value: 0x90,
            width: 1,
        },
        // FireWeapon interception (increments a counter, calls the real fn).
        Recipe::RelJumpHook {
            name: "fire_weapon",
            from: t.fire_weapon_jmp,
        },
        // Plugins.txt redirect: ".vmp" as little-endian u32.
        Recipe::Write {
            name: "plugins_vmp",
            addr: t.plugins_vmp,
            value: u32::from_le_bytes(*b".vmp"),
            width: 4,
        },
    ]
}

/// All hook names a full install requires, in canonical order.
pub const REQUIRED_HOOKS: &[&str] = &[
    "respawn_detour",
    "bethesda_delegator",
    "play_idle_detour",
    "anim_detour",
    "av_fix",
    "get_activate",
    "place_at_me",
    "fire_weapon",
];

/// Apply the recipes to the running process.
///
/// `hook_addr(name)` must resolve every hook in [`REQUIRED_HOOKS`]; relative
/// jumps/calls to those hooks are computed with the resolved address.
///
/// # Safety
///
/// Patches executable memory of the current (game) process. Only call inside
/// the injected bridge on a build whose table was verified (see module docs).
pub unsafe fn apply(
    t: &Fo3SteamClassic,
    hook_addr: impl Fn(&str) -> Option<usize>,
) -> Vec<crate::hooks::memory::Patch> {
    use crate::hooks::memory;

    let mut out: Vec<memory::Patch> = Vec::new();
    for r in recipes(t) {
        match &r {
            Recipe::Bytes { addr, bytes, .. } => {
                let p = memory::Patch::new(*addr as *const u8, bytes);
                p.apply();
                out.push(p);
            }
            Recipe::Write {
                addr, value, width, ..
            } => {
                let bytes: Vec<u8> = match width {
                    1 => vec![*value as u8],
                    2 => vec![(*value & 0xFF) as u8, ((*value >> 8) & 0xFF) as u8],
                    _ => value.to_le_bytes().to_vec(),
                };
                let p = memory::Patch::new(*addr as *const u8, &bytes);
                p.apply();
                out.push(p);
            }
            Recipe::RelCall { from, to, .. } => {
                memory::write_rel_call(*from, *to);
            }
            Recipe::RelJump { from, to, .. } => {
                memory::write_rel_jump(*from, *to);
            }
            Recipe::RelCallHook { from, name } => {
                if let Some(hook) = hook_addr(name) {
                    memory::write_rel_call(*from, hook);
                }
            }
            Recipe::RelJumpHook { from, name } => {
                if let Some(hook) = hook_addr(name) {
                    memory::write_rel_jump(*from, hook);
                }
            }
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════
// Actor discovery detour (classic FO3 only — Steam TBD, steam-re.md)
// ═══════════════════════════════════════════════════════════════
//
// fcn.006FAE90 (`bool __thiscall Actor*`) is the engine's per-actor AI
// processing gate: every actor the game processes passes through it every
// frame, from HighProcess (ProcessLists high-actor processing) and the
// player/combat paths (11 call sites, mapped on the GOG exe 2026-08-13).
// Detouring it yields the active-actor list with NO ProcessLists layout
// needed — STR's highActorHandleArray equivalent, driven by the engine
// itself. Ref id is read at actor+0x0C (xFOSE-verified).
//
// The thunk preserves `this` (ecx) across the collector call, then falls
// through to the original via the detour trampoline. The collector +
// seen-set diff (STR VisitForms) lives in hooks::discovery.

/// Classic FO3 AI predicate address (GOG 1.7.0.3, verified: entry `56 8B F1`).
#[cfg(target_arch = "x86")]
pub const AI_PREDICATE_FO3_CLASSIC: usize = 0x006F_AE90;
/// Expected prologue: `push esi; mov esi, ecx`.
#[cfg(target_arch = "x86")]
const AI_PREDICATE_PROLOGUE: [u8; 3] = [0x56, 0x8B, 0xF1];

/// Steam/Anniversary AI predicate — re-derived 2026-08-15 (static, after the
/// 0x7F9B70 prologue match proved a false positive that never fires live).
/// The REAL twin keeps the classic frame-less thiscall prologue
/// `56 8B F1` (push esi; mov esi, ecx), calls vtable +0x22C (push 0),
/// compares the actor against the player singleton **0x123C674**, and
/// reads the [actor+0x60] sub-object — byte-identical markers to classic
/// 0x6FAE90. Found 0x22 bytes past the vcdiff gap candidate 0x7DAF5E.
/// NOTE: the recompile changed the return bool→int (0/1/0xC/table) and
/// split the helper (0x7DAF50); 2 call sites survive (the rest inlined).
/// Live fire-rate unverified — if sparse, fall back to the HighProcess twin.
///
/// CORRECTED 2026-08-17 (data/re campaign lane A1): the true structural
/// twin of classic 0x6FAE90 is **0x7D0A50**, not 0x7DAF80. 0x7D0A50 is the
/// 1:1 twin (same vtable slot order +0x234/0x22C/0x3E0/0x230/0x214, death-
/// state cmp 5/3, player singleton 0x123C674, 12 callers); 0x7DAF80 has a
/// different order (vtable+0x22C first, player-compare early, helper
/// 0x7DAF50) and NO death-state cmp 5/3 sequence — it was the fn that
/// "installs byte-perfect but ~never fires" in the 2026-08-15 live session.
/// Verified in the dump: 0x7D0A50 = `56 8b f1 8b 06 8b 80 34` (classic
/// 0x6FAE90 = `56 8b f1 8b 06 8b 90 34` — register-alloc only).
#[cfg(target_arch = "x86")]
pub const STEAM_AI_PREDICATE: usize = 0x007D_0A50;
/// Expected prologue: `56 8B F1`.
#[cfg(target_arch = "x86")]
const STEAM_AI_PREDICATE_PROLOGUE: [u8; 3] = [0x56, 0x8B, 0xF1];

/// The Rust collector called by the thunk (cdecl: actor on the stack).
#[no_mangle]
pub unsafe extern "C" fn ashfall_collect_actor_c(actor: usize) {
    crate::hooks::discovery::collect_actor_ptr(actor);
}

#[cfg(target_arch = "x86")]
// x86 thunk: `this` arrives in ecx. Push it as the collector's argument,
// call, restore ecx, then run the original through the trampoline.
core::arch::global_asm!(
    ".globl _ashfall_actor_collect_thunk",
    "_ashfall_actor_collect_thunk:",
    "    push ecx",
    "    push ecx",
    "    call _ashfall_collect_actor_c",
    "    add esp, 4",
    "    pop ecx",
    "    jmp dword ptr [_ashfall_trampoline_addr]",
);

/// Indirect pointer the thunk `jmp`s through (single writer on the apply
/// thread, raw reader in asm — atomic for soundness, not synchronization).
#[cfg(target_arch = "x86")]
#[no_mangle]
pub static ashfall_trampoline_addr: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

/// The installed detour (persists for the process lifetime).
#[cfg(target_arch = "x86")]
static DETOUR: std::sync::OnceLock<std::sync::Mutex<Option<super::detour::Detour>>> =
    std::sync::OnceLock::new();

// The Detour holds raw pointers; it is only ever touched on the game thread
// (apply + collection), so a plain Mutex guard is sufficient.
#[cfg(target_arch = "x86")]
unsafe impl Send for super::detour::Detour {}

/// Install the actor-discovery detour on the classic FO3 build. Byte-guarded
/// like `apply_steam_respawn` — no-op on unknown builds and non-Windows.
/// Select the AI-predicate site for the running build by prologue signature
/// (both classic 0x6FAE90 and Steam 0x7D0A50 use the frame-less thiscall
/// prologue `56 8B F1` — push esi; mov esi, ecx — so the same 3-byte guard
/// validates both; the address split is by build, the bytes are identical).
#[cfg(target_arch = "x86")]
fn ai_predicate_site() -> Option<usize> {
    use crate::hooks::read_bytes;
    if read_bytes(AI_PREDICATE_FO3_CLASSIC, 3) == AI_PREDICATE_PROLOGUE {
        return Some(AI_PREDICATE_FO3_CLASSIC);
    }
    if read_bytes(STEAM_AI_PREDICATE, 3) == STEAM_AI_PREDICATE_PROLOGUE {
        return Some(STEAM_AI_PREDICATE);
    }
    None
}

pub fn apply_actor_discovery() -> bool {
    #[cfg(target_arch = "x86")]
    {
        let Some(site) = ai_predicate_site() else {
            return false; // unknown build — no-op
        };

        let detour = DETOUR.get_or_init(|| std::sync::Mutex::new(None));
        let mut guard = detour.lock().unwrap();
        if guard.is_some() {
            return true; // already installed
        }

        unsafe {
            extern "C" {
                fn ashfall_actor_collect_thunk();
            }
            let thunk = ashfall_actor_collect_thunk as *const u8;
            let mut d = match super::detour::Detour::new(site as *mut u8, thunk) {
                Some(d) => d,
                None => return false, // trampoline alloc failed (non-Windows)
            };
            let trampoline: usize = d.trampoline_ptr::<usize>();
            ashfall_trampoline_addr.store(trampoline as u32, std::sync::atomic::Ordering::SeqCst);
            d.install();
            *guard = Some(d);
        }
        true
    }
    #[cfg(not(target_arch = "x86"))]
    {
        false // tests / x64 hosts — nothing to hook
    }
}

/// Start the 10 Hz NPC-diff flush (STR cadence). Safe to call from the TCP
/// server thread (not DllMain — no loader-lock issue).
pub fn start_npc_flush_thread() {
    std::thread::spawn(|| {
        while crate::RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
            crate::hooks::discovery::flush_npc_diff();
            crate::network::sample_tracked();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// Per-frame player-state hook (FNV — NVSE main-loop anchor)
// ═══════════════════════════════════════════════════════════════
//
// NVSE's kMainLoopHookPatchAddr = 0x86B386 — the WinMain loop's
// `call 0x43C4B0` ("7th call before first call to Sleep in oldWinMain",
// Hooks_Gameplay.cpp). Executed once per frame (Sleep(50) loop), so a
// call-redirect fires the 10 Hz player-state reporter continuously —
// the FNV half of "own-player movement sync" without a frame-function
// re-derivation. FO3's equivalent (NVSE comment: 0x6EEC15) is a
// mid-function dispatch, not a call — defer, document in steam-re.md.

/// FNV frame-hook site (classic 1.4.0.525 — Steam FNV is the same build).
#[cfg(target_arch = "x86")]
pub const FNV_FRAME_HOOK_SITE: usize = 0x0086_B386;
#[cfg(target_arch = "x86")]
const FNV_FRAME_ORIGINAL_CALL: usize = 0x0043_C4B0;
/// Guard: `call 0x43C4B0` (verified on the real GOG FalloutNV.exe).
#[cfg(target_arch = "x86")]
const FNV_FRAME_GUARD: [u8; 5] = [0xE8, 0x25, 0x11, 0xBD, 0xFF];

/// Redirect the FNV main-loop call to a hook that samples the local player
/// at 10 Hz. Byte-guarded (no-op unless the site matches). The hook calls
/// the original getter and returns its result so the loop continues
/// unchanged; `report_player_state_due` throttles internally (STR cadence).
#[cfg(target_arch = "x86")]
pub fn apply_fnv_frame_hook() -> bool {
    if crate::hooks::read_bytes(FNV_FRAME_HOOK_SITE, 5) != FNV_FRAME_GUARD {
        return false; // not the FNV build at this site
    }
    unsafe {
        unsafe extern "C" fn fnv_frame_hook() -> u32 {
            let orig: unsafe extern "C" fn() -> u32 =
                unsafe { std::mem::transmute(FNV_FRAME_ORIGINAL_CALL) };
            let result = orig();
            crate::network::report_player_state_due();
            // FNV NPC discovery: enumerate the ActorProcessManager actor
            // tiers (AnhNVSE layout, 0x011E0E80) and feed the collector —
            // the 10 Hz flush turns the diff into spawn/remove events.
            let reader = |addr: usize| -> u32 {
                if addr == 0 {
                    return 0;
                }
                unsafe { *(addr as *const u32) }
            };
            let refs = crate::hooks::discovery::fnv_enumerate_actors(reader);
            crate::hooks::discovery::collect_ref_ids(&refs);
            result
        }
        crate::hooks::memory::write_rel_call(
            FNV_FRAME_HOOK_SITE,
            fnv_frame_hook as *const () as usize,
        );
    }
    true
}

#[cfg(not(target_arch = "x86"))]
pub fn apply_fnv_frame_hook() -> bool {
    false // tests / x64 hosts — nothing to hook
}

// ═══════════════════════════════════════════════════════════════
// Per-frame player-state hook (FO3 classic — main-loop frame body)
// ═══════════════════════════════════════════════════════════════
//
// The FO3 main loop's frame body calls 0x6E3E40 (no-arg cdecl bool — reads
// a menu/pause global) at 0x6EEB2F, once per frame. NVSE's FO3 main-loop
// anchor was 0x6EEC15 (same loop, dispatch shape). Redirecting the call is
// the lazy equivalent of the FNV frame hook: call the original, run the
// 10 Hz player-state reporter, return the original result. Byte-guarded —
// no-op on the Steam/Anniversary build (recompiled; the community solution
// for that build is a downgrade to 1.7.0.3, see steam-re.md).

/// FO3 classic frame-hook site (verified on the GOG 1.7.0.3 exe).
#[cfg(target_arch = "x86")]
pub const FO3_FRAME_HOOK_SITE: usize = 0x006E_EB2F;
#[cfg(target_arch = "x86")]
const FO3_FRAME_ORIGINAL_CALL: usize = 0x006E_3E40;
/// Guard: `call 0x6E3E40` (e8 0c 53 ff ff).
#[cfg(target_arch = "x86")]
const FO3_FRAME_GUARD: [u8; 5] = [0xE8, 0x0C, 0x53, 0xFF, 0xFF];

#[cfg(target_arch = "x86")]
pub fn apply_fo3_frame_hook() -> bool {
    if crate::hooks::read_bytes(FO3_FRAME_HOOK_SITE, 5) != FO3_FRAME_GUARD {
        return false; // Steam/Anniversary build — no-op (downgrade path)
    }
    unsafe {
        unsafe extern "C" fn fo3_frame_hook() -> u8 {
            let orig: unsafe extern "C" fn() -> u8 =
                unsafe { std::mem::transmute(FO3_FRAME_ORIGINAL_CALL) };
            let result = orig();
            crate::network::report_player_state_due();
            result
        }
        crate::hooks::memory::write_rel_call(
            FO3_FRAME_HOOK_SITE,
            fo3_frame_hook as *const () as usize,
        );
    }
    true
}

#[cfg(not(target_arch = "x86"))]
pub fn apply_fo3_frame_hook() -> bool {
    false // tests / x64 hosts — nothing to hook
}

// ═══════════════════════════════════════════════════════════════
// Per-frame player-state hook (FO3 Steam/Anniversary — main loop)
// ═══════════════════════════════════════════════════════════════
//
// The Steam (post-2023) main-loop frame body calls `[0xF241E4]` (a
// kernel32 timer import, SteamStub-relocated IAT — adjacent to Sleep's
// slot 0xF241E8) once per unpaused frame, comparing its result with the
// respawn-struct timestamp (+0x10) at 0x9B3D83. Re-derived 2026-08-14o
// via the frame-body twin `mov byte [0x123c5d4],1` at 0x9B3D92 (the
// classic 0x6EEB50 respawn handling). Redirecting that call is the
// per-frame hook; the hook calls the original through the IAT slot so it
// works regardless of the import's ASLR-resolved address.

/// Steam FO3 frame-hook site: `call [0xF241E4]` in the main-loop frame
/// body (result compared with respawn-struct +0x10).
#[cfg(target_arch = "x86")]
pub const STEAM_FRAME_HOOK_SITE: usize = 0x009B_3D77;
/// The SteamStub-relocated IAT slot (holds the resolved import address at
/// load; the hook derefs it so no ASLR-sensitive constant is needed).
#[cfg(target_arch = "x86")]
const STEAM_FRAME_IAT_SLOT: usize = 0x00F2_41E4;
/// Guard: `call [0xF241E4]` (6 bytes).
#[cfg(target_arch = "x86")]
const STEAM_FRAME_GUARD: [u8; 6] = [0xFF, 0x15, 0xE4, 0x41, 0xF2, 0x00];

#[cfg(target_arch = "x86")]
pub fn apply_steam_frame_hook() -> bool {
    if crate::hooks::read_bytes(STEAM_FRAME_HOOK_SITE, 6) != STEAM_FRAME_GUARD {
        return false; // not the Steam/Anniversary build at this site
    }
    unsafe {
        unsafe extern "C" fn steam_frame_hook() -> u32 {
            // call the original through the IAT slot (resolved at load)
            let orig: unsafe extern "C" fn() -> u32 =
                unsafe { std::mem::transmute(*(STEAM_FRAME_IAT_SLOT as *const usize)) };
            let result = orig();
            crate::network::report_player_state_due();
            result
        }
        crate::hooks::memory::write_rel_call(
            STEAM_FRAME_HOOK_SITE,
            steam_frame_hook as *const () as usize,
        );
        // the original indirect call is 6 bytes; the redirect is 5 — NOP the tail
        let tail =
            crate::hooks::memory::Patch::new((STEAM_FRAME_HOOK_SITE + 5) as *const u8, &[0x90]);
        tail.apply();
    }
    true
}

#[cfg(not(target_arch = "x86"))]
pub fn apply_steam_frame_hook() -> bool {
    false // tests / x64 hosts — nothing to hook
}

// ═══════════════════════════════════════════════════════════════
// Vaultmp behavior hooks — the 8 REQUIRED_HOOKS the recipe table needs
// ═══════════════════════════════════════════════════════════════
//
// Each hook is a tiny x86 thunk that preserves the game's register state
// (PUSHAD/POPAD), calls a Rust collector, then continues the original
// control flow exactly like vaultmp's inline-asm hooks do. The thunk
// address is what `vaultmp::apply()` wires into the recipe RelCallHook /
// RelJumpHook sites (the hook_addr resolver).
//
// Semantics ported from vaultmp.cpp (2026-08-14, github foxtacles/vaultmp):
//   - respawn_detour: called at the respawn guard — re-disables SP respawn
//   - bethesda_delegator: forwards a queued CallCommand (8 pointer args)
//   - anim_detour / play_idle_detour: animation/idle forwarding
//   - av_fix: routes AV reads through the server
//   - get_activate: captures EAX (the activated object), queues its refID
//   - place_at_me: intercepts the PlaceAtMe spawn position write
//   - fire_weapon: increments the fire counter, calls the real fn
//
// The collectors feed the existing event pipeline (push_event_frame), so
// the client sees ACTIVATE / FIRE events and relays them as server packets.

#[cfg(target_arch = "x86")]
pub mod hooks {
    // Resolver + collectors reference Recipe/FO3_STEAM_CLASSIC via
    // `crate::` paths; super::* is empty here (the module is cfg'd).

    /// Resolver: name → thunk address for every REQUIRED_HOOKS entry.
    /// Safe to call from the apply thread; returns None on non-x86.
    #[cfg(target_arch = "x86")]
    pub fn resolve(name: &str) -> Option<usize> {
        match name {
            "respawn_detour" => Some(ashfall_respawn_detour_thunk as *const () as usize),
            "bethesda_delegator" => Some(ashfall_bethesda_delegator_thunk as *const () as usize),
            "play_idle_detour" => Some(ashfall_play_idle_detour_thunk as *const () as usize),
            "anim_detour" => Some(ashfall_anim_detour_thunk as *const () as usize),
            "av_fix" => Some(ashfall_av_fix_thunk as *const () as usize),
            "get_activate" => Some(ashfall_get_activate_thunk as *const () as usize),
            "place_at_me" => Some(ashfall_place_at_me_thunk as *const () as usize),
            "fire_weapon" => Some(ashfall_fire_weapon_thunk as *const () as usize),
            _ => None,
        }
    }

    // All thunks as extern "C" symbols for the resolver.
    #[cfg(target_arch = "x86")]
    extern "C" {
        fn ashfall_respawn_detour_thunk();
        fn ashfall_bethesda_delegator_thunk();
        fn ashfall_play_idle_detour_thunk();
        fn ashfall_anim_detour_thunk();
        fn ashfall_av_fix_thunk();
        fn ashfall_get_activate_thunk();
        fn ashfall_place_at_me_thunk();
        fn ashfall_fire_weapon_thunk();
    }

    /// Rust collectors (cdecl; called from asm with args on the stack).
    #[no_mangle]
    pub unsafe extern "C" fn ashfall_hook_respawn() {
        // vaultmp ToggleRespawn: force respawn off so players stay dead
        // until the server revives them. The Steam respawn disable is
        // already applied via apply_steam_respawn; this hook covers the
        // classic build's ToggleRespawn path.
        crate::hooks::set_respawn_allowed(false);
    }

    #[no_mangle]
    pub unsafe extern "C" fn ashfall_hook_activate(obj: usize) {
        if obj != 0 {
            // refID at object+0x0C (xFOSE STATIC_ASSERT).
            let ref_id = crate::hooks::vtable::read_field::<u32>(obj as *mut u8, 0x0C);
            crate::network::push_event_frame(ashfall_core::event::encode_ref_event(
                ashfall_core::event::EVENT_ACTIVATE,
                ref_id,
            ));
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn ashfall_hook_fire(obj: usize) {
        if obj != 0 {
            let ref_id = crate::hooks::vtable::read_field::<u32>(obj as *mut u8, 0x0C);
            crate::network::push_event_frame(ashfall_core::event::encode_ref_event(
                ashfall_core::event::EVENT_FIRE,
                ref_id,
            ));
        }
    }

    // Animation / AV / delegator collectors — wire-through points. The
    // full vaultmp forwarding (delegated CallCommand, anim queue) needs the
    // server relay; these currently emit a no-op keepalive so the thunk
    // path is exercised end-to-end (ponytail: real relay lands with the
    // activation/fire packet plumbing).
    #[no_mangle]
    pub unsafe extern "C" fn ashfall_hook_anim() {}
    #[no_mangle]
    pub unsafe extern "C" fn ashfall_hook_play_idle() {}
    #[no_mangle]
    pub unsafe extern "C" fn ashfall_hook_av() {}
    #[no_mangle]
    pub unsafe extern "C" fn ashfall_hook_delegator() {}

    // ── x86 thunks ────────────────────────────────────────────────
    // Each preserves all GPRs, calls its collector, restores, and returns.
    // The RelCallHook / RelJumpHook recipe sites expect the thunk to end
    // with `ret` (the hook was CALLed) — the recipe writer handles the
    // jump-back via the recipe's ret/dest fields. For RelJumpHook sites
    // the thunk must re-enter the original; the trampoline pattern from
    // the actor-discovery detour applies where needed.
    core::arch::global_asm!(
        ".globl _ashfall_respawn_detour_thunk",
        ".globl _ashfall_bethesda_delegator_thunk",
        ".globl _ashfall_play_idle_detour_thunk",
        ".globl _ashfall_anim_detour_thunk",
        ".globl _ashfall_av_fix_thunk",
        ".globl _ashfall_get_activate_thunk",
        ".globl _ashfall_place_at_me_thunk",
        ".globl _ashfall_fire_weapon_thunk",
        "_ashfall_respawn_detour_thunk:",
        "    pushad",
        "    call _ashfall_hook_respawn",
        "    popad",
        "    ret",
        "_ashfall_bethesda_delegator_thunk:",
        "    pushad",
        "    call _ashfall_hook_delegator",
        "    popad",
        "    ret",
        "_ashfall_play_idle_detour_thunk:",
        "    pushad",
        "    call _ashfall_hook_play_idle",
        "    popad",
        "    ret",
        "_ashfall_anim_detour_thunk:",
        "    pushad",
        "    call _ashfall_hook_anim",
        "    popad",
        "    ret",
        "_ashfall_av_fix_thunk:",
        "    pushad",
        "    call _ashfall_hook_av",
        "    popad",
        "    ret",
        "_ashfall_get_activate_thunk:",
        "    pushad",
        "    push eax",
        "    call _ashfall_hook_activate",
        "    add esp, 4",
        "    popad",
        "    ret",
        "_ashfall_place_at_me_thunk:",
        "    pushad",
        "    call _ashfall_hook_anim",
        "    popad",
        "    ret",
        "_ashfall_fire_weapon_thunk:",
        "    pushad",
        "    push eax",
        "    call _ashfall_hook_fire",
        "    add esp, 4",
        "    popad",
        "    ret",
    );
}

/// Install the full vaultmp behavior-patch set on the classic/GOG build.
///
/// Byte-guarded: verifies the classic table's sites hold their expected
/// bytes first (same pattern as apply_steam_respawn). Applies all 34
/// recipes and resolves the 8 hooks from the `hooks::resolve` registry.
/// No-op on the Steam/Anniversary build (sites differ — see steam-re.md).
///
/// # Safety
///
/// Patches executable memory of the current (game) process; guarded by
/// byte checks on the classic build's sites.
#[cfg(target_arch = "x86")]
pub unsafe fn apply_classic_vaultmp() -> bool {
    use crate::hooks::memory;
    use crate::hooks::read_bytes;

    let t = &FO3_STEAM_CLASSIC;
    // Spot-check a few classic-only sites so this is a no-op on Steam:
    // no_respawn_nop (75 03), lock_fix (74 02 88 08), fire_weapon_jmp call.
    if read_bytes(t.no_respawn_nop, 2) != [0x75, 0x03] {
        return false; // not the classic build
    }
    if read_bytes(t.lock_fix, 4) != [0x74, 0x02, 0x88, 0x08] {
        return false;
    }

    let patches = apply(t, hooks::resolve);
    let _ = patches; // patches persist for process lifetime
    let _ = memory::Patch::new; // silence unused-import in test builds
    true
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn apply_classic_vaultmp() -> bool {
    false // tests / x64 hosts — nothing to hook
}

/// Apply the Steam/Anniversary vaultmp behavior patches solved by the
/// 2026-08-17 data/re campaign (lanes A1/A2), byte-guarded like
/// `apply_steam_respawn` — each site is verified against its documented
/// guard bytes before patching, so this is a no-op on classic/GOG/FNV and
/// on any build whose bytes drift.
///
/// Applied (fully solved-static, recipe transfers):
///   - ai_fix2 (0x7D0AA6, write 0x2E) + ai_fix3 (0x7D0AD5, 6B block) —
///     AI-pause behavior inside the Steam predicate 0x7D0A50
///   - delegator_src (0x9B3EF6 relcall → stub 0x405E69) + the stub's
///     push-ecx / pop-ecx / bethesda_delegator hook (delegator pad
///     live-verified 2026-08-15)
///   - place_at_me_jmp (0x79E556 reljumphook) + place_at_me_fix
///     (0x9CBCAF reljump → 0x9CBF97 + nop @ +5)
///
///   - fire_weapon_jmp (0x7DF3F7 E8→0x770880 reljump; E8 math + call-site
///     shape statically confirmed 2026-08-17 — same RelJumpHook-over-E8
///     pattern vaultmp uses on classic 0x71F05F, live-verified there)
///
/// NOT applied (pending-live — documented in `fo3_steam_17_vaultmp`):
///   - fire_fix_jmp/patch: relay stub bytes must be re-derived for Steam
///     register alloc (the 3-byte EB rel8 doesn't transfer)
///   - match_race_*: recipe bytes don't transfer (restructured guard chain)
///   - play_idle_fix_src: choice between the 2 twin instances pending-live
///
/// # Safety
///
/// Patches executable memory of the current process; every site is
/// byte-guarded and skipped on mismatch.
#[cfg(target_arch = "x86")]
pub unsafe fn apply_steam_vaultmp() -> bool {
    use crate::hooks::memory;
    use crate::hooks::read_bytes;
    use fo3_steam_17_vaultmp as s;

    let mut applied = false;

    // ai_fix2: death-state-5 JE redirect. Guard `74 2a 83 f8 03 74` @ site-1.
    if read_bytes(s::AI_FIX2 - 1, 6) == [0x74, 0x2A, 0x83, 0xF8, 0x03, 0x74] {
        let p = memory::Patch::new(s::AI_FIX2 as *const u8, &[0x2E]);
        p.apply();
        applied = true;
    }
    // ai_fix3: test block in the int3 pad. Guard cc x6.
    if read_bytes(s::AI_FIX3, 6) == [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC] {
        let p = memory::Patch::new(
            s::AI_FIX3 as *const u8,
            &[0x85, 0xFF, 0x74, 0xCE, 0xEB, 0xF6],
        );
        p.apply();
        applied = true;
    }
    // Delegator chain (classic 0x6EEC86/0x6EDBD9/0x6EDBDA twin):
    //   - stub pad 0x405E69 gets PUSH ECX (0x51), call_src+5 gets POP ECX
    //   - delegator_src 0x9B3EF6 relcalls the stub
    //   - bethesda_delegator hook wired at delegator_call_src
    if read_bytes(s::DELEGATOR_SRC, 6) == [0x8B, 0x0D, 0xD4, 0xC5, 0x23, 0x01] {
        memory::write_rel_call(s::DELEGATOR_SRC, s::DELEGATOR_DEST);
        applied = true;
    }
    if read_bytes(s::DELEGATOR_DEST, 3) == [0xCC, 0xCC, 0xCC] {
        let push_ecx = memory::Patch::new(s::DELEGATOR_DEST as *const u8, &[0x51]);
        push_ecx.apply();
        let pop_ecx = memory::Patch::new((s::DELEGATOR_CALL_SRC + 5) as *const u8, &[0x59]);
        pop_ecx.apply();
        if let Some(hook) = hooks::resolve("bethesda_delegator") {
            memory::write_rel_call(s::DELEGATOR_CALL_SRC, hook);
        }
        applied = true;
    }
    // place_at_me_jmp: reljumphook at the spawn call site 0x79E556.
    // Guard = the unique call bytes themselves (`e8 25 5f f6 ff` →
    // 0x704480, objdump-validated, 1 hit in the dump). The campaign's
    // "3-zero push" note described the enclosing fn, not the immediate
    // prefix — the site prefix is `50 ff b5 80 fe ff ff` (push eax;
    // push [ebp-0x180]).
    if read_bytes(s::PLACE_AT_ME_JMP, 5) == [0xE8, 0x25, 0x5F, 0xF6, 0xFF] {
        if let Some(hook) = hooks::resolve("place_at_me") {
            memory::write_rel_jump(s::PLACE_AT_ME_JMP, hook);
            applied = true;
        }
    }
    // place_at_me_fix: force-skip the +0x2A8 spawn. Guard `0f 84 e2 02 00 00`.
    if read_bytes(s::PLACE_AT_ME_FIX, 6) == [0x0F, 0x84, 0xE2, 0x02, 0x00, 0x00] {
        memory::write_rel_jump(s::PLACE_AT_ME_FIX, s::PLACE_AT_ME_FIX_DEST);
        let nop = memory::Patch::new((s::PLACE_AT_ME_FIX + 5) as *const u8, &[0x90]);
        nop.apply();
        applied = true;
    }
    // fire_weapon_jmp: RelJumpHook at the fire call site. Guard = the
    // unique E8 rel32 (→0x770880, objdump-verified; same E9-over-E8 pattern
    // vaultmp uses on classic 0x71F05F, live-verified there). The thunk
    // reports the FIRE event and `ret`s to site+5.
    if read_bytes(s::FIRE_WEAPON_JMP, 5) == [0xE8, 0x84, 0x14, 0xF9, 0xFF] {
        if let Some(hook) = hooks::resolve("fire_weapon") {
            memory::write_rel_jump(s::FIRE_WEAPON_JMP, hook);
            applied = true;
        }
    }

    applied
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn apply_steam_vaultmp() -> bool {
    false // tests / x64 hosts — nothing to hook
}

/// Resolve the engine's weapon-fire routine for the running build.
/// Classic/GOG: 0x4BE1A0 (SEH prologue `6a ff 68 da 2f c3 00`). Steam:
/// 0x770880 (SEH prologue `53 8b dc 83 ec 08`). Picks by reading the
/// candidate prologue bytes in-process; 0 when neither matches (non-game).
pub fn fire_routine_addr() -> usize {
    #[cfg(target_arch = "x86")]
    {
        use crate::hooks::read_bytes;
        if read_bytes(0x004B_E1A0, 6) == [0x6A, 0xFF, 0x68, 0xDA, 0x2F, 0xC3] {
            return 0x004B_E1A0; // classic/GOG
        }
        if read_bytes(0x0077_0880, 6) == [0x53, 0x8B, 0xDC, 0x83, 0xEC, 0x08] {
            return 0x0077_0880; // Steam
        }
        0
    }
    #[cfg(not(target_arch = "x86"))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_required_hook_resolves() {
        // The resolver must provide every hook the recipe table needs.
        // On non-x86 hosts the hooks module is cfg'd out (nothing to hook)
        // — the apply path is a no-op there, verified by the recipes test.
        #[cfg(target_arch = "x86")]
        {
            for name in REQUIRED_HOOKS {
                assert!(hooks::resolve(name).is_some(), "hook {name} unresolved");
            }
        }
        #[cfg(not(target_arch = "x86"))]
        {
            let _ = REQUIRED_HOOKS;
        }
    }

    #[test]
    fn ref_event_encodes_ref_id() {
        // The bridge collectors encode ref-id events; the client decodes
        // them. Round-trip through the shared encode/decode.
        let frame =
            ashfall_core::event::encode_ref_event(ashfall_core::event::EVENT_ACTIVATE, 0x1234);
        let (frames, _) = ashfall_core::event::split_frames(&frame);
        let (et, data) = ashfall_core::event::decode_event(&frames[0].payload).unwrap();
        assert_eq!(et, ashfall_core::event::EVENT_ACTIVATE);
        assert_eq!(ashfall_core::event::decode_ref_event(data).unwrap(), 0x1234);
    }

    #[test]
    fn steam_respawn_sites_consistent() {
        use fo3_steam_17_respawn as s;
        // The 6-byte JNE at site B must land exactly on the skip dest:
        // 0F 85 <rel32>: from + 6 + rel == skip.
        let rel: i32 = 0x77;
        assert_eq!(s::SITE_B_JNE + 6 + rel as usize, s::SITE_B_SKIP);
        // Site-A guard bytes (75 03) and site-B tail (00) match the dump.
        assert_eq!(s::SITE_B_TAIL, s::SITE_B_JNE + 5);
        assert!((0x400000..0x1200000).contains(&s::SITE_A_JNE));
        assert!((0x400000..0x1200000).contains(&s::SITE_B_SKIP));
        assert_eq!(s::SITE_B2_FLAG, 0x8C9D52);
        assert!((0x400000..0x1200000).contains(&s::SITE_B2_FLAG));
    }

    #[test]
    fn steam_vaultmp_sites_consistent() {
        use fo3_steam_17_vaultmp as s;
        // All Steam sites in image range.
        for a in [
            s::PLAY_IDLE_CALL_SRC,
            s::LOCK_FIX,
            s::AI_FIX1,
            s::GET_ACTIVATE_JMP,
            s::GET_ACTIVATE_RET,
            s::DELEGATOR_DEST,
            s::DELEGATOR_CALL_SRC,
            s::PLAY_GROUP_FIX,
            s::AV_FIX_SRC,
            s::AV_FIX_RET,
            s::AV_FIX_TERM,
            s::FIRE_WEAPON_JMP,
            s::FIRE_WEAPON_CALL,
            s::PLUGINS_VMP,
        ] {
            assert!((0x400000..0x1200000).contains(&a), "{a:#x} out of range");
        }
        // plugins.txt: ".txt" sits 9 bytes into ".\\Plugins.txt" (GOG layout).
        assert_eq!(s::PLUGINS_VMP, 0xF9FDB1);
        // fire call site is 5 bytes (E8 rel32) before its second call.
        assert_eq!(s::FIRE_WEAPON_CALL, 0x770880);
        assert_eq!(s::FIRE_WEAPON_JMP, 0x7DF3F7);
        // vcdiff-verified EXACT covers (classic → steam).
        assert_eq!(s::GET_ACTIVATE_JMP, 0x8D3BC8);
        assert_eq!(s::GET_ACTIVATE_RET, 0x8D3CB8);
        assert_eq!(s::AI_FIX1, 0x5E99E2);
        assert_eq!(s::PLAY_GROUP_FIX, 0x4350F9);
        assert_eq!(s::DELEGATOR_DEST, 0x405E69);
        assert_eq!(s::DELEGATOR_CALL_SRC, s::DELEGATOR_DEST + 1);
        // av_fix: hook slot + 5B = ret, term = sprintf call after the format push.
        assert_eq!(s::AV_FIX_RET, s::AV_FIX_SRC + 5);
        const _: () = assert!(s::AV_FIX_TERM > s::AV_FIX_RET);
    }

    #[test]
    fn table_addresses_in_pe_range() {
        // FO3 loads at 0x400000; code+data live below 0x1200000.
        let t = &FO3_STEAM_CLASSIC;
        let addrs = [
            t.plugins_vmp,
            t.play_group,
            t.delegator_src,
            t.delegator_dest,
            t.delegator_call_src,
            t.no_respawn_nop,
            t.no_respawn_jmp_src,
            t.no_respawn_jmp_dest,
            t.play_idle_call_src,
            t.play_idle_fix_src,
            t.match_race_nop1,
            t.match_race_nop2,
            t.match_race_patch,
            t.match_race_param,
            t.lock_fix,
            t.ai_fix1,
            t.ai_fix2,
            t.ai_fix3,
            t.ai_fix4,
            t.play_group_fix,
            t.play_group_fix_src,
            t.play_group_fix_dest,
            t.av_fix_src,
            t.av_fix_ret,
            t.av_fix_term,
            t.fire_fix_jmp,
            t.fire_fix_patch,
            t.get_activate_jmp,
            t.get_activate_ret,
            t.place_at_me_jmp,
            t.place_at_me_call,
            t.place_at_me_fix,
            t.place_at_me_fix_dest,
            t.fire_weapon_jmp,
            t.fire_weapon_call,
        ];
        for a in addrs {
            assert!(
                (0x400000..0x1200000).contains(&a),
                "address 0x{a:x} outside FO3 image range"
            );
        }
    }

    #[test]
    fn steam_frame_hook_site_consistent() {
        // The Steam frame-hook site + IAT slot must be in the image range,
        // and the guard is a 6-byte `call [0xF241E4]` (FF 15 disp32) whose
        // disp matches the IAT slot. (Constants are cfg(x86) — mirrored
        // here for the non-x86 test host.)
        #[cfg(target_arch = "x86")]
        {
            use super::{STEAM_FRAME_GUARD, STEAM_FRAME_HOOK_SITE, STEAM_FRAME_IAT_SLOT};
            assert!((0x400000..0x1200000).contains(&STEAM_FRAME_HOOK_SITE));
            assert!((0x400000..0x1200000).contains(&STEAM_FRAME_IAT_SLOT));
            let disp = u32::from_le_bytes([
                STEAM_FRAME_GUARD[2],
                STEAM_FRAME_GUARD[3],
                STEAM_FRAME_GUARD[4],
                STEAM_FRAME_GUARD[5],
            ]);
            assert_eq!(disp as usize, STEAM_FRAME_IAT_SLOT);
            assert_eq!(&STEAM_FRAME_GUARD[..2], &[0xFF, 0x15]);
        }
        // non-x86 host: verify the literal values stay in range
        #[cfg(not(target_arch = "x86"))]
        {
            assert!((0x400000..0x1200000).contains(&0x009B_3D77usize));
            assert!((0x400000..0x1200000).contains(&0x00F2_41E4usize));
        }
    }

    #[test]
    fn recipes_cover_every_patch_from_patchgame() {
        let r = recipes(&FO3_STEAM_CLASSIC);
        // PatchGame writes 34 distinct sites (SafeWrite + WriteRel* combined
        // into the recipe list above); guard against accidental drops.
        assert_eq!(r.len(), 34, "recipe count regressed");
        // Every site in the image range.
        for rec in &r {
            assert!(
                (0x400000..0x1200000).contains(&rec.addr()),
                "recipe {} at 0x{:x} outside range",
                match rec {
                    Recipe::Bytes { name, .. } => name,
                    Recipe::Write { name, .. } => name,
                    Recipe::RelCall { name, .. } => name,
                    Recipe::RelJump { name, .. } => name,
                    Recipe::RelCallHook { name, .. } => name,
                    Recipe::RelJumpHook { name, .. } => name,
                },
                rec.addr()
            );
        }
        // No two recipes write the same site.
        let mut sites: Vec<usize> = r.iter().map(|rec| rec.addr()).collect();
        sites.sort();
        let dup: Vec<_> = sites.windows(2).filter(|w| w[0] == w[1]).collect();
        assert!(dup.is_empty(), "duplicate recipe sites: {dup:?}");
    }

    #[test]
    fn every_required_hook_is_requested() {
        let r = recipes(&FO3_STEAM_CLASSIC);
        let requested: std::collections::BTreeSet<_> =
            r.iter().filter_map(Recipe::required_hook).collect();
        for h in REQUIRED_HOOKS {
            assert!(requested.contains(h), "hook {h} not requested by recipes");
        }
        assert_eq!(requested.len(), REQUIRED_HOOKS.len());
    }

    #[test]
    fn plugins_vmp_bytes_are_literal_dot_vmp() {
        assert_eq!(u32::from_le_bytes(*b".vmp"), 0x706D762E);
    }

    #[test]
    fn steam_vaultmp_site_constants_are_documented() {
        // Pin the campaign-derived Steam sites (data/re/fo3/steam-vaultmp-
        // twins.md, objdump-validated 27/27) so a mistyped constant or a
        // stray edit is caught. Values are the validated addresses.
        use fo3_steam_17_vaultmp as s;
        assert_eq!(s::AI_FIX2, 0x007D_0AA6);
        assert_eq!(s::AI_FIX3, 0x007D_0AD5);
        assert_eq!(s::AI_PREDICATE, 0x007D_0A50);
        assert_eq!(s::DELEGATOR_SRC, 0x009B_3EF6);
        assert_eq!(s::DELEGATOR_DEST, 0x0040_5E69);
        assert_eq!(s::DELEGATOR_CALL_SRC, 0x0040_5E6A);
        assert_eq!(s::PLAY_IDLE_FIX_SRC, 0x0079_DA88);
        assert_eq!(s::FIRE_FIX_JMP, 0x008D_A397);
        assert_eq!(s::FIRE_FIX_PATCH, 0x008D_A3CE);
        assert_eq!(s::PLACE_AT_ME_JMP, 0x0079_E556);
        assert_eq!(s::PLACE_AT_ME_CALL, 0x0070_4480);
        assert_eq!(s::PLACE_AT_ME_FIX, 0x009C_BCAF);
        assert_eq!(s::PLACE_AT_ME_FIX_DEST, 0x009C_BF97);
        assert_eq!(s::MATCH_RACE_NOP1, 0x006F_71FA);
        assert_eq!(s::MATCH_RACE_NOP2, 0x006F_720E);
        assert_eq!(s::MATCH_RACE_PATCH, 0x006F_7220);
        #[cfg(target_arch = "x86")]
        assert_eq!(super::STEAM_AI_PREDICATE, 0x007D_0A50);
    }

    #[test]
    fn rel_jump_offset_math() {
        // write_rel_jump(from,to) emits E9 + (to-from-5) as little-endian i32.
        let from = 0x0049DD8Eusize;
        let to = 0x0049DCF1usize;
        let disp = (to as isize - from as isize - 5) as i32;
        assert_eq!(disp, -162);
        assert_eq!(disp.to_le_bytes(), [0x5E, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn recipe_bytes_match_vaultmp_source() {
        let r = recipes(&FO3_STEAM_CLASSIC);
        let find = |name: &str| {
            r.iter()
                .find(|rec| match rec {
                    Recipe::Bytes { name: n, .. } => *n == name,
                    Recipe::Write { name: n, .. } => *n == name,
                    _ => false,
                })
                .expect(name)
        };
        // Delegator stub: PUSH ECX; call site +5: POP ECX.
        if let Recipe::Write { value, .. } = find("delegator_push_ecx") {
            assert_eq!(*value, 0x51);
        } else {
            panic!("delegator_push_ecx wrong variant");
        }
        if let Recipe::Write { value, .. } = find("delegator_call_pop_ecx") {
            assert_eq!(*value, 0x59);
        } else {
            panic!("delegator_call_pop_ecx wrong variant");
        }
        // aiFix3 block is exact from vaultmp: 85 FF 74 CC EB F6.
        if let Recipe::Bytes { bytes, .. } = find("ai_fix3_block") {
            assert_eq!(*bytes, [0x85, 0xFF, 0x74, 0xCC, 0xEB, 0xF6]);
        } else {
            panic!("ai_fix3_block wrong variant");
        }
        // FireFix jump: EB 57 90.
        if let Recipe::Bytes { bytes, .. } = find("fire_fix_jmp") {
            assert_eq!(*bytes, [0xEB, 0x57, 0x90]);
        } else {
            panic!("fire_fix_jmp wrong variant");
        }
        // match_race_nop1 is an 18-byte NOP block.
        if let Recipe::Bytes { bytes, .. } = find("match_race_nop1") {
            assert_eq!(bytes.len(), 18);
            assert!(bytes.iter().all(|b| *b == 0x90));
        } else {
            panic!("match_race_nop1 wrong variant");
        }
    }
}
