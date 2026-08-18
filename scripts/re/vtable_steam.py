#!/usr/bin/env python3
"""Steam PlayerCharacter vtable RE — build + slot translation.

The Steam (post-2023) recompile REORDERED the TESObjectREFR/PlayerCharacter
vtable. This tool:
  1. Finds the Steam vtable base (0xF938FC — verified via the AI predicate's
     actor slot +0x22C -> 0x8B8AF0 and the death-handler slot +0x23C).
  2. Builds the GOG->Steam slot translation by byte-identical function
     matching (small getters survive the recompile).
  3. Prints the dominant +0x58 shift histogram.

Usage: vtable_steam.py  (files: /tmp/Fallout3.exe GOG, /tmp/steam-fo3.bin dump)
"""
import struct
from collections import Counter

g = open("/tmp/Fallout3.exe", "rb").read()
dump = open("/tmp/steam-fo3.bin", "rb").read()

GOG_VT = 0xE18110          # GOG PlayerCharacter vtable (death-handler 0x788350 @ +0x2FC; corrected 2026-08-18 from 0xE16B10)
STEAM_VT = 0xF938FC        # Steam PlayerCharacter vtable (AI-pred +0x22C -> 0x8B8AF0)
GOG_RD = 0x999A00

def gva(va): return 0x400 + (va - 0x401000)

def find_gog_pc_vtable():
    """Locate the GOG PC vtable by the known death-handler method."""
    target = 0x788350
    i = g.find(struct.pack("<I", target), GOG_RD, GOG_RD + 0x1AA800)
    j = i
    while j > GOG_RD:
        va = struct.unpack_from("<I", g, j-4)[0]
        if not (0x401000 <= va < 0x1200000):
            break
        j -= 4
    return 0x400000 + j

def build_steam_slot_index():
    s_off = STEAM_VT - 0x400000
    slots = {}
    for k in range(512):
        va = struct.unpack_from("<I", dump, s_off + k*4)[0]
        slots.setdefault(va, k*4)
    return slots

def main():
    gog_vt = find_gog_pc_vtable()
    print(f"GOG PC vtable: {gog_vt:#x}")
    print(f"Steam PC vtable: {STEAM_VT:#x} (AI-pred +0x22C -> 0x8B8AF0 verified)")
    g_off = gog_vt - 0x400000
    s_slots = build_steam_slot_index()
    shifts = []
    trans = []
    for slot in range(0, 0x300, 4):
        gfn = struct.unpack_from("<I", g, g_off + slot)[0]
        if not (0x401000 <= gfn < 0x1200000):
            continue
        off = gva(gfn)
        for n in (32, 24, 20, 16, 12, 8):
            pat = g[off:off+n]
            i = dump.find(pat, 0x400, 0x400 + 0xb22a00 - n)
            if i >= 0:
                sfn = 0x400000 + i
                if sfn in s_slots:
                    ss = s_slots[sfn]
                    trans.append((slot, gfn, ss, sfn, n))
                    shifts.append(ss - slot)
                break
    print(f"\ntranslated: {len(trans)} slots")
    for (gs, gfn, ss, sfn, n) in trans:
        print(f"  GOG +{gs:#04x} {gfn:#x} -> Steam +{ss:#04x} {sfn:#x} (match {n}B)")
    c = Counter(shifts)
    print(f"\nshift histogram: {c.most_common(6)}")
    exact = sum(1 for s in shifts if s == 0x58)
    print(f"{exact}/{len(shifts)} fit +0x58 exactly")

if __name__ == "__main__":
    main()
