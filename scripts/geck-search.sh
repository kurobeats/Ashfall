#!/usr/bin/env bash
# Search the GECK function index (docs/geck/geck_function_index.txt).
# Usage: ./scripts/geck-search.sh <pattern> [pattern...]
# Format: Name | Origin | Signature | Summary | Categories
set -euo pipefail

INDEX="$(dirname "$0")/../docs/geck/geck_function_index.txt"
if [[ ! -f "$INDEX" ]]; then
  echo "index not found: $INDEX" >&2
  exit 1
fi

if [[ $# -eq 0 ]]; then
  echo "usage: $0 <pattern> [pattern...]" >&2
  exit 1
fi

# Case-insensitive grep; multiple patterns narrow the results.
grep -iE "$1" "$INDEX" | {
  if [[ $# -gt 1 ]]; then
    shift
    grep -iE "$1"
  else
    cat
  fi
}
