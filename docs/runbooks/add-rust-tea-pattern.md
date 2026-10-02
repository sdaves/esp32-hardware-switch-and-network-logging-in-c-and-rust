# Runbook: Port the TEA platform to Rust (tea-core / tea-platform / firmware / host)

Status: **executed — Phase 1 done (2026-10-02)**. See the completion log below.
Audience: firmware maintainers and coding agents
Scope: stand up the Rust "Functional Core, Imperative Shell" platform — one pure
use-case folder per UC in `tea-core`, a typed event bus + module registry in
`tea-platform`, a `register_modules!` list and std `esp-idf-svc` engine in
`crates/firmware`, and host tests in `crates/host`. UC-1 is wired end-to-end;
UC-2..UC-5 are registered logic-only stubs so the boot banner reports five
modules. Wi-Fi/UC-5 parity is a later pass.

This is a companion to [`add_rust_embassy_port.md`](add_rust_embassy_port.md),
which remains the authoritative phased plan. This runbook implements the
Phase-1 increment with the decisions locked below.

This runbook is a set of instructions. It is **not executed by tooling**. All
file writes go under `rust/` (plus `docs/runbooks/` status notes). Never modify
`main/`, `test/`, `scripts/`, or `tests/velxio/` — reuse them read-only.

---

## 0. How to run this runbook

- Trigger it by asking an agent to "run `docs/runbooks/add-rust-tea-pattern.md`".
- Work top to bottom; keep the tree compiling between steps.
- Firmware builds and emulator scenarios run **inside the Velxio container**
  through the host listener (`curl http://host.docker.internal:2222/...`).
  Host `cargo test` can run the same way while the devcontainer mount is not
  rebuilt (see §8).

### Phase 1 completion log (2026-10-02)

- [x] `crates/{tea-core,tea-platform,host}` created; workspace now has all four
  members. Host tests: `tea-core` 5 unit + `host` `toggle_led` (2) and
  `event_bus` (3) pass; `cargo clippy --all-targets -- -D warnings` clean for
  our crates.
- [x] `firmware` engine + five plugin modules; boot serial prints
  `Platform Engine Initializing: Found 5 Autonomous Modules.`
- [x] `rust/dist/firmware.merged.bin` = 4194304 bytes; `uc1_button_toggle.yaml`
  → `RESULT: PASS`. `/opt/esp-idf-v4.4` clean.
- [x] `bash scripts/test.sh` chains host tests + build + scenario successfully.

Three deviations from the text below, all required to make it run:

1. **Workspace `default-members`.** `esp-idf-sys` refuses to build for non-ESP
   targets (`Unsupported target 'x86_64-...'`). A bare workspace `cargo test`
   would therefore try to host-compile the firmware and fail. `rust/Cargo.toml`
   sets `default-members` to the three host crates, and `scripts/build.sh` passes
   `-p rust-firmware`.
2. **Makefile env.** The Makefile must *not* override `HOME`/`IDF_TOOLS_PATH`:
   the container's ESP-IDF tools live at the ambient `/root/.espressif`, and
   IDF's `export.sh` resolves the compiler/GDB tools from it. Only
   `RUSTUP_HOME`/`CARGO_HOME` are routed into the `esp-rust` volume.
3. **Exhaustive-match style.** `let command = match ...` (rather than
   `let mut command` + assignment) keeps `clippy -D warnings` clean; the unit
   tests still observe the same `Cmd` values.

Also corrected during execution: UC-5 in the C tree *does* have a producer for
`EVENT_DB_QUERY_RESULT` (`fetch-and-uart/setup.c` polls GPIO4 and publishes it),
contrary to §7.4's note. Phase 1 still keeps UC-5 as a logic-only stub; that
producer moves in Phase 2.

### Locked decisions (from the design review)

| Decision | Value |
|---|---|
| Use-case layout | **Option A**: pure logic in `tea-core/src/use_cases/<name>/`, hardware plugins in `firmware/src/use_cases/<name>.rs` |
| Module registration | **`register_modules!` macro list** in the firmware (explicit, GC-safe; the runbook §5.2 fallback) |
| Scope of this pass | Foundation + **UC-1 real**; UC-2..UC-5 registered logic-only stubs |
| Firmware runtime | std `esp-idf-svc` 0.48.x, IDF 4.4.7, `--cfg espidf_time32` (unchanged) |

---

## 1. Prerequisites / current state

- `rust/crates/{tea-core,tea-platform,host}` exist only as empty directory
  skeletons (no `Cargo.toml`). `rust/crates/firmware` is the Phase-0 spike.
- The devcontainer currently has **no `cargo`** (`/opt/esp-rust` is not mounted
  until VS Code rebuilds the container). The Velxio container has the toolchain
  at `/opt/esp-rust`; run host/firmware commands via the listener (§8).
- Listener health:

  ```sh
  curl -sS --max-time 10 http://host.docker.internal:2222/health   # -> ok
  ```

- The C project's `main/use_cases/*/{domain.h,logic.c,setup.c,commands.c}` is the
  behavioural spec. Read them before porting.

