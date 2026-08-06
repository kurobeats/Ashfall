# Proton runtime testing — Fallout 3

Status of the bridge inside the real game (FO3 GOTY, Steam, Proton
Experimental, verified 2026-08-06 on test host — see
[scripts/re/README.md](../scripts/re/README.md) for the result matrix).

## What works

- **Injection**: `ashfall-bridge-proxy` shipped as `dinput8.dll` loads in
  the game process. `WINEDLLOVERRIDES="bridge=n,b"` does NOT work (nothing
  imports `bridge`). Full instructions: [proton-setup.md](./proton-setup.md).
- **Bridge TCP server**: `127.0.0.1:1771` LISTENING inside Fallout3.exe.
- **Pipe protocol**: wakeup (`01` → `01`) and stub-path commands
  (`OP_GET_DEAD`, `OP_GET_ACTOR_VALUE` — no engine deref) round-trip.
- **Game stability**: stable at main menu for 70s+ untouched.

## What crashes the game (verified)

Any command reaching a real vtable call while no save is loaded:

| Command | Path | Why it crashes |
|---------|------|----------------|
| `OP_GET_POS` / `OP_SET_POS` / `OP_GET_ANGLE` / `OP_GET_CELL` / `OP_GET_PARENT_CELL` | `vtable::get_pos` etc. | refID 0x14 (player) is garbage at the main menu — no player ref exists until a save loads |
| `OP_IS_MOVING` / `OP_GET_ACTOR_STATE` | `vtable::get_actor_state` | same, plus anim-struct offsets (below) are unverified — reproduced: broken pipe + game exit |

**Rule for the next session**: never send vtable-path commands at the main
menu. Only test them after a save is loaded, and expect crashes until the
offsets are re-verified — a crash is a data point, not a surprise.

## In-game verification plan

1. Launch: Steam → Fallout 3 → launcher → Enter (SteamStub DRM: must go
   through Steam; launcher is the default target and needs the Play click).
2. Load any save (player ref 0x14 now valid).
3. Pipe round trip from the host (python, no tools needed):

```python
import socket, struct
s = socket.create_connection(("127.0.0.1", 1771), timeout=10)
def cmd(op, refid=0x14):
    s.sendall(b"\x02" + struct.pack("<I", op) + struct.pack("<I", op)
              + bytes([4]) + struct.pack("<I", refid))
    return s.recv(64).hex()
print("get_dead:", cmd(0x19))          # stub path — safe everywhere
print("get_pos:", cmd(0x0001))         # vtable — needs loaded save
print("get_cell:", cmd(0x0005))        # vtable — needs loaded save
print("is_moving:", cmd(0x1B))         # vtable — needs loaded save
s.close()
```

4. Expected once offsets are right: `get_pos` returns 12 bytes (3 × f32 LE),
   `get_cell` 4 bytes (formID LE), `is_moving` 1 byte, and the game stays
   alive.

## Constants to verify (the only unverified set left)

All in `crates/ashfall-bridge/src/hooks/vtable.rs`. Verified sources are
xFOSE headers + vaultmp-extended `vaultmp.cpp` — static source, never
runtime-verified.

| Constant | Value (FO3 1.7) | Source | Verify against |
|----------|-----------------|--------|----------------|
| `VTBL_REF_GET_POS` | VTable+0x30 | xFOSE GameObjects.h | loaded save, `get_pos` = player coords |
| `VTBL_REF_GET_BASE_FORM` | VTable+0x10 | xFOSE | `get_base` = player base formID |
| `VTBL_ACTOR_GET_VALUE` | VTable+0x68 | xFOSE | `get_actor_value(0x14, 0x14)` = health |
| `VTBL_ACTOR_GET_BASE_VALUE` | VTable+0x70 (estimated) | vaultmp | base value |
| `VTBL_ACTOR_ANIM_DATA` | VTable+0x01E4 | vaultmp.cpp GetActorState | `is_moving` |
| `OFFSET_ANIM_MOVING` | 0x4E | vaultmp.cpp | `is_moving` |
| `OFFSET_ANIM_WEAPON` | 0x54 | vaultmp.cpp | weapon state |
| `OFFSET_ANIM_IDLE_PTR` | 0x118 → 0x2C → 0x0C | vaultmp.cpp | idle anim formID |
| pos/rot/parentCell offsets | 0x2C/0x30/0x34, 0x20/0x24/0x28, 0x3C | xFOSE + 3,877 `[reg+0x3C]` reads in binary | loaded save |

Re-verification method options (in order of laziness):

1. **Runtime**: load save → call each getter → compare against expected
   values (player position from the map, health from pip-boy). The
   reference implementation is vaultmp-extended, which works, so wrong
   offsets here are transcription bugs, not ground truth.
2. **Static**: r2/objdump cross-check of the VTable layout against the
   xFOSE `STATIC_ASSERT`s (already done for addresses; vtables are the
   remaining gap) — see [scripts/re](./scripts/re).

## Safe command set (stub path — usable at any time)

`OP_GET_DEAD` (0x19), `OP_GET_ACTOR_VALUE` (0x08), `OP_IS_MOVING` (0x1B,
only via `get_actor_state` — NOT safe at menu). Everything else in
`commands.rs` either stubs (returns zeros) or hits a vtable — check the
`crate::hooks::` call in the dispatch arm before sending it.
