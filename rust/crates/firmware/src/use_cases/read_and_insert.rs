use tea_core::use_cases::read_and_insert::{self, Cmd};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

pub struct ReadAndInsert {
    model: read_and_insert::Model,
}

impl ReadAndInsert {
    pub fn new() -> Self {
        Self {
            model: read_and_insert::init(),
        }
    }
}

impl PlatformModule for ReadAndInsert {
    fn name(&self) -> &'static str {
        "read-and-insert"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::DbRowReady]
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        let PlatformEvent::DbRow(msg) = event else {
            return None;
        };
        let result = read_and_insert::update(self.model, *msg);
        self.model = result.next;
        execute(&result.command);
        None
    }
}

fn execute(command: &Cmd) {
    match command {
        Cmd::None => {}
        // Phase 2: INSERT the SQLite row.
        Cmd::InsertDbRow { .. } => {}
    }
}
