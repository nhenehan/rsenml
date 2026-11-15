#!/usr/bin/env bash
# Runs every program of the guide, in the guide's order.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
cargo build -q --bins
for bin in first_pack formats resolve validate stream fixed_buffer errors; do
    echo "== $bin"
    cargo run -q --bin "$bin"
done
