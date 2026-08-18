#!/usr/bin/env python3
"""Bridge runtime probe — talks the ashfall-bridge pipe protocol.

Run against a live game process (the bridge TCP server on 127.0.0.1:1771
inside Proton/Wine — Wine maps Windows loopback to the host's, so connect
from the Linux side or through an SSH tunnel).

Wire format (length-prefixed, matches ashfall_core::event::encode_frame):
  command:  [len:2B LE][0x02][key:4B LE][func:4B LE][param_count:1B][params...]
  response: [len:2B LE][0x03][key:4B LE][result...]
  OP_DUMP_IMAGE result: [0x04][size:4B LE][image bytes]

Usage:
  bridge_probe.py --action probe   --host H --port 1771
  bridge_probe.py --action dump    --host H --port 1771 --out /tmp/image.bin
  bridge_probe.py --action dead    --host H --port 1771     # stub-path sanity
  bridge_probe.py --action pos     --host H --port 1771     # vtable-path (needs table match)
"""

import argparse
import socket
import struct

PIPE_OP_COMMAND = 0x02
PIPE_OP_RETURN = 0x03
PIPE_OP_RETURN_BIG = 0x04

OP_GET_POS = 0x0001
OP_GET_DEAD = 0x0019
OP_GET_ACTOR_STATE = 0x0007
OP_DUMP_IMAGE = 0x00FC
OP_PROBE_CODE = 0x00FD
OP_PROBE_PTR = 0x00FA

# Steam respawn-flag globals (see docs/steam-re.md):
# [0x123C5D4] -> struct ptr, byte +2 = respawn flag; 0x1228871 = death-handled.
STEAM_RESPAWN_STRUCT_G = 0x123C5D4
STEAM_DEATH_HANDLED = 0x1228871


def cmd(conn, func, params=b"", key=7):
    # Length-prefixed pipe frame (matches ashfall_core::event::encode_frame):
    #   [len:2 LE][opcode][payload]  payload = [key:4 LE][func:4 LE][count:1][params...]
    payload = struct.pack("<I", key) + struct.pack("<I", func) + bytes([len(params)]) + params
    frame = struct.pack("<H", len(payload)) + bytes([PIPE_OP_COMMAND]) + payload
    conn.sendall(frame)
    # Response is one length-prefixed frame: [len:2][opcode][key:4][result...]
    data = b""
    while len(data) < 3:
        chunk = conn.recv(65536)
        if not chunk:
            return b""
        data += chunk
    ln = struct.unpack("<H", data[0:2])[0]
    while len(data) < 3 + ln:
        chunk = conn.recv(1 << 20)
        if not chunk:
            break
        data += chunk
    body = data[3:3 + ln]
    # body = [key:4][result...]; return the result (after the key).
    return body[4:] if len(body) >= 4 else b""


def action_probe(conn):
    """OP_PROBE_CODE: 16 bytes at each classic-table address. Compare against
    the GOG/classic build to see what the Steam build holds there."""
    raw = cmd(conn, OP_PROBE_CODE)
    # Result: [addr:4][16B] x10
    print(f"probe returned {len(raw)} bytes")
    for i in range(0, len(raw), 20):
        if i + 20 > len(raw):
            break
        addr = struct.unpack("<I", raw[i:i+4])[0]
        blob = raw[i+4:i+20]
        print(f"  0x{addr:08x}: {' '.join(f'{b:02x}' for b in blob)}")


def action_dump(conn, out):
    """OP_DUMP_IMAGE: full unpacked image of the game process."""
    raw = cmd(conn, OP_DUMP_IMAGE)
    if not raw:
        print("no response")
        return 1
    if raw[0] == PIPE_OP_RETURN_BIG and len(raw) >= 5:
        size = struct.unpack("<I", raw[1:5])[0]
        blob = raw[5:5+size]
        with open(out, "wb") as fh:
            fh.write(blob)
        print(f"saved {size} bytes -> {out}")
        return 0
    if raw[0] == 0x05:
        print(f"dump failed in-process (err {raw[1:5].hex() if len(raw) > 1 else '?'}) — game not unpacked?")
        return 1
    print(f"unexpected response: {raw[:32].hex()}")
    return 1


