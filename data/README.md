# data/ — layout, what's tracked, how to regenerate

```
data/
├── fallout3/          # FO3 files (GOG 1.7.0.3) + its server database — NOT tracked
│   ├── Fallout3.exe   # 32-bit game exe (static-analysis reference)
│   ├── Fallout3.esm   # base master (276MB)
│   ├── Anchorage.esm, BrokenSteel.esm, ThePitt.esm, PointLookout.esm, Zeta.esm
│   └── fallout3.sqlite3   # imported DB — server default (see config [database] path)
├── falloutnv/         # FNV files (GOG 1.4.0.525(a)) + its server database — NOT tracked
│   ├── FalloutNV.exe
│   ├── FalloutNV.esm
│   ├── DeadMoney.esm, HonestHearts.esm, OldWorldBlues.esm, LonesomeRoad.esm,
│   │   GunRunnersArsenal.esm, CaravanPack.esm, ClassicPack.esm, MercenaryPack.esm,
│   │   TribalPack.esm
│   └── falloutnv.sqlite3
└── plugins/           # vendored NVMP server-mod ESPs — TRACKED (reference corpus)
    └── NVMP-Q.esp, NVMP_-_Pres.esp, NVMP_-_Rebalance.esp, NVMP_-_Wachter.esp
```

## Why the game files and databases aren't in git

- **Game files** (`.esm`, `.exe`): copyrighted Bethesda data, hundreds of MB.
  They are runtime prerequisites, not project sources. Get them from GOG
  (`innoextract` the installer) or Steam, then drop them in the per-game dir.
- **Databases** (`*.sqlite3`): runtime artifacts, regenerable in seconds from
  the `.esm` files (import is one transaction). Gitignored so a checkout
  doesn't carry 35MB of derived data. The verified reference numbers are
  documented in the README (records/weapons/NPCs/refs per game).

`.gitignore` tracks only `data/plugins/` and this file.

## Regenerate a database

```bash
# FO3 — base first (index 0), then each DLC with a distinct load-order index
cargo run -p ashfall-server -- --import-esm data/fallout3/Fallout3.esm \
    --import-game fo3 --import-db data/fallout3/fallout3.sqlite3 --import-index 0
cargo run -p ashfall-server -- --import-esm data/fallout3/ThePitt.esm \
    --import-game fo3 --import-db data/fallout3/fallout3.sqlite3 --import-index 1
# ... BrokenSteel=2, PointLookout=3, Zeta=4, Anchorage=5 (any distinct order)

# FNV
cargo run -p ashfall-server -- --import-esm data/falloutnv/FalloutNV.esm \
    --import-game fnv --import-db data/falloutnv/falloutnv.sqlite3 --import-index 0
# ... 5 story DLC + 4 packs with indices 1..9

# Verify against the vaultmp dump corpus (FO3 only — no FNV dumps exist):
python3 scripts/verify-esm-dumps.py --dumps /path/to/vaultmp/other/data3 \
    --db data/fallout3/fallout3.sqlite3
```

Verified totals (GOG builds, 2026-08-07):

| | FO3 | FNV |
|---|---|---|
| records | 124,540 | 141,502 |
| weapons | 299 | 496 |
| NPCs | 3,613 | 6,455 |
| refs | 747,106 | 427,089 |
| factions | 451 | 772 |

## Point the server at a DB

Server config (TOML, `--config <file>` or `server.ini`):

```toml
[database]
path = "./data/fallout3/fallout3.sqlite3"   # or falloutnv/falloutnv.sqlite3

[server]
game_type = "fo3"                            # or "fnv"
```

## Load-order verification (mod policy)

The server can require clients to match a load order (optional — off by
default). Print the ready-to-paste config lines for a game's files:

```bash
cargo run -p ashfall-server -- --list-mod-crc data/fallout3
# mod = "Fallout3.esm:C092218B"
# mod = "Anchorage.esm:A4BA9D10"
# ...
```

Then paste into `[server]` config:

```ini
mod = "Fallout3.esm:C092218B"
mod = "Anchorage.esm:A4BA9D10"
```

Clients whose load order differs (wrong file, order, or CRC) are rejected at
connect. CRCs are IEEE CRC-32 of the raw file bytes — the same implementation
on both sides (`ashfall_core::crc32`), verified byte-for-byte against zlib on
every file above. Empty `mod` list = no policy.
