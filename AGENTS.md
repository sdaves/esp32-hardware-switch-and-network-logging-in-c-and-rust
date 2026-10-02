# 📄 AGENTS.md

This document serves as the absolute technical specification and implementation source for a **Zero-Allocation, Linker-Reflected Elm Architecture (TEA)** executed entirely within **Pure Production C (C11/C23)** on the **ESP32-S3** microcontroller.

---

## 🏗️ 1. Architectural Architecture & Core Rules

To eliminate the traditional instability of embedded C software, this system enforces a **"Functional Core, Imperative Shell"** pattern through four rigid structural boundaries:

1. **Strict Pass-by-Value Semantics (`logic.c`):** Business logic modules accept state structs and inbound messages strictly **by value on the CPU stack**, returning a compound `UpdateResult` struct containing the next state and an output Command declaration. There are **zero pointers, zero `malloc()` calls, and zero heap operations** allowed in the functional core. This structurally eliminates race conditions, memory leaks, and null-pointer dereferencing panics.
2. **Linker-Reflected Modular Autonomy (`setup.c`):** Top-level orchestrators (`main.c`) contain **zero feature includes, forward declarations, or manual registration arrays**. Use-case features register themselves autonomously: each `setup.c` calls `USE_CASE_REGISTER(tag, self_module)`, which emits an `__attribute__((constructor))` that appends its `UseCaseModule` to the platform registry at boot (`registry_add_module`). No central list and no ordering, so adding or deleting a feature requires no modification to the core runtime framework. Because ESP-IDF links with `--gc-sections`, the component force-references each constructor with `-Wl,--undefined=_uc_register_<tag>` (see `main/CMakeLists.txt`) so the `.ctors` slot and constructor body are retained. Registration is autonomous at *runtime*, but adding a plugin still requires two *build-time* edits in `main/CMakeLists.txt` (source list and the `UC_MODULES` tag list); see §6.2.
3. **Decoupled Universal Event Bus (`registry.c`):** Features never directly invoke or reference other feature folders. Inter-module communication occurs via an asymmetric Publish-Subscribe pattern managed by the platform runtime. Message structures are safely moved across the bus through raw stack-value block copies (`memcpy`).
4. **Exhaustive Pattern Matching Enforcement (`domain.h`):** All runtime event and command pipelines utilize plain C `switch` blocks over strict `enum` types **with all `default:` keywords explicitly banned**. Paired with compiler configuration flags (`-Wswitch-enum -Werror=switch`), the compiler will instantly fail the system build if an item payload or feature message variant is added but unhandled.

---

## 📁 2. Standardized Directory Layout

```text
my_esp32_project/
├── CMakeLists.txt                       # Top-Level ESP-IDF Project Entry Point
├── .gitignore                           # Build Artifact & sdkconfig Exclusions
├── sdkconfig.defaults                   # 4 MB flash (matches Velxio QEMU machine)
├── docker-compose.yaml                  # OSS Velxio service (emulator + ESP-IDF toolchain)
├── main/
│   ├── CMakeLists.txt                  # Production ESP-IDF Component Recipe Configuration
│   ├── main.c                          # Sealed Immutable Platform Runtime Engine Shell
│   ├── registry.c                      # Backing Store for Event Bus & Plugin Registry
│   ├── registry.h                      # Public Type Contracts & Autonomous Registration Macros
│   └── use_cases/
│       ├── toggle-physical-led/         # [UC-1] Dynamic Physical Input Pin Toggler
│       │   ├── domain.h                #   ├── Local type structures & algebraic enums
│       │   ├── logic.c                 #   ├── 100% Pure stack-by-value business rules
│       │   ├── commands.c              #   ├── Imperative physical peripheral driver
│       │   └── setup.c                 #   └── Abstract plugin connector & bus bindings
│       ├── read-analog-sensor/          # [UC-2] Conditional Server Poster with Error Routing
│       │   ├── domain.h | logic.c | commands.c | setup.c
│       ├── fetch-and-save/              # [UC-3] HTTP GET with ISO8601 File Writer
│       │   ├── domain.h | logic.c | commands.c | setup.c
│       ├── read-and-insert/             # [UC-4] Periodic ADC to SQLite Transact Logger
│       │   ├── domain.h | logic.c | commands.c | setup.c
│       └── fetch-and-uart/              # [UC-5] SQLite Query -> UART -> Network Sync Chain
│           ├── domain.h | logic.c | commands.c | setup.c
├── scripts/
│   ├── build.sh                         # In-container ESP-IDF build + 4 MB merge-bin
│   └── test.sh                          # Native tests + firmware build + UC-1 scenario
├── test/                                # Native PC Desktop Testing Suite Sandbox
│   ├── Makefile                         # Desktop build pipeline and runner orchestrator
│   ├── mock_registry.c                  # Non-FreeRTOS local simulation of the Event Bus
│   ├── test_toggle_led.c                # Unit validation checking for [UC-1] Logic rules
│   └── test_event_bus.c                 # Integration validation checking for multi-step routing
└── tests/velxio/                        # Python WebSocket simulator harness
    ├── diagram.json                     # Wokwi circuit (parts + board connections)
    ├── scenarios/*.yaml                 # Per-use-case scenarios (all run by `test.sh`)
    ├── esp32simulated.vlx               # Importable Velxio project for the browser GUI
    └── runner/                          # ws_client / diagram_map / scenario / run_scenario
```

