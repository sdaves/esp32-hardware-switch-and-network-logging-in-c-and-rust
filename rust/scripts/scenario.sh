#!/usr/bin/env bash
# Run one Rust emulator scenario by name: ./scripts/scenario.sh uc1_button_toggle
set -euo pipefail
cd "$(dirname "$0")/.."
NAME="${1:?usage: scenario.sh <name>}"
VELXIO_WS="${VELXIO_WS:-ws://localhost}"
REPO_ROOT="$(cd .. && pwd)"

( cd "$REPO_ROOT" && python3 -m tests.velxio.runner.run_scenario \
    --server "$VELXIO_WS" \
    --firmware rust/dist/firmware.merged.bin \
    --diagram tests/velxio/diagram.json \
    --scenario "rust/scenarios/${NAME}.yaml" )
