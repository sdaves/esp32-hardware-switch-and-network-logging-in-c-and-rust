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

# The UC-5 firmware does a real Wi-Fi + HTTP GET to the slirp gateway
# (192.168.4.2), which is the container. Make sure a server is listening on
# :8000 so the fetch has something to reach.
./scripts/dev-http-server.sh start

# Run every scenario in tests/velxio/scenarios/, in filename order, so adding a
# use case's YAML automatically extends `make test`. Any failing scenario aborts
# the run (set -e + explicit exit).
scenarios=(tests/velxio/scenarios/*.yaml)
if [ ! -e "${scenarios[0]}" ]; then
  echo "no scenarios found in tests/velxio/scenarios/" >&2
  exit 2
fi

for scenario in "${scenarios[@]}"; do
  echo "== scenario: $(basename "$scenario")"
  python3 -m tests.velxio.runner.run_scenario \
    --server "$VELXIO_WS" \
    --firmware build/firmware.merged.bin \
    --diagram tests/velxio/diagram.json \
    --scenario "$scenario"
done
