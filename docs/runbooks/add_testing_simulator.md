# Runbook: Add the Velxio simulator testing harness

Status: proposed
Audience: firmware maintainers
Scope: run the real ESP32-S3 firmware in a self-hosted OSS Velxio instance and drive it
with automated per-use-case tests, without hardware.

> **Superseded in part (2026-10).** The target is now the **classic ESP32**, not the S3:
> the QEMU fork only models a Wi-Fi radio on `esp32-picsimlab`, and only under ESP-IDF 4.4.
> The harness, circuit, `.vlx`, and build target therefore use `esp32`, UC-5 does a real
> Wi-Fi + HTTP fetch, and `scripts/dev-http-server.py` reverse-proxies it. See `AGENTS.md` §5
> for the current setup; the protocol/step vocabulary below is still accurate.

This runbook is a set of instructions. It is not executed by tooling.

---

## 1. Goal

Validate each use case end to end on emulated ESP32-S3 silicon:

- the **production firmware** is the same binary that flashes to a real devkit;
- physical inputs (buttons, ADC) are driven exactly as a real peripheral would drive them;
- outputs (GPIO pins, UART) are asserted by an automated test;
- everything runs locally, offline, and free.

The first deliverable is a single passing test for **UC-1 (toggle-physical-led)**.
The harness is use-case agnostic, so later use cases add scenario files, not code.

## 2. Background: why not `velxio-cli`

Velxio's emulator, backend, frontend and CLI are all open source, but the CLI is only a
client of Velxio's **CI service**, whose endpoints live under `/api/pro/ci/*` inside a
private overlay. The OSS image does not ship them. Evidence:

- The complete OSS router list is `compile`, `compile_chip`, `compile_rom`, `flash`,
  `intellisense`, `iot_gateway`, `libraries`, `micropython_libs`, `news`, `simulation`.
  There is no `ci` router.
- `backend/app/main.py` states auth/projects/admin/metrics routers "moved out of upstream…
  now live under the private overlay's `pro/backend/app/api/routes/`".
- The CLI hard-codes `/api/pro/ci/whoami`, `/api/pro/ci/auth/device`, and a `run.create`
  protocol under the same prefix.

Consequences:

- `velxio-cli run --server http://localhost:3080` fails at the handshake against the OSS
  image.
- Velxio CI is a paid feature (no free minutes), so it cannot be the local default either.

Therefore this runbook drives the **OSS simulation WebSocket** directly
(`/api/simulation/ws/<client_id>`), which the OSS backend does implement. The scenario
file format is kept Wokwi/velxio-cli compatible so the same files could later run on
velxio.dev unchanged.

## 3. Prerequisites

- A Docker-capable host (PC, NAS, or remote server). Termux/Android cannot run the
  devcontainer: there is no Docker daemon and the Xtensa toolchain is glibc, not bionic.
- VS Code with the Dev Containers extension.
- Network access on first container build (pulls the multi-GB Velxio image).
- An ESP32-S3 devkit with a BOOT button on GPIO0 and an LED on GPIO2 (for the
  hardware-parity check in step 9).

## 4. Architecture

```
VS Code devcontainer
  └── OSS Velxio image (nginx :80 -> :3080, uvicorn 127.0.0.1:8001)
        └── libqemu-xtensa.so  (emulated ESP32-S3, register-level GPIO/UART)
              ▲
              │ ws://localhost:3080/api/simulation/ws/<client_id>
              │   start_esp32 { board, firmware_b64 }
              │   esp32_gpio_in / esp32_adc_set / esp32_serial_input
              │   < serial_output / gpio_change / system
              │
tests/velxio/runner/*.py  (Python websockets + PyYAML)
              ▲
              │
tests/velxio/scenarios/uc1_button_toggle.yaml
```

## 5. Repository layout to add

