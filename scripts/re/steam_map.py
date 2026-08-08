#!/usr/bin/env python3
"""VA <-> file-offset mapping for the Steam FO3 image dump.

The dump (data/fallout3/steam-fo3.bin) is a FLAT memory dump: dump_image()
reads SizeOfImage bytes contiguously from the image base 0x400000, so

    file offset = VA - 0x400000

NOT PE section math. r2 -m 0x400000 PE-parses the dump and shifts .text
addresses by +0xC00 (section alignment vs file alignment) — subtract 0xC00
from any r2-derived address, or use this script. Verified live on tetsuo via
OP_PROBE_CODE (2026-08-08).

Usage:
  python3 steam_map.py va 0x8c9ce0      # VA -> file offset
  python3 steam_map.py off 0x4c9ce0     # file offset -> VA
  python3 steam_map.py scan <pattern>   # hex-pattern scan in .text (flat),
                                        # prints VA hits
"""
import sys

BASE = 0x400000
TEXT_END = 0xF2397C  # end of .text (dump is flat; sections irrelevant)


def va2off(va):
    return va - BASE


def off2va(off):
    return BASE + off


def main():
    data = open(sys.argv[1], "rb").read()
    cmd = sys.argv[2]
    if cmd == "va":
        va = int(sys.argv[3], 0)
        print(hex(va2off(va)))
    elif cmd == "off":
        off = int(sys.argv[3], 0)
        print(hex(off2va(off)))
    elif cmd == "scan":
        pat = bytes.fromhex(sys.argv[3])
        hits = []
        st = 0
        while True:
            i = data.find(pat, st)
            if i < 0:
                break
            va = off2va(i)
            if BASE <= va < TEXT_END:  # .text only
                hits.append(va)
            st = i + 1
        print(len(hits), [hex(h) for h in hits])
    else:
        print("unknown cmd", cmd)
        sys.exit(1)


if __name__ == "__main__":
    main()
