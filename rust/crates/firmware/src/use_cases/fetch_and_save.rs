use tea_core::use_cases::fetch_and_save::{self, Cmd};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

pub struct FetchAndSave {
    model: fetch_and_save::Model,
}

impl FetchAndSave {
    pub fn new() -> Self {
        Self {
            model: fetch_and_save::init(),
        }
    }
}

impl PlatformModule for FetchAndSave {
    fn name(&self) -> &'static str {
        "fetch-and-save"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::HttpResponseReceived]
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        let PlatformEvent::HttpResponse(msg) = event else {
            return None;
        };
        let result = fetch_and_save::update(self.model, *msg);
        self.model = result.next;
        execute(&result.command);
        None
    }
}

fn execute(command: &Cmd) {
    match command {
        Cmd::None => {}
        // Phase 2: write the ISO8601 file.
        Cmd::SaveIso8601File => {}
    }
}
