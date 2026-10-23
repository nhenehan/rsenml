#!/usr/bin/env bash
# Regenerate the EXI grammar tables of the RFC 8428 schema (spec/senml.xsd) with
# rexi-schema-gen from the pinned rexi (in the cache of fetch-third-party.sh).
#
#   scripts/gen-tables.sh           write crates/rsenml-exi/src/generated/senml.rs
#   scripts/gen-tables.sh --check   exit 1 if the committed tables are stale
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/crates/rsenml-exi/src/generated/senml.rs"
CHECK=()
case "${1:-}" in
    --check) CHECK=(--check) ;;
    "") ;;
    *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

REXI="$("$ROOT/scripts/fetch-third-party.sh" --print-dir)/rexi"
[[ -f "$REXI/Cargo.toml" ]] || { echo "run scripts/fetch-third-party.sh first" >&2; exit 1; }
mkdir -p "$(dirname "$OUT")"
cargo run -q --manifest-path "$REXI/Cargo.toml" -p rexi-schema-gen \
    --target-dir "$ROOT/target/rexi" -- \
    "$ROOT/spec/senml.xsd" --out "$OUT" --name GRAMMARS "${CHECK[@]}"
