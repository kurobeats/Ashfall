# Proton / Steam Deck Setup

Ashfall server + client run natively on Linux. Fallout 3 / New Vegas runs under Proton. A cross-compiled bridge DLL connects them.

## Architecture

```
Linux Host
├── ashfall-server        (native, tokio/UDP)
├── ashfall-client        (native, egui)
│   └── IPC: TCP 127.0.0.1:1771
└── Proton/Wine
    └── Fallout3.exe
        └── dinput8.dll proxy (runs bridge init, forwards DInput)
            └── TCP server on 127.0.0.1:1771
```

## Prerequisites

```bash
# Rust targets
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu   # for bridge.dll

# MinGW cross-compiler (Debian/Ubuntu)
sudo apt install mingw-w64

# Arch
sudo pacman -S mingw-w64-gcc

# Fedora
sudo dnf install mingw64-gcc
```

## Build

```bash
# Build native binaries
cargo build --release

# Build bridge.dll (FOSE/NVSE plugin path — needs an xSE loader)
cargo build --release --target i686-pc-windows-gnu -p ashfall-bridge

# Build dinput8 proxy (recommended — no FOSE needed, verified under Proton)
cargo build --release --target i686-pc-windows-gnu -p ashfall-bridge-proxy
```

> ⚠️ FO3/FNV are **32-bit** executables, so a 64-bit DLL cannot load into
> them. Always use the `i686-pc-windows-gnu` target.

## Injection

`WINEDLLOVERRIDES="bridge=n,b"` does **not** load the bridge — DLL overrides
only apply when something *imports* that DLL, and the game imports nothing
called `bridge`. Verified against real FO3 GOTY under Proton Experimental.

Use the **dinput8 proxy** instead: the game imports `dinput8.dll`, and a
native copy in the game dir wins over wine's builtin. `DllMain` runs the
bridge init; `DirectInput8Create` forwards to wine's builtin dinput8.

```bash
cp target/i686-pc-windows-gnu/release/ashfall_bridge_proxy.dll \
  "$FALLOUT3_DIR/dinput8.dll"
```

> ⚠️ The Steam version of Fallout3.exe is **SteamStub-packed** (no import
> table). It must be launched **through Steam** — running it directly via
> `proton run` exits silently with code 0. If your game came with a launcher
> (FO3 GOTY default target is `Fallout3Launcher.exe`), press Enter on it to
> start the real game.

### Save location

Steam creates the prefix under the **game library's** compatdata (not
`~/.local/share/Steam`):

```bash
# find the real prefix:
find ~/.local/share -maxdepth 4 -type d -path "*/compatdata/22370"

# FO3 saves (.fos) go in:
$REAL_PREFIX/pfx/drive_c/users/steamuser/Documents/My Games/Fallout3/Saves/
```

`SLocalSavePath=Saves\` in FALLOUT.INI — saves live in the `Saves/`
subdir, not the Fallout3 root. A prefix created by a manual
`proton run` (default `~/.local/share/Steam/steamapps/compatdata`) is a
decoy the game never reads.

### 3. Start Ashfall

```bash
# Terminal 1: master server (optional for LAN play)
cargo run -p ashfall-master

# Terminal 2: dedicated server
cargo run -p ashfall-server

# Terminal 3: client (connects to game via TCP 127.0.0.1:1771)
cargo run -p ashfall-client
```

## Configuration

Client config (`~/.config/ashfall/client.ini`):

```ini
[general]
name = "Wanderer"
master = "127.0.0.1"     ; or public master server

[ipc]
mode = "proton"           ; proton | native | stub
port = 1771             ; bridge.dll TCP port

[server]
address = "127.0.0.1"
port = 1770
```

Server config (`~/.config/ashfall/server.ini`):

```ini
[server]
host = "0.0.0.0"
port = 1770
connections = 4
announce = "127.0.0.1"    ; master server address

[scripts]
path = "./scripts"

[database]
path = "./data/fallout3.sqlite3"
```

> ⚠️ Strings must be double-quoted (valid TOML). Unquoted values silently
> fail parsing and the server falls back to defaults (warns in the log).

## Steam Deck

Same as desktop. Proton version ≥ 9 recommended. Bridge DLL works identically.

```bash
# On Steam Deck (Desktop Mode terminal):
# 1. Install Rust + mingw-w64 (pacman)
# 2. Build as above
# 3. Copy dinput8 proxy (ashfall_bridge_proxy.dll) to game directory on SD card or internal
# 4. Set launch options in Steam
# 5. Launch ashfall-client from terminal or add as non-Steam game
```

## Troubleshooting

### dinput8 proxy doesn't load
- Verify `dinput8.dll` (built from `ashfall-bridge-proxy`) is in the same directory as Fallout3.exe
- Run with `WINEDEBUG=+loaddll` in launch options (`WINEDEBUG=+loaddll %command%`) to see DLL load logs
- `ss -tlnp | grep 1771` shows the bridge listener once loaded

### Client can't connect to bridge
- Ensure game is running and past the main menu
- Check port: `ss -tlnp | grep 1771` should show LISTEN on 127.0.0.1
- Firewall not an issue — TCP on loopback never leaves the machine

### Game crashes on startup
- bridge.dll has memory-patching primitives and VTable getters, but `hooks::install()`
  does not patch any engine addresses yet — hooks are inert
- **VTable getter commands (OP_GET_POS/GET_ACTOR_STATE/IS_MOVING etc.) crash the game
  before a save is loaded** — no player ref exists at the main menu (refID 0x14 is
  garbage) and anim-struct offsets are still unverified. Use stub-path commands
  (OP_GET_DEAD, OP_GET_ACTOR_VALUE at menu) or test with a loaded save
- Remove dinput8.dll to bypass entirely
- Check Proton logs: `PROTON_LOG=1 %command%` as a launch option

### Build fails for i686-pc-windows-gnu
- Install MinGW-w64: `sudo apt install mingw-w64` / `sudo dnf install mingw64-gcc`
- Or skip bridge build and use stub mode: set `mode = stub` in client config
- Stub mode returns canned responses — enough for client/server development

## Development (no game engine)

```ini
# ~/.config/ashfall/client.ini
[ipc]
mode = stub    # Canned position/angle/state responses
```

The client runs standalone, connects to server, and uses fake position data. Full client+server development without the game.
