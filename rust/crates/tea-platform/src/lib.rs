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
