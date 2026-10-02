#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# Defaults target the Velxio container, where nginx listens on :80. From the
# host set VELXIO_HTTP/VELXIO_WS to the published port, e.g. :3080.
VELXIO_HTTP="${VELXIO_HTTP:-http://localhost}"
VELXIO_WS="${VELXIO_WS:-ws://localhost}"

make -C test
./scripts/build.sh

curl -fsS "$VELXIO_HTTP/health" >/dev/null

python3 -m tests.velxio.runner.run_scenario \
  --server "$VELXIO_WS" \
  --firmware build/firmware.merged.bin \
  --diagram tests/velxio/diagram.json \
  --scenario tests/velxio/scenarios/uc1_button_toggle.yaml
