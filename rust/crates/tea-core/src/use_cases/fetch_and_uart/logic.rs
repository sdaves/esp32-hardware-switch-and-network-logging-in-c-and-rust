use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model {
        synced: false,
        led_on: false,
    }
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
            command = Cmd::SyncNetwork {
                led_on: model.led_on,
            };
            next.synced = true;
        }
        Msg::LedToggled { on } => {
            next.led_on = on;
            command = Cmd::SyncNetwork { led_on: on };
        }
    }

    UpdateResult { next, command }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn led_toggled_fetches_with_new_state() {
        let result = update(init(), Msg::LedToggled { on: true });
        assert!(result.next.led_on);
        assert_eq!(result.command, Cmd::SyncNetwork { led_on: true });
    }

    #[test]
    fn uart_done_uses_cached_led_state() {
        let after_led = update(init(), Msg::LedToggled { on: true });
        let result = update(after_led.next, Msg::UartTxDone);
        assert!(result.next.synced);
        assert_eq!(result.command, Cmd::SyncNetwork { led_on: true });
    }

    #[test]
    fn uart_done_defaults_led_off() {
        let result = update(init(), Msg::UartTxDone);
        assert_eq!(result.command, Cmd::SyncNetwork { led_on: false });
    }

    #[test]
    fn db_query_result_sends_uart() {
        let result = update(init(), Msg::DbQueryResultReady { success: true });
        assert_eq!(result.command, Cmd::SendUart);
    }
}
