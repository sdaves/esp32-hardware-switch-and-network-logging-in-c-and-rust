# Rust port workspace entry points.
#
# All toolchain state lives under RUST_ROOT (default /opt/esp-rust), which is
# the `esp-rust` named volume from docker-compose.yaml. Route rustup/cargo (and
# espup's provisioned toolchain) there. Do NOT override HOME or IDF_TOOLS_PATH:
# the container's ESP-IDF tools live at the ambient $IDF_TOOLS_PATH
# (/root/.espressif) and IDF's export.sh resolves the compiler/GDB tools from it.

RUST_ROOT      ?= /opt/esp-rust
export RUSTUP_HOME     = $(RUST_ROOT)/rustup
export CARGO_HOME      = $(RUST_ROOT)/cargo
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
	$(ESP_ENV) cargo run -p tea-host --bin sim

scenario:
	./scripts/scenario.sh $(NAME)

wasm:
	./scripts/wasm.sh

clean:
	cargo clean
	rm -rf dist