---

## 2. Target layout

```text
rust/crates/
├── tea-core/                  # pure, no_std, no heap
│   └── src/
│       ├── lib.rs
│       └── use_cases/
│           ├── mod.rs
│           ├── toggle_physical_led/{mod.rs,domain.rs,logic.rs}   # UC-1 (real logic)
│           ├── read_analog_sensor/{mod.rs,domain.rs,logic.rs}    # UC-2
│           ├── fetch_and_save/{mod.rs,domain.rs,logic.rs}        # UC-3
│           ├── read_and_insert/{mod.rs,domain.rs,logic.rs}       # UC-4
│           └── fetch_and_uart/{mod.rs,domain.rs,logic.rs}        # UC-5
├── tea-platform/              # no_std + alloc: typed bus, trait, registry
│   └── src/lib.rs
├── host/                      # std: sim engine + cargo integration tests
│   ├── src/{lib.rs,bin/sim.rs}
│   └── tests/{toggle_led.rs,event_bus.rs}
└── firmware/                  # std esp-idf-svc engine + plugin shells
    └── src/
        ├── main.rs
        └── use_cases/
            ├── mod.rs
            ├── toggle_physical_led.rs
            ├── read_analog_sensor.rs
            ├── fetch_and_save.rs
            ├── read_and_insert.rs
            └── fetch_and_uart.rs
```

The "one folder per use case under a use-cases folder" lives in
`tea-core/src/use_cases/`; each UC's imperative shell is a sibling file in
`firmware/src/use_cases/`. This keeps the pure rules host-testable and
reusable by the future iced UI while preserving the C four-file split's
separation of concerns.

---

## 3. Workspace manifest

Edit `rust/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/tea-core",
    "crates/tea-platform",
    "crates/host",
    "crates/firmware",
]
```

Leave `[patch.crates-io]` and the rest untouched.

---

## 4. `tea-core` — pure use-case logic

`rust/crates/tea-core/Cargo.toml`:

```toml
[package]
name = "tea-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true
```

`rust/crates/tea-core/src/lib.rs`:

```rust
//! Pure TEA core: `update(Model, Msg) -> UpdateResult { next, command }`.
//! `no_std`, no heap, no I/O, no `unsafe`. Unit-tested on the host.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod use_cases;
```

`rust/crates/tea-core/src/use_cases/mod.rs`:

```rust
pub mod fetch_and_save;
pub mod fetch_and_uart;
pub mod read_analog_sensor;
pub mod read_and_insert;
pub mod toggle_physical_led;
```

### 4.1 UC-1 `toggle_physical_led` (real)

`.../toggle_physical_led/mod.rs`:

```rust
pub mod domain;
pub mod logic;

pub use domain::{Cmd, Model, Msg, UpdateResult};
pub use logic::{init, update};
```

`.../toggle_physical_led/domain.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    ButtonDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    ToggleLed { pin: u8, level: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub led_on: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
```

`.../toggle_physical_led/logic.rs`:

```rust
use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { led_on: false }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;
    let mut command = Cmd::None;

    match msg {
        Msg::ButtonDown => {
            next.led_on = !model.led_on;
            command = Cmd::ToggleLed {
                pin: 2,
                level: u8::from(next.led_on),
            };
        }
    }

    UpdateResult { next, command }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_off() {
        assert!(!init().led_on);
    }

    #[test]
    fn button_down_toggles_on() {
        let result = update(init(), Msg::ButtonDown);
        assert!(result.next.led_on);
        assert_eq!(result.command, Cmd::ToggleLed { pin: 2, level: 1 });
    }

    #[test]
    fn second_press_toggles_off() {
        let first = update(init(), Msg::ButtonDown);
        let second = update(first.next, Msg::ButtonDown);
        assert!(!second.next.led_on);
        assert_eq!(second.command, Cmd::ToggleLed { pin: 2, level: 0 });
    }
}
```

### 4.2 UC-2 `read_analog_sensor`

`.../read_analog_sensor/domain.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    AnalogSampleReady { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    PostAnalog { millivolts: i32 },
    PostAnalogError { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub last_millivolts: i32,
    pub in_error: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
```

`.../read_analog_sensor/logic.rs`:

```rust
use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model {
        last_millivolts: 0,
        in_error: false,
    }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;
    let mut command = Cmd::None;

    match msg {
        Msg::AnalogSampleReady { millivolts } => {
            next.last_millivolts = millivolts;
            next.in_error = millivolts < 0;
            command = if next.in_error {
                Cmd::PostAnalogError { millivolts }
            } else {
                Cmd::PostAnalog { millivolts }
            };
        }
    }

    UpdateResult { next, command }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_sample_routes_to_error() {
        let result = update(init(), Msg::AnalogSampleReady { millivolts: -5 });
        assert!(result.next.in_error);
        assert_eq!(result.command, Cmd::PostAnalogError { millivolts: -5 });
    }

    #[test]
    fn positive_sample_routes_to_post() {
        let result = update(init(), Msg::AnalogSampleReady { millivolts: 2000 });
        assert!(!result.next.in_error);
        assert_eq!(result.command, Cmd::PostAnalog { millivolts: 2000 });
    }
}
```

