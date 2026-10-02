# ESP32-S3 Simulated — Zero-Allocation Elm Architecture

A production firmware project for the **ESP32-S3** written in **pure C11** using
The Elm Architecture (TEA). It is built with ESP-IDF, runs on real silicon, and
is exercised end-to-end on emulated ESP32-S3 silicon by a self-hosted
[Velxio](https://github.com/davidmonterocrespo24/velxio) stack — no hardware
required.

The whole system is driven by one top-level **`Makefile`**; this README is the
human guide to those commands.

---

## What it is

- **Functional Core, Imperative Shell.** Each use case is split into a pure
  `logic.c` (state structs passed strictly by value, no pointers, no `malloc`,
  no side effects) and an imperative `commands.c` that performs the actual
  GPIO/network/file I/O.
- **Autonomous use-case plugins.** Every feature folder has a `setup.c` that
  calls `USE_CASE_REGISTER(tag, self_module)`. A boot-time constructor appends
  the plugin to the platform registry — no central list, no ordering, no edits
  to `main.c`.
- **Decoupled event bus.** Plugins never call each other directly. They publish
  and subscribe to `SystemEventId` events; messages cross the bus as
  stack-value block copies (`memcpy`).
- **Exhaustive `switch` enforcement.** All pipelines switch over strict enums
  with `default:` banned. The build uses `-Wswitch-enum -Werror`, so adding an
  enum variant without handling it fails the build by design.

At runtime `app_main` creates one FreeRTOS queue per registered plugin, runs
hardware init and subscription wiring, drives inputs on a 100 ms `esp_timer`
tick, and dispatches queue items to each plugin's pure update function.

```
        constructor ──▶ registry_add_module ──▶ app_main
                                                   │
                        ┌──────────────────────────┼──────────────────────────┐
                        ▼                           ▼                          ▼
                 plugin queues                esp_timer 100 ms          event bus (pub/sub)
                 (FreeRTOS, by value)         poll_timer_tick           memcpy, size-matched
                        │
                        ▼
              logic.c (pure) ──▶ Cmd ──▶ commands.c (I/O)
```

---

## Requirements

| Where | Needs |
|---|---|
| **Host** (PC / NAS / server) | Docker + Docker Compose. Runs the Velxio image (ESP-IDF v5.5, Xtensa toolchain, QEMU, nginx). |
| **This devcontainer** | Python 3 and `curl` only. It has **no Docker daemon**, so builds and emulator runs go through the host listener. |

The Velxio image ships ESP-IDF and QEMU but **no host C compiler**; the Compose
startup installs `gcc make libc6-dev` plus the Python test deps on each start.

---

## Quick start

### 1. Start the host listener (on the host)

```sh
python3 .devcontainer/docker-build-listener.py     # or: make listener
```

It listens on `0.0.0.0:2222` and drives `docker compose` on your behalf. From
the devcontainer it is reached at `host.docker.internal:2222`.

### 2. Start the Velxio stack

```sh
make up          # docker compose up --build -d (first run pulls a multi-GB image)
make ps
make logs
```

### 3. Build and test

```sh
make build       # idf.py build + merge a 4 MB firmware image
make test        # native tests + firmware build + UC-1 emulator scenario
```

`make test` runs the native C unit tests, builds the firmware, and drives the
UC-1 button/LED scenario in the emulator. A successful run ends with
`RESULT: PASS` and exit code `0`; the boot log reports
`Platform Engine Initializing: Found 5 Autonomous Modules.`

### 4. Iterate

```sh
make native-test                 # fast local C tests only, no Docker
make scenario NAME=uc1_button_toggle
make exec CMD='ls -la /workspace'
make down
```

> Running from the host itself? Point the Makefile at the host listener:
> `make LISTENER=http://localhost:2222 up test`.

---

## Makefile commands

| Command | What it does |
|---|---|
| `make listener` | Start the host build/test listener (run on the host). |
| `make up` | `docker compose up --build -d` — pull + start the Velxio stack. |
| `make down` | Stop the stack. |
| `make restart` | `down` then `up`. |
| `make logs` | Tail the Velxio container log. |
| `make ps` | Show container status. |
| `make build` | Build firmware: `idf.py build` + 4 MB `merge-bin`. |
| `make test` | Native tests + firmware build + UC-1 emulator scenario. |
| `make native-test` | Desktop C unit tests only (`make -C test`). |
| `make scenario NAME=<name>` | Run one emulator scenario (default `uc1_button_toggle`). |
| `make exec CMD='<shell>'` | Run a shell command inside the Velxio container. |
| `make clean` | Remove native binaries and ESP-IDF build output. |
| `make help` | List the targets (the default target). |

Override the listener with `LISTENER=http://…` (e.g. `http://localhost:2222`
when the listener runs on the same host as your shell).

---

## Project layout

```
.
├── Makefile                  # top-level command surface (this README)
├── CMakeLists.txt            # ESP-IDF project entry
├── docker-compose.yaml       # OSS Velxio service (emulator + ESP-IDF)
├── sdkconfig.defaults        # 4 MB flash (matches the QEMU machine)
├── main/
│   ├── main.c                # imperative shell / platform runtime
│   ├── registry.{c,h}        # event bus + autonomous plugin registry
│   └── use_cases/
│       ├── toggle-physical-led/   # [UC-1] boot button -> LED (implemented)
│       └── …                      # other use-case plugins (stubs)
├── test/                     # native desktop C tests (mock event bus)
├── tests/velxio/             # Wokwi circuit + YAML scenarios + Python WS runner
├── scripts/                  # build.sh / test.sh / scenario.sh
└── AGENTS.md                 # architecture + iteration contract
```

`build/`, `sdkconfig`, and Python caches are generated and gitignored.

---

## Testing

**Native (host) tests** — pure logic with a mock event bus, no hardware:

```sh
make native-test
# test_toggle_led: PASS
# test_event_bus: PASS
```

**Emulator tests** — the real firmware on emulated ESP32-S3, driven over a
WebSocket session. Each use case gets a `diagram.json` circuit and a
`scenarios/*.yaml` file:

```sh
make build
make scenario NAME=uc1_button_toggle
```

Because the OSS emulator bridge does not reliably emit output-pin `gpio_change`
events, UC-1 asserts the LED through a firmware `printf`
(`# LED ON (gpio 2)` / `# LED OFF (gpio 2)`) and `wait-serial`, rather than
reading the pin. Input injection (`esp32_gpio_in`) does work.

---

## Status

- **UC-1 (`toggle-physical-led`)** is implemented end-to-end: BOOT button on
  GPIO0 toggles the LED on GPIO2, validated natively and in the emulator.
- **UC-5 (`fetch-and-uart`)** is implemented end-to-end: a DB-query result sends
  over UART and then syncs the network, triggered by the shared BOOT button and
  validated natively and in the emulator (`make scenario NAME=fetch_and_uart`).
- The remaining use-case plugins (`read-analog-sensor`, `fetch-and-save`,
  `read-and-insert`) exist and compile but their `commands.c` implementations
  are stubs.

---

## Extending

Run **`docs/runbooks/add_use_case.md`** to take one use-case plugin from stub to
end-to-end working — it selects the next stub, then walks logic, commands,
registration, native tests, and the emulator scenario. Re-runnable once per use
case.

The operational recipes live in **`AGENTS.md`**:

- **§6.2** — add a use case (the two required `main/CMakeLists.txt` edits, and
  why they are needed).
- **§6.3** — add a scenario / extend the harness (step vocabulary, circuit
  mapping, the `gpio_change` caveat).
- **§6.5–6.6** — definition of done and a troubleshooting table of issues
  actually observed while building this project.

Full emulator protocol background and rationale:
`docs/runbooks/add_testing_simulator.md`.
