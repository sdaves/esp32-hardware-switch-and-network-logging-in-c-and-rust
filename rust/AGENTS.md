# rust/AGENTS.md — Rust port of the TEA platform

This is the working contract for the `rust/` workspace: a **standard (std)
`esp-idf-svc` firmware** for the classic ESP32, plus (later) a host CLI
simulator and an iced dashboard. It reimplements the C platform in
`main/`, `main/use_cases/*` and `test/`.

**The C project is the specification and is read-only.** Do not edit `main/`,
`test/`, `scripts/`, or `tests/velxio/` — reuse them read-only. All new code
goes under `rust/`. The only files outside `rust/` this project may touch are
`docker-compose.yaml` and `.devcontainer/devcontainer.json` (already done, §3),
and `docs/runbooks/` for status notes.

The authoritative phased plan is
[`docs/runbooks/add_rust_embassy_port.md`](../docs/runbooks/add_rust_embassy_port.md).
Read it before starting a new phase. This file is the quick operational
distillation.

---

## 1. Status and goal

| Phase | Scope | State |
|---|---|---|
| 0 | toolchain + hello-world + Wi-Fi spikes | **done** (2026-10-02) |
| 1 | `tea-core` + `tea-platform` + `firmware` + `host`, UC-1 | **done** (2026-10-02) |
| 2 | UC-2..UC-5 parity, emulator scenarios | **done** (2026-10-03) — UC-1 (GPIO + `LedToggled`) and UC-5 (Wi-Fi + HTTP + LED-state query) real; UC-2..UC-4 logic-only empty commands, matching C |
| 3 | iced dashboard (`ui`, native + wasm) | pending |

UC-5 status: the NET button runs UART → Wi-Fi (up to 3 attempts at boot) → HTTP
GET, and UC-1's LED toggle now publishes a `LedToggled` event that makes UC-5
fetch `/?led=on|off` with the new state. No request is sent at boot; Wi-Fi is
brought up once at boot and reused. Scenarios: `uc1_button_toggle`,
`uc5_fetch_and_uart`, `uc5_led_toggle_fetch`.

Goal: one pure Rust TEA core (`tea-core`) shared by every shell, one std
`esp-idf-svc` firmware that boots on the emulated ESP32 and reaches parity with
the C use cases, and a host simulator/test suite — with the zero-allocation,
pass-by-value, exhaustively-matched design of the C project preserved.

---

## 2. Hard rules (do not violate)

These mirror the C project's architecture (root `AGENTS.md` §1). The Rust port
must not relax them.

1. **Pure functional core.** `tea-core` stays `#![no_std]`-compatible, by-value,
   zero pointers, zero heap. `logic` functions take `Model` + `Msg` and return a
   `UpdateResult { next, command }`. No side effects, no `&mut`.
2. **Imperative shell.** Only `crates/firmware` (and later `crates/host`) may do
   IO/GPIO/network/file work. It executes declared `Cmd`s; the core never does.
3. **Feature isolation.** Use-case modules never reference each other. Cross-
   feature communication goes through the `tea-platform` event bus.
4. **Exhaustive matching.** Every `match` over a message/command enum enumerates
   all variants. **Never write a `_ =>` arm** for these enums. New variants must
   cause a compile error until handled (the Rust replacement for
   `-Wswitch-enum -Werror=switch`).
5. **Module registration.** Registration uses the `register_modules!` fallback
   (runbook §5.2): `crates/firmware/src/main.rs` lists every module in one macro
   invocation. Adding/removing a feature takes one line there plus its plugin
   module — `linkme`/`inventory` reflection was not used.
6. **Do not edit the C tree or the shared IDF tree.** In particular, never leave
   `/opt/esp-idf-v4.4` dirty (§5, finding 5).
7. **Pin the stack** (table below). Do not opportunistically bump crates; the
   versions are load-bearing for IDF 4.4 emulator Wi-Fi.

### Locked decisions