(The `mod.rs` for UC-2..UC-5 is identical in shape to UC-1's: re-export
`Cmd, Model, Msg, UpdateResult` and `init, update`.)

### 4.3 UC-3 `fetch_and_save`

`.../fetch_and_save/domain.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    HttpResponseReady { success: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    SaveIso8601File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub saved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
```

`.../fetch_and_save/logic.rs`:

```rust
use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { saved: false }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;
    let mut command = Cmd::None;

    match msg {
        Msg::HttpResponseReady { success } => {
            next.saved = success;
            if success {
                command = Cmd::SaveIso8601File;
            }
        }
    }

    UpdateResult { next, command }
}
```

### 4.4 UC-4 `read_and_insert`

`.../read_and_insert/domain.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    AdcRowReady { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    InsertDbRow { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub rows_logged: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
```

`.../read_and_insert/logic.rs`:

```rust
use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { rows_logged: 0 }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;
    let mut command = Cmd::None;

    match msg {
        Msg::AdcRowReady { millivolts } => {
            next.rows_logged = model.rows_logged + 1;
            command = Cmd::InsertDbRow { millivolts };
        }
    }

    UpdateResult { next, command }
}
```

### 4.5 UC-5 `fetch_and_uart`

`.../fetch_and_uart/domain.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    DbQueryResultReady { success: bool },
    UartTxDone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    SendUart,
    SyncNetwork,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub synced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
```

`.../fetch_and_uart/logic.rs`:

```rust
use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { synced: false }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;
    let mut command = Cmd::None;

    match msg {
        Msg::DbQueryResultReady { success } => {
            if success {
                command = Cmd::SendUart;
            }
        }
        Msg::UartTxDone => {
            command = Cmd::SyncNetwork;
            next.synced = true;
        }
    }

    UpdateResult { next, command }
}
```

---

## 5. `tea-platform` — typed bus + module registry

`rust/crates/tea-platform/Cargo.toml`:

```toml
[package]
name = "tea-platform"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
tea-core = { path = "../tea-core" }
```

`rust/crates/tea-platform/src/lib.rs`:

```rust
//! Platform contracts: the typed event bus, the module "vtable", the
//! `register_modules!` registry, and a small dispatch helper. `no_std` + alloc
//! only, so the same types compile for host tests and the std firmware.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

use tea_core::use_cases::{
    fetch_and_save, fetch_and_uart, read_analog_sensor, read_and_insert, toggle_physical_led,
};

/// The five platform events, mirroring `main/registry.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemEventId {
    HardwareAlertTriggered,
    SensorReadingReady,
    HttpResponseReceived,
    DbRowReady,
    DbQueryResult,
}

/// A typed bus payload. Replaces the C `memcpy` + declared message size: a
/// mismatched payload can no longer be delivered silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformEvent {
    HardwareAlert(toggle_physical_led::Msg),
    SensorReading(read_analog_sensor::Msg),
    HttpResponse(fetch_and_save::Msg),
    DbRow(read_and_insert::Msg),
    DbQueryResult(fetch_and_uart::Msg),
}

impl PlatformEvent {
    pub fn event_id(&self) -> SystemEventId {
        match self {
            PlatformEvent::HardwareAlert(_) => SystemEventId::HardwareAlertTriggered,
            PlatformEvent::SensorReading(_) => SystemEventId::SensorReadingReady,
            PlatformEvent::HttpResponse(_) => SystemEventId::HttpResponseReceived,
            PlatformEvent::DbRow(_) => SystemEventId::DbRowReady,
            PlatformEvent::DbQueryResult(_) => SystemEventId::DbQueryResult,
        }
    }
}

/// Mirrors the C `UseCaseModule` vtable. `poll_timer_tick`/`process` return an
/// optional event instead of publishing directly, so the engine can own the
/// modules and drain produced events without aliasing borrows.
pub trait PlatformModule {
    fn name(&self) -> &'static str;
    fn subscriptions(&self) -> &'static [SystemEventId];

    fn init_hardware(&mut self) {}

    fn poll_timer_tick(&mut self) -> Option<PlatformEvent> {
        None
    }

    fn process(&mut self, _event: &PlatformEvent) -> Option<PlatformEvent> {
        None
    }
}

/// Explicit, GC-safe registry (runbook §5.2 fallback). Expanding this in the
/// firmware defines `platform_modules()`.
#[macro_export]
macro_rules! register_modules {
    ($($module:expr),* $(,)?) => {
        pub fn platform_modules()
            -> $crate::__alloc::Vec<$crate::__alloc::Box<dyn $crate::PlatformModule>>
        {
            let mut modules = $crate::__alloc::Vec::new();
            $(
                modules.push(
                    $crate::__alloc::Box::new($module)
                        as $crate::__alloc::Box<dyn $crate::PlatformModule>,
                );
            )*
            modules
        }
    };
}

