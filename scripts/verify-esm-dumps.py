#!/usr/bin/env python3
"""Cross-check Ashfall's ESM import against the vaultmp data3 dump corpus.

The vaultmp repo ships precomputed FO3 ESM data dumps (other/data3) —
an independent third source for the game's record counts. The numbers are
known-good: REFR+ACHR+ACRE = 573,610 for the base game, matching Ashfall's
own python-walker verification (README).

Usage:
    python3 scripts/verify-esm-dumps.py --dumps /path/to/data3 --db data/fallout3.sqlite3

Prints a per-table comparison and exits non-zero on mismatch.
"""

import argparse
import glob
import os
import sqlite3
import sys

# Dump glob patterns → (table, expected_db_column)
# dumps are grouped by DLC prefix (main, bsn, mz, oa, pl, tp).
DUMP_PATTERNS = {
    "weapons": ("*_weapons.txt", "weapons"),
    "npcs": ("*_npc.txt", "npcs"),
    "refs": ("*_refr.txt", "refs"),   # REFR only; ACHR/ACRE added below
    "refs_actors": ("*_achr.txt", "refs"),
    "refs_actors2": ("*_acre.txt", "refs"),
    "races": ("*_races.txt", "races"),
    "terminals": ("*_TERM.txt", "terminals"),
}


def count_lines(files):
    total = 0
    for f in files:
        with open(f, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if line and not line.startswith("#"):
                    total += 1
    return total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dumps", required=True, help="vaultmp other/data3 dir")
    ap.add_argument("--db", required=True, help="ashfall sqlite db")
    ap.add_argument("--skip-db", action="store_true", help="dump totals only")
    args = ap.parse_args()

    if not os.path.isdir(args.dumps):
        sys.exit(f"dumps dir not found: {args.dumps}")

    # ACHR + ACRE are REFR-shaped records — include them in the refs count.
    refs_files = []
    for key, (pat, _) in DUMP_PATTERNS.items():
        if key == "refs":
            refs_files = sorted(glob.glob(os.path.join(args.dumps, pat)))
    achr = sorted(glob.glob(os.path.join(args.dumps, DUMP_PATTERNS["refs_actors"][0])))
    acre = sorted(glob.glob(os.path.join(args.dumps, DUMP_PATTERNS["refs_actors2"][0])))

    dump_counts = {
        "weapons": count_lines(sorted(glob.glob(os.path.join(args.dumps, "*_weapons.txt")))),
        "npcs": count_lines(sorted(glob.glob(os.path.join(args.dumps, "*_npc.txt")))),
        "refs": count_lines(refs_files + achr + acre),
        "races": count_lines(sorted(glob.glob(os.path.join(args.dumps, "*_races.txt")))),
        "terminals": count_lines(sorted(glob.glob(os.path.join(args.dumps, "*_TERM.txt")))),
    }

    print(f"{'table':12s} {'dump':>9s} {'db':>9s} {'delta':>9s}")
    mismatched = False
    for table in ["weapons", "npcs", "refs", "races", "terminals"]:
        db_count = None
        if not args.skip_db and os.path.exists(args.db):
            con = sqlite3.connect(args.db)
            db_count = con.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
            con.close()
        elif os.path.exists(args.db):
            db_count = sqlite3.connect(args.db).execute(
                f"SELECT COUNT(*) FROM {table}"
            ).fetchone()[0]

        d = dump_counts[table]
        if db_count is None:
            print(f"{table:12s} {d:9d} {'—':>9s} {'—':>9s}")
            continue
        delta = db_count - d
        flag = ""
        # npcs: dump counts NPC_ only; DB also imports CREA → expect db >= dump.
        if table == "npcs":
            if delta < 0:
                flag = "  <-- ERROR"
                mismatched = True
        elif delta != 0:
            flag = "  <-- MISMATCH"
            mismatched = True
        print(f"{table:12s} {d:9d} {db_count:9d} {delta:+9d}{flag}")

    if not os.path.exists(args.db):
        print(f"\nNOTE: {args.db} not found — import an ESM first, then re-run.",
              file=sys.stderr)
        return 0
    return 1 if mismatched else 0


if __name__ == "__main__":
    sys.exit(main())