```
.devcontainer/
  Dockerfile
  devcontainer.json
tests/
  __init__.py
  velxio/
    __init__.py
    diagram.json
    scenarios/uc1_button_toggle.yaml
    runner/
      __init__.py
      ws_client.py
      diagram_map.py
      scenario.py
      run_scenario.py
scripts/
  build.sh
  test.sh
```

The `__init__.py` files (can be empty) let the harness run as a module
(`python -m tests.velxio.runner.run_scenario`) from the repository root.

## 6. Step 1 — Add a real BOOT-button input to UC-1

The current firmware has no input path: `event_bus_publish` is never called in
production, no GPIO is configured as an input, and `EVENT_HARDWARE_ALERT_TRIGGERED` is
only subscribed (`main/use_cases/toggle-physical-led/setup.c`). A real button would do
nothing on hardware either. Add the input as production code (no simulator-only branch).

### 6.1 `main/use_cases/toggle-physical-led/commands.c`

Add a button pin next to the LED pin and configure it in `init_use_case_hardware`.
Keep the existing `#ifdef ESP_PLATFORM` shell.

```c
#ifdef ESP_PLATFORM
#include "driver/gpio.h"
#define TOGGLE_LED_GPIO    GPIO_NUM_2
#define TOGGLE_BUTTON_GPIO GPIO_NUM_0
#endif

void init_use_case_hardware(void) {
#ifdef ESP_PLATFORM
    gpio_reset_pin(TOGGLE_LED_GPIO);
    gpio_set_direction(TOGGLE_LED_GPIO, GPIO_MODE_OUTPUT);

    gpio_config_t btn = {
        .pin_bit_mask = 1ULL << TOGGLE_BUTTON_GPIO,
        .mode         = GPIO_MODE_INPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type    = GPIO_INTR_DISABLE,
    };
    gpio_config(&btn);
#endif
}

/* Active-low BOOT button: pressed == 0. */
bool toggle_button_pressed(void) {
#ifdef ESP_PLATFORM
    return gpio_get_level(TOGGLE_BUTTON_GPIO) == 0;
#else
    return false;
#endif
}
```

`execute_toggle_led_hardware()` is unchanged.

### 6.2 `main/use_cases/toggle-physical-led/setup.c`

Add a queue handle, store it in `local_subscriptions`, and implement the timer poll.
`main.c` already invokes `poll_timer_tick` every 100 ms, so a 2-tick debounce is ~200 ms.

```c
extern bool toggle_button_pressed(void);

static Model local_model;
static QueueHandle_t local_queue;

/* Debounce state. */
static int s_stable_level = 1;
static int s_candidate_level = 1;
static int s_stable_ticks = 0;

static void local_poll_timer_tick(void) {
    int level = toggle_button_pressed() ? 0 : 1;
    if (level == s_candidate_level) {
        if (level != s_stable_level && ++s_stable_ticks >= 2) {
            s_stable_level = level;
            s_stable_ticks = 0;
            if (level == 0) {
                Msg press = { .type = MSG_BUTTON_DOWN };
                event_bus_publish(EVENT_HARDWARE_ALERT_TRIGGERED, &press, sizeof(Msg));
            }
        }
    } else {
        s_candidate_level = level;
        s_stable_ticks = 0;
    }
}

static void local_subscriptions(QueueHandle_t my_queue) {
    local_queue = my_queue;
    local_model = toggle_led_init();
    event_bus_subscribe(EVENT_HARDWARE_ALERT_TRIGGERED, my_queue, sizeof(Msg));
}
```

Set `.poll_timer_tick = local_poll_timer_tick` in `self_module`.

Do not change `domain.h` or `logic.c`. Publishing through the bus keeps the pure core
untouched and exercises the real inter-module path.

### 6.3 Host tests

`make -C test` must still pass (it only compiles `logic.c`).

## 7. Step 2 — Add the devcontainer

### 7.1 `.devcontainer/Dockerfile`

```dockerfile
FROM ghcr.io/davidmonterocrespo24/velxio:master

RUN apt-get update && apt-get install -y --no-install-recommends \
      esptool \
      inotify-tools \
      gdb \
      python3-pip \
      python3-venv \
 && rm -rf /var/lib/apt/lists/* \
 && pip install --no-cache-dir websockets pyyaml

WORKDIR /workspace
```