/// Re-exports for `register_modules!` expansion.
#[doc(hidden)]
pub mod __alloc {
    pub use alloc::boxed::Box;
    pub use alloc::vec::Vec;
}

/// Deliver `event` to every module subscribed to its id; return any events the
/// modules produced so the engine can queue and re-dispatch them.
pub fn dispatch(
    modules: &mut [Box<dyn PlatformModule>],
    event: &PlatformEvent,
) -> Vec<PlatformEvent> {
    let id = event.event_id();
    let mut produced = Vec::new();
    for module in modules.iter_mut() {
        if module.subscriptions().contains(&id) {
            if let Some(next) = module.process(event) {
                produced.push(next);
            }
        }
    }
    produced
}
```

Design note: `register_modules!` is the sanctioned `register_modules!` fallback
from `add_rust_embassy_port.md` §5.2. It gives an autonomous-looking module list
without C's `.ctors` / `--gc-sections` fragility. An upgrade to
`linkme::distributed_slice` can come later.

---

## 6. `host` — std sim engine + tests

`rust/crates/host/Cargo.toml`:

```toml
[package]
name = "tea-host"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
tea-core = { path = "../tea-core" }
tea-platform = { path = "../tea-platform" }
```

`rust/crates/host/src/lib.rs`:

```rust
//! Host-side engine used by the sim binary and the integration tests. Mirrors
//! the firmware poll/drain loop without any hardware.

use std::collections::VecDeque;

use tea_platform::{dispatch, PlatformEvent, PlatformModule};

pub struct SimEngine {
    modules: Vec<Box<dyn PlatformModule>>,
    queue: VecDeque<PlatformEvent>,
}

impl SimEngine {
    pub fn new(modules: Vec<Box<dyn PlatformModule>>) -> Self {
        Self {
            modules,
            queue: VecDeque::new(),
        }
    }

    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    pub fn publish(&mut self, event: PlatformEvent) {
        self.queue.push_back(event);
    }

    /// One 100 ms platform tick: poll every module, then drain the bus.
    pub fn tick(&mut self) {
        for module in self.modules.iter_mut() {
            if let Some(event) = module.poll_timer_tick() {
                self.queue.push_back(event);
            }
        }
        self.drain();
    }

    fn drain(&mut self) {
        while let Some(event) = self.queue.pop_front() {
            for next in dispatch(&mut self.modules, &event) {
                self.queue.push_back(next);
            }
        }
    }
}
```

`rust/crates/host/src/bin/sim.rs`:

```rust
//! Scripted CLI sim: feed `button` lines on stdin and watch the pure UC-1
//! update emit commands. No hardware.

use std::io::{self, BufRead};

use tea_core::use_cases::toggle_physical_led::{self, Msg};

fn main() {
    let mut model = toggle_physical_led::init();
    println!("initial led_on={}", model.led_on);

    for line in io::stdin().lock().lines() {
        let line = line.expect("stdin");
        if line.trim() == "button" {
            let result = toggle_physical_led::update(model, Msg::ButtonDown);
            model = result.next;
            println!("{:?} -> led_on={}", result.command, model.led_on);
        }
    }
}
```

`rust/crates/host/tests/toggle_led.rs` (mirrors `test/test_toggle_led.c`):

```rust
use tea_core::use_cases::toggle_physical_led::{self, Cmd, Msg};

#[test]
fn initial_state_is_led_off() {
    assert!(!toggle_physical_led::init().led_on);
}

#[test]
fn button_down_emits_toggle_led() {
    let result = toggle_physical_led::update(toggle_physical_led::init(), Msg::ButtonDown);
    assert!(result.next.led_on);
    assert_eq!(result.command, Cmd::ToggleLed { pin: 2, level: 1 });
}
```

`rust/crates/host/tests/event_bus.rs` (mirrors `test/test_event_bus.c`):

```rust
use std::cell::RefCell;
use std::rc::Rc;

use tea_core::use_cases::{read_analog_sensor, toggle_physical_led};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};
use tea_host::SimEngine;

struct Recorder {
    subscriptions: &'static [SystemEventId],
    seen: Rc<RefCell<Vec<PlatformEvent>>>,
}

impl PlatformModule for Recorder {
    fn name(&self) -> &'static str {
        "recorder"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        self.subscriptions
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        self.seen.borrow_mut().push(*event);
        None
    }
}

fn button_event() -> PlatformEvent {
    PlatformEvent::HardwareAlert(toggle_physical_led::Msg::ButtonDown)
}

#[test]
fn single_subscriber_receives() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut engine = SimEngine::new(vec![Box::new(Recorder {
        subscriptions: &[SystemEventId::HardwareAlertTriggered],
        seen: seen.clone(),
    })]);

    engine.publish(button_event());
    engine.tick();

    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(seen.borrow()[0], button_event());
}

