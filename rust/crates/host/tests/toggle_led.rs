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
