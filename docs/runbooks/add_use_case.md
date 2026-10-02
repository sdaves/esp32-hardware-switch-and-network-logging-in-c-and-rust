# Runbook: Add a use case

Status: ready
Audience: firmware maintainers and coding agents
Scope: take exactly one use-case plugin from stub to end-to-end working — pure logic,
imperative commands, autonomous registration, native tests, and an emulator scenario.

This runbook is a set of instructions. It is **not executed by tooling**. It may be run any
number of times; **each run implements exactly one use case and then stops**.

---

## 0. How to run this runbook

- Trigger it by asking an agent to "run `docs/runbooks/add_use_case.md`".
- Optionally name the target: *"run the add-use-case runbook for `read-analog-sensor`"*
  (`USE_CASE=read-analog-sensor`).
- If no target is named, §1 selects one deterministically.
- Complete §1–§9 once. Do **not** batch multiple use cases into one run.
- The executor may skip a section only when it is explicitly marked *new folder only* and the
  target already exists.

---

## 1. Select the target use case (the decision)

Run this from the repo root.

1. List the plugins:
   ```sh
   for d in main/use_cases/*/; do
     echo "== $(basename "$d")"
     sed -n '1,120p' "$d/commands.c"
   done
   ```
2. Classify each plugin:
   - **implemented** — its `commands.c` performs a real side effect (GPIO/network/file/UART),
     or it is marked done in `README.md` §Status.
   - **stub** — every `execute_*_hardware()` body is only the `if (!command || command->type ==
     CMD_NONE) return;` guard, with no side effect.
3. Choose the target:
   - If `USE_CASE=<name>` was supplied, use that folder.
   - Otherwise pick the **lowest-numbered stub** in the intended order, which is the `[UC-n]`
     list in `AGENTS.md` §2 (`toggle-physical-led` = UC-1, then the remaining folders).
   - If no stub remains, print `all use cases implemented` and **stop**.
4. Record the target contract before writing code:

   | Field | Value |
   |---|---|
   | folder | `main/use_cases/<name>` |
   | registration tag | `<snake_case>` (must be unique; match `UC_MODULES`) |
   | `self_module.name` | `"<name>"` (the folder name) |
   | inbound event(s) | e.g. `EVENT_SENSOR_READING_READY` |
   | trigger | bus event, 100 ms `poll_timer_tick`, or both |
   | commands | e.g. `CMD_POST_ANALOG`, `CMD_POST_ANALOG_ERROR` |
   | emulator input/output | which Wokwi part/pin, or a `printf` marker |

---

## 2. Read the existing stub as the contract

The four files already exist and compile; they define the intended types. Read them and keep
their naming:

- `domain.h` — `MsgType`/`Msg`, `CmdType`/`Cmd`, `Model`, `UpdateResult`, and the function
  prototypes.
- `logic.c` — `*_init()` and `*_update(Model, Msg)` (already a pure, compiling skeleton).
- `commands.c` — `*_init_use_case_hardware()` and `execute_*_hardware(const Cmd *)` (empty stubs).
- `setup.c` — the `UseCaseModule` vtable and `event_bus_subscribe(...)`.

Also read the one-line intent for the target in `AGENTS.md` §2.

Then write down the behavior contract in one paragraph: what messages arrive, how `Model`
changes, which `Cmd` is emitted, and how failures are routed.

A stub's existing `logic.c` may encode only a partial chain (UC-5's stub marked the model
`synced` on the first message and never emitted `CMD_SYNC_NETWORK`). Treat it as a starting
point, not a fixed contract: finalize the transitions to the intent in `AGENTS.md` §2, and for a
multi-stage chain add a feedback `MsgType` that the next stage consumes.

---

## 3. Conventions (hard rules)

- **Four-file split.** Do not collapse files or add cross-feature includes.
- **Pure core.** `logic.c` takes and returns structs strictly by value. No pointers, no
  `malloc`, no heap, no I/O. `commands.c` is the only place with side effects.
