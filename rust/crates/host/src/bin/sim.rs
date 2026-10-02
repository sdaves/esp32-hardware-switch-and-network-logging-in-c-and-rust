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
