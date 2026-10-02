use std::time::Duration;

use esp_idf_svc::sys;

// Phase 0.2 hello-world spike. Phase 1 replaces this with the platform engine
// (registry + use-case plugins) that prints the real module count.
fn main() {
    // Required exactly once: installs the patches that bind the Rust std
    // runtime to ESP-IDF (allocator, threads, time, etc.).
    sys::link_patches();

    // Route the `log` crate through ESP-IDF's logging so it reaches UART.
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Platform Engine Initializing: Found 0 Autonomous Modules.");
    println!("# RUST HELLO (idf 4.4)");

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
