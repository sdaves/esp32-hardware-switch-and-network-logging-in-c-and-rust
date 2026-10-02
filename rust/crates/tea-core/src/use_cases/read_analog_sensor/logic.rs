use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model {
        last_millivolts: 0,
        in_error: false,
    }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;

    let command = match msg {
        Msg::AnalogSampleReady { millivolts } => {
            next.last_millivolts = millivolts;
            next.in_error = millivolts < 0;
            if next.in_error {
                Cmd::PostAnalogError { millivolts }
            } else {
                Cmd::PostAnalog { millivolts }
            }
        }
    };

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