def action_probe_ptr(conn, addr):
    """OP_PROBE_PTR: 16 dwords at an address."""
    raw = cmd(conn, OP_PROBE_PTR, struct.pack("<I", addr))
    if len(raw) < 64:
        print(f"probe_ptr(0x{addr:x}) -> {raw.hex()}")
        return
    dw = struct.unpack("<16I", raw[:64])
    print(f"0x{addr:08x}: " + " ".join(f"{d:08x}" for d in dw))


def action_respawn(conn):
    """Steam respawn-flag state: struct ptr, flag byte (+2), death-handled."""
    raw = cmd(conn, OP_PROBE_PTR, struct.pack("<I", STEAM_RESPAWN_STRUCT_G))
    if len(raw) < 4:
        print(f"no respawn-struct response ({raw.hex()})")
        return
    struct_ptr = struct.unpack("<I", raw[:4])[0]
    print(f"[0x{STEAM_RESPAWN_STRUCT_G:08x}] -> struct 0x{struct_ptr:x}")
    if struct_ptr:
        r2 = cmd(conn, OP_PROBE_PTR, struct.pack("<I", struct_ptr))
        if len(r2) >= 4:
            d0 = struct.unpack("<I", r2[:4])[0]
            flag = (d0 >> 16) & 0xFF
            print(f"  respawn flag (struct+2) = {flag} ({'SET!' if flag else 'clear'})")
    r3 = cmd(conn, OP_PROBE_PTR, struct.pack("<I", STEAM_DEATH_HANDLED & ~3))
    if len(r3) >= 4:
        d = struct.unpack("<I", r3[:4])[0]
        off = STEAM_DEATH_HANDLED & 3
        dh = (d >> (8 * off)) & 0xFF
        print(f"  death-handled flag 0x{STEAM_DEATH_HANDLED:x} = {dh} ({'set' if dh else 'clear'})")


# ── Wired-patch verification (2026-08-18 — guards from the bridge's own
# byte-guards; every site below is patched or called by the bridge, and its
# guard bytes must hold in the running image). The probe reads 16 dwords at
# each address via OP_PROBE_PTR and compares the first N bytes. Pass = the
# build matches the site's guard (auto-detect: the winning guard set tells
# you which build you are on). Run BEFORE exercising any patch.
VERIFY_SITES = [
    # (name, addr, expected_hex)
    ("gog_frame_hook_0x6EEB2F", 0x6EEB2F, "e80c53ffff"),
    ("gog_kill_0x71AC50", 0x71AC50, "558bec"),
    ("gog_kill_death_0x71C280", 0x71C280, "558bec"),
    ("gog_play_sound_0xBCFBB0", 0xBCFBB0, "6aff6868"),
    ("gog_ai_pred_0x6FAE90", 0x6FAE90, "568bf1"),
    ("steam_frame_hook_0x9B3D77", 0x9B3D77, "ff15e441f200"),
    ("steam_ai_pred_0x7D0A50", 0x7D0A50, "568bf1"),
    ("steam_aifix2_0x7D0AA5", 0x7D0AA5, "742a83f80374"),
    ("steam_aifix3_0x7D0AD5", 0x7D0AD5, "cccccccccc"),
    ("steam_place_at_me_0x79E556", 0x79E556, "e8255ff6ff"),
    ("steam_fire_weapon_0x7DF3F7", 0x7DF3F7, "e88414f9ff"),
    ("steam_respawn_A_0x9C43A5", 0x9C43A5, "7503"),
    ("steam_respawn_B_0x8C9CE0", 0x8C9CE0, "0f8577000000"),
    ("steam_respawn_B2_0x8C9D52", 0x8C9D52, "c6400201"),
    # newly wired 2026-08-18 (Ghidra decompile-derived):
    ("steam_kill_0x7F3200", 0x7F3200, "558bec51"),
    ("steam_kill_death_0x7D4F40", 0x7D4F40, "558bec83"),
    ("steam_kill_handler_0x798800", 0x798800, "558bec83"),
    ("steam_match_race_0x6F7220", 0x6F7220, "8b82100100003b8110010000750c"),
    ("steam_play_sound_0x9CC980", 0x9CC980, "558bec6aff"),
    ("fnv_frame_hook_0x86B386", 0x86B386, "e82511bdff"),
    ("fnv_kill_0x8B86E0", 0x8B86E0, "558bec83ec08"),
    ("fnv_lock_setter_0x60CA30", 0x60CA30, "558bec51"),
    ("fnv_lock_handler_0x5C7280", 0x5C7280, "558bec83"),
    ("fnv_play_sound_0x5C4B30", 0x5C4B30, "538bdc83"),
]


