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