#[test]
fn broadcast_to_two() {
    let a = Rc::new(RefCell::new(Vec::new()));
    let b = Rc::new(RefCell::new(Vec::new()));
    let mut engine = SimEngine::new(vec![
        Box::new(Recorder {
            subscriptions: &[SystemEventId::HardwareAlertTriggered],
            seen: a.clone(),
        }),
        Box::new(Recorder {
            subscriptions: &[SystemEventId::HardwareAlertTriggered],
            seen: b.clone(),
        }),
    ]);

    engine.publish(button_event());
    engine.tick();

    assert_eq!(a.borrow().len(), 1);
    assert_eq!(b.borrow().len(), 1);
}

#[test]
fn non_subscribed_event_ignored() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut engine = SimEngine::new(vec![Box::new(Recorder {
        subscriptions: &[SystemEventId::SensorReadingReady],
        seen: seen.clone(),
    })]);

    // A hardware-alert event must not reach a sensor subscriber.
    engine.publish(button_event());
    engine.tick();

    assert!(seen.borrow().is_empty());

    // Sanity: the subscriber does receive its own event.
    engine.publish(PlatformEvent::SensorReading(
        read_analog_sensor::Msg::AnalogSampleReady { millivolts: 1 },
    ));
    engine.tick();
    assert_eq!(seen.borrow().len(), 1);
}
```

---

## 7. `firmware` — std engine + plugin shells

`rust/crates/firmware/Cargo.toml` — add the two path deps:

```toml
[dependencies]
esp-idf-svc = "=0.48.1"
log = "0.4"
tea-core = { path = "../tea-core" }
tea-platform = { path = "../tea-platform" }
```

### 7.1 `rust/crates/firmware/src/main.rs`

Replace the Phase-0 spike with the engine:

```rust
use std::collections::VecDeque;
use std::time::Duration;

use esp_idf_svc::log::EspLogger;
use esp_idf_svc::sys;

use tea_platform::{dispatch, PlatformEvent, PlatformModule};

mod use_cases;

use use_cases::fetch_and_save::FetchAndSave;
use use_cases::fetch_and_uart::FetchAndUart;
use use_cases::read_analog_sensor::ReadAnalogSensor;
use use_cases::read_and_insert::ReadAndInsert;
use use_cases::toggle_physical_led::TogglePhysicalLed;

// The explicit registry list. Adding a use case = one line here plus its
// plugin module.
tea_platform::register_modules!(
    TogglePhysicalLed::new(),
    ReadAnalogSensor::new(),
    FetchAndSave::new(),
    ReadAndInsert::new(),
    FetchAndUart::new(),
);

fn main() {
    sys::link_patches();
    EspLogger::initialize_default();

    let mut modules = platform_modules();
    log::info!(
        "Platform Engine Initializing: Found {} Autonomous Modules.",
        modules.len()
    );

    for module in modules.iter_mut() {
        module.init_hardware();
        log::info!("# MODULE READY ({})", module.name());
    }

    let mut events: VecDeque<PlatformEvent> = VecDeque::new();

    loop {
        for module in modules.iter_mut() {
            if let Some(event) = module.poll_timer_tick() {
                events.push_back(event);
            }
        }

        while let Some(event) = events.pop_front() {
            for next in dispatch(&mut modules, &event) {
                events.push_back(next);
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
```

### 7.2 `rust/crates/firmware/src/use_cases/mod.rs`

```rust
pub mod fetch_and_save;
pub mod fetch_and_uart;
pub mod read_analog_sensor;
pub mod read_and_insert;
pub mod toggle_physical_led;
```

### 7.3 UC-1 plugin `toggle_physical_led.rs`

GPIO mirrors the C `commands.c` exactly (GPIO0 button, GPIO2 LED). Symbol names
below are from the generated `esp-idf-sys` bindings for IDF 4.4.7.

```rust
use esp_idf_svc::sys;

use tea_core::use_cases::toggle_physical_led::{self, Cmd};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

const BUTTON_GPIO: sys::gpio_num_t = 0;
const LED_GPIO: sys::gpio_num_t = 2;

pub struct TogglePhysicalLed {
    model: toggle_physical_led::Model,
    last_level: i32,
    armed: bool,
}

impl TogglePhysicalLed {
    pub fn new() -> Self {
        Self {
            model: toggle_physical_led::init(),
            last_level: 1,
            armed: true,
        }
    }

    fn button_pressed(&self) -> bool {
        unsafe { sys::gpio_get_level(BUTTON_GPIO) == 0 }
    }

    fn execute(&self, command: &Cmd) {
        match command {
            Cmd::None => {}
            Cmd::ToggleLed { pin, level } => {
                unsafe {
                    sys::gpio_set_level(LED_GPIO, u32::from(*level));
                }
                println!(
                    "# LED {} (gpio {})",
                    if *level != 0 { "ON" } else { "OFF" },
                    pin
                );
            }
        }
    }
}

impl PlatformModule for TogglePhysicalLed {
    fn name(&self) -> &'static str {
        "toggle-physical-led"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::HardwareAlertTriggered]
    }

    fn init_hardware(&mut self) {
        unsafe {
            sys::gpio_reset_pin(LED_GPIO);
            sys::gpio_set_direction(LED_GPIO, sys::gpio_mode_t_GPIO_MODE_OUTPUT);
            sys::gpio_set_level(LED_GPIO, 0);

            let mut cfg = sys::gpio_config_t::default();
            cfg.pin_bit_mask = 1u64 << (BUTTON_GPIO as u32);
            cfg.mode = sys::gpio_mode_t_GPIO_MODE_INPUT;
            cfg.pull_up_en = sys::gpio_pullup_t_GPIO_PULLUP_ENABLE;
            cfg.pull_down_en = sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE;
            cfg.intr_type = sys::gpio_int_type_t_GPIO_INTR_DISABLE;
            sys::gpio_config(&cfg);
        }
    }

    fn poll_timer_tick(&mut self) -> Option<PlatformEvent> {
        // Edge detection with release re-arm, copied from the C setup.c: fire
        // once on high->low and latch until the button returns high.
        let level = if self.button_pressed() { 0 } else { 1 };
        let mut event = None;
        if level == 0 && self.last_level == 1 && self.armed {
            self.armed = false;
            event = Some(PlatformEvent::HardwareAlert(
                toggle_physical_led::Msg::ButtonDown,
            ));
        } else if level == 1 {
            self.armed = true;
        }
        self.last_level = level;
        event
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        let PlatformEvent::HardwareAlert(msg) = event else {
            return None;
        };
        let result = toggle_physical_led::update(self.model, *msg);
        self.model = result.next;
        self.execute(&result.command);
        None
    }
}
```

### 7.4 Stub plugins (UC-2..UC-5)

Each is registered, subscribes to its event, and runs the pure logic, but its
command execution is a no-op (matching the C `commands.c` stubs). UC-5 keeps the
Phase-0 Wi-Fi code as dead code for the later Phase-2 pass.

`read_analog_sensor.rs`:

```rust
use tea_core::use_cases::read_analog_sensor::{self, Cmd};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

pub struct ReadAnalogSensor {
    model: read_analog_sensor::Model,
}

impl ReadAnalogSensor {
    pub fn new() -> Self {
        Self {
            model: read_analog_sensor::init(),
        }
    }
}

impl PlatformModule for ReadAnalogSensor {
    fn name(&self) -> &'static str {
        "read-analog-sensor"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::SensorReadingReady]
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        let PlatformEvent::SensorReading(msg) = event else {
            return None;
        };
        let result = read_analog_sensor::update(self.model, *msg);
        self.model = result.next;
        execute(&result.command);
        None
    }
}

