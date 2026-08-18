#!/usr/bin/env python3
"""Steam twin search: classic GOG site bytes -> byte-search in Steam flat dump.

Sites = vaultmp PatchGame() table (classic Steam 1.7, == GOG 1.7.0.3 per docs).
For each site, dump N bytes from the GOG PE, search the Steam flat dump
(offset = VA - 0x400000) for the anchor, print hits with VA + context.
"""
import struct, sys, os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gog_bytes import va_to_off

GOG = "/tmp/Fallout3.exe"       # local copy path override via argv
STEAM = "/tmp/steam-fo3.bin"

# site name -> (VA, dump-len)  — vaultmp table + known Steam re-derived anchors
SITES = {
    # already solved (sanity anchors)
    "respawn_siteA": (0x6D5965, 24),
    "respawn_siteB": (0x78B230, 24),
    "ai_predicate":  (0x6FAE90, 40),   # classic entry, Steam twin 0x7D0A50 (corrected 2026-08-17)
    # remaining groups
    "ai_fix1":       (0x72051E, 24),
    "ai_fix2":       (0x6FAEE8, 32),
    "ai_fix3":       (0x6FAF19, 24),
    "ai_fix4":       (0x42FBDC, 24),
    "fire_fix_jmp":  (0x79236C, 24),
    "fire_fix_patch":(0x7923C5, 24),
    "fire_weapon_jmp":(0x71F05F, 24),
    "fire_weapon_call":(0x4BE1A0, 16),
    "get_activate_jmp":(0x78A68D, 24),
    "get_activate_ret":(0x78A995, 16),
    "place_at_me_jmp":(0x539785, 24),
    "place_at_me_call":(0x43DEF0, 16),
    "place_at_me_fix":(0x6F1CB6, 24),
    "place_at_me_fix_dest":(0x6F1F6E, 16),
    "match_race_nop1":(0x52F4DD, 24),
    "match_race_nop2":(0x52F50F, 24),
    "match_race_patch":(0x52F513, 16),
    "match_race_param":(0xF51ADC, 8),
    "lock_fix":      (0x527F33, 16),
    "delegator_src": (0x6EEC86, 24),
    "delegator_dest":(0x6EDBD9, 16),
    "delegator_call_src":(0x6EDBDA, 16),
    "play_idle_call_src":(0x73BB20, 24),
    "play_idle_fix_src":(0x534D8D, 24),
    "play_group":    (0x45F704, 16),
    "play_group_fix":(0x49DD6A, 16),
    "play_group_fix_src":(0x49DD8E, 16),
    "play_group_fix_dest":(0x49DCF1, 24),
    "av_fix_src":    (0x473D35, 24),
    "av_fix_ret":    (0x473D3B, 16),
    "av_fix_term":   (0x473E85, 16),
    "plugins_vmp":   (0xE10FF1, 16),
}

def main():
    gog = sys.argv[1] if len(sys.argv) > 1 else GOG
    steam = sys.argv[2] if len(sys.argv) > 2 else STEAM
    gdata = open(gog, "rb").read()
    sdata = open(steam, "rb").read()
    print(f"GOG {gog} ({len(gdata)}B)  STEAM {steam} ({len(sdata)}B, flat)\n")
    for name, (va, n) in SITES.items():
        off = va_to_off(gdata, va)
        if off is None:
            print(f"{name:20s} VA {va:#010x}  NO SECTION in GOG"); continue
        anchor = gdata[off:off + n]
        # search steam (flat)
        pat = anchor
        hits = []
        i = 0
        while True:
            i = sdata.find(pat, i)
            if i < 0: break
            hits.append(0x400000 + i)
            i += 1
        if hits:
            print(f"{name:20s} {va:#010x}  {anchor[:16].hex(' ')}...  ->  {len(hits)} hit(s): " +
                  " ".join(f"{h:#010x}" for h in hits[:6]))
        else:
            # try shorter prefix (first 12 bytes, then first 8)
            for pn in (12, 8):
                hits = []
                pat = anchor[:pn]
                i = 0
                while True:
                    i = sdata.find(pat, i)
                    if i < 0: break
                    hits.append(0x400000 + i)
                    i += 1
                if hits:
                    print(f"{name:20s} {va:#010x}  {anchor[:16].hex(' ')}...  ->  {len(hits)} hit(s) (prefix {pn}B): " +
                          " ".join(f"{h:#010x}" for h in hits[:6]))
                    break
            else:
                print(f"{name:20s} {va:#010x}  {anchor[:16].hex(' ')}...  ->  NO HIT (any prefix)")

if __name__ == "__main__":
    main()