- **Exhaustive switches.** Every `switch` over `MsgType`/`CmdType` enumerates *all* variants and
  has **no `default:`**. The build uses `-Wswitch-enum -Werror`, so a missing case is a build
  failure by design.
- **Host-compilable.** Guard real hardware with `#ifdef ESP_PLATFORM` so `logic.c`/`commands.c`
  still compile under `PLATFORM_HOST_TEST`. `domain.h` must not include FreeRTOS/ESP headers.
- **Queue message.** The module's `message_size` is `sizeof(Msg)`; the queue carries the `Msg`
  struct by value.
- **Unique tag.** `USE_CASE_REGISTER(<tag>, self_module)` — `<tag>` is unique across the
  component and, for a new folder, must also be added to `UC_MODULES` (§6).
- **Stable serial markers.** If the emulator cannot read an output pin, emit a fixed firmware
  `printf` marker such as `# <FEATURE> <STATE>` and assert it with `wait-serial` (see §8).

---

## 4. Template

Replace `<feature>` (snake_case), `<FEATURE>` (upper), `<EVENT>`, `<ACTION>`, and `<tag>`.

### `domain.h`
```c
#ifndef <FEATURE>_DOMAIN_H
#define <FEATURE>_DOMAIN_H

#include <stdbool.h>
#include <stdint.h>

typedef enum {
    MSG_<EVENT> = 0,
    /* add further inbound variants here */
} MsgType;

typedef struct {
    MsgType type;
    /* inbound payload fields */
} Msg;

typedef enum {
    CMD_NONE = 0,
    CMD_<ACTION>,
    /* add further outbound variants here */
} CmdType;

typedef struct {
    CmdType type;
    /* outbound payload fields */
} Cmd;

typedef struct {
    /* model fields */
} Model;

typedef struct {
    Model next;
    Cmd command;
} UpdateResult;

Model <feature>_init(void);
UpdateResult <feature>_update(Model model, Msg msg);

void <feature>_init_use_case_hardware(void);
void execute_<feature>_hardware(const Cmd *command);

#endif
```

### `logic.c`
```c
#include "domain.h"

Model <feature>_init(void)
{
    return (Model){ /* initial fields */ };
}

UpdateResult <feature>_update(Model model, Msg msg)
{
    UpdateResult result = {
        .next = model,
        .command = { .type = CMD_NONE },
    };

    switch (msg.type) {
    case MSG_<EVENT>:
        /* pure transition; declare a command when work is required */
        break;
    }

    return result;
}
```

### `commands.c`
```c
#include "domain.h"

#ifdef ESP_PLATFORM
/* real peripheral headers, pins, handles */
#endif

void <feature>_init_use_case_hardware(void)
{
#ifdef ESP_PLATFORM
    /* configure pins / peripherals once at boot */
#endif
}

void execute_<feature>_hardware(const Cmd *command)
{
    if (!command || command->type == CMD_NONE) {
        return;
    }
#ifdef ESP_PLATFORM
    /* perform the side effect (network / file / UART / GPIO) */
#endif
    /* optional device+host marker for the emulator scenario */
}
```

### `setup.c`
```c
#include "domain.h"
#include "registry.h"

static Model local_model;
static QueueHandle_t local_queue;

static void local_process_queue_item(const void *item)
{
    const Msg *msg = (const Msg *)item;
    UpdateResult result = <feature>_update(local_model, *msg);
    local_model = result.next;
    execute_<feature>_hardware(&result.command);
}

static void local_subscriptions(QueueHandle_t my_queue)
{
    local_queue = my_queue;
    local_model = <feature>_init();
    event_bus_subscribe(EVENT_<X>, my_queue, sizeof(Msg));
}

static const UseCaseModule self_module = {
    .name = "<name>",
    .queue_length = 8,
    .message_size = sizeof(Msg),
    .init_hardware = <feature>_init_use_case_hardware,
    .process_queue_item = local_process_queue_item,
    .poll_timer_tick = NULL,
    .wire_subscriptions = local_subscriptions,
};

USE_CASE_REGISTER(<tag>, self_module);
```