fn execute(command: &Cmd) {
    match command {
        Cmd::None => {}
        // Phase 2: POST the sample / route the error.
        Cmd::PostAnalog { .. } => {}
        Cmd::PostAnalogError { .. } => {}
    }
}
```

`fetch_and_save.rs`:

```rust
use tea_core::use_cases::fetch_and_save::{self, Cmd};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

pub struct FetchAndSave {
    model: fetch_and_save::Model,
}

impl FetchAndSave {
    pub fn new() -> Self {
        Self {
            model: fetch_and_save::init(),
        }
    }
}

impl PlatformModule for FetchAndSave {
    fn name(&self) -> &'static str {
        "fetch-and-save"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::HttpResponseReceived]
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        let PlatformEvent::HttpResponse(msg) = event else {
            return None;
        };
        let result = fetch_and_save::update(self.model, *msg);
        self.model = result.next;
        execute(&result.command);
        None
    }
}

fn execute(command: &Cmd) {
    match command {
        Cmd::None => {}
        // Phase 2: write the ISO8601 file.
        Cmd::SaveIso8601File => {}
    }
}
```

`read_and_insert.rs`:

```rust
use tea_core::use_cases::read_and_insert::{self, Cmd};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

pub struct ReadAndInsert {
    model: read_and_insert::Model,
}

impl ReadAndInsert {
    pub fn new() -> Self {
        Self {
            model: read_and_insert::init(),
        }
    }
}

impl PlatformModule for ReadAndInsert {
    fn name(&self) -> &'static str {
        "read-and-insert"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::DbRowReady]
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        let PlatformEvent::DbRow(msg) = event else {
            return None;
        };
        let result = read_and_insert::update(self.model, *msg);
        self.model = result.next;
        execute(&result.command);
        None
    }
}

fn execute(command: &Cmd) {
    match command {
        Cmd::None => {}
        // Phase 2: INSERT the SQLite row.
        Cmd::InsertDbRow { .. } => {}
    }
}
```

`fetch_and_uart.rs`:

```rust
use core::convert::TryInto;
use std::io::{Read, Write};
use std::net::TcpStream;

use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition,
};

use tea_core::use_cases::fetch_and_uart::{self, Cmd, Msg};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

pub struct FetchAndUart {
    model: fetch_and_uart::Model,
}

impl FetchAndUart {
    pub fn new() -> Self {
        Self {
            model: fetch_and_uart::init(),
        }
    }

