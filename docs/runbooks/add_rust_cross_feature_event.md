# Runbook: Add a cross-feature, event-driven behavior to the Rust TEA port

Status: **executed (2026-10-03) — reusable template**. The reference run added the
UC-5 LED-state query; §3 is the historical record and §7 is the blank re-run checklist.
Audience: firmware maintainers and coding agents
Scope: wire one feature to **react to another feature's state without a direct
cross-feature reference**, using the typed event bus in `crates/tea-platform`. The
reference implementation is UC-1's LED toggle causing UC-5 to `GET /?led=on|off`.

This is a companion to [`add-rust-tea-pattern.md`](add-rust-tea-pattern.md) (Phase 1
foundation) and [`add_rust_embassy_port.md`](add_rust_embassy_port.md) (authoritative
phased plan). It assumes that foundation already builds and passes.

This runbook is a set of instructions. It is **not executed by tooling**. It may be run
any number of times; **each run adds exactly one cross-feature behavior and then stops**.
All file writes go under `rust/` (plus status notes under `docs/runbooks/`). Never modify
`main/`, `test/`, `scripts/`, or `tests/velxio/` — reuse them read-only.

---

## 0. How to run this runbook

- Trigger it by asking an agent to "run `docs/runbooks/add_rust_cross_feature_event.md`".
- Optionally name the behavior, e.g. *"…for the UC-5 LED-state query"*.
- Work top to bottom; keep the tree compiling between steps (§4).
- Firmware builds and emulator scenarios run **inside the Velxio container** through the
  host listener (`curl http://host.docker.internal:2222/...`, per root `AGENTS.md` §5).
  Host `cargo test` may run in the devcontainer once its mount is rebuilt, or via the
  listener in the meantime.
- Complete §4 once. Do **not** batch multiple behaviors into one run.

### When to use this pattern

Use it when feature **B** must do something in response to feature **A**'s state, and
`A` and `B` must not know about each other (rust/AGENTS.md hard rule 3 — use-case modules
never reference each other; cross-feature communication goes through `tea-platform`).

Reference behavior: UC-1 (BOOT button) toggles the LED; UC-5 (NET button) fetches
`/?led=on|off`. The LED toggle publishes a new bus event; UC-5 subscribes and issues a
network fetch with the new state. The NET button's own behavior is unchanged.

---

## 1. The pattern

```text
  Feature A (producer)                        Feature B (consumer)
  ────────────────────                        ────────────────────
  process(event) -> core update -> Cmd         subscriptions() += new_event_id
      |                                           |
      | shell executes Cmd (GPIO/IO)              | process(new_event) ->
      |                                           |   core update(model, Msg::…) ->
      v                                           v   shell executes Cmd (fetch/IO)
  return Some(PlatformEvent::NewThing { .. })  bus delivers to B
```

Invariants (do not relax them):

1. **Pure core.** `tea-core` stays by-value, zero pointers/heap/IO. `update` returns
   `UpdateResult { next, command }`; shells execute `Cmd`.
2. **Neutral bus payload.** A `PlatformEvent` variant carries a *self-contained* value
   (`{ on: bool }`), not another feature's `Msg` type.
3. **Exhaustive matching.** Every `match` over a message/command/event enum enumerates
   all variants. **Never write `_ =>`** for these enums — a new variant must be a compile
   error until handled.
4. **One-shot hardware singletons.** Wi-Fi/ESP-IDF singletons are taken once and kept
   alive; never re-take them (see §6).

---

## 2. Prerequisites

- [x] Phase 1 foundation exists: `crates/{tea-core,tea-platform,firmware,host}` and the
  `register_modules!` list all build (see `add-rust-tea-pattern.md`).
- [x] `make build` produces a 4 MB image and `cargo test` passes.
- [x] Sibling scenarios in `rust/scenarios/` run via the C runner
  (`tests/velxio/runner/run_scenario.py`).
- [x] Listener health: `curl -sS --max-time 10 http://host.docker.internal:2222/health` → `ok`.

---

## 3. Reference implementation (UC-5 LED-state query) — what we did

All boxes are `[x]`: this run is complete and is the model to copy.

### 3.1 Files changed