`build/` and the root `sdkconfig`/`sdkconfig.old` are generated by the ESP-IDF build
(gitignored); the native `test/` binaries and objects are also ignored.

---

## 🛠️ 3. Production Component Manifest

The runtime is composed of a small, fixed platform core plus self-registering use-case plugins. Each file below already exists in the repository; the source is the single source of truth, and this section only summarizes its responsibility.

### `main/registry.h`
Public type contracts for the whole platform. Declares the `SystemEventId` event enum, the `UseCaseModule` "vtable" (name, queue, message size, and the `init_hardware` / `process_queue_item` / `poll_timer_tick` / `wire_subscriptions` callbacks), the subscriber/message-size limits, the event-bus API, the `registry_add_module` / `registry_modules` runtime store, and the `USE_CASE_REGISTER(tag, module)` macro that emits an autonomous registration constructor. It also branches on `PLATFORM_HOST_TEST` to swap FreeRTOS queue types for a plain host type, so the same headers compile on desktop.

### `main/registry.c`
The event bus backing store plus the plugin registry. Holds a static subscription matrix (queue handles + declared message sizes per event) and count. `event_bus_subscribe` records a subscriber with the message size it expects; `event_bus_publish` broadcasts a payload by `memcpy` only to subscribers whose declared size matches, with bounds checks on the event id and subscriber limit. `registry_add_module` / `registry_module_count` / `registry_modules` back the autonomous plugin list.

### `main/main.c`
The imperative shell / sealed engine host. On boot it reads the autonomously registered modules from `registry_modules()`, validates each message size, creates a FreeRTOS queue per module, then runs hardware init and subscription wiring. A periodic `esp_timer` drives `poll_timer_tick` on every module, and the main loop non-blockingly drains each module queue and dispatches to `process_queue_item`, sleeping briefly when idle.

### Use-case plugins (`main/use_cases/*`)
Each feature folder is autonomous and follows the same four-file split, so features never reference each other directly:
- `domain.h` — local state/message/command type contracts and algebraic enums.
- `logic.c` — the pure functional core; stack-by-value rules with no pointers, heap, or side effects.
- `commands.c` — the imperative shell; translates a declared command into real peripheral/network/file actions.
- `setup.c` — the connector; declares the `UseCaseModule`, implements callbacks, and wires bus subscriptions.

The five plugins cover physical LED toggling (UC-1), conditional analog-sensor posting with error routing (UC-2), HTTP GET to an ISO8601 file writer (UC-3), periodic ADC-to-SQLite logging (UC-4), and a SQLite-query → UART → network-sync chain (UC-5).

---

## 🧪 4. Non-Embedded Local Testing Architecture

The same pure logic and bus contracts are exercised on a desktop PC by substituting the FreeRTOS-backed bus with a static host mock, so no hardware is required to validate behavior.

### `test/mock_registry.c`
Host-only (`PLATFORM_HOST_TEST`) replacement for the event bus. Implements the same `event_bus_subscribe` / `event_bus_publish` API using fixed static mailboxes instead of queues, and provides `mock_bus_create_mailbox`, `mock_bus_has_message`, and `mock_bus_read` so tests can inspect delivered payloads.

