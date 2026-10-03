use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use esp_idf_svc::sys;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

use tea_core::use_cases::fetch_and_uart::{self, Cmd, Msg};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

// Mirrors main/use_cases/fetch-and-uart/commands.c: the NET button on GPIO4.
const BUTTON_GPIO: sys::gpio_num_t = 4;
const SSID: &str = "Espressif";
const URL_HOST: &str = "192.168.4.2:8000";

// Bring-up retries (the emulated radio can be slow to associate).
const WIFI_MAX_ATTEMPTS: u32 = 3;
const WIFI_RETRY_DELAY: Duration = Duration::from_millis(500);

pub struct FetchAndUart {
    model: fetch_and_uart::Model,
    last_level: i32,
    armed: bool,
    // Wi-Fi is initialised once and kept alive for the life of the module. The
    // C code guards this with `static bool s_wifi_ready`: re-running
    // `esp_netif_create_default_wifi_sta` (or dropping/re-taking the ESP-IDF
    // singletons) fails with ESP_ERR_INVALID_STATE and resets the chip.
    wifi: Option<BlockingWifi<EspWifi<'static>>>,
    wifi_ready: bool,
}

impl FetchAndUart {
    pub fn new() -> Self {
        Self {
            model: fetch_and_uart::init(),
            last_level: 1,
            armed: true,
            wifi: None,
            wifi_ready: false,
        }
    }

    /// Bring up Wi-Fi once, retrying the association a few times on failure.
    /// Later calls are a no-op; the handle lives for the life of the module.
    fn wifi_with_retry(&mut self) {
        if self.wifi_ready {
            return;
        }

        // The modem peripheral must outlive the stored `EspWifi<'static>`.
        // Leak the owned `Peripherals` once, mirroring the C file-static
        // singletons (`esp_netif_create_default_wifi_sta` is a one-shot).
        let peripherals: &'static mut esp_idf_svc::hal::peripherals::Peripherals =
            match esp_idf_svc::hal::peripherals::Peripherals::take() {
                Ok(p) => Box::leak(Box::new(p)),
                Err(err) => {
                    log::error!("# WIFI SPIKE FAILED: {err:?}");
                    return;
                }
            };

        let sys_loop = match esp_idf_svc::eventloop::EspSystemEventLoop::take() {
            Ok(s) => s,
            Err(err) => {
                log::error!("# WIFI SPIKE FAILED: {err:?}");
                return;
            }
        };
        let nvs = match esp_idf_svc::nvs::EspDefaultNvsPartition::take() {
            Ok(n) => n,
            Err(err) => {
                log::error!("# WIFI SPIKE FAILED: {err:?}");
                return;
            }
        };

        // Build the Wi-Fi driver once; retry only the association.
        let esp_wifi = match EspWifi::new(&mut peripherals.modem, sys_loop.clone(), Some(nvs)) {
            Ok(w) => w,
            Err(err) => {
                log::error!("# WIFI SPIKE FAILED: {err:?}");
                return;
            }
        };
        let mut wifi = match BlockingWifi::wrap(esp_wifi, sys_loop) {
            Ok(w) => w,
            Err(err) => {
                log::error!("# WIFI SPIKE FAILED: {err:?}");
                return;
            }
        };

        let config = Configuration::Client(ClientConfiguration {
            ssid: SSID.try_into().unwrap(),
            bssid: None,
            auth_method: AuthMethod::None,
            password: "".try_into().unwrap(),
            channel: None,
        });

        for attempt in 1..=WIFI_MAX_ATTEMPTS {
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                wifi.set_configuration(&config)?;
                wifi.start()?;
                wifi.connect()?;
                wifi.wait_netif_up()?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    log::info!("# WIFI CONNECTED ({SSID}, attempt {attempt})");
                    self.wifi = Some(wifi);
                    self.wifi_ready = true;
                    return;
                }
                Err(err) => {
                    log::error!("# WIFI ATTEMPT {attempt}/{WIFI_MAX_ATTEMPTS} FAILED: {err:?}");
                    let _ = wifi.stop();
                    std::thread::sleep(WIFI_RETRY_DELAY);
                }
            }
        }
        log::error!("# WIFI SPIKE FAILED after {WIFI_MAX_ATTEMPTS} attempts");
    }

    fn button_pressed(&self) -> bool {
        unsafe { sys::gpio_get_level(BUTTON_GPIO) == 0 }
    }

    fn execute(&mut self, command: &Cmd) -> Option<PlatformEvent> {
        match command {
            Cmd::None => None,
            Cmd::SendUart => {
                // The console write is the observable UART0 transmit; this
                // mirrors the C `printf("# UART SENT\n")`. The C code then
                // publishes MSG_UART_TX_DONE onto the same event.
                println!("# UART SENT");
                Some(PlatformEvent::DbQueryResult(Msg::UartTxDone))
            }
            Cmd::SyncNetwork { led_on } => {
                self.fetch(*led_on);
                None
            }
        }
    }

    /// Fetch `/?led=on|off`; Wi-Fi is brought up on the first request (retried).
    fn fetch(&mut self, led_on: bool) {
        self.wifi_with_retry();
        if !self.wifi_ready {
            log::error!("# WIFI SPIKE FAILED: skipping request");
            return;
        }
        let path = if led_on { "/?led=on" } else { "/?led=off" };
        match http_get(path) {
            Ok(()) => {}
            Err(err) => log::error!("# HTTP GET FAILED: {err:?}"),
        }
    }
}

