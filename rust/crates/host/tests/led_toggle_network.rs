use std::cell::RefCell;
use std::rc::Rc;

use tea_core::use_cases::fetch_and_uart::{self, Cmd, Msg};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};
use tea_host::SimEngine;

struct Recorder {
    subscriptions: &'static [SystemEventId],
    seen: Rc<RefCell<Vec<PlatformEvent>>>,
}

impl PlatformModule for Recorder {
    fn name(&self) -> &'static str {
        "led-recorder"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        self.subscriptions
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        self.seen.borrow_mut().push(*event);
        None
    }
}

#[test]
fn led_toggled_event_reaches_subscriber() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut engine = SimEngine::new(vec![Box::new(Recorder {
        subscriptions: &[SystemEventId::LedToggled],
        seen: seen.clone(),
    })]);

    engine.publish(PlatformEvent::LedToggled { on: true });
    engine.tick();

    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(seen.borrow()[0], PlatformEvent::LedToggled { on: true });
}

#[test]
fn led_toggled_drives_fetch_with_new_state() {
    let result = fetch_and_uart::update(fetch_and_uart::init(), Msg::LedToggled { on: true });
    assert!(result.next.led_on);
    assert_eq!(result.command, Cmd::SyncNetwork { led_on: true });
}