### `test/test_toggle_led.c`
Unit tests for the UC-1 pure logic. Verifies the initial state is LED-off and that a `MSG_BUTTON_DOWN` message produces the expected `CMD_TOGGLE_LED` command with the correct payload.

### `test/test_event_bus.c`
Integration tests for routing. Covers single-subscriber delivery, broadcast to multiple subscribers, and correct ignoring of payloads whose size does not match a subscriber's declared message size.

### `test/Makefile`
Desktop build/run harness. Compiles each test target with strict C11 warnings (`-Wall -Wextra -Wswitch-enum -Werror`) against the mock bus and the relevant pure logic, then runs both executables.

---

## 🖥️ 5. Velxio Simulator Testing (emulated ESP32)

The real production firmware is built with ESP-IDF and run on emulated ESP32 silicon by a
self-hosted OSS Velxio stack, then driven by Python WebSocket scenarios. See
`docs/runbooks/add_testing_simulator.md` for the full protocol and rationale.

**Target board:** the emulator's QEMU fork only models a Wi-Fi radio on the **classic ESP32**
(`esp32-picsimlab`), not the ESP32-S3 (`hw/xtensa/esp32s3.c` never attaches a NIC). UC-5 does a
real Wi-Fi + HTTP fetch, so the build target, the circuit, the `.vlx`, and the harness all use
`esp32`. Additionally, the fork's radio only initialises under **ESP-IDF 4.4** — IDF 5.x's
`esp_phy_enable` asserts on a modem-clock register the fork does not model. `scripts/build.sh`
therefore prefers an ESP-IDF v4.4.7 tree at `/opt/esp-idf-v4.4` when present.

### `docker-compose.yaml`
Runs the OSS Velxio image (`ghcr.io/davidmonterocrespo24/velxio:master`) with the repo mounted
at `/workspace`. Because the image ships ESP-IDF but no host C compiler, the startup command
installs `gcc make libc6-dev` (for the native tests) and the `websockets pyyaml` Python deps
before launching `/app/entrypoint.sh` **from `/app`** (the backend imports `app.main`, which is
only importable with that working directory). Ports: host `3080 → nginx :80`, host `8001 → uvicorn`.

