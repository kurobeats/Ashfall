#!/usr/bin/env python3
"""Dump bytes at VAs in a PE file (no pefile dep — manual section math)."""
import struct, sys

def va_to_off(data, va):
    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    magic = struct.unpack_from("<H", data, e_lfanew + 24)[0]
    # Optional header: image base at +28 (PE32) / +24 (PE32+)
    if magic == 0x10B:  # PE32
        image_base = struct.unpack_from("<I", data, e_lfanew + 24 + 28)[0]
    else:
        image_base = struct.unpack_from("<Q", data, e_lfanew + 24 + 24)[0]
    nsec = struct.unpack_from("<H", data, e_lfanew + 6)[0]
    optsz = struct.unpack_from("<H", data, e_lfanew + 20)[0]
    sec = e_lfanew + 24 + optsz
    rva = va - image_base
    for i in range(nsec):
        off = sec + i * 40
        vsz, vaddr, rsz, roff = struct.unpack_from("<IIII", data, off + 8)
        if vaddr <= rva < vaddr + max(vsz, rsz):
            return roff + (rva - vaddr)
    return None

def dump(path, va, n):
    data = open(path, "rb").read()
    off = va_to_off(data, va)
    if off is None:
        return None
    return data[off:off + n]

if __name__ == "__main__":
    path, va, n = sys.argv[1], int(sys.argv[2], 0), int(sys.argv[3])
    b = dump(path, va, n)
    if b is None:
        print("NO SECTION")
    else:
        print(b.hex(" "))
