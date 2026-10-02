# Runbook: Port the platform to Rust (esp-idf-svc + iced)

Status: Phase 0 (provisioning + hello-world + Wi-Fi spikes) complete; Phase 1 next
Audience: firmware maintainers and coding agents
Scope: stand up a self-contained `rust/` workspace that reimplements the C platform in Rust —
a **standard (std) `esp-idf-svc` firmware** for the classic ESP32, a native host simulator, and
an iced browser dashboard — without editing the C project (one compose-volume exception, §8).

This runbook is a set of instructions. It is **not executed by tooling**. It is phased; each
phase is a checkpoint and must be run to completion before the next. **Do not port a use case
before the Phase 0 gate passes.**

> **Stack change (why std, not Embassy).** The original plan targeted bare-metal `no_std`
> Embassy + `esp-hal`. It was replaced with **`esp-idf-svc` (std)** because Velxio's emulator
> only brings up the Wi-Fi radio under **ESP-IDF 4.4** (`esp_phy_enable` asserts on the modem
> registers IDF 5.x touches). `esp-idf-svc` calls the *same* C `esp_wifi_*` / `esp_netif` APIs
> the working C firmware drives, so building Rust against the repo's existing
> `/opt/esp-idf-v4.4` tree gives Rust Wi-Fi parity in the emulator. The price is leaving the
> pure async Embassy environment and pinning older crates (details below). iced is unchanged:
> it is a network client, not firmware.

---

## 0. How to run this runbook

- Trigger it by asking an agent to "run `docs/runbooks/add_rust_embassy_port.md`".
- Optionally name a phase: *"run Phase 1 of the rust port runbook"* (`PHASE=1`).
- If no phase is named, start at the first incomplete phase.
- Phases are ordered and gated: Phase 0 decides the emulator strategy for everything after it.
- All file writes go under `rust/` (plus the two container edits in §8 and §4.0). Never modify
  `main/`, `test/`, `scripts/`, or `tests/velxio/` — reuse the last read-only.
- Host-side Rust work (`make provision`, `cargo test`, `make sim`, `make wasm`) runs **in the
  devcontainer** now that it shares the `esp-rust` volume (§4.0). Firmware and emulator work
  happens **inside the Velxio container**, driven through the host listener
  (`curl http://host.docker.internal:2222/...`); see `AGENTS.md` §5–§6.

---

## 1. Stack decision (why esp-idf-svc, why iced is only a client)

- **`esp-idf-svc` is correct here.** It is `std` Rust and re-exports `esp-idf-hal` and
  `esp-idf-sys` (`esp_idf_svc::hal`, `esp_idf_svc::sys`). Every Wi-Fi/HTTP call bottoms out in
  the exact same C ESP-IDF libraries the existing firmware uses. Because the emulator's QEMU
  fork models the IDF 4.4 radio (proven by the C UC-5 scenario), building Rust against the
  **same IDF 4.4.7 tree** is what makes UC-5 run in the emulator from Rust.
- **Version pin is load-bearing.** `esp-idf-svc` 0.49.0 (2024-06) deprecated ESP-IDF v4.4, and
  current releases (0.5x) test only IDF 5.1+. To stay on IDF 4.4 we pin the last
  pre-deprecation line: **`esp-idf-svc = "=0.48.1"`** (it pulls a matching `esp-idf-sys`,
  ~0.36). This is the first thing the Phase 0 spike must prove.
- **IDF 4.4 requires `--cfg espidf_time32`.** IDF 5.0+ uses 64-bit `time_t` (`espidf_time64`);
  v4.4 uses 32-bit. Setting the wrong one breaks `libc`/`time_t`.
- **iced cannot run on the MCU.** iced is a desktop/browser GUI (native + `wasm32`). It is the
  **network client/dashboard** that connects to the ESP; it is not firmware. The firmware needs
  a small HTTP/WebSocket server for iced to reach (Phase 3) — `esp-idf-svc` ships
  `EspHttpServer`, so this is now straightforward.
- The C project's "Elm Architecture" maps cleanly to Rust enums and to iced's Elm model: keep
  one pure TEA core (`tea-core`) shared by every shell.

### Toolchain reality (measured)

The Velxio container is **aarch64** and ships **no Rust** (`rustc`, `cargo`, `rustup`,
`espflash` were all missing). Xtensa needs the Espressif rustc fork, the LLVM fork, a GCC
linker (`espup`), and for std also `rust-src` + `ldproxy`. This is provisioned at runtime,
never in the image.

```
uname -m          -> aarch64
rustc/cargo/...   -> MISSING (until provision)
curl/gcc/make     -> present
/workspace avail  -> ~49 GB
```

---

## 2. Constraints and locked decisions