impl PlatformModule for FetchAndUart {
    fn name(&self) -> &'static str {
        "fetch-and-uart"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::DbQueryResult, SystemEventId::LedToggled]
    }

    fn init_hardware(&mut self) {
        unsafe {
            let mut cfg: sys::gpio_config_t = core::mem::zeroed();
            cfg.pin_bit_mask = 1u64 << (BUTTON_GPIO as u32);
            cfg.mode = sys::gpio_mode_t_GPIO_MODE_INPUT;
            cfg.pull_up_en = sys::gpio_pullup_t_GPIO_PULLUP_ENABLE;
            cfg.pull_down_en = sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE;
            cfg.intr_type = sys::gpio_int_type_t_GPIO_INTR_DISABLE;
            sys::gpio_config(&cfg);
        }
        // Wi-Fi comes up at boot so triggered fetches are immediate; no
        // request is sent until an event. (Starting it in `process` would
        // stall the poll loop and drop button edges during association.)
        self.wifi_with_retry();
    }

    fn poll_timer_tick(&mut self) -> Option<PlatformEvent> {
        // Edge detection with release re-arm, copied from the C setup.c: fire
        // once on high->low and latch until the button returns high.
        let level = if self.button_pressed() { 0 } else { 1 };
        let mut event = None;
        if level == 0 && self.last_level == 1 && self.armed {
            self.armed = false;
            event = Some(PlatformEvent::DbQueryResult(Msg::DbQueryResultReady {
                success: true,
            }));
        } else if level == 1 {
            self.armed = true;
        }
        self.last_level = level;
        event
    }

    fn process(&mut self, event: &PlatformEvent) -> Option<PlatformEvent> {
        match event {
            PlatformEvent::DbQueryResult(msg) => {
                let result = fetch_and_uart::update(self.model, *msg);
                self.model = result.next;
                self.execute(&result.command)
            }
            PlatformEvent::LedToggled { on } => {
                let result = fetch_and_uart::update(self.model, Msg::LedToggled { on: *on });
                self.model = result.next;
                self.execute(&result.command)
            }
            // Other events are not subscribed here; ignore defensively.
            PlatformEvent::HardwareAlert(_)
            | PlatformEvent::SensorReading(_)
            | PlatformEvent::HttpResponse(_)
            | PlatformEvent::DbRow(_) => None,
        }
    }
}

/// HTTP GET of `path` against the slirp gateway. Wi-Fi is already up.
fn http_get(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(URL_HOST)?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {URL_HOST}\r\nConnection: close\r\n\r\n"
    )?;

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..n]);
    let status = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("?");

    log::info!("# HTTP GET {path}");
    log::info!("-> status {status}");

    Ok(())
}