The base image already contains ESP-IDF v5.5.4, the Xtensa toolchain, arduino-cli and the
QEMU shared library plus ROM blobs, so no QEMU build or license key is needed.

### 7.2 `.devcontainer/devcontainer.json`

```json
{
  "name": "embedded-velxio",
  "build": { "dockerfile": "Dockerfile" },
  "overrideCommand": true,
  "remoteUser": "root",
  "forwardPorts": [3080, 8001],
  "postStartCommand": "nohup /app/entrypoint.sh >/tmp/velxio.log 2>&1 &",
  "customizations": {
    "vscode": {
      "extensions": [
        "velxio.velxio-simulator",
        "espressif.esp-idf-extension"
      ],
      "settings": {
        "velxio.defaultBoard": "esp32-s3",
        "velxio.arduinoCliPath": "/usr/local/bin/arduino-cli"
      }
    }
  }
}
```

Notes:

- `overrideCommand` keeps the shell alive instead of the image's nginx+uvicorn CMD;
  `postStartCommand` starts the Velxio server in the background.
- The Velxio VS Code extension is Arduino-sketch oriented and Pro-licensed. It is
  installed for convenience only; it does not build this pure-ESP-IDF project. The
  Python harness is the test runner.
- Optional: mount named volumes for `/root/.espressif`, `/var/cache/ccache` and
  `/var/lib/velxio-build` to keep builds warm.

## 8. Step 3 — Circuit and scenario

### 8.1 `tests/velxio/diagram.json`

Wokwi format. Verify part ids and pin names against the installed Velxio/Wokwi element
set; the runner resolves parts to board pins by walking these connections.

```json
{
  "version": 1,
  "parts": [
    { "type": "board-esp32-s3-devkitc-1", "id": "esp",  "top": 0,   "left": 0,   "attrs": {} },
    { "type": "wokwi-pushbutton",         "id": "btn1", "top": -90, "left": 200, "attrs": { "color": "green", "label": "BOOT" } },
    { "type": "wokwi-resistor",           "id": "r1",   "top": 0,   "left": 240, "attrs": { "value": "220" } },
    { "type": "wokwi-led",                "id": "led1", "top": -70, "left": 320, "attrs": { "color": "red", "label": "LED" } }
  ],
  "connections": [
    ["esp:0",     "btn1:1",  "green", []],
    ["btn1:2",    "esp:GND.1","black", []],
    ["esp:2",     "r1:1",    "green", []],
    ["r1:2",      "led1:A",  "green", []],
    ["led1:C",    "esp:GND.2","black", []]
  ]
}
```

### 8.2 `tests/velxio/scenarios/uc1_button_toggle.yaml`

```yaml
name: UC-1 button toggles the LED
version: 1
steps:
  - wait-serial: "Platform Engine Initializing"
  - expect-pin:  { part-id: esp,  pin: "2", expected: 0 }
  - set-control: { part-id: btn1, control: pressed, value: 1 }
  - delay: 300ms
  - set-control: { part-id: btn1, control: pressed, value: 0 }
  - delay: 200ms
  - expect-pin:  { part-id: esp,  pin: "2", expected: 1 }
  - set-control: { part-id: btn1, control: pressed, value: 1 }
  - delay: 300ms
  - set-control: { part-id: btn1, control: pressed, value: 0 }
  - delay: 200ms
  - expect-pin:  { part-id: esp,  pin: "2", expected: 0 }
```

The `delay` after a press exceeds the 2-tick debounce, and `expect-pin` is read
mid-window rather than tied to a serial line.

## 9. Step 4 — Python WebSocket harness

Protocol reference (subset the OSS `simulation` router implements):

