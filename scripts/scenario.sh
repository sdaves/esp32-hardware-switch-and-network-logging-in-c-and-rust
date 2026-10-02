#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "$#" -lt 1 ]; then
  echo "usage: $0 <scenario-name|path> [extra run_scenario args...]" >&2
  echo "  e.g. $0 uc1_button_toggle" >&2
  exit 2
fi

NAME="$1"
shift

case "$NAME" in
  */*|*.yaml) SCENARIO="$NAME" ;;
  *) SCENARIO="tests/velxio/scenarios/${NAME}.yaml" ;;
esac

: "${VELXIO_WS:=ws://localhost}"
: "${VELXIO_FIRMWARE:=build/firmware.merged.bin}"
: "${VELXIO_DIAGRAM:=tests/velxio/diagram.json}"
: "${VELXIO_TIMEOUT:=15}"

exec python3 -m tests.velxio.runner.run_scenario \
  --server "$VELXIO_WS" \
  --firmware "$VELXIO_FIRMWARE" \
  --diagram "$VELXIO_DIAGRAM" \
  --scenario "$SCENARIO" \
  --timeout "$VELXIO_TIMEOUT" \
  "$@"
