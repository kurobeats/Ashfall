#!/usr/bin/env python3
"""MSVC RTTI vtable walker for Gamebryo editor builds (GECK).

The GECK editor binaries retain full RTTI (the game exes strip it) —
e.g. FNV GECK v1.4 has 2,322 `.?AV` type descriptors. This walks
TypeDescriptor -> CompleteObjectLocator -> vftable and prints the named
vtable slots, so a class's vtable layout can be enumerated statically.

Usage: rtti_walk.py <exe> [ClassName ...]
  rtti_walk.py Geck.exe                  # all classes summary
  rtti_walk.py Geck.exe Actor TESForm    # named class vtable slots

Caveats (verified 2026-08-14 on FNV GECK 1.4, md5 6ecfb21d...):
- COL signature is 0 in this build (not the usual 1) — the walker matches
  `[0][0][0][pTypeDescriptor]` in .rdata/.data.
- Editor builds STUB runtime-simulation methods (ActorValueOwner AV getters
  are `xor eax,eax; ret 4`) — the GECK cannot supply real method bodies for
  AV/combat code.
- The GECK vtable LAYOUT differs from the game's (editor-specific virtuals,
  stub overrides) — slot offsets do NOT transfer to Fallout3.exe/
  FalloutNV.exe. Use RTTI only for class-structure confirmation.
"""
import struct
import re
import sys
from collections import defaultdict


def sections(data):
    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    magic = struct.unpack_from("<H", data, e_lfanew + 24)[0]
    image_base = struct.unpack_from("<I", data, e_lfanew + 24 + 28)[0]
    nsec = struct.unpack_from("<H", data, e_lfanew + 6)[0]
    optsz = struct.unpack_from("<H", data, e_lfanew + 20)[0]
    sec = e_lfanew + 24 + optsz
    out = []
    for i in range(nsec):
        off = sec + i * 40
        name = data[off:off + 8].rstrip(b"\x00").decode(errors="replace")
        vsz, vaddr, rsz, roff = struct.unpack_from("<IIII", data, off + 8)
        out.append((name, image_base, vaddr, vsz, roff, rsz))
    return out


def va_to_off(data, secs, va):
    for name, ib, vaddr, vsz, roff, rsz in secs:
        rva = va - ib
        if vaddr <= rva < vaddr + max(vsz, rsz):
            return roff + (rva - vaddr)
    return None


def is_code_ptr(ib, v):
    return ib + 0x1000 <= v < ib + 0x1200000


def main():
    path = sys.argv[1]
    data = open(path, "rb").read()
    secs = sections(data)
    ib = secs[0][1]

    td_name = {}
    for m in re.finditer(rb"\.\?AV([A-Za-z0-9_$@?]+)@@\x00", data):
        va = None
        for name, ib2, vaddr, vsz, roff, rsz in secs:
            if roff <= m.start() < roff + rsz:
                va = ib2 + vaddr + (m.start() - roff)
                break
        if va is None:
            continue
        td = va - 8
        toff = va_to_off(data, secs, td)
        if toff is None or toff < 8:
            continue
        if is_code_ptr(ib, struct.unpack_from("<I", data, toff)[0]):
            td_name[td] = m.group(1).decode(errors="replace")
    print(f"TypeDescriptors: {len(td_name)}")

    # COLs: [0][0][0][pTypeDescriptor] (signature field is 0 in this build)
    cols = {}
    for name, ib2, vaddr, vsz, roff, rsz in secs:
        if name not in (".rdata", ".data"):
            continue
        blob = data[roff:roff + rsz]
        for i in range(0, len(blob) - 32, 4):
            if (struct.unpack_from("<I", blob, i)[0] == 0
                    and struct.unpack_from("<I", blob, i + 4)[0] == 0
                    and struct.unpack_from("<I", blob, i + 8)[0] == 0):
                ptd = struct.unpack_from("<I", blob, i + 12)[0]
                if ptd in td_name:
                    cols[ib2 + vaddr + i] = ptd
    print(f"COLs: {len(cols)}")

    # vftable = (position of dword == col) + 4
    vft_for_col = defaultdict(list)
    for name, ib2, vaddr, vsz, roff, rsz in secs:
        if name not in (".rdata", ".data"):
            continue
        blob = data[roff:roff + rsz]
        for i in range(0, len(blob) - 4, 4):
            v = struct.unpack_from("<I", blob, i)[0]
            if v in cols:
                vft_for_col[v].append(ib2 + vaddr + i + 4)
    print(f"vtables linked: {len(vft_for_col)}")

    classes = defaultdict(list)
    for col, td in cols.items():
        for vft in vft_for_col.get(col, []):
            classes[td_name[td]].append(vft)

    want = sys.argv[2:] or list(classes.keys())
    for w in want:
        vfts = sorted(set(classes.get(w, [])))
        print(f"\n=== {w}: {len(vfts)} vtable(s) ===")
        for vft in vfts[:4]:
            vo = va_to_off(data, secs, vft)
            slots = []
            for k in range(256):
                s = struct.unpack_from("<I", data, vo + k * 4)[0]
                if is_code_ptr(ib, s):
                    slots.append(s)
                else:
                    break
            print(f"  vft={vft:#x} ({len(slots)} slots):")
            for row in range(0, len(slots), 8):
                print(f"    +{row*4:#04x}: " + " ".join(
                    hex(s) for s in slots[row:row + 8]))


if __name__ == "__main__":
    main()
