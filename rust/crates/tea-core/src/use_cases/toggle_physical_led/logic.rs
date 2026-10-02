use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { led_on: false }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;

    let command = match msg {
        Msg::ButtonDown => {
            next.led_on = !model.led_on;
            Cmd::ToggleLed {
                pin: 2,
                level: u8::from(next.led_on),
            }
        }
    };

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
