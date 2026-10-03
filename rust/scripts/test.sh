#!/usr/bin/env bash
# Host cargo tests + firmware build + UC-1 emulator scenario.
set -euo pipefail
cd "$(dirname "$0")/.." # rust/

RUST_ROOT="${RUST_ROOT:-/opt/esp-rust}"
export RUSTUP_HOME="${RUSTUP_HOME:-$RUST_ROOT/rustup}"
export CARGO_HOME="${CARGO_HOME:-$RUST_ROOT/cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

VELXIO_WS="${VELXIO_WS:-ws://localhost}"
REPO_ROOT="$(cd .. && pwd)"

echo "== host: cargo test"
cargo test

echo "== firmware: build"
./scripts/build.sh

echo "== emulator: rust scenarios"
for scenario in scenarios/*.yaml; do
  name="$(basename "$scenario" .yaml)"
  echo "-- $name"
  ( cd "$REPO_ROOT" && python3 -m tests.velxio.runner.run_scenario \
      --server "$VELXIO_WS" \
      --firmware rust/dist/firmware.merged.bin \
      --diagram tests/velxio/diagram.json \
      --scenario "rust/$scenario" )
done
