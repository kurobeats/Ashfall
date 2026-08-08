#!/usr/bin/env python3
"""Scan the player object's fields for the baseForm pointer (player base 0x707)."""
import socket
import struct

s = socket.create_connection(("127.0.0.1", 1771), timeout=10)


def cmd(func, params=b"", key=7):
    frame = bytes([2]) + struct.pack("<I", key) + struct.pack("<I", func) + bytes([len(params)]) + params
    s.sendall(frame)
    data = b""
    while len(data) < 5:
        c = s.recv(65536)
        if not c:
            break
        data += c
    return data[5:]


def u32(b, o):
    return struct.unpack("<I", b[o:o + 4])[0] if len(b) >= o + 4 else None


raw = cmd(0x00FB, struct.pack("<I", 0x14))
print("probe len:", len(raw), "(expect 340)")
if len(raw) < 340:
    print("short probe — abort")
    raise SystemExit(1)
obj = u32(raw, 0)
vtable = u32(raw, 4)
print(f"obj=0x{obj:08x} vtable=0x{vtable:08x}")
fields = [u32(raw, 84 + i * 4) for i in range(64)]

# Candidate base-form pointers: into the game heap, and (indirect) whose
# +0x0C reads a plausible formID. We can't deref remotely, so flag fields
# whose value points into the heap AND the following fields look coherent.
print("heap-like fields (0x01000000..0x06000000):")
for i, v in enumerate(fields):
    if v >= 0x01000000 and v < 0x06000000:
        print(f"  field +0x{i * 4:02x} = 0x{v:08x}")

# Also: the base form of a REFR in the engine often sits right after the
# vtable ptr in some builds — print fields 0..16 raw for manual inspection.
print("fields 0..16:")
print("  " + " ".join(f"{v:08x}" for v in fields[:16]))
s.close()
