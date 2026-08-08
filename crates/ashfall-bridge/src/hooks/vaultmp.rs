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

/// Steam FO3 (post-2023) respawn-disable sites — re-derived 2026-08-08 by
/// side-by-side disassembly vs GOG (see docs/steam-re.md). Same semantics as
/// vaultmp's ToggleRespawn disable: NOP the site-A predicate JNE + jump the
/// site-B guard over the respawn-flag write (`mov byte [eax+2],1` @ 0x8CA8EB).
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
    // Site B: unconditional JMP over the respawn-flag write (0x8CA8EB).
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
            bytes: &[0x85, 0xC9, 0x0F, 0x84, 0xFF, 0x00, 0x00, 0x00, 0x8B, 0x71, 0x0C, 0x85, 0xF6, 0xEB, 0x6A],
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
            Recipe::Write { addr, value, width, .. } => {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn table_addresses_in_pe_range() {
        // FO3 loads at 0x400000; code+data live below 0x1200000.
        let t = &FO3_STEAM_CLASSIC;
        let addrs = [
            t.plugins_vmp, t.play_group, t.delegator_src, t.delegator_dest,
            t.delegator_call_src, t.no_respawn_nop, t.no_respawn_jmp_src,
            t.no_respawn_jmp_dest, t.play_idle_call_src, t.play_idle_fix_src,
            t.match_race_nop1, t.match_race_nop2, t.match_race_patch,
            t.match_race_param, t.lock_fix, t.ai_fix1, t.ai_fix2, t.ai_fix3,
            t.ai_fix4, t.play_group_fix, t.play_group_fix_src,
            t.play_group_fix_dest, t.av_fix_src, t.av_fix_ret, t.av_fix_term,
            t.fire_fix_jmp, t.fire_fix_patch, t.get_activate_jmp,
            t.get_activate_ret, t.place_at_me_jmp, t.place_at_me_call,
            t.place_at_me_fix, t.place_at_me_fix_dest, t.fire_weapon_jmp,
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
        let requested: std::collections::BTreeSet<_> = r
            .iter()
            .filter_map(Recipe::required_hook)
            .collect();
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
