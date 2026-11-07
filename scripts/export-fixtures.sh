#!/usr/bin/env bash
# Writes the conformance vectors and test cases, with rsenml's results and bytes,
# as TSV into crates/rsenml-tests/fixtures/export (compare/src/export.rs). Other
# implementations (SenML.jl) test against these files.
#
#   scripts/export-fixtures.sh           regenerate
#   scripts/export-fixtures.sh --check   exit 1 if the committed files differ
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/crates/rsenml-tests/fixtures/export"

CHECK=0
case "${1:-}" in
    --check) CHECK=1 ;;
    "") ;;
    *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
(cd "$ROOT/compare" && cargo run --release -q -- export "$WORK")

if [[ $CHECK -eq 1 ]]; then
    diff -r "$OUT" "$WORK" >/dev/null || {
        echo "fixtures/export is out of date: run scripts/export-fixtures.sh" >&2
        exit 1
    }
    echo "ok       fixtures/export"
else
    mkdir -p "$OUT"
    cp "$WORK"/vectors.tsv "$WORK"/cases.tsv "$OUT"/
    echo "wrote $OUT"
fi