| File | Change |
|---|---|
| `rust/crates/tea-core/src/use_cases/fetch_and_uart/domain.rs` | add `Msg::LedToggled { on: bool }`; add `Model.led_on`; change `Cmd::SyncNetwork` → `SyncNetwork { led_on: bool }` |
| `rust/crates/tea-core/src/use_cases/fetch_and_uart/logic.rs` | `LedToggled` sets `led_on` and emits `SyncNetwork { led_on }`; `UartTxDone` emits `SyncNetwork { led_on: model.led_on }`; unit tests |
| `rust/crates/tea-platform/src/lib.rs` | add `SystemEventId::LedToggled`; add `PlatformEvent::LedToggled { on: bool }`; update `event_id()` |
| `rust/crates/host/tests/led_toggle_network.rs` | new: bus routing + core-command test |
| `rust/crates/firmware/src/use_cases/toggle_physical_led.rs` | `process` returns `Some(PlatformEvent::LedToggled { on })` after executing `Cmd::ToggleLed` |
| `rust/crates/firmware/src/use_cases/fetch_and_uart.rs` | subscribe to `LedToggled`; cache `led_on`; fetch `/?led=on|off`; Wi-Fi one-shot at boot with 3 retries |
| `rust/scenarios/uc5_fetch_and_uart.yaml` | assert `# HTTP GET /?led=off` + `status 200` |
| `rust/scenarios/uc5_led_toggle_fetch.yaml` | new: BOOT press → `# LED ON (gpio 2)` → `?led=on` → `status 200`; again → `# LED OFF` → `?led=off` |
| `rust/scripts/test.sh` | loop over `scenarios/*.yaml` instead of one scenario |
| `rust/scenarios/uc1_button_toggle.yaml`, `uc5_fetch_and_uart.yaml`, `uc5_led_toggle_fetch.yaml` | wait for `# MODULE READY (fetch-and-uart)` + settle delay before pressing |
| `rust/AGENTS.md`, `docs/runbooks/add-rust-tea-pattern.md` | phase-2 status note |

### 3.2 Decisions and why

- **Dedicated neutral payload** `PlatformEvent::LedToggled { on: bool }` rather than
  reusing UC-5's `Msg` — keeps UC-1's event independent of UC-5's domain (rule 2).
- **Producer returns an event from `process`** rather than publishing directly — the
  engine owns the module list and drains produced events, avoiding aliasing borrows.
- **Wi-Fi on a one-shot at boot**, no request until an event (see §6 for why not lazy
  and why not a worker thread).
- **`/?led=on|off` built in the shell**, not the core — the core emits
  `Cmd::SyncNetwork { led_on }` and the shell maps booleans to paths (pure core).

---

## 4. Step-by-step (re-runnable)

### 4.1 `tea-core` — pure logic

- [ ] In the consumer's `domain.rs`, add the inbound message variant (e.g.
  `Msg::LedToggled { on: bool }`), any cached model field, and any `Cmd` payload the
  shell needs (e.g. `Cmd::SyncNetwork { led_on: bool }`).
- [ ] In `logic.rs`, handle it and every existing arm. Prefer
  `let command = match msg { … };` over `let mut command = Cmd::None;` + reassignment
  (avoids `unused_assignments` and `clippy::needless_late_init`).
- [ ] Add unit tests for: the new transition, and any cached-state path (e.g. the
  existing trigger reusing the cached value).
- [ ] Run `cargo test -p tea-core` (or workspace `cargo test`).

### 4.2 `tea-platform` — bus contract

- [ ] Add the `SystemEventId` variant.
- [ ] Add a **neutral** `PlatformEvent` variant carrying a self-contained payload.
- [ ] Update `event_id()` exhaustively.
- [ ] Run `cargo test -p tea-platform`.

### 4.3 `host` — routing test (mirrors `test/test_event_bus.c`)

- [ ] Add a test that publishes the new event through `SimEngine`, asserts a subscriber
  receives it, and asserts the consumer core `update` emits the expected `Cmd`.
- [ ] Run `cargo test -p tea-host`.

### 4.4 Producer shell (`firmware/src/use_cases/<producer>.rs`)

- [ ] After executing its `Cmd`, return `Some(PlatformEvent::<NewThing> { .. })` from
  `process`. Keep the existing serial markers unchanged.
- [ ] Match on `result.command` exhaustively (`Cmd::None` and each variant), no `_ =>`.