| Decision | Value |
|---|---|
| Firmware runtime | `esp-idf-svc` **0.48.x**, std, target `xtensa-esp32-espidf` |
| `esp-idf-svc` source | vendored `rust/vendor/esp-idf-svc` (0.48.1 + `c_char` fix) via `[patch.crates-io]` |
| ESP-IDF | **v4.4.7** at `/opt/esp-idf-v4.4` (shared with C; `ESP_IDF_TOOLS_INSTALL_DIR=fromenv`) |
| Rust cfg | `--cfg espidf_time32` (IDF 4.4 uses 32-bit `time_t`; never `time64`) |
| Firmware toolchain | `cargo +esp` (`rustc 1.97.0-nightly`), `-Zbuild-std=std,panic_abort` |
| Host toolchain | `stable-aarch64-unknown-linux-gnu` |
| Flash image | exactly 4 MB, custom 3 MB factory partition (`partitions.csv`) |
| UI runtime (Phase 3) | iced, native + `wasm32-unknown-unknown`, **network client only** |

---

## 3. Where commands run

Docker + ESP-IDF + the emulator live in the **velxio container**; the
devcontainer has no Docker daemon. Firmware builds and emulator runs therefore
go through the **host listener**.

| Work | Where | How |
|---|---|---|
| Firmware build / emulator | velxio container | `curl http://host.docker.internal:2222/exec` (or `/build-firmware`, `/test`) |
| Host `cargo test` / host sim / wasm | devcontainer | after the mount is active (`§` below) |

The `esp-rust` volume (`/opt/esp-rust`) is shared between both containers so they
use one toolchain and crate cache. `docker-compose.yaml` now has
`name: esp32simulated` and `.devcontainer/devcontainer.json` has the mount/env
edits. **Until the devcontainer is rebuilt, host-side `cargo` is unavailable**;
run everything in velxio via the listener in the meantime.

Listener health: `curl -sS --max-time 10 http://host.docker.internal:2222/health`
(expects `ok`). Requests are serialised; a busy listener returns HTTP 503.

---

## 4. Build and test

### Firmware (the normal loop)

`rust/scripts/build.sh` handles everything: it clears ambient v5 IDF vars, sources
IDF 4.4.7 **and** `/opt/esp-rust/export-esp.sh`, sets
`ESP_IDF_TOOLS_INSTALL_DIR=fromenv`, `ESP_IDF_SYS_ROOT_CRATE=rust-firmware`,
`ESP_IDF_GLOB_*` for `partitions.csv`, `--cfg espidf_time32`, builds with
`cargo +esp build --release --target xtensa-esp32-espidf -Zbuild-std=std,panic_abort`,
then merges a 4 MB image with `espflash save-image --merge`.

Run it (from the devcontainer, via the listener):

```sh
curl -sS --max-time 2400 -G \
  --data-urlencode 'cmd=cd /workspace/rust && make build' \
  http://host.docker.internal:2222/exec
# artifact: rust/dist/firmware.merged.bin  (must be exactly 4194304 bytes)
```

A build takes a few minutes cold (ESP-IDF); incremental Rust-only rebuilds are
seconds. Because the listener streams until the command exits, either wait, or
launch with `nohup ... > dist/build.log 2>&1 &` and poll the log (see the
`/exec` pattern used in Phase 0).

### Emulator scenarios

Reuse the C runner and circuit **read-only**; Rust scenarios live under
`rust/scenarios/`.

```sh
# inside velxio: start the HTTP endpoint UC-5 talks to (slirp gateway 192.168.4.2:8000)
cd /workspace && ./scripts/dev-http-server.sh start

python3 -m tests.velxio.runner.run_scenario \
  --server ws://localhost \
  --firmware rust/dist/firmware.merged.bin \
  --diagram tests/velxio/diagram.json \
  --scenario rust/scenarios/uc1_button_toggle.yaml \
  --timeout 40
# exit 0 = pass, 1 = fail, 2 = config error
```

Assert firmware behaviour with a `printf`/`log::info!` marker +
`wait-serial`. Output-pin `gpio_change` events are unreliable in the OSS bridge
(root `AGENTS.md` §6.3).

### Host tests (Phase 1+)

```sh
cd rust && cargo test        # pure tea-core/tea-platform; requires the devcontainer mount
```

`rust/Makefile` targets `provision`, `build`, `test`, `sim`, `scenario`, `clean`
work today. `wasm` is a stub until the `ui` crate exists (Phase 3).

---

## 5. Phase 0 gotchas (already fixed — keep them fixed)