    fn execute(&self, command: &Cmd) -> Option<PlatformEvent> {
        match command {
            Cmd::None => None,
            Cmd::SendUart => {
                // The console write is the observable UART0 transmit; this
                // mirrors the C `printf("# UART SENT\n")`.
                println!("# UART SENT");
                Some(PlatformEvent::DbQueryResult(Msg::UartTxDone))
            }
            Cmd::SyncNetwork => {
                // Phase 2: call `wifi_and_fetch()` here.
                println!("# NETWORK SYNCED");
                None
            }
        }
    }
}

impl PlatformModule for FetchAndUart {
    fn name(&self) -> &'static str {
        "fetch-and-uart"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::DbQueryResult]
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        let PlatformEvent::DbQueryResult(msg) = event else {
            return None;
        };
        let result = fetch_and_uart::update(self.model, *msg);
        self.model = result.next;
        self.execute(&result.command)
    }
}

// Phase 0 Wi-Fi + HTTP spike, preserved for Phase 2 wiring. Not called yet.
#[allow(dead_code)]
fn wifi_and_fetch() -> Result<(), Box<dyn std::error::Error>> {
    const SSID: &str = "Espressif";
    const URL_HOST: &str = "192.168.4.2:8000";
    const URL_PATH: &str = "/";

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::None,
        password: "".try_into().unwrap(),
        channel: None,
    }))?;

    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    log::info!("# WIFI CONNECTED ({SSID})");

    let mut stream = TcpStream::connect(URL_HOST)?;
    write!(
        stream,
        "GET {URL_PATH} HTTP/1.1\r\nHost: {URL_HOST}\r\nConnection: close\r\n\r\n"
    )?;

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..n]);
    let mut lines = response.lines();
    if let Some(status_line) = lines.next() {
        let code = status_line.split_whitespace().nth(1).unwrap_or("?");
        log::info!("-> status {code}");
    }
    let body: String = lines.collect::<Vec<_>>().join("\n");
    log::info!("# HTTP BODY {body}");
    log::info!("# NETWORK SYNCED");

    Ok(())
}
```

Note: UC-5 has no producer for `EVENT_DB_QUERY_RESULT` yet (same as C today),
so the its chain only runs if something publishes that event. That is expected
until Phase 2.

---

## 8. Scenarios and scripts

### 8.1 Scenario

Add `rust/scenarios/uc1_button_toggle.yaml`:

```yaml
name: UC-1 button toggles the LED (Rust)
version: 1
steps:
  - wait-serial: "Platform Engine Initializing: Found 5 Autonomous Modules."
  - set-control: { part-id: btn1, control: pressed, value: 1 }
  - delay: 200ms
  - set-control: { part-id: btn1, control: pressed, value: 0 }
  - delay: 250ms
  - wait-serial: "# LED ON"
  - set-control: { part-id: btn1, control: pressed, value: 1 }
  - delay: 200ms
  - set-control: { part-id: btn1, control: pressed, value: 0 }
  - delay: 250ms
  - wait-serial: "# LED OFF"
```

Delete `rust/scenarios/phase0_hello.yaml` and `rust/scenarios/phase0_wifi.yaml`:
they assert the old `Found 0` banner and boot-time Wi-Fi and would now fail. The
Wi-Fi code is preserved in `fetch_and_uart.rs`.

### 8.2 `rust/scripts/test.sh` (new)

```bash
#!/usr/bin/env bash
# Host cargo tests + firmware build + UC-1 emulator scenario.
set -euo pipefail
cd "$(dirname "$0")/.." # rust/

RUST_ROOT="${RUST_ROOT:-/opt/esp-rust}"
export RUSTUP_HOME="${RUSTUP_HOME:-$RUST_ROOT/rustup}"
export CARGO_HOME="${CARGO_HOME:-$RUST_ROOT/cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

VELXIO_WS="${VELXIO_WS:-ws://localhost}"
REPO_ROOT="$(cd .. && pwd)"

echo "== host: cargo test"
cargo test

echo "== firmware: build"
./scripts/build.sh

echo "== emulator: uc1_button_toggle"
( cd "$REPO_ROOT" && python3 -m tests.velxio.runner.run_scenario \
    --server "$VELXIO_WS" \
    --firmware rust/dist/firmware.merged.bin \
    --diagram tests/velxio/diagram.json \
    --scenario rust/scenarios/uc1_button_toggle.yaml )
```

### 8.3 `rust/scripts/scenario.sh` (new; referenced by the Makefile)

```bash
#!/usr/bin/env bash
# Run one Rust emulator scenario by name: ./scripts/scenario.sh uc1_button_toggle
set -euo pipefail
cd "$(dirname "$0")/.."
NAME="${1:?usage: scenario.sh <name>}"
VELXIO_WS="${VELXIO_WS:-ws://localhost}"
REPO_ROOT="$(cd .. && pwd)"

( cd "$REPO_ROOT" && python3 -m tests.velxio.runner.run_scenario \
    --server "$VELXIO_WS" \
    --firmware rust/dist/firmware.merged.bin \
    --diagram tests/velxio/diagram.json \
    --scenario "rust/scenarios/${NAME}.yaml" )
