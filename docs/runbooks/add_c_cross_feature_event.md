# Runbook: Add a C cross-feature, event-driven behavior

Status: planned — current run is recorded in §9.1 (`EVENT_LED_TOGGLED`, UC-1 → UC-5)
Audience: firmware maintainers and coding agents
Scope: wire one use case to **react to another use case's state through the C event
bus**, with no cross-feature `#include`. Parameterized by a `NEW_EVENT_NAME` and the
producer/consumer features supplied when the runbook is invoked; the worked sample is
UC-1's LED toggle causing UC-5 to fetch `/?led=on|off`. Neutral event payloads live in
`main/events.h` (§2.1).

This is the C twin of [`add_rust_cross_feature_event.md`](add_rust_cross_feature_event.md).
It teaches the mechanism **piece by piece**: first a simple message passes from a publisher
to a listener and the listener runs on receipt, then the message gains meaning, then the
worked behavior is completed. Only the bus/payload mechanics are covered here — existing
network code (UC-5's one-shot Wi-Fi) stays exactly as it is.

This runbook is a set of instructions. It is **not executed by tooling**. It may be run any
number of times; **each run adds exactly one cross-feature event and then stops**.

**Writability.** This run may edit `main/`, `test/`, `tests/velxio/scenarios/`, and
`docs/runbooks/`. It must **never** touch `rust/`. Reuse everything else read-only.

---

## 0. How to run this runbook

- Trigger it by asking an agent to "run `docs/runbooks/add_c_cross_feature_event.md`",
  ideally with the parameters, e.g.:

  ```text
  run docs/runbooks/add_c_cross_feature_event.md
  NEW_EVENT_NAME=EVENT_LED_TOGGLED
  producer=toggle-physical-led
  consumer=fetch-and-uart
  ```

- **If any of `NEW_EVENT_NAME`, `producer`, or `consumer` is not supplied in the prompt, ask
  for it before doing anything else.** Ask once, together, then proceed. Do not guess.
- Record the three values and carry them verbatim through every step.
- Work top to bottom; keep the tree compiling between stages. Do **not** advance a stage
  until the previous one verifies (§6).
- Complete §4–§8 once. Do **not** batch multiple events into one run.

### Parameter prompts (resolve before §1)

| Prompt | Meaning | Example |
|---|---|---|
| `NEW_EVENT_NAME` | the new `SystemEventId` variant | `EVENT_LED_TOGGLED` |
| `producer` | use-case folder that publishes it | `toggle-physical-led` |
| `consumer` | use-case folder that subscribes and runs | `fetch-and-uart` |

---

## 1. Goal / when to use

Use this when feature **B** must do something in response to feature **A**'s state, and the
two features must not know about each other (root `AGENTS.md` §1, architecture rule 3:
features never reference each other; all inter-module communication goes through the event
bus).

The worked sample: UC-1 (`toggle-physical-led`) toggles the LED on the BOOT button; UC-5
(`fetch-and-uart`) fetches `/?led=on|off`. UC-1's LED toggle is published on a new event;
UC-5 subscribes and issues its network fetch with the new state. UC-5's own NET-button
behavior is unchanged.

---

## 2. The input contract (fill from §0, then keep this table updated)

The table below is filled for the **current run** (LED toggle → fetch); blank it for the
next run.

| Field | Value |
|---|---|
| Event name | `EVENT_LED_TOGGLED` |
| Producer folder | `main/use_cases/toggle-physical-led` |
| Consumer folder | `main/use_cases/fetch-and-uart` |
| Payload struct + size | `LedToggledEvent { bool on; }`, `sizeof(LedToggledEvent)` |
| Payload header | `main/events.h` |
| Consumer inbound `MsgType` | `MSG_LED_TOGGLED` (added in Stage 2) |
| Consumer emitted `CmdType` | `CMD_SYNC_NETWORK` (reused; `Cmd` gains `bool led_on`) |
| Emulator trigger | `btn1` on GPIO0 (already wired in `diagram.json`) |
| Emulator assertion | `printf` marker + `wait-serial` |

### 2.1 Neutral payloads live in `main/events.h` (design decision)

Cross-feature event payloads go in a dedicated **`main/events.h`**, next to `registry.h`,
**not** in either feature's `domain.h`. Event ids stay in `registry.h` (`SystemEventId`);
`events.h` carries only what travels on the bus:

```c
#ifndef EVENTS_H
#define EVENTS_H

#include <stdbool.h>
#include <stdint.h>

/* Cross-feature event payloads. Neutral by design: no feature's Msg/Cmd
 * types appear here, so producer and consumer each include this header and
 * never reference each other. Event ids live in registry.h (SystemEventId);
 * this header carries only what travels on the bus. */

typedef struct {
    bool on;
} LedToggledEvent;

#endif
```

Why this is the right home:

- **Solves the size contract.** Both sides include `events.h`, so they publish and
  subscribe with the *identical* type and `sizeof` — no scratch payload, no size
  discriminator, no `sizeof` guesswork (`registry.c` skips size-mismatched subscribers).
- **Preserves rule 2 (no cross-feature includes).** Neither feature names the other's
  `Msg`/`Cmd`; the neutral payload and the shared `SystemEventId` are the only coupling.
- **No build edit.** `main/CMakeLists.txt` already has `"."` in `INCLUDE_DIRS`, so a
  `main/events.h` is found without touching the component file.

This mirrors the Rust twin's neutral `PlatformEvent::LedToggled { on: bool }`
(`add_rust_cross_feature_event.md` §3.2: "Dedicated neutral payload … rather than reusing
UC-5's `Msg`").

---

## 3. How the C event bus works (read this before writing code)

- The event set is one shared enum, `SystemEventId`, in `main/registry.h`. Add the new
  variant **immediately before the `EVENT_ID_COUNT` terminator**; the bounds checks
  (`event >= EVENT_ID_COUNT`) and the `-Wswitch-enum` builds depend on that terminator.
- The payload set is `main/events.h`: one neutral, self-contained struct per event
  (§2.1). Producer and consumer include it, so their `sizeof` always matches.
- `event_bus_subscribe(event, queue, message_size)` records a subscriber and the size of
  the payload it expects. It is called from a module's `wire_subscriptions` during boot.
- `event_bus_publish(event, message, message_size)` copies the payload **by `memcpy`** to
  every subscriber whose event matches **and whose declared `message_size` equals the
  published `message_size`** (`main/registry.c`). A size mismatch is skipped silently.
- `main.c` drains each module's FreeRTOS queue in the main loop and calls
  `process_queue_item(item)` — that is "the listener running when it gets the message".

```text
  producer/logic.c (pure)          producer/setup.c                 consumer/setup.c
  ───────────────────────          ────────────────                 ────────────────
  update(model, msg) -> Cmd   ──▶  process_queue_item(item):
                                     execute_<producer>_hardware(&cmd)
                                     event_bus_publish(NEW_EVENT, &
                                       payload, sizeof(payload))     [main/events.h]
                                            │  memcpy (size must match)
                                            ▼
                                     local_process_queue_item(item) ──▶ consumer/logic.c
                                        update(model, *(Msg*)item) -> Cmd
                                        execute_<consumer>_hardware(&cmd)
```

### Invariants (do not relax)

1. **Pure core.** `logic.c` takes/returns structs strictly by value. No pointers, no
   `malloc`, no I/O. `commands.c`/`setup.c` hold the side effects.
2. **No cross-feature includes.** Producer and consumer each use their own `domain.h`; the
   shared `SystemEventId` is the only coupling.
3. **Exhaustive switches.** Every `switch` over `MsgType`/`CmdType` enumerates all variants
   with **no `default:`** (`-Wswitch-enum -Werror=switch`).
4. **Size contract.** Publish and subscribe with the **same** `sizeof`.
5. **Host-compilable.** Guard hardware with `#ifdef ESP_PLATFORM`; `domain.h` must not
   include FreeRTOS/ESP headers.

---

## 4. The pattern, in three stages (piece by piece)

Do the stages in order; each is a checkpoint in §6. Do not advance until the current stage
is verified.

### Stage 1 — simple message passing (delivery only)

Goal: prove `<NEW_EVENT_NAME>` travels from `<producer>` to `<consumer>` and the consumer
runs on receipt. No semantics yet.

- Add `<NEW_EVENT_NAME>` to `SystemEventId` in `main/registry.h`.
- Add the neutral payload struct to `main/events.h` (§2.1). Both sides include
  `events.h`, so the `sizeof` matches without any scratch type or discriminator.
- Producer: after its command runs, `event_bus_publish(NEW_EVENT, &payload, sizeof(payload))`.
- Consumer: `event_bus_subscribe(NEW_EVENT, my_queue, sizeof(payload))` and, in
  `process_queue_item`, read the item, log a `printf` marker (e.g. `# <EVENT> RECEIVED`),
  and return `CMD_NONE`.
- Verify by scenario (`wait-serial: "# <EVENT> RECEIVED"`). Commit nothing until it passes.

### Stage 2 — listening with semantics

Goal: the consumer's pure core reacts.

- Consumer `domain.h`: add the inbound `MsgType` (and payload fields on `Msg` if the event
  carries data), any `Model` field to cache state, and the outbound `CmdType`/`Cmd`.
- Consumer `logic.c`: handle the new message; extend `Model`; emit the real `Cmd`. Keep the
  switch exhaustive.
- Consumer `setup.c`: copy the raw `item` into a `Msg` and call `update`; execute the
  returned command. (In Stage 1 the raw `LedToggledEvent` can be handled directly; by
  Stage 2 the event is folded into the consumer's `Msg` so it routes through `update`.)
- Producer: if Stage 1's scratch payload was not the real one, give the producer's
  `domain.h` the field needed to carry the state the consumer wants, and publish it.
  With `main/events.h` (§2.1) the payload is already final, so nothing here changes.
- Add a native test for the new transition (see §7).
- Verify by scenario (`wait-serial` for the consumer's observable marker).

### Stage 3 — complete the worked sample (LED → fetch)

Goal: `<producer>` publishes the LED state; `<consumer>` fetches `/?led=on|off`.

- Producer: publish the post-toggle LED state (the level that `execute_*_hardware` just
  wrote). Keep the producer's own serial markers unchanged.
- Consumer: cache `led_on` from the message and build the request path from it in
  `commands.c`. **Do not change the existing network code** — UC-5's `wifi_init_once`
  one-shot is already correct; just choose the path.
- Scenario: BOOT press → `# LED ON (gpio 2)` → `# HTTP GET /?led=on` → `-> status 200`;
  press again → `# LED OFF (gpio 2)` → `# HTTP GET /?led=off`.
- Note: the C sample currently has no LED→fetch wiring, so Stage 3 is what this run adds.
  If `NEW_EVENT_NAME`/producer/consumer are different, Stage 3 replaces this example with
  the equivalent end-to-end behavior for those parameters.

---

## 5. Files by stage (parameterized)

| File | Stage | Change |
|---|---|---|
| `main/registry.h` | 1 | add `<NEW_EVENT_NAME>` before `EVENT_ID_COUNT` |
| `main/events.h` | 1 | add the neutral `<Payload>` struct (§2.1; new file) |
| `main/use_cases/<producer>/setup.c` | 1/3 | `event_bus_publish(...)` after the command executes |
| `main/use_cases/<consumer>/domain.h` | 2 | new `MsgType`, `Model`, `Cmd` |
| `main/use_cases/<consumer>/logic.c` | 2 | handle the message; exhaustive switch |
| `main/use_cases/<consumer>/setup.c` | 1/2 | `event_bus_subscribe(...)`; handle in `process_queue_item` |
| `main/use_cases/<consumer>/commands.c` | 3 | consumer side effect (path/URL/etc.) |
| `main/CMakeLists.txt` | new folder only | `SRCS`, `INCLUDE_DIRS`, `UC_MODULES` |
| `test/test_<consumer>.c` | 2 | new transition assertions |
| `test/Makefile` | 2 | compile/link rules for the new test |
| `tests/velxio/scenarios/*.yaml` | 3 | trigger + `wait-serial` assertions |
| docs (`README.md` §Status, `AGENTS.md` §2) | 3 | reflect reality if intent changed |

---

## 6. Step-by-step (re-runnable)

### 6.1 Add the event + payload

- [ ] In `main/registry.h`, add `<NEW_EVENT_NAME>` to `SystemEventId` **before**
  `EVENT_ID_COUNT`. Do not reorder existing values.
- [ ] In `main/events.h`, add the neutral payload struct (§2.1). It is a new file the
  first time; `INCLUDE_DIRS` already has `"."`, so no build edit is needed.

### 6.2 Producer publishes

- [ ] In `main/use_cases/<producer>/setup.c`, `#include "events.h"` and, after
  `execute_<producer>_hardware(&result.command)`, publish the state:
  ```c
  <Payload> out = { /* fields */ };
  event_bus_publish(<NEW_EVENT_NAME>, &out, sizeof(out));
  ```
- [ ] If the state lives on `result.command`/`result.next`, build the payload from those;
  do not read hardware again.
- [ ] Follow the existing feedback-publish idiom (UC-5's `setup.c` publishes
  `MSG_UART_TX_DONE` right after `CMD_SEND_UART`).

### 6.3 Consumer subscribes

- [ ] In `main/use_cases/<consumer>/setup.c`, `#include "events.h"` and add
  `event_bus_subscribe(<NEW_EVENT_NAME>, my_queue, sizeof(<Payload>));` in
  `wire_subscriptions`. Because both sides use the `events.h` type, the size always equals
  the producer's published size.

### 6.4 Consumer handles

- [ ] In `process_queue_item`, in **Stage 1** you may read the raw `events.h` payload
  directly (`const <Payload> *ev = (const <Payload> *)item;`) and log a marker. By
  **Stage 2**, fold the event into the consumer's `Msg` so it routes through `update`:
  ```c
  const Msg *msg = (const Msg *)item; /* item is the published payload bytes */
  UpdateResult r = <consumer>_update(local_model, *msg);
  local_model = r.next;
  execute_<consumer>_hardware(&r.command);
  ```
- [ ] Ensure the consumer's `self_module.message_size` can hold the event and that what
  you cast `item` to matches the published payload type/size.

### 6.5 Register (new folder only)

Skip if both folders already exist.

- [ ] In `main/CMakeLists.txt`, add the new folder's `logic.c`/`commands.c`/`setup.c` to
  `SRCS`, the folder to `INCLUDE_DIRS`, and the tag to `UC_MODULES`.
- [ ] Why: IDF compiles only `SRCS`, and links with `--gc-sections`, so the registration
  constructor is discarded unless `-Wl,--undefined=_uc_register_<tag>` is passed. A missing
  tag makes the module silently vanish — the symptom is a smaller `Found N` banner.

### 6.6 Native test

- [ ] Add/extend `test/test_<consumer>.c`: assert the new transition and emitted `Cmd`
  (mirror `test/test_fetch_and_uart.c`).
- [ ] Update `test/Makefile`: add the object rule for the consumer's `logic.c`,
  link it with your test object (plus `mock_registry.o` if using the mock bus), add it to
  `all` and `clean`.
- [ ] **Include ordering matters:** every plugin header is named `domain.h`. Put the new
  use-case dir before the existing toggle path, or give the test its own
  `-I../main -I../main/use_cases/<consumer>` set. Wrong order yields
  `implicit declaration` / `no member named …`.
- [ ] Run `make native-test`; every target must print `PASS`.

### 6.7 Scenario

- [ ] Add/extend `tests/velxio/scenarios/<name>.yaml` using the step vocabulary
  (`delay`, `wait-serial`, `write-serial`, `set-control`, `expect-pin`), one key per step.
- [ ] Trigger the producer with an existing physical input if possible (UC-1's BOOT button
  is on GPIO0 and is already in `diagram.json`). Otherwise add a `poll_timer_tick` that
  publishes the event, or drive it via `write-serial`.
- [ ] Assert with a firmware `printf` marker + `wait-serial`; output-pin `gpio_change` is
  unreliable. Allow ≥400 ms hold/release margins for the 100 ms poll/debounce.
- [ ] Run `make scenario NAME=<name>` → `RESULT: PASS`.

### 6.8 Docs

- [ ] Update `README.md` §Status / `AGENTS.md` §2 if the feature's intent changed.
- [ ] Tick §9 and commit only under `main/`, `test/`, `tests/velxio/scenarios/`,
  `docs/runbooks/`.

---

## 7. Verification / definition of done

1. `make native-test` — every target, including the new one, prints `PASS`.
2. `make build` — exits 0 with no new warnings; boot serial shows
   `Platform Engine Initializing: Found N Autonomous Modules.` (N = folders in
   `main/use_cases/`).
3. `make scenario NAME=<name>` — `RESULT: PASS`, exit 0.
4. `make test` — still `RESULT: PASS` (native tests + firmware + all scenarios).
5. No new compiler warnings under `-Wswitch-enum -Werror=switch`.
6. No file under `rust/` changed.

---

## 8. Failure modes (C-specific)

| Symptom | Cause | Fix |
|---|---|---|
| Event published but consumer never runs | `message_size` mismatch: `event_bus_publish` skips subscribers whose declared size differs (`registry.c`) | both sides include `main/events.h` and use `sizeof(<Payload>)`; check `self_module.message_size` |
| `Found N-1 Autonomous Modules` | new tag missing from `UC_MODULES`, or sources missing from `SRCS` (new folder only) | apply §6.5 |
| `-Wswitch-enum`/`-Werror=switch` build error | a `switch` misses an enum value, or a `default:` was added | enumerate all cases; remove `default:` |
| Build error: FreeRTOS/ESP headers in host test | hardware code not guarded | wrap it in `#ifdef ESP_PLATFORM` |
| Native test `implicit declaration` / `no member named …` | wrong `domain.h` picked up by include order | order the new dir before `toggle-physical-led`, or use a dedicated `-I` set (§6.6) |
| Consumer crashes/undefined on payload | read the raw `item` as a larger struct than was published | match the cast to the exact `main/events.h` payload type/size |
| Reentrancy worries | publishing from inside `process_queue_item` | it is safe — the main loop drains queues; **never** call another feature's `execute_*` directly, always publish |
| Scenario `expect-pin` never matches | output-pin `gpio_change` not reliably emitted | assert with `printf` + `wait-serial` |
| Second network trigger aborts the chip | (only if the consumer does Wi-Fi) re-running Wi-Fi init | keep init one-shot (`wifi_init_once` in UC-5); out of scope here |
| Listener HTTP 503 | another build/test is running | wait; requests are serialised |
| Listener 404 for a route in the file | host still running an old listener process | restart `.devcontainer/docker-build-listener.py` on the host |

---

## 9. Re-run checklist (copy for the next event)

Parameters: `NEW_EVENT_NAME` = ________  `producer` = ________  `consumer` = ________

- [ ] Stage 1: event id added to `registry.h`; neutral payload added to `main/events.h`;
      producer publishes; consumer subscribes and logs receipt; scenario confirms delivery
- [ ] Stage 2: consumer `MsgType`/`Model`/`Cmd` + logic + exhaustive switch; native test
      passes
- [ ] Stage 3: producer publishes real state; consumer acts; scenario asserts end-to-end
- [ ] `main/CMakeLists.txt` updated (new folder only; `events.h` needs no edit)
- [ ] `test/Makefile` include order correct; `make native-test` all `PASS`
- [ ] `make build` 0 warnings; banner `Found N`
- [ ] `make scenario NAME=<consumer>` `RESULT: PASS`; `make test` green
- [ ] Docs updated; nothing under `rust/` touched
- [ ] Re-read §2 (contract) and §8 (failure modes) before finishing

### 9.1 Current run record — LED toggle → fetch (`EVENT_LED_TOGGLED`)

Parameters: `NEW_EVENT_NAME` = `EVENT_LED_TOGGLED`  `producer` = `toggle-physical-led`
`consumer` = `fetch-and-uart`

| Stage | Deliverable | Status |
|---|---|---|
| 1 | `EVENT_LED_TOGGLED` in `registry.h`; `LedToggledEvent` in `main/events.h`; UC-1 publishes; UC-5 subscribes and logs `# EVENT_LED_TOGGLED RECEIVED`; delivery scenario passes | planned |
| 2 | UC-5 `domain.h`: `MSG_LED_TOGGLED`, `Model.led_on`, `Cmd.led_on`; `logic.c` caches and emits `CMD_SYNC_NETWORK`; native test passes | planned |
| 3 | UC-5 fetches `/?led=on|off` (one-shot Wi-Fi unchanged); `tests/velxio/scenarios/uc5_led_toggle_fetch.yaml`; `make test` green | planned |

Design notes carried through the run:

- **Payload home:** `main/events.h` (§2.1) — neutral struct, included by both features, so
  the `sizeof` contract holds and neither feature names the other's `Msg`/`Cmd`.
- **Stage 1 consumer path:** read the raw `LedToggledEvent` directly and log; fold it into
  UC-5's `Msg` in Stage 2 so the transition routes through the pure `update`.
- **NET button unchanged:** `MSG_UART_TX_DONE` keeps emitting `CMD_SYNC_NETWORK`, reusing
  the cached `led_on` (default `false` → `/?led=off`).
- **Boundary:** nothing under `rust/` is modified; the Rust twin
  (`add_rust_cross_feature_event.md`) already implements this behavior.

---

## 10. References

- `main/registry.h`, `main/registry.c` — `SystemEventId`, `event_bus_subscribe/publish`.
- `main/events.h` — neutral cross-feature event payloads (new file this run).
- `main/main.c` — engine, queues, `process_queue_item` drain.
- `main/use_cases/toggle-physical-led/` — producer reference (logic/setup/commands).
- `main/use_cases/fetch-and-uart/` — consumer reference (feedback publish, one-shot Wi-Fi).
- `test/test_event_bus.c`, `test/test_fetch_and_uart.c`, `test/mock_registry.c`,
  `test/Makefile` — native test references.
- `tests/velxio/scenarios/*.yaml` — emulator scenarios.
- [`add_use_case.md`](add_use_case.md) — single use-case plugin end to end.
- [`add_rust_cross_feature_event.md`](add_rust_cross_feature_event.md) — the Rust twin.
- [`add-rust-tea-pattern.md`](add-rust-tea-pattern.md),
  [`add_rust_embassy_port.md`](add_rust_embassy_port.md) — the Rust port.
- Root `AGENTS.md` §1 (architecture), §6 (iteration, registration, troubleshooting).
