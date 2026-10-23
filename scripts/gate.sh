#!/usr/bin/env bash
# Everything a commit must pass:
#   pins, formatting, clippy (no warnings), rustdoc, tests (also on the oldest
#   supported Rust, rust-version in Cargo.toml), no_std builds of every
#   representation for Cortex-M4F, generated EXI tables and EXIficient fixtures
#   up to date.
#
#   scripts/gate.sh           full gate
#   scripts/gate.sh --quick   skip the EXIficient check (needs Java)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
QUICK=0
[[ "${1:-}" == "--quick" ]] && QUICK=1

step() { printf '\n== %s\n' "$*"; }

step "third-party pins"
scripts/fetch-third-party.sh --check

step "rustfmt"
cargo fmt --all --check

step "clippy"
cargo clippy --workspace --all-targets --no-deps -- -D warnings

step "rustdoc (no warnings, every public item documented)"
RUSTDOCFLAGS="-D warnings" cargo doc -q --workspace --no-deps

step "tests"
cargo test --workspace -q

step "tests on the oldest supported Rust (1.88)"
# Own target directory, so the two compilers do not rebuild each other's files.
CARGO_TARGET_DIR=target/msrv cargo +1.88 test --workspace -q \
    || { echo "needs: rustup toolchain install 1.88" >&2; exit 1; }

step "no_std builds (thumbv7em-none-eabihf)"
for features in "" json cbor xml exi; do
    if [[ -z "$features" ]]; then
        args=()
        label="all representations"
    else
        args=(--no-default-features --features "$features")
        label="$features only"
    fi
    cargo build -q -p rsenml --target thumbv7em-none-eabihf "${args[@]}"
    echo "ok       $label"
done

step "generated EXI tables"
scripts/gen-tables.sh --check

if [[ $QUICK -eq 0 ]]; then
    step "EXIficient reference bytes"
    scripts/gen-exi-fixtures.sh --check 2>&1 | grep -v '^WARNING:'
fi

printf '\ngate passed\n'