| Direction | Message |
|---|---|
| to server | `{"type":"start_esp32","data":{"board":"esp32-s3","firmware_b64":"<b64>","wifi_enabled":false}}` |
| to server | `{"type":"esp32_gpio_in","data":{"pin":N,"state":0|1}}` |
| to server | `{"type":"esp32_adc_set","data":{"channel":N,"millivolts":M}}` |
| to server | `{"type":"esp32_serial_input","data":{"bytes":[...],"uart":0}}` |
| to server | `{"type":"stop_esp32"}` |
| from server | `{"type":"serial_output","data":{"data":"...","uart":0}}` |
| from server | `{"type":"gpio_change","data":{"pin":N,"state":0|1}}` |
| from server | `{"type":"system","data":{"event":"booting|booted|crash|reboot"}}` |
| from server | `{"type":"error","data":{"message":"..."}}` |

### 9.1 `tests/velxio/runner/ws_client.py`

Async client that owns one simulation session. Use a distinct `client_id` (e.g.
`test-<scenario>-<pid>`) so a browser tab cannot collide with a test.

Responsibilities:

- connect to `ws://localhost:3080/api/simulation/ws/<client_id>`;
- send `start_esp32` with the merged image (base64);
- expose an async event queue of `serial_output` / `gpio_change` / `system` / `error`;
- methods: `set_pin(pin, state)`, `set_adc(channel, mv)`, `write_serial(bytes, uart=0)`,
  `stop()`;
- keep a rolling serial buffer for substring assertions.

Skeleton:

```python
import asyncio, base64, json
import websockets

class SimSession:
    def __init__(self, server: str, client_id: str):
        self.url = f"{server}/api/simulation/ws/{client_id}"
        self.ws = None
        self.events = asyncio.Queue()

    async def connect(self):
        self.ws = await websockets.connect(self.url, max_size=None)
        asyncio.create_task(self._reader())

    async def _reader(self):
        async for raw in self.ws:
            await self.events.put(json.loads(raw))

    async def boot(self, firmware_path: str):
        b64 = base64.b64encode(open(firmware_path, "rb").read()).decode()
        await self._send("start_esp32", {
            "board": "esp32-s3", "firmware_b64": b64, "wifi_enabled": False,
        })

    async def set_pin(self, pin: int, state: int):
        await self._send("esp32_gpio_in", {"pin": pin, "state": int(state)})

    async def set_adc(self, channel: int, millivolts: int):
        await self._send("esp32_adc_set", {"channel": channel, "millivolts": millivolts})

    async def write_serial(self, data: bytes, uart: int = 0):
        await self._send("esp32_serial_input", {"bytes": list(data), "uart": uart})

    async def stop(self):
        try:
            await self._send("stop_esp32", {})
        finally:
            if self.ws:
                await self.ws.close()

    async def _send(self, mtype, data):
        await self.ws.send(json.dumps({"type": mtype, "data": data}))
```

### 9.2 `tests/velxio/runner/diagram_map.py`

Resolve a part id and control to a board pin by reading `diagram.json` connections.
For UC-1: `btn1 -> esp:0`, `led1 -> esp:2`. Keep it small; a lookup table keyed by part
id is acceptable for the first version if connection walking is deferred.

### 9.3 `tests/velxio/runner/scenario.py`

Parse the Wokwi-style YAML and translate each step:

| Scenario step | Harness action |
|---|---|
| `delay: <n>ms` | `await asyncio.sleep(n/1000)` |
| `wait-serial: <s>` | poll the rolling serial buffer until substring appears or timeout |
| `write-serial: <s \| [bytes]>` | `session.write_serial(...)` |
| `set-control` (pushbutton `pressed`) | `session.set_pin(pin, 0 if pressed else 1)` (active-low) |
| `set-control` (potentiometer `value`) | `session.set_adc(channel, mv)` |
| `expect-pin` | wait for a matching `gpio_change`, else fail with the last seen level |

Fail fast on the first unsatisfied step, and print a per-step PASS/FAIL line.

### 9.4 `tests/velxio/runner/run_scenario.py`

CLI entrypoint:

