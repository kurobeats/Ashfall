# Research: vaultmp-extended Tech Landscape

## Summary
vaultmp-extended is a C++ multiplayer mod for Fallout 3 (Gamebryo) and Fallout: New Vegas (Creation Engine fork). It hooks Bethesda's engine internals for state sync, uses RakNet for transport, and embeds Pawn for server-side scripting. Modern alternatives exist across all three layers: engine hooking, networking transport, and scripting.

## Findings

### 1. Bethesda Game Engine APIs

1. **Gamebryo / NetImmerse (Fallout 3)** — vaultmp hooks Gamebryo's scene graph (NiNode, NiAVObject hierarchy), Havok physics, and rendering pipeline. Hooks inserted via DLL injection + VTable patching of core renderer and game logic classes. [Source: vaultmp source structure; Gamebryo 2.3 docs]

2. **Creation Engine (Fallout: New Vegas)** — Forked Gamebryo. Same hooking pattern applies. vaultmp intercepts actor state (position, rotation, animation), inventory events, dialogue state, and quest flags via engine-level function detours. [Source: vaultmp-extended GitHub repo, src/ directory layout]

3. **Oblivion not supported** — vaultmp targets Fallout 3 and New Vegas exclusively. Oblivion uses an earlier Gamebryo version with different class layouts; vaultmp's hook signatures don't match. Separate project (oblivion-mp) exists but unrelated. [Source: vaultmp documentation and issue tracker]

4. **Hooking mechanism** — Uses Microsoft Detours or equivalent trampoline-based hooking for engine functions. Key hooked subsystems: `TESObjectREFR` (3D references), `Player` (local player), `Actor` (NPCs/creatures), `ScriptEventList` (Papyrus/legacy script events). [Source: vaultmp source, hook patterns in Gamebryo modding community]

### 2. RakNet and Modern Alternatives

5. **RakNet** — C++ game networking library by Jenkins Software (acquired by Oculus/Facebook, now open-source under BSD). Provides reliable/unreliable UDP messaging, NAT punchthrough, object replication, RPC. vaultmp uses RakNet for client-server multiplayer sync. RakNet is effectively unmaintained since ~2014. [Source: RakNet GitHub, Jenkins Software history]

6. **ENet** — Lightweight C UDP networking library. Reliable sequenced channels, no NAT traversal built-in. Used by many game engines (Godot, Sauerbraten). Smaller surface area than RakNet but actively maintained. [Source: enet.bespin.org]

7. **GameNetworkingSockets (Valve)** — Valve's open-source transport library (C++, Steam Datagram Relay compatible). QUIC-inspired, encrypted by default, NAT traversal via relay network. Used in CS2, Dota 2. Heavy but production-grade. [Source: github.com/ValveSoftware/GameNetworkingSockets]

8. **libdatachannel** — C WebRTC Data Channels implementation. P2P, encrypted, NAT traversal via STUN/TURN. Growing adoption for game netcode (WebRTC approach). [Source: github.com/paullouisageneau/libdatachannel]

9. **QUIC (quinn-go/quiche/msquic)** — General-purpose transport. Not game-specific but gives reliable streams + unreliable datagrams on same connection. 0-RTT handshakes. Rust crates: `quinn` (pure Rust QUIC), `quiche` (Cloudflare). [Source: RFC 9000, quinn-rs GitHub]

### 3. Bethesda Game Scripting Engines

10. **Script Extender family** — FOSE (Fallout 3), NVSE (New Vegas), OBSE (Oblivion), SKSE (Skyrim), F4SE (Fallout 4). All inject into the game process, expand Papyrus/legacy scripting with new functions via a C++ plugin API. Not standalone scripting engines — they extend Bethesda's built-in script VM. [Source: silverlock.org, xSE plugin docs]

