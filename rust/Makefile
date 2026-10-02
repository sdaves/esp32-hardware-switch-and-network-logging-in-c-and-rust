# Rust port workspace entry points.
#
# All toolchain state lives under RUST_ROOT (default /opt/esp-rust), which is
# the `esp-rust` named volume from docker-compose.yaml. Every target runs with
# these env vars so rustup/cargo/espup resolve into the volume regardless of
# the container's ambient state.

RUST_ROOT      ?= /opt/esp-rust
export RUSTUP_HOME     = $(RUST_ROOT)/rustup
export CARGO_HOME      = $(RUST_ROOT)/cargo
export HOME            = $(RUST_ROOT)/home
export IDF_TOOLS_PATH  = $(HOME)/.espressif
export PATH           := $(CARGO_HOME)/bin:$(PATH)
export ESP_ROOT        = $(CURDIR)

# Source the esp toolchain export (Xtensa GCC/LLVM on PATH) for firmware work.
ESP_ENV = . $(RUST_ROOT)/export-esp.sh 2>/dev/null || true;

.PHONY: provision build test sim scenario wasm clean

provision:
	./scripts/provision-rust.sh

build:
	./scripts/build.sh

test:
	./scripts/test.sh

sim:
	$(ESP_ENV) cargo run -p host

scenario:
	./scripts/scenario.sh

wasm:
	./scripts/wasm.sh

clean:
	cargo clean
	rm -rf dist