| Decision | Value |
|---|---|
| Firmware runtime | `esp-idf-svc` **0.48.x** (std), target `xtensa-esp32-espidf` |
| `esp-idf-svc` source | vendored `rust/vendor/esp-idf-svc` (0.48.1 + `c_char` cast fix) via `[patch.crates-io]` |
| ESP-IDF | **v4.4.7** at `/opt/esp-idf-v4.4`, reused via `IDF_PATH` + `ESP_IDF_TOOLS_INSTALL_DIR=fromenv` |
| Rust cfg | `--cfg espidf_time32` (never `espidf_time64` on v4.4) |
| UI runtime | iced, native + `wasm32-unknown-unknown`, network client only |
| First increment | pure core + event bus + **UC-1** |
| Module registration | `inventory`/`linkme`; fall back to a `register_modules!` macro list |
| Toolchain persistence | **Docker named volume** (accepted; §8) |
| C project edits | **none**, except the §8 compose volume/mount |
| Emulator harness | reuse `tests/velxio/` runner + `diagram.json` **read-only** |
| Fallback if Wi-Fi won't run in Velxio | host tests + real-HW verification (**not** a second firmware, §12) |

---

## 3. Target workspace layout

```
rust/
├── Cargo.toml                 # workspace
├── rust-toolchain.toml        # esp toolchain pin
├── .cargo/config.toml         # xtensa-esp32-espidf target, build-std, espidf_time32, ldproxy
├── sdkconfig.defaults         # 4 MB flash, Wi-Fi on, custom partition table
├── partitions.csv             # large factory app (std+IDF binary > 1 MB default)
├── Makefile                   # provision / build / test / wasm / sim / scenario
├── .gitignore                 # target/, dist/, pkg/, .embuild/
├── crates/
│   ├── tea-core/              # pure TEA (model/msg/cmd/update), all use cases
│   ├── tea-platform/          # event bus + module trait + registry
│   ├── firmware/              # std esp-idf-svc bin (plugins live here)
│   ├── host/                  # std CLI simulator + cargo integration tests
│   └── ui/                    # iced app (native + wasm32) that connects to the device
├── vendor/
│   └── esp-idf-svc/           # 0.48.1 + the c_char casts fix ([patch.crates-io])
├── scenarios/                 # Rust-side emulator scenarios (phase0_hello/wifi.yaml)
├── scripts/
│   ├── provision-rust.sh      # rustup + espup + espflash + ldproxy + rust-src + wasm, idempotent
│   ├── build.sh               # cargo +esp build -> dist/firmware.merged.bin (4 MB)
│   ├── test.sh                # cargo test + velxio scenarios on the rust bin
│   └── wasm.sh                # trunk build of ui
└── dist/                      # merged bin artifacts (gitignored)
```

Only `crates/firmware` and the two Phase 0 scenarios exist so far; the other crates,
`scripts/test.sh`, and `scripts/wasm.sh` arrive in Phases 1–3.

`tea-core` and `tea-platform` stay `#![no_std]`-compatible pure crates so the same logic is
unit-tested on the desktop and shipped to silicon; the **firmware shell** is std. (They remain
the zero-allocation functional core the C project enforces.)

---

## 4. Phase 0 — Provisioning and feasibility spikes (the gate)

Do this before porting anything. It de-risks the entire project in about a day.

### Phase 0 progress log

- [x] **0.1a Base provision** — done. The `esp-rust` volume (§8) is mounted, and
  `rust/scripts/provision-rust.sh` installed: `stable-aarch64-unknown-linux-gnu` (with
  `wasm32-unknown-unknown`), the `esp` toolchain, `cargo install espup espflash`, and the
  Xtensa GCC/LLVM export at `/opt/esp-rust/export-esp.sh`.
- [x] **0.1a-share Devcontainer shares the Rust volume** — **config applied; rebuild pending**.
  `docker-compose.yaml` gained a top-level `name: esp32simulated` and
  `.devcontainer/devcontainer.json` gained the `mounts` / `containerEnv` / `remoteEnv` /
  `postCreateCommand` / `postStartCommand` from §4.0. The devcontainer itself still needs a
  VS Code "Rebuild Container" before host-side `cargo` appears; firmware work in this run went
  through velxio's `/exec`, so this step is not on the critical path.