### 4.5 Consumer shell (`firmware/src/use_cases/<consumer>.rs`)

- [ ] Add the new id to `subscriptions()` (alongside existing ids).
- [ ] Handle the new event in `process`; route through the core `update`; execute the
  returned `Cmd`.
- [ ] Keep the "other events" arm explicit: enumerate every remaining `PlatformEvent`
  variant returning `None` — do **not** use `_ =>`.
- [ ] If the `Cmd` triggers a network action, implement it as in §6 (one-shot Wi-Fi).

### 4.6 Scenario

- [ ] Add/extend a `rust/scenarios/*.yaml`. Assert behavior with `printf`/`log::info!`
  markers + `wait-serial` (output-pin `gpio_change` is unreliable; root `AGENTS.md` §5).
- [ ] **Gate on readiness.** Because boot-time work (e.g. Wi-Fi) runs in `init_hardware`
  before the poll loop starts, wait for `# MODULE READY (fetch-and-uart)` and add a short
  `delay` before any `set-control`. See §6.

### 4.7 Scripts, docs

- [ ] Confirm `rust/scripts/test.sh` runs every scenario (`for scenario in scenarios/*.yaml`).
- [ ] Update `rust/AGENTS.md` phase/status and, if useful, the companion runbook.
- [ ] Tick the boxes in §7 and commit under `rust/` + `docs/runbooks/` only.

---

## 5. Verification / definition of done

- [ ] Host `cargo test` passes (core units + host integration).
- [ ] `cargo clippy --all-targets -- -D warnings` clean for our crates (dependency lints
  are capped by `--cap-lints allow`).
- [ ] `make build` exits 0; `rust/dist/firmware.merged.bin` is exactly **4194304 bytes**.
- [ ] Boot serial contains `Platform Engine Initializing: Found 5 Autonomous Modules.`
- [ ] Every scenario under `rust/scenarios/` prints `RESULT: PASS` for the Rust bin.
- [ ] `bash scripts/test.sh` is green end-to-end.
- [ ] `/opt/esp-idf-v4.4` is clean: `git -C /opt/esp-idf-v4.4 status --short` is empty.

Run it (host listener):

```sh
# full chain: host tests + build + all rust scenarios
curl -sS --max-time 3000 -G \
  --data-urlencode 'cmd=cd /workspace/rust && bash scripts/test.sh' \
  http://host.docker.internal:2222/exec

# one scenario, no rebuild
curl -sS --max-time 300 -G \
  --data-urlencode 'cmd=cd /workspace && python3 -m tests.velxio.runner.run_scenario --server ws://localhost --firmware rust/dist/firmware.merged.bin --diagram tests/velxio/diagram.json --scenario rust/scenarios/uc5_led_toggle_fetch.yaml --timeout 30' \
  http://host.docker.internal:2222/exec
```

---

## 6. Pitfalls and gotchas (learned the hard way)

Each is *symptom → cause → fix*, taken from the reference run.

