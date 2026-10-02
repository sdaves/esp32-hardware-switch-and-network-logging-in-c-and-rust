#!/usr/bin/env bash
# Ensure ESP-IDF v4.4.7 is installed for the emulated ESP32 Wi-Fi build.
#
# The Velxio image ships only IDF v5.x, whose esp_phy_enable asserts on the
# QEMU fork's radio. This installs the v4.4.7 tree the emulator needs.
#
# Idempotent: safe to run on every container start (a present tree just runs
# the fast tool check). `docker-compose.yaml` calls it and keeps /opt/esp-idf-v4.4
# in a named volume, so it survives container recreation.
set -euo pipefail

IDF_DIR="${IDF44_DIR:-/opt/esp-idf-v4.4}"
IDF_TAG="${IDF44_TAG:-v4.4.7}"

if [ ! -f "$IDF_DIR/export.sh" ]; then
  echo "[provision-idf44] cloning ESP-IDF $IDF_TAG into $IDF_DIR ..."
  git clone -b "$IDF_TAG" --depth 1 --recursive \
    https://github.com/espressif/esp-idf.git "$IDF_DIR"
fi

echo "[provision-idf44] installing ESP-IDF $IDF_TAG tools for esp32 ..."
"$IDF_DIR/install.sh" esp32

echo "[provision-idf44] ESP-IDF $IDF_TAG ready at $IDF_DIR"
