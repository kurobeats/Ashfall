#!/usr/bin/env python3
"""Scan .rdata vtable runs for a known getter pair (e.g. the lock-state
getter `8a 41 0a 24 01 c3`) and report the pair's slot offset per vtable.

The reliable way to pin a vtable slot for a method: scan all contiguous
runs of code pointers in .rdata and find where the method pointer appears.
Unlike the GECK-RTTI route (whose COL can point at a SECONDARY vtable, not
vtbl[0]), this reflects the actual vtable layout of every class that
shares the method.

Usage: lock_scan.py <gog|steam>   (uses /tmp/Fallout3.exe or the flat dump)
"""
import struct
import sys


def run(game, pair):
    def is_code(v):
        return 0x401000 <= v < 0x1200000

    blob = game
    n = len(blob)
    results = []
    i = 0
    while i + 8 <= n:
        v = struct.unpack_from("<I", blob, i)[0]
        if not is_code(v):
            i += 4
            continue
        j = i
        while j + 4 <= n and is_code(struct.unpack_from("<I", blob, j)[0]):
            j += 4
        if (j - i) // 4 >= 8:
            for k in range(i, j - 4, 4):
                v1 = struct.unpack_from("<I", blob, k)[0]
                v2 = struct.unpack_from("<I", blob, k + 4)[0]
                if v1 == pair[0] and v2 == pair[1]:
                    results.append((0x400000 + i, k - i))
        i = j
    from collections import Counter
    c = Counter(slot for _, slot in results)
    print(f"vtables with pair ({pair[0]:#x},{pair[1]:#x}): {len(results)}")
    print("slot distribution:", {hex(s): cnt for s, cnt in c.most_common()})
    return results


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else "gog"
    if mode == "steam":
        # Steam lock getter pair (flat dump)
        run(open("/tmp/steam-fo3.bin", "rb").read(), (0x57C770, 0x57C780))
    else:
        run(open("/tmp/Fallout3.exe", "rb").read(), (0x4017E0, 0x4017F0))
