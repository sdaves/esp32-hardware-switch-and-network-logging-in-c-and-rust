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
