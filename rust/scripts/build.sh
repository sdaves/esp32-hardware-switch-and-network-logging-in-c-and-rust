#!/usr/bin/env bash
# Build the Rust firmware against the reused ESP-IDF v4.4.7 tree and emit a
# 4 MB merged flash image at rust/dist/firmware.merged.bin.
#
# Mirrors scripts/build.sh: the emulator's QEMU fork only initialises its Wi-Fi
# radio under IDF 4.4, so prefer /opt/esp-idf-v4.4 and clear any ambient v5
# IDF_PATH first. All toolchain state lives under RUST_ROOT (the `esp-rust`
# named volume).
set -euo pipefail
cd "$(dirname "$0")/.." # rust/

RUST_ROOT="${RUST_ROOT:-/opt/esp-rust}"
export RUSTUP_HOME="${RUSTUP_HOME:-$RUST_ROOT/rustup}"
export CARGO_HOME="${CARGO_HOME:-$RUST_ROOT/cargo}"
# Keep espup's ~/.espressif inside the volume; don't clobber a real HOME.
export HOME="${ESP_RUST_HOME:-$HOME}"
export IDF_TOOLS_PATH="${IDF_TOOLS_PATH:-$HOME/.espressif}"
export PATH="$CARGO_HOME/bin:$PATH"

if [ -f /opt/esp-idf-v4.4/export.sh ]; then
  # The container starts with IDF_PATH/IDF_PYTHON_ENV_PATH pointing at v5.5;
  # clear them so v4.4's export does not resolve back to the v5 tree.
  unset IDF_PATH IDF_PYTHON_ENV_PATH IDF_TOOLS_EXPORT_CMD IDF_TOOLS_INSTALL_CMD
  # shellcheck disable=SC1091
  . /opt/esp-idf-v4.4/export.sh
elif [ -f /opt/esp-idf/export.sh ]; then
  # shellcheck disable=SC1091
  . /opt/esp-idf/export.sh
fi

# espup's export script: puts the Xtensa LLVM on PATH and sets LIBCLANG_PATH
# (bindgen needs libclang) plus the `xtensa-esp-elf-gcc` that cc probes for.
if [ -f "$RUST_ROOT/export-esp.sh" ]; then
  # shellcheck disable=SC1091
  . "$RUST_ROOT/export-esp.sh"
fi

# Reuse the already-installed IDF 4.4.7 toolchain instead of downloading one.
export ESP_IDF_TOOLS_INSTALL_DIR="${ESP_IDF_TOOLS_INSTALL_DIR:-fromenv}"
export ESP_IDF_SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults"
export SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults"
# esp-idf-sys cannot infer the root crate in a workspace; name it explicitly.
export ESP_IDF_SYS_ROOT_CRATE="rust-firmware"
# Copy rust/partitions.csv into the esp-idf-sys CMake build dir so the IDF build
# uses our 3 MB factory partition (not the ~1 MB default). The glob variable
# must be `ESP_IDF_GLOB[_XXX]_YYY` -- note the underscore before YYY.
export ESP_IDF_GLOB_BASE="$PWD"
export ESP_IDF_GLOB_FILES="partitions.csv"
# IDF 4.4 => 32-bit time_t.
export RUSTFLAGS="${RUSTFLAGS:-} --cfg espidf_time32"
# esp-idf-svc 0.48.1 expects `char` to be signed (`*const i8`), but the clang
# that espup ships now defaults Xtensa `char` to unsigned, so bindgen emits
# `*const u8` and the crate fails to type-check. Force the historical signedness.
export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:-} -fsigned-char"

TARGET="xtensa-esp32-espidf"
BIN_NAME="rust-firmware"

cargo +esp build --release --target "$TARGET" -Zbuild-std=std,panic_abort

TARGET_DIR="$(cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
BUILD_ROOT="$TARGET_DIR/$TARGET/release"
APP_ELF="$BUILD_ROOT/$BIN_NAME"
if [ ! -f "$APP_ELF" ]; then
  echo "firmware ELF not found at $APP_ELF" >&2
  exit 1
fi

# esp-idf-sys builds ESP-IDF (and its bootloader) in a CMake dir under the
# cargo target tree; locate bootloader.bin there.
BOOTLOADER="$(find "$BUILD_ROOT" -path '*/bootloader/bootloader.bin' -print -quit 2>/dev/null || true)"
if [ -z "$BOOTLOADER" ]; then
  echo "could not locate the esp-idf bootloader under $BUILD_ROOT" >&2
  exit 1
fi

mkdir -p dist
OUTPUT="$PWD/dist/firmware.merged.bin"
# --merge bundles bootloader + our custom partition table + the app, padding to
# the 4 MB flash size the QEMU machine is fixed at.
espflash save-image --chip esp32 --merge \
  --flash-size 4mb \
  --bootloader "$BOOTLOADER" \
  --partition-table "$PWD/partitions.csv" \
  --partition-table-offset 0x8000 \
  "$APP_ELF" "$OUTPUT"
echo "built $OUTPUT ($(stat -c '%s' "$OUTPUT") bytes)"

# esp-idf-sys applies its `esp_app_desc` weak patch in place to the shared IDF
# tree but only skips re-applying via a reverse-check. Restore the tree so the
# C firmware keeps building against pristine sources; the next Rust build
# re-applies it during the (uncached) esp-idf-sys build script.
if [ -d "$IDF_PATH/.git" ]; then
  git -C "$IDF_PATH" checkout -- components/app_update/esp_app_desc.c 2>/dev/null || true
fi
