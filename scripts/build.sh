#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f /opt/esp-idf/export.sh ]; then
  # shellcheck disable=SC1091
  . /opt/esp-idf/export.sh
elif [ -n "${IDF_PATH:-}" ] && [ -f "${IDF_PATH}/export.sh" ]; then
  # shellcheck disable=SC1091
  . "${IDF_PATH}/export.sh"
fi

if [ ! -f sdkconfig ] || ! grep -q '^CONFIG_IDF_TARGET="esp32"' sdkconfig; then
  idf.py set-target esp32
fi
idf.py build

OUTPUT="$(pwd)/build/firmware.merged.bin"
idf.py merge-bin --fill-flash-size 4MB -o "$OUTPUT"
echo "built $OUTPUT"
