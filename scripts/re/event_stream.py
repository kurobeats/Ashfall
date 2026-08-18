#!/usr/bin/env python3
"""Dump bridge event frames (spawn/remove/state/fire) from the live game."""
import socket, struct, sys, time

HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 1771
DURATION = float(sys.argv[3]) if len(sys.argv) > 3 else 30.0

EV = {7: "PLAYER_STATE", 8: "NPC_SPAWN", 9: "NPC_REMOVE", 10: "NPC_STATE", 11: "ACTIVATE", 12: "FIRE"}

s = socket.create_connection((HOST, PORT))
s.settimeout(2.0)
buf = b""
counts = {}
start = time.time()
refs = {}
try:
    while time.time() - start < DURATION:
        try:
            c = s.recv(65536)
        except socket.timeout:
            c = b""
        if not c:
            continue
        buf += c
        while len(buf) >= 3:
            ln = struct.unpack("<H", buf[0:2])[0]
            if len(buf) < 3 + ln:
                break
            op, payload = buf[2], buf[3:3 + ln]
            buf = buf[3 + ln:]
            if op != 0x07:  # PIPE_OP_EVENT
                print(f"op={op:#x} len={ln}")
                continue
            et = struct.unpack("<I", payload[:4])[0] if len(payload) >= 4 else -1
            name = EV.get(et, f"EV{et}")
            counts[name] = counts.get(name, 0) + 1
            if et in (8, 9) and len(payload) >= 12:
                ref, base = struct.unpack("<II", payload[4:12])
                refs.setdefault(name, set()).add(ref)
                print(f"{time.time()-start:6.1f}s {name} ref={ref:#010x} base={base:#010x}")
            elif et == 10 and len(payload) >= 16:
                ref = struct.unpack("<I", payload[4:8])[0]
                print(f"{time.time()-start:6.1f}s NPC_STATE ref={ref:#010x} len={ln}")
            elif et == 12:
                print(f"{time.time()-start:6.1f}s FIRE len={ln}")
            elif et == 7:
                print(f"{time.time()-start:6.1f}s PLAYER_STATE len={ln}")
finally:
    print("=== totals:", counts)
    for k, v in refs.items():
        print(f"  {k} distinct refs: {len(v)}")
