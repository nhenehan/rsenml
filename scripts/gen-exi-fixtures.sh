#!/usr/bin/env bash
# Reference EXI bytes from EXIficient 1.0.7 for every rsenml test case.
#
# Each case (crates/rsenml-tests: RFC examples and synthetic packs) is written as
# SenML XML by rsenml-xml, then encoded by EXIficient through the oracle of the
# pinned exip (utils/xsd2exip in the cache of fetch-third-party.sh) with the RFC 8428 section 8
# settings: strict, options in the header, schemaId "a"; bit-packed ("bp") and
# byte-aligned ("ba"). Output: crates/rsenml-tests/fixtures/exificient/
#   <case>.xml  <case>.bp.exi  <case>.ba.exi  manifest.tsv
#
#   scripts/gen-exi-fixtures.sh           regenerate
#   scripts/gen-exi-fixtures.sh --check   exit 1 if the committed fixtures differ
#
# Env: RSENML_ORACLE_JAR  use this oracle jar instead of building the pinned one
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/crates/rsenml-tests/fixtures/exificient"
ORACLE_DIR="$("$ROOT/scripts/fetch-third-party.sh" --print-dir)/exip/utils/xsd2exip"
JAR="${RSENML_ORACLE_JAR:-$ORACLE_DIR/target/xsd2exip-cli.jar}"
XSD="$ROOT/spec/senml.xsd"

CHECK=0
case "${1:-}" in
    --check) CHECK=1 ;;
    "") ;;
    *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

for cmd in java cargo sha256sum; do
    command -v "$cmd" >/dev/null || { echo "missing required command: $cmd" >&2; exit 1; }
done
if [[ ! -f "$JAR" ]]; then
    [[ -d "$ORACLE_DIR" ]] || { echo "run scripts/fetch-third-party.sh first" >&2; exit 1; }
    command -v mvn >/dev/null || { echo "missing required command: mvn" >&2; exit 1; }
    echo "building the oracle jar with maven ($ORACLE_DIR)" >&2
    mvn -q -f "$ORACLE_DIR/pom.xml" -DskipTests package >&2
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/out"

cargo run -q --manifest-path "$ROOT/Cargo.toml" -p rsenml-tests --bin fixture-xml -- "$WORK/out"

oracle() {
    local err="$WORK/oracle.err" rc=0
    java -jar "$JAR" "$@" 2>"$err" || rc=$?
    if [[ $rc -ne 0 ]]; then
        grep -v SLF4J "$err" >&2 || true
        echo "oracle failed (exit $rc): $*" >&2
        exit 1
    fi
}

{
    printf 'case\tprofile\tbytes\tsha256\toracle\n'
    while read -r name; do
        for profile in bp ba; do
            alignment=bit-packed
            [[ $profile == ba ]] && alignment=byte-aligned
            exi="$WORK/out/$name.$profile.exi"
            oracle encode-xml --xsd "$XSD" --xml "$WORK/out/$name.xml" --out "$exi" \
                --strict --include-options --schema-id a --alignment "$alignment"
            printf '%s\t%s\t%s\t%s\t%s\n' "$name" "$profile" "$(stat -c %s "$exi")" \
                "$(sha256sum "$exi" | cut -c1-64)" "EXIficient 1.0.7"
        done
    done <"$WORK/out/cases.txt"
} >"$WORK/out/manifest.tsv"
rm "$WORK/out/cases.txt"

if [[ $CHECK -eq 1 ]]; then
    if diff -r "$OUT" "$WORK/out" >/dev/null; then
        echo "fixtures up to date"
    else
        diff -r "$OUT" "$WORK/out" >&2 || true
        echo "fixtures are stale: run $0" >&2
        exit 1
    fi
else
    rm -rf "$OUT"
    mkdir -p "$(dirname "$OUT")"
    cp -r "$WORK/out" "$OUT"
    echo "wrote $(find "$OUT" -type f | wc -l) files to $OUT"
fi