| Symptom | Cause | Fix |
|---|---|---|
| `EspError(259)` on the 2nd+ trigger (`# WIFI SPIKE FAILED`) | `259` = `ESP_ERR_INVALID_STATE`. The Wi-Fi stack was built inside a function and **dropped on return**; re-taking ESP-IDF singletons (`Peripherals`, `EspSystemEventLoop`, `EspDefaultNvsPartition`) fails. | One-shot: hold `BlockingWifi<EspWifi<'static>>` in the module and never re-init. Leak `Peripherals` once via `Box::leak` (the Rust equivalent of the C file-static `s_wifi_ready`). |
| `# HTTP GET FAILED: Os { code: 104, kind: ConnectionReset }` | The `scripts/dev-http-server.py` reverse proxy forwarding to an unreachable parent (`host.docker.internal:8000`). Firmware is fine. | Test against a real server: `rust/scripts/py-http-server.sh start` (plain `python3 -m http.server` on `:8000`). Confirm the guest request lands in `/tmp/velxio-pyhttp8000.log`. |
| Button presses stop being detected after adding a triggered network call | The single-threaded engine stalls while `wifi.connect()`/`wait_netif_up()` associate (~10 s) if called from `process`/`poll_timer_tick`. Edges during the stall are lost. | Bring Wi-Fi up **once at boot in `init_hardware`** (runs before the poll loop), then keep it. Triggered fetches are then immediate. |
| A worker thread "fix" makes emulator GPIO injection stop working | A `std::thread` owning Wi-Fi interfered with the emulator's GPIO4 input in our runs; GPIO read a constant level. | Do **not** move Wi-Fi/HTTP to a worker thread. Keep it on the engine's `init_hardware` (boot) and `process` (triggered) paths. |
| Scenario fails at the first button step with "serial never contained …" | After boot-time Wi-Fi, the banner prints **before** `init_hardware` finishes; the poll loop has not started when the scenario presses. | Scenario must `wait-serial` for `# MODULE READY (fetch-and-uart)` and `delay: 500ms` before `set-control`. |
| False boot press, or flaky first press on a button | Transient GPIO level right after `gpio_config`/boot. Trying to "help" by re-sampling the pin at boot (`last_level` from `gpio_get_level`) makes it *worse*. | Keep the C init verbatim: `last_level = 1`, `armed = true`. Let the release re-arm logic handle it. |
| `clippy::needless_late_init` / `unused_assignments` | `let mut command = Cmd::None;` then reassigning in a single-arm `match`. | Use `let command = match msg { … };`. |
| New enum variant breaks the build | This is intended (`-Wswitch-enum` analogue). | Handle it in every `match`. **Never** add `_ =>` for message/command/event enums. |
| Bare `cargo test` at the workspace root tries to build firmware and fails with `Unsupported target 'x86_64-…'` | `esp-idf-sys` rejects non-ESP targets. | `rust/Cargo.toml` sets `default-members` to the three host crates; `scripts/build.sh` selects `-p rust-firmware`. Keep both. |
| IDF export errors (`tool xtensa-esp32-elf has no installed versions`) | The Makefile overrode `HOME`/`IDF_TOOLS_PATH`, pointing IDF at an empty `.espressif`. | Do not override `HOME`/`IDF_TOOLS_PATH`; the container's tools live at the ambient `/root/.espressif`. Only route `RUSTUP_HOME`/`CARGO_HOME` into `/opt/esp-rust`. |
| Listener returns HTTP 503 / `busy`, `/health` also busy | An aborted `/exec` still holds the serialised listener lock. | Restart the host listener, or from inside the container `pkill -f <script>` the leftover process. Wait, then retry. |

---

## 7. Re-run checklist (copy for the next behavior)

Target: ________________________________  (e.g. `read-and-insert` reacts to `DbRowReady`)

- [ ] §4.1 `tea-core` message/model/command + exhaustive matches + unit tests
- [ ] §4.2 `tea-platform` event id + neutral payload + `event_id()`
- [ ] §4.3 `host` routing + core-command test
- [ ] §4.4 producer shell returns `Some(PlatformEvent::…)`
- [ ] §4.5 consumer shell subscribes, processes, executes `Cmd`; explicit other-arm
- [ ] §4.6 scenario with readiness gate (`# MODULE READY (fetch-and-uart)` + delay)
- [ ] §4.7 `test.sh` loops all scenarios; docs updated; only `rust/` + `docs/runbooks/` touched
- [ ] §5 `cargo test` + clippy clean; 4 MB image; banner `Found 5`; every scenario `PASS`;
      `/opt/esp-idf-v4.4` clean
- [ ] Re-read §6 pitfalls before touching Wi-Fi/HTTP or scenario timing

---

## 8. References

- [`add-rust-tea-pattern.md`](add-rust-tea-pattern.md) — Phase 1 foundation (crates, bus,
  registry, engine, tests) and its completion log.
- [`add_rust_embassy_port.md`](add_rust_embassy_port.md) — authoritative phased plan.
- [`add_use_case.md`](add_use_case.md) — taking a single use-case plugin end to end.
- [`add_testing_simulator.md`](add_testing_simulator.md) — harness protocol and step vocabulary.
- Root `AGENTS.md` — C architecture, listener workflow, emulator/WebSocket protocol.
- `rust/AGENTS.md` — hard rules, layout, definition of done, troubleshooting.
- C behavioral spec: `main/registry.{c,h}`, `main/use_cases/*/{domain.h,logic.c,setup.c,commands.c}`.
- `rust/scripts/{build,test,scenario,py-http-server}.sh` and `rust/scenarios/*.yaml`.
