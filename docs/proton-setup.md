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
        └── bridge.dll    (injected via WINEDLLOVERRIDES)
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

# Build bridge.dll (optional — prebuilt available in CI artifacts)
cargo build --release --target x86_64-pc-windows-gnu -p ashfall-bridge
```

## Proton Setup

### 1. Copy bridge.dll

```bash
# Fallout 3 (Steam)
FALLOUT3_DIR="$HOME/.steam/steam/steamapps/common/Fallout 3 goty"
cp target/x86_64-pc-windows-gnu/release/bridge.dll "$FALLOUT3_DIR/"

# Fallout: New Vegas
FNV_DIR="$HOME/.steam/steam/steamapps/common/Fallout New Vegas"
cp target/x86_64-pc-windows-gnu/release/bridge.dll "$FNV_DIR/"
```

### 2. Launch game with DLL override

```bash
# Steam launch options for Fallout 3:
#   WINEDLLOVERRIDES="bridge=n,b" %command%

# Or from terminal:
WINEDLLOVERRIDES="bridge=n,b" \
  steam steam://rungameid/22370
```

`bridge=n,b` means: load bridge as **n**ative (not builtin), and load **b**oth native and builtin for other DLLs.

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
name = Wanderer
master = 127.0.0.1     ; or public master server

[ipc]
mode = proton           ; proton | native | stub
port = 1771             ; bridge.dll TCP port

[server]
address = 127.0.0.1
port = 1770
```

Server config (`~/.config/ashfall/server.ini`):

```ini
[server]
host = 0.0.0.0
port = 1770
connections = 4
announce = 127.0.0.1    ; master server address

[scripts]
path = ./scripts

[database]
path = ./data/fallout3.sqlite3
```

## Steam Deck

Same as desktop. Proton version ≥ 9 recommended. Bridge DLL works identically.

```bash
# On Steam Deck (Desktop Mode terminal):
# 1. Install Rust + mingw-w64 (pacman)
# 2. Build as above
# 3. Copy bridge.dll to game directory on SD card or internal
# 4. Set launch options in Steam
# 5. Launch ashfall-client from terminal or add as non-Steam game
```

## Troubleshooting

### bridge.dll doesn't load
- Check `WINEDLLOVERRIDES` spelling: `bridge=n,b` (comma, no spaces)
- Verify bridge.dll is in the same directory as Fallout3.exe
- Run with `WINEDEBUG=+loaddll` to see DLL load logs

### Client can't connect to bridge
- Ensure game is running and past the main menu
- Check port: `ss -tlnp | grep 1771` should show LISTEN on 127.0.0.1
- Firewall not an issue — TCP on loopback never leaves the machine

### Game crashes on startup
- bridge.dll hooks are stubs by default (no VTable patching yet)
- Remove bridge.dll or set `WINEDLLOVERRIDES=""` to bypass
- Check Proton logs: `PROTON_LOG=1 %command%`

### Build fails for x86_64-pc-windows-gnu
- Install MinGW-w64: `sudo apt install mingw-w64`
- Or skip bridge build and use stub mode: set `mode = stub` in client config
- Stub mode returns canned responses — enough for client/server development

## Development (no game engine)

```ini
# ~/.config/ashfall/client.ini
[ipc]
mode = stub    # Canned position/angle/state responses
```

The client runs standalone, connects to server, and uses fake position data. Full client+server development without the game.
