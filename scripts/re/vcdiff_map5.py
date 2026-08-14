#!/usr/bin/env python3
"""Parse the FalloutAnniversaryPatcher vcdiff into a classic->Steam byte map.

The downgrade delta (Anniversary Steam -> classic 1.7.0.3) encodes every byte
that survived the recompile as a CPY_0 instruction: target (classic) offset
comes from source (Steam). Decode both sides with xdelta3 first:

    xdelta3 -d -s steam-anniv.exe patch_steam.vcdiff classic_out.exe
    xdelta3 printdelta patch_steam.vcdiff > vcdiff_insts.txt

Trap: printdelta's Offset and S@ columns are DECIMAL, not hex. CPY_0 = copy
from Steam source; Offset = classic target position; S@ = Steam source pos.
Runs are verified: classic[tgt:tgt+size] == steam[src:src+size].

Usage: vcdiff_map5.py [insts.txt] [classic_out.exe] [steam-anniv.exe]
Output: /tmp/classic_steam_map.txt lines "classic_va steam_va size"
"""
import re
import sys

def parse(path):
    insts = []
    for line in open(path):
        line = line.strip()
        if not line or line.startswith("VCDIFF") or line.startswith("  Offset"):
            continue
        m = re.match(r"^([0-9]+)\s+\d+\s+(.*)$", line)
        if not m:
            continue
        off = int(m.group(1), 10)  # DECIMAL target offset
        for (t, s, a) in re.findall(r"(\S+)\s+(\d+)\s+(S@[0-9]+|T@[0-9]+|\(nil\))", m.group(2)):
            size = int(s)
            if t == "CPY_0" and a.startswith("S@"):
                insts.append((off, size, int(a[2:], 10)))  # S@ DECIMAL
    return insts

def main():
    inst_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/vcdiff_insts.txt"
    classic_path = sys.argv[2] if len(sys.argv) > 2 else "/tmp/classic_out.exe"
    steam_path = sys.argv[3] if len(sys.argv) > 3 else "/tmp/steam-anniv.exe"
    steam = open(steam_path, "rb").read()
    classic = open(classic_path, "rb").read()
    insts = parse(inst_path)
    print(f"CPY_0: {len(insts)}")
    TEXT = (0x400, 0x999A00)
    ok = bad = 0
    out = []
    for (tgt, size, src) in insts:
        if not (TEXT[0] <= tgt < TEXT[1]):
            continue
        if src + size > len(steam) or tgt + size > len(classic):
            bad += 1
            continue
        if classic[tgt:tgt+size] == steam[src:src+size]:
            ok += 1
            cva = 0x400000 + (tgt - 0x400) + 0x1000
            sva = 0x400000 + (src - 0x400) + 0x1000
            out.append((cva, sva, size))
        else:
            bad += 1
    print(f".text CPY_0 verified: ok={ok} bad={bad}")
    with open("/tmp/classic_steam_map.txt", "w") as f:
        for (c, s, n) in out:
            f.write(f"{c:#x} {s:#x} {n}\n")
    print(f"saved {len(out)} mappings to /tmp/classic_steam_map.txt")

if __name__ == "__main__":
    main()
