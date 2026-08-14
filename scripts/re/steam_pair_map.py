#!/usr/bin/env python3
"""Pair GOG patch-site functions to Steam twins.

For each classic site: find the containing GOG function, then score every
Steam function by size proximity (recompile preserves function size
roughly). Print top candidates for manual side-by-side disasm verification.
"""
import json, sys, os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from steam_twin_search import SITES

GOG_FNS = "/tmp/gog_fns.json"
STEAM_FNS = "/tmp/steam_fns.json"

def load(p):
    return json.load(open(p))

def containing(fns, va):
    """Return the function whose [minaddr,maxaddr] covers va (or nearest start)."""
    best = None
    for f in fns:
        if f.get("minaddr", f["addr"]) <= va <= f.get("maxaddr", f["addr"] + f["size"]):
            if best is None or f["size"] < best["size"]:
                best = f
    return best

def main():
    gog = load(sys.argv[1] if len(sys.argv) > 1 else GOG_FNS)
    steam = load(sys.argv[2] if len(sys.argv) > 2 else STEAM_FNS)
    only = sys.argv[3] if len(sys.argv) > 3 else None
    for name, (va, _) in SITES.items():
        if only and only not in name:
            continue
        f = containing(gog, va)
        if f is None:
            print(f"{name:20s} {va:#010x}  NO containing fn in GOG list")
            continue
        # candidates by size proximity (Steam size typically 0.7-1.3x classic)
        lo, hi = int(f["size"] * 0.55), int(f["size"] * 1.6)
        cands = [s for s in steam if lo <= s["size"] <= hi]
        cands.sort(key=lambda s: abs(s["size"] - f["size"]))
        top = cands[:5]
        print(f"{name:20s} GOG fn {f['addr']:#010x} ({f['name']}) size={f['size']}B "
              f"[{f['minaddr']:#010x}..{f['maxaddr']:#010x}]")
        for s in top:
            print(f"    cand {s['addr']:#010x} ({s['name']}) size={s['size']}B "
                  f"ninstrs={s.get('ninstrs')} cc={s.get('cc')}")

if __name__ == "__main__":
    main()
