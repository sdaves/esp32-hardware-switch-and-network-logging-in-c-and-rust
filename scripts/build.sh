#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# The emulated ESP32 Wi-Fi radio (Velxio's QEMU fork) only initialises under
# ESP-IDF 4.4; IDF 5.x's esp_phy_enable asserts on a modem-clock register the
# fork does not model. Prefer the v4.4.7 tree when it is installed.
if [ -f /opt/esp-idf-v4.4/export.sh ]; then
  # The container starts with IDF_PATH/IDF_PYTHON_ENV_PATH pointing at v5.5;
  # clear them so v4.4's export does not resolve back to the v5 tree.
  unset IDF_PATH IDF_PYTHON_ENV_PATH IDF_TOOLS_EXPORT_CMD IDF_TOOLS_INSTALL_CMD
  # shellcheck disable=SC1091
  . /opt/esp-idf-v4.4/export.sh
elif [ -f /opt/esp-idf/export.sh ]; then
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
# IDF 4.4's `idf.py merge-bin` has no --fill-flash-size, so merge the standard
# ESP32 offsets directly and pad to the QEMU machine's fixed 4 MB.
ESPTOOL="$(find "${IDF_PATH}/components/esptool_py/esptool" -maxdepth 1 -name esptool.py | head -1)"
python "$ESPTOOL" --chip esp32 merge_bin \
  --output "$OUTPUT" --fill-flash-size 4MB \
  0x1000 build/bootloader/bootloader.bin \
  0x8000 build/partition_table/partition-table.bin \
  0x10000 build/esp32simulated.bin
echo "built $OUTPUT"