If the use case has a periodic input, add a poll function and reference it in `self_module`
(the platform calls it every 100 ms). Copy the debounce pattern from
`main/use_cases/toggle-physical-led/setup.c`:

```c
static void local_poll_timer_tick(void)
{
    /* read hardware, debounce, publish a Msg on the bus */
}
```

---

## 5. Implement

Work through the files in this order; keep the build green between steps.

1. **`domain.h`** — finalize `Msg`/`Cmd` payloads, `Model`, and the `UpdateResult`.
2. **`logic.c`** — implement every state transition and its emitted `Cmd`. Keep it pure.
3. **`commands.c`** — implement `init_use_case_hardware` (pin/peripheral setup, guarded by
   `ESP_PLATFORM`) and `execute_*_hardware` (the real action for each command).
4. **`setup.c`** — wire the vtable, subscribe to the inbound event(s), and add a poll tick only
   if the use case is input-driven.
5. **New event (only if required).** If no existing `SystemEventId` fits, add one in
   `main/registry.h` before its `EVENT_ID_COUNT` terminator. This is a shared platform
   contract: it affects every subscriber, so prefer reusing an existing event.
6. **New hardware/config** must live behind `ESP_PLATFORM`; never break the native build.

---

## 6. Register the plugin (new folder only)

An existing stub is already listed; skip to §7 after confirming the tag/name.

For a brand-new folder, edit `main/CMakeLists.txt`:

- add `logic.c`, `commands.c`, `setup.c` to `SRCS`;
- add the folder to `INCLUDE_DIRS`;
- add `<tag>` to the `UC_MODULES` list.

Why both edits are required: IDF compiles only the sources named in `SRCS`, and it links with
`--gc-sections` under `-ffunction-sections`, so the registration constructor is discarded unless
`-Wl,--undefined=_uc_register_<tag>` is passed. A missing tag makes the module silently vanish —
the only symptom is a smaller `Found N Autonomous Modules.` banner. See `AGENTS.md` §6.2.

---

## 7. Native tests

1. Add `test/test_<feature>.c`, mirroring `test/test_toggle_led.c`:
   - assert the initial state from `<feature>_init()`;
   - drive each `MsgType` through `<feature>_update()` and assert the emitted `Cmd` and next
     state (including the error/no-op path).
2. Update `test/Makefile`:
   - add a rule compiling `../main/use_cases/<name>/logic.c` with
     `-I../main -I../main/use_cases/<name>`;
   - link that object with `test_<feature>.o` (plus `mock_registry.o` if the test uses the mock
     bus) into `test_<feature>`;
   - add the binary to `all`, and the objects/binary to `clean`;
   - **include ordering matters.** Every plugin's header is named `domain.h`, and the existing
     `INC` already points at `toggle-physical-led`. Appending `-I../main/use_cases/<name>`
     *after* it makes the compiler read the wrong `domain.h` (symptoms: `implicit declaration`,
     `no member named …`). Put the new directory *before* the toggle path, or give the new test
     its own `-I../main -I../main/use_cases/<name>` set without the toggle path.
3. Run:
   ```sh
   make native-test
   ```
   All targets, including the new one, must print `PASS`.

---

## 8. Emulator scenario

1. **Circuit** — if the use case introduces new parts/pins, extend `tests/velxio/diagram.json`
   (Wokwi format). `diagram_map.py` maps a part id to a board GPIO by walking `connections`,
   ignoring power pins (`GND*`, `VIN*`, `3V3*`, `5V*`, `EN`) and preferring numeric pins. A
   part already in the circuit may be shared with another use case (UC-5 reuses UC-1's BOOT
   button on GPIO0), so a new pin is often unnecessary.
2. **Scenario** — add `tests/velxio/scenarios/<feature>.yaml` (snake_case the folder name, e.g.
   `fetch-and-uart` → `fetch_and_uart.yaml`) using the step vocabulary from `AGENTS.md` §6.3
   (`delay`, `wait-serial`, `write-serial`, `set-control`, `expect-pin`). One key per step.
