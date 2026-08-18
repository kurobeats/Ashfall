# Ashfall game modes (WASM)

The server loads every `.wasm` module from its `scripts/` directory
(default `~/.config/ashfall/scripts` or the `[scripts] path` config).
Modes run side-by-side — each exports callbacks the server invokes
(auth, chat, spawn, death, quest stages, …); host functions let them
drive server state (chat, weather, time, kill/resurrect, UI messages).

## Modes

| Mode | What it demonstrates | Path |
|------|---------------------|------|
| freeroam | Base mode: spawn, auth, chat relay, `!pvp` / `!time` / `!weather` / `!resurrect` commands, friendly-fire rule (on_hit gate) | `scripts/freeroam` |
| shared-quest | Server-wide shared quest: all players' NPC kills count toward one quest, progress broadcast, stage advance (3 → 5 → 8 kills), player deaths don't count. `!quest` shows progress, `!reset` resets | `scripts/shared-quest` |

## Building

```bash
# wasm32 target (once)
rustup target add wasm32-unknown-unknown

cargo build --manifest-path scripts/freeroam/Cargo.toml \
    --target wasm32-unknown-unknown --release
cargo build --manifest-path scripts/shared-quest/Cargo.toml \
    --target wasm32-unknown-unknown --release

# deploy: copy the .wasm files to the server's scripts dir
cp scripts/freeroam/target/wasm32-unknown-unknown/release/ashfall_freeroam.wasm \
   scripts/shared-quest/target/wasm32-unknown-unknown/release/ashfall_shared_quest.wasm \
   ~/.config/ashfall/scripts/
```

## Testing

`crates/ashfall-server/tests/script_runtime.rs` exercises the engine with
inline WAT (hermetic, no toolchain needed). `test_shared_quest_mode_real_wasm`
loads the **actual built** shared-quest wasm and asserts the quest flow
(stage completion broadcasts, player-death no-count) — it skips when the
wasm hasn't been compiled, so the default workspace run stays hermetic.

## Writing a mode

Mirror the freeroam structure: declare host imports with
`#[link(wasm_import_module = "env")]` (names must match
`crates/ashfall-server/src/script/host.rs`) and export the callbacks the
engine dispatches (see `crates/ashfall-server/src/script/engine.rs`).
The `ashfall-script` crate provides the `host_fn!` macros + string
helpers. `crates/ashfall-script/src/lib.rs` documents the ABI.
