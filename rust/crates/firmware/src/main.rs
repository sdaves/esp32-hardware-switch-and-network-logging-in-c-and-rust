use core::convert::TryInto;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::sys;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};

// Matches main/use_cases/fetch-and-uart/commands.c: the emulator's slirp network
// is 192.168.4.0/24 and the container's dev-http-server answers on :8000.
const SSID: &str = "Espressif";
const URL_HOST: &str = "192.168.4.2:8000";
const URL_PATH: &str = "/";

// Phase 0 feasibility spikes (hello-world + Wi-Fi + HTTP). Phase 1 replaces this
// with the platform engine (registry + use-case plugins).
fn main() {
    // Required exactly once: binds the Rust std runtime to ESP-IDF.
    sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Platform Engine Initializing: Found 0 Autonomous Modules.");
    println!("# RUST HELLO (idf 4.4)");

    if let Err(err) = wifi_and_fetch() {
        log::error!("# WIFI SPIKE FAILED: {err:?}");
    }

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// One-shot Wi-Fi bring-up (the C fix: never re-run `EspWifi::new` /
/// `esp_netif_create_default_wifi_sta` on a later trigger) then an HTTP GET.
fn wifi_and_fetch() -> Result<(), Box<dyn std::error::Error>> {
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::None,
        password: "".try_into().unwrap(),
        channel: None,
    }))?;

    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    log::info!("# WIFI CONNECTED ({SSID})");

    let mut stream = TcpStream::connect(URL_HOST)?;
    write!(
        stream,
        "GET {URL_PATH} HTTP/1.1\r\nHost: {URL_HOST}\r\nConnection: close\r\n\r\n"
    )?;

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..n]);

    let mut lines = response.lines();
    if let Some(status_line) = lines.next() {
        let code = status_line.split_whitespace().nth(1).unwrap_or("?");
        log::info!("-> status {code}");
    }
    let body: String = lines.collect::<Vec<_>>().join("\n");
    log::info!("# HTTP BODY {body}");
    log::info!("# NETWORK SYNCED");

    Ok(())
}