3. **Trigger** — an event-driven use case only runs when its inbound event is published, and
   some stubs' events have **no producer** anywhere in the firmware (`fetch-and-uart` subscribes
   to `EVENT_DB_QUERY_RESULT`, which nothing else publishes). Decide how the emulator reaches the
   logic and record it in the §1 contract table:
   - reuse an existing physical input (a button/pin already in `diagram.json`);
   - add a `poll_timer_tick` that publishes the inbound event once or on a cadence;
   - read UART RX via a `write-serial` step.
   For a multi-stage chain, publish the feedback `MsgType` on the same event (the message size
   must equal `sizeof(Msg)`) or add a new `SystemEventId` per §5.5.
4. **Assertion strategy** — the OSS bridge does not reliably emit output-pin `gpio_change`
   events. Assert observable behaviour with a firmware `printf` marker and `wait-serial`
   (UC-1 uses `# LED ON (gpio 2)` / `# LED OFF (gpio 2)`). Input injection via `esp32_gpio_in`
   does work; allow ≥400 ms hold/release margins for the 2-tick (100 ms) debounce.
5. **Run it:**
   ```sh
   make build
   make scenario NAME=<feature>
   ```
   It must print `RESULT: PASS` (exit 0).

Do **not** change the canonical `scripts/test.sh` UC-1 smoke test unless the task explicitly
asks to promote this use case to the default; run the new scenario by name instead.

---

## 9. Definition of done

1. `make native-test` — every target, including the new one, prints `PASS`.
2. `make build` — exits 0 with no new warnings. The registration banner is a **runtime** message,
   not a build message: confirm `Platform Engine Initializing: Found N Autonomous Modules.`
   (N equal to the number of folders in `main/use_cases/`) in the emulator's boot serial, e.g.
   via the scenario's first `wait-serial`.
3. `make scenario NAME=<feature>` — `RESULT: PASS`, exit 0.
4. `make test` — still `RESULT: PASS` (native tests + firmware + the UC-1 scenario).
5. No new compiler warnings under `-Werror`.
6. Status docs reflect reality: move the use case out of "stub" in `README.md` §Status and
   update its one-line description in `AGENTS.md` §2 if the intent changed.

---

## 10. Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `Found N-1 Autonomous Modules` | new tag missing from `UC_MODULES`, or sources missing from `SRCS` | apply §6 |
| `-Wswitch-enum` / `-Werror=switch` build error | a `switch` does not handle every enum value | enumerate all cases, remove any `default:` |
| Native build fails with FreeRTOS/ESP headers | hardware code not guarded | wrap it in `#ifdef ESP_PLATFORM` |
| Native test fails with `implicit declaration` / `no member named …` | wrong `domain.h` picked up because the new include came after `toggle-physical-led` | order the new use-case dir before the toggle path, or use a dedicated include set (§7) |
| Use case never runs in the scenario | its inbound event has no producer in the firmware | add a trigger (§8.3): shared input, self-publishing poll tick, or `write-serial` |
| Scenario `wait-serial` times out | marker string differs, or output not emitted | match the firmware `printf` exactly; assert on serial, not `gpio_change` |
| Second trigger ignored | debounce margin too small | use ≥400 ms hold/release in the scenario |
| Listener 503 | another build/test is running | wait; requests are serialised |
| Listener route 404 | host still running an old listener process | restart `.devcontainer/docker-build-listener.py` on the host |

---

## 11. References

- `AGENTS.md` §2 (plugin map), §3 (four-file split), §6 (iteration, registration, troubleshooting).
- `main/use_cases/toggle-physical-led/` — the reference implemented use case.
- `test/test_toggle_led.c`, `test/Makefile` — reference native test.
- `tests/velxio/scenarios/uc1_button_toggle.yaml` — reference input-driven scenario.
- `tests/velxio/scenarios/fetch_and_uart.yaml` — reference event-driven chain scenario (shared
  input → bus event → UART → network sync).
- `docs/runbooks/add_testing_simulator.md` — emulator protocol and rationale.
