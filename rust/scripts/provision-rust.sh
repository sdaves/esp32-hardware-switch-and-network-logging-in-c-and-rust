#!/usr/bin/env bash
# Idempotently install the aarch64 Rust + Xtensa (ESP32) toolchain.
#
# The Velxio image ships no Rust. This installs rustup, the `esp` toolchain
# (Espressif's rustc/LLVM fork via espup), espflash, and the wasm32 target.
#
# All state is routed under /opt/esp-rust so it lives in the `esp-rust` named
# volume (docker-compose.yaml) and survives `down`/`up`. Re-running is cheap:
# present steps are skipped.
set -euo pipefail

RUST_ROOT="${RUST_ROOT:-/opt/esp-rust}"
export RUSTUP_HOME="${RUSTUP_HOME:-$RUST_ROOT/rustup}"
export CARGO_HOME="${CARGO_HOME:-$RUST_ROOT/cargo}"
# espup's extracted GCC/LLVM and any ~/.espressif state land in the volume too.
export HOME="${ESP_RUST_HOME:-$RUST_ROOT/home}"
export IDF_TOOLS_PATH="${IDF_TOOLS_PATH:-$HOME/.espressif}"
export PATH="$CARGO_HOME/bin:$PATH"

mkdir -p "$RUSTUP_HOME" "$CARGO_HOME" "$HOME"

log() { echo "[provision-rust] $*"; }

# Host build deps espflash needs on this Debian-based image (serialport/libudev).
if ! dpkg -s libudev-dev >/dev/null 2>&1 || ! command -v pkg-config >/dev/null 2>&1; then
  log "installing host build deps (pkg-config libudev-dev)"
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq || true
  apt-get install -y -qq --no-install-recommends pkg-config libudev-dev libssl-dev >/dev/null 2>&1 || true
fi

# 1. rustup (aarch64 linux) + a stable host toolchain.
if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
  log "installing rustup into $CARGO_HOME"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --default-toolchain stable
else
  log "rustup already present"
fi
rustup toolchain install stable --no-self-update >/dev/null 2>&1 || true
rustup default stable >/dev/null 2>&1 || true

# 2. espup + espflash (cargo binaries in the volume).
if ! command -v espup >/dev/null 2>&1; then
  log "cargo install espup"
  cargo install espup --locked
else
  log "espup already present"
fi
if ! command -v espflash >/dev/null 2>&1; then
  log "cargo install espflash"
  cargo install espflash --locked
else
  log "espflash already present"
fi

# 3. The `esp` toolchain for the classic ESP32 + the export script.
EXPORT_FILE="$RUST_ROOT/export-esp.sh"
if ! rustup toolchain list | grep -q '^esp'; then
  log "espup install --targets esp32"
  espup install --targets esp32 --export-file "$EXPORT_FILE"
else
  log "esp toolchain already present"
fi
# Make sure the export script exists even if the toolchain was preinstalled.
if [ ! -f "$EXPORT_FILE" ]; then
  espup install --targets esp32 --export-file "$EXPORT_FILE"
fi

# 4. wasm target for the iced UI (native + wasm32-unknown-unknown).
if ! rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
  log "adding wasm32-unknown-unknown target"
  rustup target add wasm32-unknown-unknown
fi

log "done. rustup=$RUSTUP_HOME cargo=$CARGO_HOME home=$HOME"
log "esp toolchains:"; rustup toolchain list || true