These cost real time to find. If you touch the toolchain, dependency versions, or
`sdkconfig.defaults`, re-check them.

1. **`core::ffi::c_char` is `u8` on this toolchain; `esp-idf-svc` 0.48.1 hardcodes
   `i8`.** `bindgen` emits `core::ffi::c_char` for C `char` unconditionally, and
   the `esp` rustc defines it unsigned for `xtensa-esp32-espidf`, so `tls.rs` /
   `private/cstr.rs` fail with `E0308`. The vendored crate under
   `rust/vendor/esp-idf-svc` uses `c_char` there. **If you bump `esp-idf-svc`,
   reapply that fix** (see `rust/vendor/esp-idf-svc/CHANGES.md`).
2. **Main-task stack.** Default (~3.5 KB) overflows when `EspWifi` starts
   (`A stack overflow in task main` → reboot loop). Keep
   `CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384` in `rust/sdkconfig.defaults`.
3. **Source espup's export.** IDF's `export.sh` alone leaves `LIBCLANG_PATH`
   unset (bindgen) and does not provide `xtensa-esp-elf-gcc`. `build.sh` sources
   `/opt/esp-rust/export-esp.sh` after the IDF export.
4. **Glob the partition table into the CMake project.** esp-idf-sys looks for
   `<out>/partitions.csv`; set `ESP_IDF_GLOB_BASE` + `ESP_IDF_GLOB_FILES=partitions.csv`
   (underscore before `FILES`). Also set `ESP_IDF_SYS_ROOT_CRATE=rust-firmware`
   in a workspace. Without this the app would be built for the ~1 MB default
   partition.
5. **esp-idf-sys patches the shared IDF tree in place** (`esp_app_format_weak_v4.4.diff`
   on `components/app_update/esp_app_desc.c`) and is not interruption-safe.
   `build.sh` restores the file after a successful build. If a build is killed
   mid-way, run
   `git -C /opt/esp-idf-v4.4 checkout -- components/app_update/esp_app_desc.c`
   before rebuilding, and leave the tree clean for the C firmware.

Additional environment facts: `LIBCLANG_PATH`, GCC and `~/.espressif` all live
under `/opt/esp-rust`; `build/`, `sdkconfig`, `rust/target/`, `rust/dist/` are
generated and gitignored; `rust/Cargo.lock` is committed (this is an application
workspace).

---

## 6. Layout and responsibilities

```text
rust/
├── Cargo.toml                 # workspace + [patch.crates-io] esp-idf-svc
├── Cargo.lock                 # committed
├── rust-toolchain.toml        # host channel pin (firmware uses cargo +esp)
├── .cargo/config.toml         # xtensa target rustflags, build-std, MCU
├── sdkconfig.defaults         # 4 MB flash, 3 MB factory, 16 KB main stack
├── partitions.csv             # nvs / phy_init / 3 MB factory
├── Makefile                   # provision/build/test/sim/scenario/wasm/clean
├── crates/
│   ├── tea-core/              # (Phase 1) pure TEA model/msg/cmd/update, all UCs
│   ├── tea-platform/          # (Phase 1) event bus + PlatformModule + registry
│   ├── firmware/              # std esp-idf-svc bin (the only surface that does IO)
│   ├── host/                  # (Phase 1) std CLI sim + cargo integration tests
│   └── ui/                    # (Phase 3) iced app, native + wasm
├── vendor/esp-idf-svc/        # 0.48.1 + c_char casts fix (CHANGES.md documents it)
├── scenarios/                 # emulator scenarios driven by the C runner
├── scripts/{provision-rust,build,test,scenario,py-http-server}.sh
└── dist/                      # firmware.merged.bin (gitignored)
```

The C reference for each piece is in the root project:

| Rust | C reference |
|---|---|
| `tea-core` use-case logic | `main/use_cases/<name>/logic.c` + `domain.h` |
| `tea-platform` bus/registry | `main/registry.{c,h}` |
| `firmware` engine + IO | `main/main.c` + `main/use_cases/<name>/{setup,commands}.c` |
| host tests | `test/test_toggle_led.c`, `test/test_event_bus.c` |
| scenarios | `tests/velxio/scenarios/*.yaml` (reuse circuit `diagram.json`) |

