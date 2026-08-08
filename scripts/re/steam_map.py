#!/usr/bin/env python3
"""Section-aware VA<->file-offset mapping for the Steam FO3 image dump.

The dump (data/fallout3/steam-fo3.bin) is a PE-layout copy of the unpacked
image, NOT a flat VA dump. file_offset != VA - 0x400000 (the old +0xC00
trap). Use these helpers; also used for GOG PE files (same PE layout).

Usage:
  python3 steam_map.py <image> va 0x8ca8e0      # VA -> file offset
  python3 steam_map.py <image> off 0x4c9d26     # file offset -> VA
  python3 steam_map.py <image> scan <pattern>   # hex-pattern scan in .text,
                                                # prints VA hits (section-aware)
"""
import struct
import sys


def sections(data: bytes):
    e = struct.unpack("<I", data[0x3C:0x40])[0]
    n = struct.unpack("<H", data[e + 6 : e + 8])[0]
    opt = e + 24
    base = struct.unpack("<I", data[opt + 28 : opt + 32])[0]
    sopt = opt + struct.unpack("<H", data[e + 20 : e + 22])[0]
    out = []
    for i in range(n):
        s = sopt + i * 40
        name = data[s : s + 8].rstrip(b"\x00").decode()
        vsize, va, rsize, roff = struct.unpack("<IIII", data[s + 8 : s + 24])
        out.append((name, base + va, vsize, roff, roff + rsize))
    return out


def va2off(secs, va):
    for _n, v, vs, ro, _re in secs:
        if v <= va < v + vs:
            return ro + (va - v)
    return None


def off2va(secs, off):
    for _n, v, _vs, ro, re in secs:
        if ro <= off < re:
            return v + (off - ro)
    return None


def main():
    data = open(sys.argv[1], "rb").read()
    secs = sections(data)
    cmd = sys.argv[2]
    if cmd == "va":
        va = int(sys.argv[3], 0)
        print(hex(va2off(secs, va)))
    elif cmd == "off":
        off = int(sys.argv[3], 0)
        print(hex(off2va(secs, off)))
    elif cmd == "scan":
        pat = bytes.fromhex(sys.argv[3])
        hits = []
        st = 0
        while True:
            i = data.find(pat, st)
            if i < 0:
                break
            va = off2va(secs, i)
            if va and 0x401000 <= va < 0xF2397C:  # .text
                hits.append(va)
            st = i + 1
        print(len(hits), [hex(h) for h in hits])
    else:
        print("unknown cmd", cmd)
        sys.exit(1)


if __name__ == "__main__":
    main()
