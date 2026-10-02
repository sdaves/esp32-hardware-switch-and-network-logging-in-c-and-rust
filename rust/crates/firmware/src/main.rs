use std::collections::VecDeque;
use std::time::Duration;

use esp_idf_svc::log::EspLogger;
use esp_idf_svc::sys;

use tea_platform::{dispatch, PlatformEvent};

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