```

Mark both executable (`chmod +x`); note the virtiofs mount may strip the bit, so
invoke via `bash scripts/test.sh` if needed.

---

## 9. Verification (through the host listener)

```sh
curl -sS --max-time 10 http://host.docker.internal:2222/health   # -> ok

# Host tests (tea-core unit + host integration).
curl -sS --max-time 600 -G \
  --data-urlencode 'cmd=cd /workspace/rust && make test' \
  http://host.docker.internal:2222/exec

# Firmware build -> rust/dist/firmware.merged.bin (must be 4194304 bytes).
curl -sS --max-time 2400 -G \
  --data-urlencode 'cmd=cd /workspace/rust && make build' \
  http://host.docker.internal:2222/exec

# UC-1 emulator scenario (single run, no rebuild).
curl -sS --max-time 300 -G \
  --data-urlencode 'cmd=cd /workspace && python3 -m tests.velxio.runner.run_scenario --server ws://localhost --firmware rust/dist/firmware.merged.bin --diagram tests/velxio/diagram.json --scenario rust/scenarios/uc1_button_toggle.yaml' \
  http://host.docker.internal:2222/exec

# Lints (host crates) and IDF tree cleanliness.
curl -sS --max-time 600 -G \
  --data-urlencode 'cmd=cd /workspace/rust && cargo clippy -- -D warnings' \
  http://host.docker.internal:2222/exec
curl -sS -G \
  --data-urlencode 'cmd=git -C /opt/esp-idf-v4.4 status --short' \
  http://host.docker.internal:2222/exec
```

`rust/scripts/test.sh` chains host tests + build + scenario in one call:

```sh
curl -sS --max-time 3000 -G \
  --data-urlencode 'cmd=cd /workspace/rust && bash scripts/test.sh' \
  http://host.docker.internal:2222/exec
```

---

## 10. Definition of done

1. Host `cargo test` passes: `tea-core` unit tests + `host` `tests/toggle_led.rs`
   and `tests/event_bus.rs`.
2. `make build` exits 0; boot serial contains
   `Platform Engine Initializing: Found 5 Autonomous Modules.`
3. `rust/scenarios/uc1_button_toggle.yaml` prints `RESULT: PASS` for the Rust bin.
4. No warnings under `cargo clippy -- -D warnings` for our crates.
5. `rust/dist/firmware.merged.bin` is exactly 4194304 bytes and boots in Velxio.
6. Built with `--cfg espidf_time32` against IDF 4.4.7; nothing outside `rust/`
   (plus status notes under `docs/runbooks/`) changed.
   `/opt/esp-idf-v4.4` is clean.

---

## 11. Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `error: failed to load manifest for workspace member .../tea-core` | member added before its `Cargo.toml` exists | create the crate manifest first, or add members incrementally |
| `can't find crate for 'std'` / `no_std` errors in `tea-platform` | used `vec!`/`std` paths in a `no_std` crate | keep `extern crate alloc;` and use `$crate::__alloc::{Box, Vec}` in the macro |
| `the trait bound ... PlatformModule is not satisfied` / method not found on `Box<dyn PlatformModule>` | trait not in scope | `use tea_platform::PlatformModule;` |
| banner says `Found 0` or fewer | plugin missing from `register_modules!`, or a module's `new()` not listed | add the constructor to the list in `main.rs` |
| `cannot borrow ... as mutable` in the engine loop | polling and dispatching in the same borrow | poll first, collect into `VecDeque`, then `dispatch` (as written) |
| `# LED ON` never appears | GPIO mismatch, or `expect-pin` used instead of serial | mirror the C pins (button GPIO0, LED GPIO2); assert with `wait-serial` |
| second press ignored | debounce/re-arm margin too small in the scenario | hold ≥400 ms in the scenario; the C pattern latches until release |
| `E0308 *const i8 vs *const u8` in esp-idf-svc | signed-char toolchain mismatch | keep the vendored `[patch.crates-io]` / `build.sh` `-fsigned-char` |
| `esp_phy_enable` assert | built against IDF 5.x | build via `rust/scripts/build.sh` (IDF 4.4.7) |
| `image not 4 MB` / won't boot | merge/partition regression | `make build` again; check `stat -c '%s' rust/dist/firmware.merged.bin` |
| `esperror: no wifi` / reboot on UC-5 later | Wi-Fi init run twice | keep `wifi_and_fetch` one-shot (single call) in Phase 2 |

---

## 12. References

- [`add_rust_embassy_port.md`](add_rust_embassy_port.md) — authoritative phased plan (Phase 1/2 scope, stack rationale).
- `rust/AGENTS.md` — hard rules, layout, definition of done, troubleshooting.
- `main/use_cases/*/{domain.h,logic.c,setup.c,commands.c}` — behavioural spec.
- `main/{main.c,registry.{c,h}}` — engine and bus contract.
- `test/test_toggle_led.c`, `test/test_event_bus.c` — host test references.
- `tests/velxio/scenarios/uc1_button_toggle.yaml` — emulator scenario reference.
- `rust/vendor/esp-idf-svc/CHANGES.md` — the only intentional dependency patch.
