#!/usr/bin/env python3
"""Cross-check Ashfall's ESM import against the vaultmp data3 dump corpus.

The vaultmp repo ships precomputed FO3 data dumps (other/data3) — an
independent third source. The dumps have two quirks this script handles:

1. **Zone files overlap**: files are keyed by region (main, bsn, mz, ...),
   not by plugin — the same form appears in several files, so lines are
   deduplicated before comparing.
2. **Placeholder mod indices**: dump formIDs keep GECK-authored index bytes
   (all DLCs share 0x01). The DB uses distinct assigned load-order indices
   (--import-index). Comparison is therefore on the low 24 bits only:
   every dump form must exist in the DB; the DB may have *more* rows
   (cross-DLC disambiguation), which is expected, not an error.

Usage:
    python3 scripts/verify-esm-dumps.py --dumps /path/to/data3 --db data/fallout3.sqlite3

Exits non-zero if any dump form is missing from the DB.
"""

import argparse
import glob
import os
import re
import sqlite3
import sys

FORMID = re.compile(r"^0x([0-9a-fA-F]{1,8})$")

# (dump glob, formID field index, db table)
DUMP_TABLES = {
    "weapons": ("*_weapons.txt", 0, "weapons"),
    "npcs": ("*_npc.txt", 0, "npcs"),
    "refs": ("*_refr.txt", 2, "refs"),
    "refs_actors": ("*_achr.txt", 2, "refs"),
    "refs_actors2": ("*_acre.txt", 2, "refs"),
    "races": ("*_races.txt", 0, "races"),
    "terminals": ("*_TERM.txt", 1, "terminals"),
}


def dump_formids(dumps_dir):
    """formID-field index per table name → set of low-24-bit formIDs."""
    out = {}
    for table, (pat, field, _) in DUMP_TABLES.items():
        ids = set()
        for f in glob.glob(os.path.join(dumps_dir, pat)):
            with open(f, encoding="utf-8", errors="replace") as fh:
                for line in fh:
                    line = line.strip()
                    if not line or line.startswith("#"):
                        continue
                    parts = line.split("|")
                    if len(parts) <= field:
                        continue
                    m = FORMID.match(parts[field].strip())
                    if not m:
                        continue
                    ids.add(int(m.group(1), 16) & 0xFFFFFF)
        out[table] = ids
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dumps", required=True, help="vaultmp other/data3 dir")
    ap.add_argument("--db", required=True, help="ashfall sqlite db")
    ap.add_argument("--skip-db", action="store_true", help="dump totals only")
    args = ap.parse_args()

    if not os.path.isdir(args.dumps):
        sys.exit(f"dumps dir not found: {args.dumps}")

    dump = dump_formids(args.dumps)

    # refs = REFR + ACHR + ACRE (all REFR-shaped records).
    dump_sets = {
        "weapons": dump["weapons"],
        "npcs": dump["npcs"],
        "refs": dump["refs"] | dump["refs_actors"] | dump["refs_actors2"],
        "races": dump["races"],
        "terminals": dump["terminals"],
    }

    print(f"{'table':12s} {'dump':>8s} {'db':>8s} {'missing':>8s} {'extra':>8s}")
    missing_any = False
    for table, dump_ids in dump_sets.items():
        db_ids = None
        if os.path.exists(args.db):
            con = sqlite3.connect(args.db)
            db_ids = {
                r[0] & 0xFFFFFF
                for r in con.execute(f"SELECT baseID FROM {table}").fetchall()
            } if table != "refs" else {
                r[0] & 0xFFFFFF
                for r in con.execute("SELECT refID FROM refs").fetchall()
            }
            con.close()

        if db_ids is None:
            print(f"{table:12s} {len(dump_ids):8d} {'—':>8s} {'—':>8s} {'—':>8s}")
            continue

        missing = dump_ids - db_ids
        extra = db_ids - dump_ids
        flag = ""
        if missing:
            flag = "  <-- MISSING"
            missing_any = True
        print(
            f"{table:12s} {len(dump_ids):8d} {len(db_ids):8d} "
            f"{len(missing):8d} {len(extra):8d}{flag}"
        )

    if not os.path.exists(args.db):
        print(f"\nNOTE: {args.db} not found — import an ESM first, then re-run.",
              file=sys.stderr)
        return 0

    if missing_any:
        print("\nImport is INCOMPLETE — missing dump forms above.", file=sys.stderr)
        return 1
    print("\nOK: every dump form is present in the DB. "
          "(npcs: dump counts NPC_ only; CREA rows show as extras.)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
