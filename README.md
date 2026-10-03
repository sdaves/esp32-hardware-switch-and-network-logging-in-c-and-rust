# ESP32-S3 Simulated — Zero-Allocation Elm Architecture

A production firmware project for **ESP32** written in **pure C11** using
The Elm Architecture (TEA). It is built with ESP-IDF, runs on real silicon, and
is exercised end-to-end on emulated ESP32 silicon by a self-hosted
[Velxio](https://github.com/davidmonterocrespo24/velxio) stack — no hardware
required. The emulated target is the **classic ESP32** because it is the only
board whose QEMU machine models a Wi-Fi radio (UC-5 does a real network fetch).

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
| **This devcontainer** | Python 3 and `curl`, plus the shared `esp-rust` Rust toolchain + crate cache mounted from the host stack (`.devcontainer/devcontainer.json`). It has **no Docker daemon**, so firmware builds and emulator runs go through the host listener. |

The Velxio image ships ESP-IDF (v5.x) and QEMU but **no host C compiler**, so the
Compose startup installs `gcc make libc6-dev` and the Python test deps. It also
provisions **ESP-IDF v4.4.7** into a named volume (`scripts/provision-idf44.sh`)
for the emulated ESP32 Wi-Fi build — the first start after a fresh clone or
`down -v` takes a few minutes to clone and install it, then it is cached.

The devcontainer is entirely described by `.devcontainer/devcontainer.json` (base
image + features) and rebuilds itself on a new machine; it needs no repo-specific
setup beyond Python/`curl`, since all firmware work happens on the host stack.

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
make test        # native tests + firmware build + ALL emulator scenarios
```

`make test` runs the native C unit tests, builds the firmware, and then runs
**every** scenario in `tests/velxio/scenarios/*.yaml` (currently UC-1 and UC-5),
in filename order. Each scenario ends with `RESULT: PASS`, the command exits `0`
only if all of them pass (a failing scenario aborts the run), and the boot log
reports `Platform Engine Initializing: Found 5 Autonomous Modules.` Adding a new
`scenarios/*.yaml` automatically extends `make test` — no edit to any script.

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
| `make test` | Native tests + firmware build + all emulator scenarios (`tests/velxio/scenarios/*.yaml`). |
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
│   └── esp32simulated.vlx    # importable Velxio project for the browser editor
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

**Emulator tests** — the real firmware on emulated ESP32 silicon, driven over a
WebSocket session. Each use case gets a `scenarios/*.yaml` file (sharing the
`diagram.json` circuit):

```sh
make test                       # native tests + firmware + every scenario
make build
make scenario NAME=uc1_button_toggle   # run just one scenario, no rebuild
```

`make test` executes **all** files under `tests/velxio/scenarios/` in filename
order and aborts on the first failure; `make scenario NAME=…` runs a single one.

Because the OSS emulator bridge does not reliably emit output-pin `gpio_change`
events, UC-1 asserts the LED through a firmware `printf`
(`# LED ON (gpio 2)` / `# LED OFF (gpio 2)`) and `wait-serial`, rather than
reading the pin. Input injection (`esp32_gpio_in`) does work.

UC-5 does a **real Wi-Fi + HTTP fetch**: pressing its NET button (GPIO4) joins
the emulator's `Espressif` AP, gets an IP, and `GET`s
`http://192.168.4.2:8000/` (the slirp gateway, i.e. this container).
`scripts/dev-http-server.py` is a reverse proxy: it forwards that request to the
parent machine at `http://host.docker.internal:8000` and returns the real
response. The scenario asserts `# WIFI CONNECTED`, `-> status 200`, a response
body, and `# NETWORK SYNCED`. When the parent's `:8000` is unreachable the proxy
answers a deterministic `200` stub so `make test` still passes
(`VELXIO_PROXY_FALLBACK=0` to require the parent instead).

> **Board and toolchain note.** The QEMU fork only models a Wi-Fi radio on the
> **classic ESP32**, and only under **ESP-IDF 4.4** (IDF 5.x's `esp_phy_enable`
> asserts on a register the fork lacks). The project therefore targets `esp32`
> and `scripts/build.sh` uses an ESP-IDF v4.4.7 tree at `/opt/esp-idf-v4.4`. The
> image does not ship it, so `docker-compose.yaml` provisions it on container
> start (via `scripts/provision-idf44.sh`) into the `idf44` named volume — the
> first `make up` after `down -v` clones and installs for a few minutes.
>
> Two buttons are wired: BOOT (GPIO0) drives UC-1, NET (GPIO4) drives UC-5.

---

## Interactive browser debugging

`make test` runs the firmware headlessly, but you can also drive the very same
firmware by hand in the Velxio web GUI — press the on-screen button, watch the
LED, and read the serial monitor live. The Python harness and the GUI share one
emulator backend, so what you see here matches the automated scenario.

The stack already serves the editor at **http://localhost:3080/editor**. No
project source files are compiled in the browser; you import a pre-built circuit
and upload the `.bin` you build here.

### 1. Build the firmware image

```sh
make build          # produces build/firmware.merged.bin (4 MB, QEMU-ready)
```

### 2. Open the editor

In a browser on the host, go to **http://localhost:3080/editor**.

### 3. Import the circuit

`tests/velxio/esp32simulated.vlx` declares the board and wiring that the CLI
scenarios use — a classic **ESP32** with two buttons and an LED:

1. In the editor, open **Import project**.
2. Choose `tests/velxio/esp32simulated.vlx`.
   (The editor imports a `.vlx` project, not a folder.)

The canvas should show the ESP32 board with:
- **BOOT** pushbutton on **GPIO0** → drives UC-1 (LED toggle)
- **NET** pushbutton on **GPIO4** → drives UC-5 (Wi-Fi + HTTP)
- `GPIO2 → 220 Ω → LED anode (A)`, `LED cathode (C) → GND`

Both buttons are active-low (external pull-up high; pressing pulls the pin low),
matching the firmware's `INPUT_PULLUP` reads. The classic ESP32 is required
because it is the only board whose emulated QEMU machine has a Wi-Fi radio.

### 4. Upload the firmware binary

1. Use the editor's **Upload firmware** action (`.bin`, `.hex`, `.elf`, `.ihex`).
2. Select `build/firmware.merged.bin` — uploading a prebuilt image bypasses the
   in-browser compiler entirely.
3. Start the simulation.

### 5. Drive it and read the output

- Open the **Serial Monitor** at **115200** baud. The boot log should read
  `Platform Engine Initializing: Found 5 Autonomous Modules.`
- Press **BOOT** (on-screen `btn1`): toggles the LED — `# LED ON (gpio 2)` /
  `# LED OFF (gpio 2)`. Each toggle also publishes `EVENT_LED_TOGGLED`, so UC-5
  additionally fetches `# HTTP GET /?led=on` / `# HTTP GET /?led=off`.
- Press **NET** (on-screen `btn2`): UC-5 joins the emulated `Espressif` AP and
  fetches over the network. The monitor prints `# UART SENT`,
  `# WIFI CONNECTED (Espressif)`, `# HTTP GET /?led=off` (the cached LED state),
  `-> status 200`, the response body, then `# NETWORK SYNCED`.

### Notes

- UC-5's fetch targets `http://192.168.4.2:8000/`, the slirp gateway (the
  container), which reverse-proxies to the parent machine at
  `http://host.docker.internal:8000` (`VELXIO_PROXY_UPSTREAM` to override).
  Make sure the proxy is up: `./scripts/dev-http-server.sh start` (the headless
  `make test` starts it for you). If the parent's `:8000` is down, the proxy
  returns a `200` stub unless `VELXIO_PROXY_FALLBACK=0`.
- The `.vlx` is a **circuit-only** project: it contains a placeholder `main.c`
  and is never compiled unless you press **Compile** without uploading a binary.
  Always upload `build/firmware.merged.bin` to run the real firmware.
- The firmware must be built with the ESP-IDF v4.4.7 tree (`scripts/build.sh`
  does this automatically when `/opt/esp-idf-v4.4` is present); an IDF 5.x build
  crashes the emulated ESP32 radio in `esp_phy_enable`.
- If you run your own server on the host's `:8000` (e.g. as the proxy upstream),
  stop VS Code from forwarding the container's `:8000`: add
  `"8000": { "onAutoForward": "ignore" }` to `remote.portsAttributes` in
  `.devcontainer/devcontainer.json`, remove the port from the Ports panel, and
  reload the window. Otherwise VS Code keeps tunneling `:8000` and collides.
- To iterate, `make build` again and re-upload the new `.bin`; no editor project
  changes are needed.
- The `.vlx` mirrors `tests/velxio/diagram.json`. If you change the circuit,
  update both so the GUI and the headless scenario stay in step.

---

## Status

- **UC-1 (`toggle-physical-led`)** is implemented end-to-end: BOOT button on
  GPIO0 toggles the LED on GPIO2, validated natively and in the emulator.
- **UC-5 (`fetch-and-uart`)** is implemented end-to-end: the NET button (GPIO4)
  triggers the chain, which sends over UART, joins the emulated Wi-Fi AP, does a
  real HTTP GET, and syncs the network. Validated natively and in the emulator
  (`make scenario NAME=fetch_and_uart`).
- **Cross-feature event (UC-1 → UC-5).** Toggling the LED on the BOOT button
  publishes `EVENT_LED_TOGGLED` (neutral payload in `main/events.h`); UC-5
  subscribes and fetches `/?led=on` when the LED is on and `/?led=off` when off,
  without either feature including the other. Validated by
  `make scenario NAME=uc5_led_toggle_fetch`.
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