```
python -m runner.run_scenario \
    --server ws://localhost:3080 \
    --firmware build/firmware.merged.bin \
    --diagram tests/velxio/diagram.json \
    --scenario tests/velxio/scenarios/uc1_button_toggle.yaml
```

Exit `0` on pass, `1` on failure, `2` on configuration error.
Always stop the session in a `finally` block.

## 10. Step 5 — Build and run scripts

### 10.1 `scripts/build.sh`

```sh
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

. /opt/esp-idf/export.sh
idf.py set-target esp32s3
idf.py build
# Velxio's QEMU machine expects a complete merged flash image.
idf.py merge-bin -o build/firmware.merged.bin
echo "built build/firmware.merged.bin"
```

### 10.2 `scripts/test.sh`

```sh
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

make -C test
./scripts/build.sh

# Ensure the self-hosted Velxio backend is reachable.
curl -fsS http://localhost:3080/health >/dev/null

python -m tests.velxio.runner.run_scenario \
  --server ws://localhost:3080 \
  --firmware build/firmware.merged.bin \
  --diagram tests/velxio/diagram.json \
  --scenario tests/velxio/scenarios/uc1_button_toggle.yaml
```

## 11. Step 6 — Verification

1. `make -C test` passes (pure logic unchanged).
2. `./scripts/build.sh` succeeds and the boot log prints
   `Platform Engine Initializing: Found 5 Autonomous Modules.` — this also confirms the
   custom linker script and `.platform_registry` reflection work under real ESP-IDF.
3. `./scripts/test.sh` drives the button and asserts LED GPIO2 toggles; exit `0`.
4. Parity: flash the same `build/` image to a real ESP32-S3 devkit and press the BOOT
   button; observe the same LED behavior.

## 12. Troubleshooting

| Symptom | Likely cause | Action |
|---|---|---|
| `WebSocket 404` / refused | backend not started, or wrong path | check `/tmp/velxio.log`; confirm `curl /health` |
| Boot log shows `Found 0 Autonomous Modules` | merged image missing/incorrect; custom linker section dropped | rebuild with the real `esp32s3.ld`; confirm `idf.py merge-bin` input includes bootloader+partition+app |
| Button press has no effect | pin mismatch or debounce too short | confirm `btn1 -> GPIO0`; ensure `delay` > 200 ms; verify pull-up config |
| `expect-pin` never matches | reading an input-only/unrouted pad, or GPIO matrix not driving the pad | assert via a firmware `printf` + `wait-serial` as a fallback, and re-check the S3 pin number |
| Firmware does not boot | image is app-only, or flash size != 4 MB | use `idf.py merge-bin`; the lcgamboa machine is fixed at 4 MB |
| Browser tab loses its stream | harness used the same `client_id` | always use a unique test `client_id` |

## 13. Out of scope / future

- UC-2 can reuse `set-control` on a potentiometer (`esp32_adc_set`). Its HTTP POST is a
  stub, so assert the logic path via serial until `analog_hw_post` is implemented.
- UC-3/4/5 cannot be tested until their `commands.c` stubs are implemented
  (HTTP/filesystem/SQLite/UART).
- SQLite is not a Velxio peripheral; it must be compiled in as an ESP-IDF component.
- If a paid Velxio plan is ever available, the same Wokwi-style scenario files and
  `diagram.json` run unchanged through `velxio-cli run` on velxio.dev. No rewrite needed.
- The Velxio VS Code extension is Arduino-sketch oriented and cannot build this pure
  ESP-IDF project; it is installed here only for the interactive simulator panel.

## 14. References

- OSS backend routes: `backend/app/api/routes/`
- Simulation WebSocket handler: `backend/app/api/routes/simulation.py`
- Browser bridge (protocol field names): `frontend/src/simulation/Esp32Bridge.ts`
- Velxio CLI (for the cloud path): https://github.com/velxio/velxio-cli
- OSS image: `ghcr.io/davidmonterocrespo24/velxio:master`
