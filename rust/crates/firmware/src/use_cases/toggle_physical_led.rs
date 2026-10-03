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

            let mut cfg: sys::gpio_config_t = core::mem::zeroed();
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

        // Announce the new LED state on the bus so other features (UC-5) can
        // react without referencing this module directly.
        match result.command {
            Cmd::ToggleLed { level, .. } => Some(PlatformEvent::LedToggled { on: level != 0 }),
            Cmd::None => None,
        }
    }
}