def action_verify(conn):
    """Byte-verify every wired patch site against the running image.

    Pass = the guard bytes hold. The matching guard set identifies the
    build (gog_*/steam_*/fnv_*). This is the FIRST thing to run on a live
    session — it proves the loaded build matches the tables before any
    patch is exercised.
    """
    passed = failed = 0
    for name, addr, exp in VERIFY_SITES:
        raw = cmd(conn, OP_PROBE_PTR, struct.pack("<I", addr))
        if len(raw) < 64:
            print(f"  FAIL {name}: short read ({raw.hex()})")
            failed += 1
            continue
        dw = struct.unpack("<16I", raw[:64])
        blob = b"".join(d.to_bytes(4, "little") for d in dw)
        want = bytes.fromhex(exp)
        got = blob[: len(want)]
        if got == want:
            print(f"  PASS {name}")
            passed += 1
        else:
            print(f"  FAIL {name}: want {exp} got {got.hex()}")
            failed += 1
    print(f"== {passed} passed, {failed} failed")
    # build guess: count PASSes per guard family
    fams = {"gog": 0, "steam": 0, "fnv": 0}
    for name, addr, exp in VERIFY_SITES:
        raw = cmd(conn, OP_PROBE_PTR, struct.pack("<I", addr))
        if len(raw) >= 64:
            dw = struct.unpack("<16I", raw[:64])
            blob = b"".join(d.to_bytes(4, "little") for d in dw)
            if blob[: len(bytes.fromhex(exp))] == bytes.fromhex(exp):
                for fam in fams:
                    if name.startswith(fam + "_"):
                        fams[fam] += 1
    total = sum(fams.values())
    if total:
        print("guard family match: " + ", ".join(f"{k} {v}/{total}" for k, v in fams.items() if v)
              + (" -> " + max(fams, key=fams.get) if total else ""))


def action_dead(conn):
    """Stub-path command — safe everywhere; proves the pipe round-trip."""
    raw = cmd(conn, OP_GET_DEAD, struct.pack("<I", 0x14))
    print(f"OP_GET_DEAD(0x14) -> {raw.hex()}")


def action_pos(conn):
    """Vtable-path command — only safe when the address table matches the
    build (classic/GOG). Crashes the game on a mismatched Steam build."""
    raw = cmd(conn, OP_GET_POS, struct.pack("<I", 0x14))
    if len(raw) >= 12:
        x, y, z = struct.unpack("<fff", raw[:12])
        print(f"OP_GET_POS(0x14) -> ({x:.1f}, {y:.1f}, {z:.1f})")
    else:
        print(f"OP_GET_POS(0x14) -> {raw.hex()} (len {len(raw)})")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--action", required=True, choices=["probe", "dump", "dead", "pos", "ptr", "respawn", "verify"])
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=1771)
    ap.add_argument("--out", default="/tmp/fo3-image.bin")
    ap.add_argument("--addr", type=lambda s: int(s, 0), default=0)
    args = ap.parse_args()

    conn = socket.create_connection((args.host, args.port), timeout=300)
    try:
        if args.action == "probe":
            action_probe(conn)
        elif args.action == "dump":
            return action_dump(conn, args.out)
        elif args.action == "dead":
            action_dead(conn)
        elif args.action == "pos":
            action_pos(conn)
        elif args.action == "ptr":
            action_probe_ptr(conn, args.addr)
        elif args.action == "respawn":
            action_respawn(conn)
        elif args.action == "verify":
            action_verify(conn)
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