### `scripts/build.sh` / `scripts/test.sh`
`build.sh` prefers an **ESP-IDF v4.4.7** tree at `/opt/esp-idf-v4.4` (clearing the container's
ambient v5 `IDF_PATH` first), sets target `esp32` once, builds, then merges a 4 MB flash image
with the bundled `esptool.py merge_bin --fill-flash-size 4MB`. `test.sh` runs `make -C test`,
then `build.sh`, then starts `scripts/dev-http-server.sh` (so UC-5's Wi-Fi GET has an endpoint on
the container's `:8000`), then runs **every** scenario in `tests/velxio/scenarios/*.yaml` in
filename order (a failing scenario aborts the run). This is the definition of `make test`: native
tests + firmware build + the full emulator scenario suite. Adding a use case's YAML under
`scenarios/` automatically extends `make test` — no edit to `test.sh`. Both scripts default to
the in-container addresses (`VELXIO_HTTP=http://localhost`, `VELXIO_WS=ws://localhost`); override
them to target the host-published ports.

### `scripts/dev-http-server.{sh,py}`
A reverse proxy on the container's `:8000`. The QEMU guest reaches the container at the slirp
gateway `192.168.4.2`, so UC-5's firmware does `GET http://192.168.4.2:8000/editor`; the server
forwards the request to the parent machine at `VELXIO_PROXY_UPSTREAM` (default
`http://host.docker.internal:8000`) and returns its real response. If the parent is unreachable,
it answers with a deterministic `200` stub (`VELXIO_PROXY_FALLBACK=1`, the default) so
`make test` stays green; set `VELXIO_PROXY_FALLBACK=0` to surface a `502`. `start|stop|status`
via the shell wrapper; `test.sh` starts it automatically.

### `tests/velxio/`
`diagram.json` (Wokwi circuit), `scenarios/*.yaml` (per-use-case steps, all driven by `test.sh`),
`esp32simulated.vlx` (importable Velxio project for the browser editor), and `runner/`
(`ws_client.py` session, `diagram_map.py` circuit→pin resolver, `scenario.py` step executor,
`run_scenario.py` CLI). `run_scenario.py` exits `0` pass, `1` fail, `2` config error.

### Host-side Docker control — `.devcontainer/docker-build-listener.py`
The devcontainer has no Docker daemon. The **host** runs
`python3 .devcontainer/docker-build-listener.py` (listens on `0.0.0.0:2222`); the agent calls it
with HTTP to drive `docker compose` and report results. Routes:

| curl (from the devcontainer) | Effect |
|---|---|
| `curl http://host.docker.internal:2222/health` | liveness (`ok`) |
| `curl -sS http://host.docker.internal:2222/build` | `docker compose up --build -d` (streams log) |
| `curl -sS http://host.docker.internal:2222/up` | `docker compose up -d` |
| `curl -sS --max-time 2400 http://host.docker.internal:2222/build-firmware` | runs `scripts/build.sh` inside the container |
| `curl -sS --max-time 1200 http://host.docker.internal:2222/test` | runs `scripts/test.sh` inside the container (native tests + firmware + all emulator scenarios) |
| `curl -sS http://host.docker.internal:2222/logs` | `docker compose logs --tail=2000 --no-color` |
| `curl -sS http://host.docker.internal:2222/ps` | `docker compose ps` |
| `curl -sS http://host.docker.internal:2222/down` | `docker compose down` |
| `curl -sS -G --data-urlencode 'cmd=<shell>' http://host.docker.internal:2222/exec` | arbitrary command in the `velxio` container |

`host.docker.internal` must resolve to the host from the devcontainer; from the host itself use
`http://localhost:2222`. The listener serialises requests (HTTP 503 if a build/test is already
running) and streams command output back as the response body.

### Emulator output reality
The QEMU machine emits `serial_output`, `system`, `gpio_pull`, `gpio_change`, etc. For UC-1 the
button input does drive the guest, but the LED assertion is done through a firmware `printf`
(`# LED ON (gpio 2)` / `# LED OFF (gpio 2)`) and `wait-serial`, because output-pin `gpio_change`
events are not reliably emitted by the OSS bridge.

---

## 🔁 6. Iterating on This Repo

This section is the operational contract for changing firmware or adding tests. It records what
was built, how to run it, and the reasoning behind each decision, so an agent can iterate without
re-discovering the toolchain.

### 6.1 Agent quickstart (host listener)

Docker and ESP-IDF live on the **host**, not in the devcontainer. Drive them through the host
listener (see §5). From the devcontainer:

```sh
# 1. Is the host listener alive?
curl -sS --max-time 10 http://host.docker.internal:2222/health      # -> ok

# 2. First time only: pull the image and start the Velxio stack.
curl -sS --max-time 2400 http://host.docker.internal:2222/build

# 3. Iterate: build firmware, then run native + emulator tests.
curl -sS --max-time 2400 http://host.docker.internal:2222/build-firmware
curl -sS --max-time 1200 http://host.docker.internal:2222/test
```

Notes:

- Requests are serialised: a second concurrent build/test returns HTTP 503 (`busy`). Wait for the
  first to finish rather than retrying.
- Use generous `--max-time` values; the first `/build` pulls a multi-GB image and the first
  firmware build compiles ~1090 targets.
- Build artifacts are written inside the container's `/workspace`, which is the host repo mount,
  so `build/firmware.merged.bin` appears on the host after a successful build.
- **The listener is a long-running host process. After editing
  `.devcontainer/docker-build-listener.py`, the host must restart it** before new routes take
  effect. (A stale process is how a route can 404 even though the file is correct.)
- Run a single scenario without rebuilding, using `/exec`:

```sh
curl -sS --max-time 300 -G --data-urlencode 'cmd=cd /workspace && python3 -m tests.velxio.runner.run_scenario --server ws://localhost --firmware build/firmware.merged.bin --diagram tests/velxio/diagram.json --scenario tests/velxio/scenarios/uc1_button_toggle.yaml' \
  http://host.docker.internal:2222/exec
```

### 6.2 Add a use case (build-time coupling)

For the full step-by-step, run `docs/runbooks/add_use_case.md` (it selects the next stub and
covers logic, commands, registration, native tests, and the emulator scenario). The build-time
summary is:

Runtime registration is autonomous, but the ESP-IDF component must still compile the new
translation units. Adding a plugin means:

1. Create `main/use_cases/<name>/{domain.h,logic.c,commands.c,setup.c}` following the four-file
   split in §3.
2. In `setup.c`, register with a unique tag:
   `USE_CASE_REGISTER(<tag>, self_module);` (e.g. the current UC-1 tag is `toggle_physical_led`).
3. Edit `main/CMakeLists.txt`:
   - add the three `.c` paths (`logic.c`, `commands.c`, `setup.c`) to `SRCS`;
   - add the directory to `INCLUDE_DIRS`;
   - add `<tag>` to the `UC_MODULES` list (this generates `-Wl,--undefined=_uc_register_<tag>`).

Why both edits are required: IDF compiles only the files named in `SRCS`, and it links with
`--gc-sections` under `-ffunction-sections`, so an unreferenced `__attribute__((constructor))` —
and its `.ctors` slot — is discarded. The `-u` flag is what keeps it. Missing either step makes
the module silently disappear; the only symptom is a smaller `Found N Autonomous Modules.` banner.

Verify: the boot log must read `Platform Engine Initializing: Found 5 Autonomous Modules.`
(the count equals the number of registered plugins).

### 6.3 Add a scenario / extend the harness

- **Circuit:** `tests/velxio/diagram.json` (Wokwi format). `diagram_map.py` resolves a component
  id to a board GPIO by walking `connections`; it ignores power pins (`GND*`, `VIN*`, `3V3*`,
  `5V*`, `EN`) and prefers numeric pins. A part wired to the board only through a passive (e.g. an
  LED behind a resistor) will not resolve, so assert against the board pin instead.
- **Scenario:** `tests/velxio/scenarios/<name>.yaml`. Each step is a mapping with exactly one key:

  | Step | Behaviour |
  |---|---|
  | `delay: 300ms` / `1.5s` | sleeps |
  | `wait-serial: "text"` | polls the rolling serial buffer for a substring (fails on timeout) |
  | `write-serial: "text"` / `[72, 73]` | sends bytes to UART RX |
  | `set-control: {part-id: btn1, control: pressed, value: 1}` | drives the mapped GPIO **active-low** (pressed = level 0) |
  | `set-control: {part-id: pot1, control: value, value: 2000}` | `esp32_adc_set` in millivolts |
  | `expect-pin: {part-id: esp, pin: "2", expected: 1}` | waits for a `gpio_change` (see caveat) |

- **CLI:** `python3 -m tests.velxio.runner.run_scenario --server ws://localhost --firmware build/firmware.merged.bin --diagram tests/velxio/diagram.json --scenario tests/velxio/scenarios/uc1_button_toggle.yaml`
  Exit codes: `0` pass, `1` fail, `2` configuration error. `--timeout` sets the per-step timeout
  (default 15 s).
- **Caveat (observed):** the OSS bridge does not reliably emit output-pin `gpio_change` events, so
  `expect-pin` can never match for an output. Assert firmware behaviour with a `printf` marker and
  `wait-serial` instead (UC-1 uses `# LED ON (gpio 2)` / `# LED OFF (gpio 2)`). Input injection via
  `esp32_gpio_in` does work, and `gpio_pull`/`gpio_change` are observed for inputs and pullups.
- A one-command wrapper (`VELXIO_WS` defaults to `ws://localhost`) is provided:
  `./scripts/scenario.sh uc1_button_toggle`.

### 6.4 Native (host) tests

`make -C test` compiles the pure logic plus `test/mock_registry.c` with
`-DPLATFORM_HOST_TEST -Wall -Wextra -Wswitch-enum -Werror` and runs each target. To add a test:
add an object rule for the use case's `logic.c` (with `-I../main -I../main/use_cases/<name>`),
link it with `mock_registry.o`, and add the binary to the `all` target. Command/event `switch`
statements must enumerate every value with no `default:` — the `-Wswitch-enum -Werror` flags make
that a hard build failure by design.

### 6.5 Definition of done (per iteration)

1. `make -C test` → `test_toggle_led: PASS`, `test_event_bus: PASS`, `test_fetch_and_uart: PASS`.
2. `/build-firmware` exits 0 and the boot log prints
   `Platform Engine Initializing: Found 5 Autonomous Modules.`
3. `/test` prints `RESULT: PASS` for every scenario under `tests/velxio/scenarios/` and exits 0
   (native tests + firmware + all emulator scenarios).
4. No new compiler warnings under `-Werror`.

### 6.6 Troubleshooting (observed)

| Symptom | Cause | Fix |
|---|---|---|
| `Found 0` (or fewer than expected) modules | plugin tag missing from `UC_MODULES`, or sources missing from `SRCS`; constructor was GC'd | add both edits from §6.2 |
| `undefined reference to __start_platform_registry` | a custom linker `.ld`/section-walk was reintroduced | do not; registration is constructor-based (`USE_CASE_REGISTER` + `registry_add_module`) |
| Backend unreachable / `curl :3080` refused | `uvicorn app.main:app` only imports from cwd `/app` | compose command must `cd /app` before `exec ./entrypoint.sh` |
| `make: command not found` inside the container | the Velxio image ships ESP-IDF but no host C toolchain | compose startup installs `gcc make libc6-dev` |
| `merge-bin` `FileNotFound: build/firmware.merged.bin` | `merge-bin` runs *inside* `build/`, so a relative `-o` becomes `build/build/...` | pass an absolute `-o "$PWD/build/firmware.merged.bin"` |
| Firmware does not boot / flash-size error | QEMU machine is fixed at 4 MB | `sdkconfig.defaults` sets 4 MB and build.sh passes `--fill-flash-size 4MB` |
| Second button press has no effect | 2-tick debounce needs hold/release margin | scenario delays ≥400 ms |
| `expect-pin` never matches | output-pin `gpio_change` not emitted by the OSS bridge | assert via firmware `printf` + `wait-serial` |
| Listener returns HTTP 503 | a build/test is already running | wait; requests are serialised |
| Listener returns 404 for a route that exists in the file | host is still running the old listener process | restart `.devcontainer/docker-build-listener.py` on the host |

### 6.7 WebSocket protocol subset

The harness speaks directly to the OSS simulation route (no `velxio-cli`; see the runbook for why):

- URL: `ws://<host>/api/simulation/ws/<client_id>` — always use a **unique** `client_id` so a
  browser tab cannot collide with a test.
- to server: `start_esp32 {board:"esp32", firmware_b64, wifi_enabled:true}`,
  `esp32_gpio_in {pin, state}`, `esp32_adc_set {channel, millivolts}`,
  `esp32_serial_input {bytes:[...], uart}`, `stop_esp32`.
- from server: `serial_output {data, uart}`, `gpio_change {pin, state}`, `gpio_pull {pin, pull}`,
  `system {event: booting|booted|...}`.
- In-container nginx listens on `:80` (host-published on `:3080`); uvicorn listens on
  `127.0.0.1:8001`. `scripts/test.sh` therefore defaults to `http://localhost` / `ws://localhost`
  inside the container; set `VELXIO_HTTP`/`VELXIO_WS` to `:3080` when running from the host.

### 6.8 Environment constraints and why

- **No Docker/ESP-IDF in the devcontainer** — all firmware builds and emulator runs go through the
  host listener and `docker compose`.
- **Velxio image has no host compiler** — the native C tests would fail with
  `make: command not found`, so the compose startup installs `gcc make libc6-dev` alongside the
  Python deps each start (the image is otherwise ESP-IDF + QEMU + arduino-cli).
- **Backend working directory** — `app.main` is importable only relative to `/app`; the repo is
  mounted at `/workspace`. The compose command `cd /app && exec ./entrypoint.sh` starts the
  backend from the right cwd while `docker compose exec` still runs in `/workspace`.
- **`--gc-sections` + `-ffunction-sections`** — the reason constructors are force-referenced with
  `-u`. This replaced an earlier attempt to make the linker reflect a custom
  `.platform_registry` section, which the generated ESP-IDF linker script dropped silently.
- **4 MB flash is fixed** by the QEMU machine; a 2 MB image does not boot.
- `build/`, `sdkconfig`, and `sdkconfig.old` are generated and gitignored; never commit them.