- [x] **0.1b std additions** — done. `provision-rust.sh` now also runs
  `cargo install ldproxy --locked` (esp-idf-svc's linker driver) and guarantees
  `rustup component add rust-src --toolchain esp` (for `-Zbuild-std`; it was already present).
  Both were re-run in velxio and verified.
- [x] **0.3 aarch64 gate** — pass. The Espressif fork publishes an aarch64 host build; the
  `esp` toolchain resolves and installs on this container (no cross-arch problem).
- [x] **0.2 Hello-world spike** — pass. `rust/dist/firmware.merged.bin` is exactly 4 MB, boots
  in Velxio (4 MB image, 3 MB factory partition) and prints
  `Platform Engine Initializing: Found 0 Autonomous Modules.` / `# RUST HELLO (idf 4.4)`.
  Scenario `rust/scenarios/phase0_hello.yaml` → `RESULT: PASS`.
- [x] **0.4 Wi-Fi spike** — pass. `BlockingWifi<EspWifi>` + `std::net` HTTP GET to
  `http://192.168.4.2:8000/` succeeds against `scripts/dev-http-server.sh`:
  `# WIFI CONNECTED (Espressif)` → `-> status 200` → `# HTTP BODY …` → `# NETWORK SYNCED`.
  Scenario `rust/scenarios/phase0_wifi.yaml` → `RESULT: PASS`.

Measured toolchain facts (use these exact paths/names elsewhere in the runbook):

| Fact | Value |
|---|---|
| Rust root / volume | `/opt/esp-rust` (`esp-rust` named volume) |
| `RUSTUP_HOME` | `/opt/esp-rust/rustup` |
| `CARGO_HOME` | `/opt/esp-rust/cargo` (binaries at `cargo/bin`) |
| `HOME` | `/opt/esp-rust/home` (so espup's `~/.espressif` lands in the volume) |
| Firmware toolchain | `esp` (`cargo +esp …`), `rustc 1.97.0-nightly` |
| Host toolchain | `stable-aarch64-unknown-linux-gnu` (default) |
| Xtensa env script | `/opt/esp-rust/export-esp.sh` (sets `PATH` + `LIBCLANG_PATH`) |
| ESP-IDF tree (reused) | `/opt/esp-idf-v4.4` (`idf44` volume), `v4.4.7` |
| `espup`/`espflash`/`ldproxy` | `/opt/esp-rust/cargo/bin/{espup,espflash,ldproxy}` |

### 4.0 Share the volume with the devcontainer (0.1a-share)

The devcontainer (`.devcontainer/devcontainer.json`, image-only, user `vscode`) has no Docker
daemon and, before this step, no Rust. Mount the velxio volume into it so both containers use
the same `/opt/esp-rust` toolchain and cargo registry cache.

> **Applied.** The `name:`/`mounts`/env/`postCreate`/`postStart` edits below are committed. The
> devcontainer still needs a VS Code "Rebuild Container" before host-side `cargo`/`rustup`
> appear; the Phase 0 spikes did not depend on it (they ran through velxio's `/exec`). Run the
> verification block below after rebuilding to confirm the mount.

**Prerequisite — pin the compose project name.** Without a top-level `name:`, compose names the
volume after the host checkout directory, so the devcontainer mount cannot find it. Add the
top-level name to `docker-compose.yaml`; this reuses the existing `esp32simulated_esp-rust`
volume (no re-download, no migration):

```yaml
name: esp32simulated

services:
  velxio:
    # ...
```

**Then add to `.devcontainer/devcontainer.json`** (alongside `image`/`features`):

```json
"mounts": [
  "source=esp32simulated_esp-rust,target=/opt/esp-rust,type=volume"
],
"containerEnv": {
  "CARGO_HOME": "/opt/esp-rust/cargo",
  "RUSTUP_HOME": "/opt/esp-rust/rustup"
},
"remoteEnv": {
  "CARGO_HOME": "/opt/esp-rust/cargo",
  "RUSTUP_HOME": "/opt/esp-rust/rustup",
  "PATH": "/opt/esp-rust/cargo/bin:${containerEnv:PATH}"
},
"postCreateCommand": "sudo apt-get update && sudo apt-get install -y --no-install-recommends gcc make pkg-config libudev-dev libssl-dev",
"postStartCommand": "bash -lc 'if [ ! -w /opt/esp-rust/cargo ]; then sudo chown -R vscode:vscode /opt/esp-rust; fi'"
```

Notes:

- `mounts` reuses the exact velxio volume; `containerEnv`/`remoteEnv` put `cargo`/`rustc`/
  `rustup`/`espflash`/`ldproxy` on `PATH` and point the crate cache at the shared registry.
- The volume is provisioned as root by velxio, so the guarded `postStartCommand` chowns it to
  `vscode` once; root (velxio) can always still write. The guard makes it a no-op after the
  first run and re-fixes ownership if velxio re-provisions.
- `postCreateCommand` installs the host libs the shared binaries need (`gcc` for linking,
  `libudev` for espflash, `openssl`/`pkg-config`).
- This is the **host** toolchain only: firmware (`xtensa`) builds still run in velxio because
  the IDF 4.4 tree (`idf44`) and the emulator live there.
- Both containers are `aarch64`, so the shared binaries are compatible; a named volume is per
  Docker host (shares containers on one daemon, not separate physical machines). Do not run
  `provision-rust.sh`/`cargo install` from both containers at the same time.

**Verify** (rebuild the devcontainer first — VS Code: "Dev Containers: Rebuild Container"):

```sh
ls /opt/esp-rust && cargo --version && rustup toolchain list
ls /opt/esp-rust/cargo/registry          # shared crate cache populated
cd rust && cargo test                    # host tests; no toolchain download
# Same bytes visible from velxio:
curl -sS -G --data-urlencode 'cmd=ls /opt/esp-rust' http://host.docker.internal:2222/exec
docker compose config | grep -A2 'volumes:'   # on the host: name is esp32simulated_esp-rust
```

1. **Provision** (idempotent, mirrors `scripts/provision-idf44.sh`):
   `rust/scripts/provision-rust.sh` installs rustup (aarch64), `cargo install espup espflash
   ldproxy`, `espup install --targets esp32`, `rustup component add rust-src --toolchain esp`,
   and adds the `wasm32-unknown-unknown` target. All roots point at the named volume (§8), so
   `down`/`up` does not re-download.
2. **Hello-world spike.** Build a minimal std `esp-idf-svc` binary (the `esp-idf-template`
   layout, or hand-written) that links the repo's IDF 4.4.7:
   ```sh
   # inside the container / via the host listener
   . /opt/esp-idf-v4.4/export.sh          # xtensa gcc + IDF env
   export ESP_IDF_TOOLS_INSTALL_DIR=fromenv IDF_PATH=/opt/esp-idf-v4.4
   cargo +esp build --release --target xtensa-esp32-espidf -Zbuild-std=std,panic_abort
   ```
   Produce a merged 4 MB image (§9) and boot it in Velxio through the **existing** runner:
   ```sh
   python3 -m tests.velxio.runner.run_scenario \
     --server ws://localhost \
     --firmware rust/dist/firmware.merged.bin \
     --diagram tests/velxio/diagram.json \
     --scenario tests/velxio/scenarios/uc1_button_toggle.yaml   # temporary; expect a marker
   ```
   Confirm the app boots and emits serial.
3. **aarch64 gate.** ~~Confirm the Espressif Rust fork publishes an **aarch64** host build~~
   **PASSED** (see progress log): the `esp` toolchain installs on aarch64.
4. **Wi-Fi spike.** Build a minimal `esp-idf-svc` `BlockingWifi<EspWifi>` DHCP image plus an
   HTTP GET to `http://192.168.4.2:8000/` and boot it in Velxio. Because it links the same C
   IDF 4.4 radio as the passing C scenario, this is **expected to pass**. Record whether the
   radio associates and the GET returns 200.

### Phase 0 gate outcome

| Spike | Result | Consequence |
|---|---|---|
| hello-world boots in Velxio | **pass** | proceed; GPIO/UART parity is viable |
| `esp-idf-svc` Wi-Fi boots in Velxio | **pass** | **one** std firmware serves all three surfaces |
| `esp-idf-svc` Wi-Fi fails in Velxio | not hit | §12 fallback not needed |

Gate decision: **proceed to Phase 1.** The std `esp-idf-svc` shell, built against the reused
IDF 4.4.7 tree, boots and reaches the Wi-Fi + HTTP success path in the emulator, so the one
firmware can serve GPIO/UART (UC-1) and the UC-5 network chain.

Raw boot serial (Phase 0.4, `rust/dist/firmware.merged.bin`):

```text
I (12618) rust_firmware: Platform Engine Initializing: Found 0 Autonomous Modules.
# RUST HELLO (idf 4.4)
I (20368) rust_firmware: # WIFI CONNECTED (Espressif)
I (20748) rust_firmware: -> status 200
I (20768) rust_firmware: # HTTP BODY Server: BaseHTTP/0.6 Python/3.12.14
I (20828) rust_firmware: # NETWORK SYNCED
```

The 0.2 boot additionally shows the bootloader reporting `ESP-IDF v4.4.7`,
`SPI Flash Size : 4MB`, and the custom partition table
(`factory app … 00010000 00300000`), i.e. the 3 MB factory partition is in effect.

### Phase 0 findings (deviations from the original plan)

The spike surfaced five concrete gotchas. They are already fixed in the committed `rust/`
tree; this list is so the next agent does not rediscover them.

1. **`core::ffi::c_char` is `u8` on this toolchain, but `esp-idf-svc` 0.48.1 hardcodes `i8`.**
   `bindgen` emits `core::ffi::c_char` for C `char` unconditionally (no clang flag changes
   this), and the current `esp` rustc defines it unsigned for `xtensa-esp32-espidf`, so
   `esp-idf-svc`'s `tls.rs` / `private/cstr.rs` fail with `E0308`. Fix: vendor the crate at
   `rust/vendor/esp-idf-svc` and use `c_char` for those casts; wired in with
   `[patch.crates-io]` in `rust/Cargo.toml`. See `rust/vendor/esp-idf-svc/CHANGES.md`. Staying
   on the 0.48.1 line preserves IDF 4.4 support (0.50.x+ fixes the casts but targets IDF 5.1+).
2. **The default main-task stack (~3.5 KB) overflows** as soon as `EspWifi` starts:
   `***ERROR*** A stack overflow in task main` → reboot loop. Fix:
   `CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384` in `rust/sdkconfig.defaults`.
3. **`bindgen`/`cc` need espup's export.** Sourcing only `/opt/esp-idf-v4.4/export.sh` leaves
   `LIBCLANG_PATH` unset and provides `xtensa-esp32-elf-gcc` (not the `xtensa-esp-elf-gcc`
   `cc` probes for). `rust/scripts/build.sh` sources `/opt/esp-rust/export-esp.sh` after the IDF
   export, which fixes both `libclang` and compiler-family detection.
4. **The custom partition table must be globbed into the CMake project.** esp-idf-sys looks for
   `<out>/partitions.csv`; set `ESP_IDF_GLOB_BASE=<rust dir>` +
   `ESP_IDF_GLOB_FILES=partitions.csv` (note the underscore before `FILES` — the glob var is
   `ESP_IDF_GLOB[_XXX]_YYY`). Without it the IDF build uses the ~1 MB default and the Rust app
   would not fit. Also set `ESP_IDF_SYS_ROOT_CRATE=rust-firmware` (required in a workspace).
5. **esp-idf-sys patches the IDF tree in place and is not interruption-safe.** It applies
   `esp_app_format_weak_v4.4.diff` to `/opt/esp-idf-v4.4/components/app_update/esp_app_desc.c`
   and skips re-applying via a reverse-check. An interrupted first build leaves the patch
   applied; a rerun then fails with `patch does not apply`. `build.sh` restores the file after
   a successful build so the shared tree stays pristine for the C firmware (`git checkout --
   components/app_update/esp_app_desc.c`); if a build was killed mid-way, run the same
   `git -C /opt/esp-idf-v4.4 checkout -- …` before rebuilding.

---

## 5. Phase 1 — Core + UC-1 (the first increment)

Mirror the C modules exactly; the only real end-to-end features are **UC-1** (GPIO) and
**UC-5** (UART → Wi-Fi HTTP). UC-2/3/4 are logic-only stubs in C (empty `commands.c`, no
producer), so port them as `tea-core` modules so the banner still reports five.

### 5.1 `tea-core` (pure, `no_std`, no heap)

Transliterate `logic.c` by value:

```rust
// crates/tea-core/src/toggle_led.rs  (UC-1)
pub enum Msg { ButtonDown }
pub enum Cmd { None, ToggleLed { pin: u8, level: u8 } }
pub struct Model { pub led_on: bool }
pub struct UpdateResult { pub next: Model, pub command: Cmd }

pub fn init() -> Model { Model { led_on: false } }

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    match msg {
        Msg::ButtonDown => {
            let led_on = !model.led_on;
            UpdateResult {
                next: Model { led_on },
                command: Cmd::ToggleLed { pin: 2, level: led_on as u8 },
            }
        }
    }
}
```

Add the UC-2..UC-5 equivalents from `read-analog-sensor`, `fetch-and-save`, `read-and-insert`,
`fetch-and-uart` logic. Exhaustive `match` replaces `-Wswitch-enum`: never write `_ =>`.

### 5.2 `tea-platform` (event bus + registry)

- `SystemEventId` — the same five events as `main/registry.h`.
- `PlatformEvent` — one enum carrying typed payloads per event, replacing the C
  `memcpy`/size-matched byte block. Typed matching removes the whole
  "declared message size must equal payload size" failure class.
- `PlatformModule` trait mirroring the C vtable:
  `name()`, `init_hardware()`, `poll_timer_tick()`, `wire_subscriptions()`, `process(event)`.
- Bus = a small static pub-sub table of `std::sync::mpsc` senders (the firmware is std). The
  same logic is exercised by host tests, so the bus is tested natively too.
- **Module registration:** try `linkme::distributed_slice` (stdlib `inventory` is the
  alternative) as the `USE_CASE_REGISTER` equivalent. Because IDF links with `--gc-sections`,
  an unreferenced registration can be discarded — if the banner reports fewer than five,
  fall back to a `register_modules!` macro that collects
  `&'static [&'static dyn PlatformModule]` (one macro list; reliable, at the cost of the C
  project's linker-reflected autonomy).

### 5.3 `firmware` (std `esp-idf-svc` shell, mirrors `main.c`)

- `fn main()` with `esp_idf_svc::sys::link_patches()`, `Peripherals::take()`, GPIO0 button
  input and GPIO2 LED output via `esp_idf_svc::hal::gpio`.
- Copy the edge-detect + release re-arm from `toggle-physical-led/setup.c` exactly (fire once on
  high→low, latch until high), driven by a 100 ms poll (a `std::thread` loop or an
  `esp-idf-svc` timer).
- Print the same serial markers so existing expectations match:
  `Platform Engine Initializing: Found 5 Autonomous Modules.`,
  `# LED ON (gpio 2)`, `# LED OFF (gpio 2)`.
- Use `esp_idf_svc::log::EspLogger` / `esp-idf-svc`'s panic handler.

### 5.4 `host` (std CLI simulator + tests)

- `cargo test` replaces `make -C test`:
  - unit: `init()` is off; `ButtonDown` → `ToggleLed { pin: 2, level: 1 }` (mirrors
    `test/test_toggle_led.c`);
  - integration: single subscriber, broadcast to two, and non-matching event ignored (mirrors
    `test/test_event_bus.c`).
- A `host` binary that drives the bus from scripted input — the "CLI sim".

### 5.5 `rust/scripts/test.sh`

1. `cargo test` (host crates);
2. `cargo +esp build --release ...` → `dist/firmware.merged.bin` (see §9);
3. run the existing runner against the **Rust** bin:
   ```sh
   python3 -m tests.velxio.runner.run_scenario \
     --server "${VELXIO_WS:-ws://localhost}" \
     --firmware rust/dist/firmware.merged.bin \
     --diagram tests/velxio/diagram.json \
     --scenario tests/velxio/scenarios/uc1_button_toggle.yaml
   ```

**Phase 1 done when:** `cargo test` passes, the boot banner reports five modules, and
`uc1_button_toggle.yaml` prints `RESULT: PASS` with the Rust bin.

---

## 6. Phase 2 — UC-2..UC-5 and full parity

- **UC-5** is the real chain. Reproduce the C behavior and markers:
  button (GPIO4) → `# UART SENT` → Wi-Fi connect → HTTP GET
  `http://192.168.4.2:8000/` → `-> status 200` / `# HTTP BODY` → `# NETWORK SYNCED`.
  Use `esp-idf-svc` `BlockingWifi`/`EspWifi` plus `esp_idf_svc::http::client` (or
  `std::net::TcpStream`, as in the crate's `tcp` example). Keep Wi-Fi init **one-shot** — a
  second `esp_netif_create_default_wifi_sta` returns NULL and reboots the chip (the C fix).
- **UC-2/3/4** — port `tea-core` logic; keep command execution a no-op unless the C side is
  upgraded (in C they are empty). Their purpose is module-count parity and native tests.
- **Emulator Wi-Fi** is the point of this stack choice. Apply the Phase 0 gate outcome:
  - pass → run `fetch_and_uart.yaml` against the Rust bin;
  - fail → scope emulator parity to UC-1 and cover UC-5 with host tests + real-hardware
    verification (§12).

---

## 7. Phase 3 — iced dashboard

This phase is deferred until Phases 1–2 are green.

- `crates/ui`: one iced app, native + `wasm32-unknown-unknown` via `trunk` (`rust/scripts/wasm.sh`),
  served as static assets.
- Add a small server to the firmware using `esp_idf_svc::http::server::EspHttpServer` (it
  supports HTTP and WebSocket handlers, as the crate's `http_ws_server` example shows). The
  iced app subscribes to events and publishes the same logical messages the use cases consume.
- Target real hardware (connect to the device's LAN IP). Browser → slirp-guest connectivity
  under Velxio is **unsolved**; do not block Phase 3 on it.

---

## 8. The two edits outside `rust/` (container toolchain persistence)

The named-volume decision requires adding a volume and mount to root `docker-compose.yaml` so
the aarch64 Xtensa toolchain survives `down`/`up`, exactly like `idf44:/opt/esp-idf-v4.4`, and
mounting that same volume into the devcontainer (§4.0) so one toolchain/crate cache is shared.
**Applied** — `docker-compose.yaml` now mounts `esp-rust:/opt/esp-rust` and declares the
`esp-rust` volume; the container was recreated with `/up` and the mount verified:

```yaml
    volumes:
      - ./:/workspace
      - idf44:/opt/esp-idf-v4.4
      - esp-rust:/opt/esp-rust
# ...
volumes:
  idf44:
  esp-rust:
```

`rust/scripts/provision-rust.sh` must export `RUSTUP_HOME=/opt/esp-rust/rustup`,
`CARGO_HOME=/opt/esp-rust/cargo`, and route espup's GCC/`~/.espressif` under `/opt/esp-rust`
as well, so **all** toolchain state lands in the volume. `rust/Makefile` sets these env vars on
every target, and — for firmware targets — clears any ambient v5 `IDF_PATH` and sources
`/opt/esp-idf-v4.4/export.sh`, exactly like `scripts/build.sh`.

Now that the devcontainer shares the volume (§4.0), host-side Rust work runs **directly in the
devcontainer** — no listener round-trip:

```sh
cd rust && make provision    # idempotent
cd rust && cargo test
cd rust && make sim
cd rust && make wasm
```

Only firmware/emulator work still goes through velxio's `/exec`:

```sh
curl -sS --max-time 2400 -G \
  --data-urlencode 'cmd=cd /workspace/rust && make test' \
  http://host.docker.internal:2222/exec
```

---

## 9. Build → merged 4 MB bin

The QEMU machine is fixed at 4 MB and boots an ESP-IDF layout (loaded at offset `0x0`). The
`esp-idf-svc` build produces an IDF app ELF plus a bootloader and partition table; because std +
IDF binaries are large, this project supplies its own partition table.

1. **Partition table.** Add `rust/partitions.csv` with a large factory app, e.g.:
   ```
   # Name,   Type, SubType, Offset,  Size, Flags
   nvs,      data, nvs,     ,        0x6000,
   phy_init, data, phy,     ,        0x1000,
   factory,  app,  factory, ,        0x300000,
   ```
   and point `sdkconfig.defaults` at it:
   ```
   CONFIG_PARTITION_TABLE_CUSTOM=y
   CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="partitions.csv"
   ```
   (The default IDF table caps the app at ~1 MB — the current C app is 808 KB; a std Rust app
   will exceed that.)
2. **Build** with the reused IDF 4.4 tree:
   ```sh
   . /opt/esp-idf-v4.4/export.sh
   export IDF_PATH=/opt/esp-idf-v4.4 ESP_IDF_TOOLS_INSTALL_DIR=fromenv
   export ESP_IDF_SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults"
   RUSTFLAGS="--cfg espidf_time32" \
     cargo +esp build --release --target xtensa-esp32-espidf -Zbuild-std=std,panic_abort
   ```
   With `ESP_IDF_TOOLS_INSTALL_DIR=fromenv`, `esp-idf-sys` uses the already-installed 4.4.7
   toolchain and emits the IDF bootloader/partition-table under its build output.
3. **Merge.** Either let cargo-espflash reuse the `esp-idf-sys`-built bootloader/partition and
   merge:
   ```sh
   cargo espflash save-image --chip esp32 --merge \
     target/xtensa-esp32-espidf/release/rust-firmware dist/firmware.merged.bin
   ```
   or mirror `scripts/build.sh` and merge with the container's IDF 4.4 `esptool.py`:
   ```sh
   python "$ESPTOOL" --chip esp32 merge_bin \
     --output "$PWD/dist/firmware.merged.bin" --fill-flash-size 4MB \
     0x1000  <idf-build>/bootloader/bootloader.bin \
     0x8000  <idf-build>/partition_table/partition-table.bin \
     0x10000 <idf-build>/rust-firmware.bin
   ```
   Either way the result must be exactly 4 MB (`0x400000`); verify with `stat -c '%s'`.
   Mirror `scripts/build.sh`'s absolute-`-o` rule (a relative `-o` inside `build/` becomes
   `build/build/...`).

---

## 10. Definition of done (mirrors `AGENTS.md` §6.5)

1. `cd rust && make test` → all `cargo test` targets pass.
2. `make build` exits 0; the boot serial contains
   `Platform Engine Initializing: Found 5 Autonomous Modules.`
3. The emulator scenario(s) in scope print `RESULT: PASS` for the **Rust** bin.
4. No warnings under `cargo clippy -- -D warnings`.
5. `dist/firmware.merged.bin` is exactly 4 MB and boots in Velxio.
6. The firmware is std (`xtensa-esp32-espidf`), built with `--cfg espidf_time32` and against
   IDF 4.4.7; `rust/partitions.csv` exists and gives the app a ≥ 3 MB factory partition.
7. Nothing outside `rust/` (plus the §8 volume) changed: `git status` shows no `main/`,
   `test/`, `scripts/`, or `tests/` modifications.

---

## 11. Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `E0308` in `esp-idf-svc` `tls.rs`/`cstr.rs` (`*const i8` vs `*const u8`) | toolchain now defines `core::ffi::c_char` unsigned on `xtensa-esp32-espidf`; 0.48.1 hardcodes `i8` | use the vendored crate: `[patch.crates-io] esp-idf-svc = { path = "vendor/esp-idf-svc" }` (see Phase 0 findings) |
| Reboot loop, `***ERROR*** A stack overflow in task main` | default main-task stack (~3.5 KB) too small for std Wi-Fi | `CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384` in `rust/sdkconfig.defaults` |
| bindgen: `Unable to find libclang` / `ToolNotFound: xtensa-esp-elf-gcc` | only IDF's `export.sh` sourced; espup's `LIBCLANG_PATH`/GCC missing | source `$RUST_ROOT/export-esp.sh` after the IDF export (build.sh does) |
| `No rule to make target '…/out/partitions.csv'` | custom partition CSV not copied into the esp-idf-sys CMake project | set `ESP_IDF_GLOB_BASE` + `ESP_IDF_GLOB_FILES=partitions.csv` (underscore before `FILES`) |
| `could not identify the root crate and ESP_IDF_SYS_ROOT_CRATE not specified` | workspace has no single root crate | set `ESP_IDF_SYS_ROOT_CRATE=rust-firmware` |
| `error: patch failed: components/app_update/esp_app_desc.c` | a killed esp-idf-sys build left the in-place weak patch applied | `git -C /opt/esp-idf-v4.4 checkout -- components/app_update/esp_app_desc.c` and rebuild |
| `cargo: command not found` in the container | toolchain not provisioned | run `rust/scripts/provision-rust.sh` into the volume |
| `error: no such command: +esp` | `espup install` missing or toolchain not exported | re-run provision; source `$RUSTUP_HOME/env`, set `CARGO_HOME` |
| `can't find crate for std` / `build-std` fails | `rust-src` component not installed for `esp` | `rustup component add rust-src --toolchain esp` (provision §4) |
| `linker 'ldproxy' not found` | `ldproxy` not installed | `cargo install ldproxy` (provision §4) |
| `/usr/bin/env: bad interpreter: Permission denied` running `rust/scripts/*.sh` | new files on the virtiofs mount are created without the execute bit | run via `bash scripts/provision-rust.sh`, or `chmod +x` the script |
| `Permission denied` writing `/opt/esp-rust/cargo` in the devcontainer | the volume is provisioned as root, but the devcontainer runs as `vscode` | the guarded `postStartCommand` chowns it; otherwise `sudo chown -R vscode:vscode /opt/esp-rust` (§4.0) |
| Devcontainer mount fails / `cargo` missing after rebuild | compose project name not pinned, so the volume is named after the checkout dir | add top-level `name: esp32simulated` and mount `source=esp32simulated_esp-rust` (§4.0) |
| I IDF `libc`/`time_t` link or size errors | wrong `espidf_time` cfg | set `RUSTFLAGS="--cfg espidf_time32"` on IDF 4.4 (not `time64`) |
| `ESP_IDF_TOOLS_INSTALL_DIR=fromenv` errors | IDF environment not activated | `. /opt/esp-idf-v4.4/export.sh` first (and unset ambient v5 vars) |
| Wrong IDF version built | ambient v5 `IDF_PATH` wins | unset `IDF_PATH IDF_PYTHON_ENV_PATH IDF_TOOLS_EXPORT_CMD IDF_TOOLS_INSTALL_CMD` before sourcing 4.4 |
| `esp-idf-svc` pulls an IDF-5-only release | version not pinned | pin `esp-idf-svc = "=0.48.1"` (last pre-v4.4-deprecation line) |
| `image_too_big` / app doesn't fit partition | default ~1 MB factory app | install `rust/partitions.csv` with a 3 MB factory app (§9) |
| Fewer than five modules in the banner | a registration was GC'd by `--gc-sections` | use the `register_modules!` macro fallback (§5.2) |
| Firmware crashes in `esp_phy_enable` (`phy_module_has_clock_bits`) | built against IDF 5.x | rebuild against `/opt/esp-idf-v4.4` (§9) |
| Second Wi-Fi trigger reboots, `assert: esp_netif_create_default_wifi_sta` | Wi-Fi init re-run | make Wi-Fi init one-shot; only reconnect |
| merged image ≠ 4 MB or fails to boot | missing bootloader/partition table or padding | re-merge with `--fill-flash-size 4MB` (§9) |
| `merge-bin` writes `build/build/...` | relative `-o` under `build/` | pass an absolute `-o` (§9) |
| Listener HTTP 503 | a build/test is already running | wait; requests are serialised |
| Listener 404 for a route in the file | host is running a stale listener process | restart `.devcontainer/docker-build-listener.py` on the host |

---

## 12. Risk register and the fallback decision

| Risk | Impact | Mitigation |
|---|---|---|
| `esp-idf-svc` 0.48.x won't compile against IDF 4.4.7 | blocks the whole stack | Phase 0.2 gate before porting; step back to the exact last-good `esp-idf-sys` if needed |
| Velxio fork won't boot an `esp-idf-svc` image | blocks all emulator parity | Phase 0.2 gate |
| `esp-idf-svc` Wi-Fi incompatible with fork | UC-5 never runs in emulator | Phase 0.4 gate; fallback below |
| std binary exceeds the flash partition | no boot | custom 3 MB factory partition (§9) |
| aarch64 Xtensa toolchain unavailable | blocks builds | Phase 0.3 gate (passed) |
| `linkme`/`inventory` registrations dropped | no autonomous registration | `register_modules!` macro fallback |
| Browser iced cannot reach the slirp guest | UI only works on real HW | scope the UI to real hardware/LAN; leave emulator UI open |

**Fallback (locked): single core, staged shells — do not build a second firmware up front.** If
the Phase 0 Wi-Fi spike fails:
1. `esp-idf-svc` remains the **only** firmware.
2. Emulator/browser parity is scoped to **UC-1** (the emulator reliably models GPIO).
3. UC-5 stays fully covered by **host `cargo test`** (pure logic) + real-hardware verification.

Rationale: the shared pure core makes deferring safe; a second firmware would double the
embedding surface for zero extra logic value.

---

## 13. References

- `AGENTS.md` §3 (module responsibilities), §5 (Velxio emulator), §6 (iteration, listener,
  WebSocket protocol, troubleshooting).
- `main/main.c`, `main/registry.{c,h}` — the engine shell and bus contract to mirror.
- `main/use_cases/toggle-physical-led/` — UC-1 reference (logic + edge-detect poll).
- `main/use_cases/fetch-and-uart/` — UC-5 reference (UART → Wi-Fi HTTP chain and markers).
- `test/test_toggle_led.c`, `test/test_event_bus.c` — host test references.
- `tests/velxio/scenarios/uc1_button_toggle.yaml`, `tests/velxio/scenarios/fetch_and_uart.yaml`.
- `scripts/build.sh`, `scripts/test.sh`, `scripts/provision-idf44.sh` — build/provision patterns.
- `docs/runbooks/add_use_case.md`, `docs/runbooks/add_testing_simulator.md`.
- esp-rs: `esp-idf-svc` (0.48.x), `esp-idf-hal`, `esp-idf-sys`; *The Rust on ESP Book*
  (std/`esp-idf-svc` track); `espflash` `save-image`.