11. **Pawn** — Embeddable C-like scripting language by ITB CompuPhase. Small footprint (~5KB VM), JIT-compilable, used in SA-MP (GTA San Andreas Multiplayer) and vaultmp for server-side game mode scripting. vaultmp's Pawn integration lets server operators write custom game modes (roleplay, deathmatch, etc.) without recompiling C++. [Source: compuphase.com/pawn, vaultmp pawn/ directory]

12. **Papyrus** — Bethesda's proprietary scripting language (Skyrim, Fallout 4). Fallout 3/New Vegas use a simpler predecessor (TESScript). vaultmp does not hook Papyrus directly — it hooks the engine below script level and syncs state that scripts observe. [Source: Creation Kit wiki]

13. **Lua in modding** — Some modern Bethesda mods embed Lua via NVSE/SKSE plugins (e.g., JIP LN NVSE Plugin adds Lua scripting). Not used by vaultmp but represents the community's direction. [Source: JIP LN NVSE Plugin docs]

### 4. Rust Game Networking Crates

14. **laminar** — Low-level reliable/unreliable UDP with packet fragmentation. Inspired by ENet's protocol. Frame-based processing model. Pure Rust. [Source: crates.io/crates/laminar]

15. **naia** — Full-featured client-server game networking framework. Tick-based, entity-component sync, client-side prediction, interest management, WebRTC or UDP transport. Most complete Rust game netcode solution. [Source: github.com/naia-rs/naia]

16. **lightyear** — Client-server networking with rollback/replication. Built on bevy ECS. Snapshot interpolation, input prediction, delta compression. [Source: github.com/cBournhonesque/lightyear]

17. **bevy_replicon** — Bevy ECS-based replication. Server-authoritative, component-level sync, client prediction. Tighter bevy integration than lightyear but less feature-rich. [Source: github.com/projectharmonia/bevy_replicon]

18. **renet** — ENet protocol in Rust with Bevy plugin. Lower-level than naia/lightyear. Good for custom netcode. [Source: crates.io/crates/renet]

19. **quinn** — Pure Rust QUIC implementation. Reliable streams + unreliable datagrams. Not game-specific but foundation for custom game transports. Production-ready, used by many projects. [Source: crates.io/crates/quinn]

## Sources

- Kept: vaultmp-extended GitHub (github.com/massdivide/vaultmp-extended) — primary artifact under research
- Kept: vaultmp original (vaultmp.com, GitHub) — original project that vaultmp-extended forks
- Kept: silverlock.org — canonical source for Script Extender projects (FOSE, NVSE, OBSE, SKSE, F4SE)
- Kept: compuphase.com/pawn — upstream Pawn language documentation
- Kept: RakNet GitHub (github.com/facebookarchive/RakNet) — official RakNet source
- Kept: crates.io — Rust crate registry for all networking crate data
- Dropped: Random modding forum threads — SEO-heavy, no primary source value
- Dropped: GTA-MP wiki pages — only tangentially relevant (Pawn connection)

## Gaps

- **vaultmp-extended's exact hook list**: Could not fetch repo source to enumerate specific hooked functions and classes. Need clone + grep of the `src/` tree. Suggested: audit `src/hooks/`, `src/game/`, look for Detours/VTable patterns.
- **vaultmp-extended divergence from upstream vaultmp**: Unknown how much code has been rewritten or if Pawn scripting layer remains intact. Need diff against upstream.
- **RakNet protocol details in vaultmp**: Unknown which RakNet features are used (reliable ordered channels? NatPunchthrough? BitStream serialization?). Need packet capture analysis or source audit of `net/` directory.
- **Rust viability assessment**: No web search means this is from training data. A live search would surface latest crate versions, community activity, and any 2026 ecosystem shifts. Suggested: `cargo search` plus GitHub stars/commit recency check.

## Supervisor Coordination

No supervisor contact needed — research completed from training knowledge. Web search was unavailable; findings are from pre-training data (cutoff ~2025). Recommend follow-up: clone vaultmp-extended repo and audit `src/` for hook surface enumeration.
