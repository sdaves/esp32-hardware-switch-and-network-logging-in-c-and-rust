//! Host-side engine used by the sim binary and the integration tests. Mirrors
//! the firmware poll/drain loop without any hardware.

use std::collections::VecDeque;

use tea_platform::{dispatch, PlatformEvent, PlatformModule};

pub struct SimEngine {
    modules: Vec<Box<dyn PlatformModule>>,
    queue: VecDeque<PlatformEvent>,
}

impl SimEngine {
    pub fn new(modules: Vec<Box<dyn PlatformModule>>) -> Self {
        Self {
            modules,
            queue: VecDeque::new(),
        }
    }

    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    pub fn publish(&mut self, event: PlatformEvent) {
        self.queue.push_back(event);
    }

    /// One 100 ms platform tick: poll every module, then drain the bus.
    pub fn tick(&mut self) {
        for module in self.modules.iter_mut() {
            if let Some(event) = module.poll_timer_tick() {
                self.queue.push_back(event);
            }
        }
        self.drain();
    }

    fn drain(&mut self) {
        while let Some(event) = self.queue.pop_front() {
            for next in dispatch(&mut self.modules, &event) {
                self.queue.push_back(next);
            }
        }
    }
}