---

## 7. Definition of done (per iteration)

1. `cd rust && cargo test` passes (host crates).
2. `make build` exits 0 and the boot serial contains
   `Platform Engine Initializing: Found 5 Autonomous Modules.` All five register
   (UC-1 and UC-5 real; UC-2..UC-4 logic-only stubs, matching the C empty
   `commands.c`), matching the C banner.
3. Every scenario in `rust/scenarios/` prints `RESULT: PASS` for the Rust bin.
4. No warnings under `cargo clippy -- -D warnings` (dependency lints are capped
   by cargo `--cap-lints allow`; keep *our* crates clean).
5. `rust/dist/firmware.merged.bin` is exactly 4 MB and boots in Velxio.
6. Built with `--cfg espidf_time32` against IDF 4.4.7; nothing outside `rust/`
   (plus the §3 container edits) changed. `/opt/esp-idf-v4.4` is clean
   (`git -C /opt/esp-idf-v4.4 status --short` empty).

---

## 8. Adding a Rust use case (Phase 2+)

Mirror the C four-file split per feature, but split by crate responsibility:

1. Add the pure logic to `crates/tea-core` (model/msg/cmd/update) and a unit
   test alongside the C test. Exhaustive matches, no `_ =>`.
2. Define its `PlatformModule` in `crates/firmware` (name, `init_hardware`,
   `poll_timer_tick`, `wire_subscriptions`, `process`).
3. Register it autonomously; if `linkme`/`inventory` entries are dropped by
   `--gc-sections`, use the `register_modules!` macro fallback (runbook §5.2).
4. Keep Wi-Fi init **one-shot** — a second `esp_netif_create_default_wifi_sta`
   returns NULL and reboots the chip.
5. Add/extend a scenario under `rust/scenarios/` and a host test; run
   `make -C test`-equivalent (`cargo test`) plus the emulator scenario.

Only pin/upgrade `esp-idf-svc` if you also verify IDF 4.4 emulator Wi-Fi; prefer
staying on 0.48.1 + the vendored `c_char` patch.

---

## 9. Troubleshooting

| Symptom | Fix |
|---|---|
| `cargo: command not found` in velxio | `cd /workspace/rust && bash scripts/provision-rust.sh` |
| `no such command: +esp` | re-run provision; source `$RUSTUP_HOME/env` |
| `can't find crate for std` / build-std | `rustup component add rust-src --toolchain esp` |
| `linker 'ldproxy' not found` | `cargo install ldproxy --locked` (provision does) |
| `E0308 *const i8` vs `*const u8` | use the vendored patched crate; don't bump esp-idf-svc |
| reboot loop `stack overflow in task main` | `CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384` |
| `Unable to find libclang` / `ToolNotFound: xtensa-esp-elf-gcc` | source `/opt/esp-rust/export-esp.sh` |
| `No rule to make target '…/partitions.csv'` | `ESP_IDF_GLOB_BASE` + `ESP_IDF_GLOB_FILES` |
| `could not identify the root crate` | `ESP_IDF_SYS_ROOT_CRATE=rust-firmware` |
| `patch does not apply` (esp_app_desc.c) | revert that file in `/opt/esp-idf-v4.4` and rebuild |
| `esp_phy_enable` / `phy_module_has_clock_bits` | building against IDF 5.x; build against `/opt/esp-idf-v4.4` |
| image not 4 MB / won't boot | re-merge with `--flash-size 4mb` and the 3 MB `partitions.csv` |
| `expect-pin` never matches | assert with a firmware `printf` + `wait-serial` instead |
| listener HTTP 503 / 404 | a build is running, or the host listener is stale and must be restarted |

See root `AGENTS.md` §6 for the full C/emulator troubleshooting set and the
WebSocket protocol.

---

## 10. References

- `docs/runbooks/add_rust_embassy_port.md` — phased plan, Phase 0 log, findings.
- Root `AGENTS.md` — C architecture, emulator/velxio workflow, listener, scenarios.
- `rust/vendor/esp-idf-svc/CHANGES.md` — the only intentional dependency patch.
- `rust/scripts/build.sh` / `provision-rust.sh` — the authoritative env + build setup.
