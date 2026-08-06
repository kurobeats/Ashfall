# TOOL 2: opcode verification via radare2 (search + memory dump).
# The reads (string search, pointer search, entry dump, name dump) are all
# performed by r2; python only generates commands and parses output.
import subprocess, re, struct

GECK = "/home/user/Downloads/fallout3/app/GECK.exe"

va = {"PlaceAtMe": "0x00d4ad2c", "AddItem": "0x00d4afbc", "RemoveItem": "0x00d4aa78",
      "EquipItem": "0x00d49dac", "UnequipItem": "0x00d49d94", "ForceActorValue": "0x00d49a08",
      "KillActor": "0x00d4a658", "SetRestrained": "0x00d49d04", "PlayGroup": "0x00d4ae44",
      "UnLock": "0x00d4a7e0", "SetOwnership": "0x00d4985c", "SetStage": "0x00d4abe4",
      "SetAlert": "0x00d4a9d8"}

def r2(cmd):
    return subprocess.run(["r2", "-q", "-c", cmd, GECK], capture_output=True, text=True).stdout

print("TOOL 2 (radare2) — opcodes via r2 /x search + p8 dump:")
for name, sva in va.items():
    ptr_le = struct.pack("<I", int(sva, 16)).hex()
    o = r2("e search.in=io.maps; /x " + ptr_le)
    hits = re.findall(r"(0x[0-9a-f]+) hit", o)
    entry = None
    for h in hits:
        a = int(h, 16)
        if 0xEB0000 <= a < 0xED0000:  # command table VA region (.data)
            entry = h
            break
    if entry is None:
        print("%-16s NO entry in table region %s" % (name, hits[:3]))
        continue
    b = bytes.fromhex(r2("p8 16 @ " + entry).strip())
    op = struct.unpack("<H", b[8:10])[0]
    print("%-16s entry@%s opcode=0x%04X" % (name, entry, op))

print("Activate — all exact-string candidates:")
for sva in ["0x00d389a8", "0x00d49b3f", "0x00d4dd5e", "0x00d76fc2", "0x00d7830b", "0x00dc917d"]:
    ptr_le = struct.pack("<I", int(sva, 16)).hex()
    o = r2("e search.in=io.maps; /x " + ptr_le)
    for h in re.findall(r"(0x[0-9a-f]+) hit", o):
        if 0xEB0000 <= int(h, 16) < 0xED0000:
            b = bytes.fromhex(r2("p8 16 @ " + h).strip())
            print("  string@%s entry@%s opcode=0x%04X" % (sva, h, struct.unpack("<H", b[8:10])[0]))

print("Lock — table scan for opcode 0x1072 (bytes dumped by r2):")
dump = r2("p8 0x20000 @ 0xEB0000").strip()
tbl = bytes.fromhex(dump)
for off in range(0, len(tbl) - 16, 4):
    op = struct.unpack("<H", tbl[off + 8:off + 10])[0]
    if op == 0x1072:
        name_ptr = struct.unpack("<I", tbl[off:off + 4])[0]
        nm = r2("ps @ " + format(name_ptr, '#x')).strip()
        print("  entry VA 0x%X name=%r" % (0xEB0000 + off, nm))
