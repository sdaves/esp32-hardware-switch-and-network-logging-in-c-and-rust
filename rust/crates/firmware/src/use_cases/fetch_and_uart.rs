use std::io::{Read, Write};
use std::net::TcpStream;

use esp_idf_svc::sys;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

use tea_core::use_cases::fetch_and_uart::{self, Cmd, Msg};
use tea_platform::{PlatformEvent, PlatformModule, SystemEventId};

// Mirrors main/use_cases/fetch-and-uart/commands.c: the NET button on GPIO4.
const BUTTON_GPIO: sys::gpio_num_t = 4;
const SSID: &str = "Espressif";
const URL_HOST: &str = "192.168.4.2:8000";
const URL_PATH: &str = "/";

pub struct FetchAndUart {
    model: fetch_and_uart::Model,
    last_level: i32,
    armed: bool,
    // Wi-Fi is initialised once and kept alive for the life of the module. The
    // C code guards this with `static bool s_wifi_ready`: re-running
    // `esp_netif_create_default_wifi_sta` (or dropping/re-taking the ESP-IDF
    // singletons) fails with ESP_ERR_INVALID_STATE and resets the chip.
    wifi: Option<BlockingWifi<EspWifi<'static>>>,
}

impl FetchAndUart {
    pub fn new() -> Self {
        Self {
            model: fetch_and_uart::init(),
            last_level: 1,
            armed: true,
            wifi: None,
        }
    }

    /// First call brings up Wi-Fi and holds the handle; later calls reuse it.
    fn wifi_once(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.wifi.is_some() {
            return Ok(());
        }

        // The modem peripheral must outlive the stored `EspWifi<'static>`.
        // Leak the owned `Peripherals` once, mirroring the C file-static
        // singletons (`esp_netif_create_default_wifi_sta` is a one-shot).
        let peripherals: &'static mut esp_idf_svc::hal::peripherals::Peripherals =
            Box::leak(Box::new(esp_idf_svc::hal::peripherals::Peripherals::take()?));
        let sys_loop = esp_idf_svc::eventloop::EspSystemEventLoop::take()?;
        let nvs = esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;

        let mut wifi = BlockingWifi::wrap(
            EspWifi::new(&mut peripherals.modem, sys_loop.clone(), Some(nvs))?,
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

        self.wifi = Some(wifi);
        Ok(())
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
            Cmd::SyncNetwork => {
                // One-shot Wi-Fi bring-up, then an HTTP GET on every press.
                if let Err(err) = self.wifi_once() {
                    log::error!("# WIFI SPIKE FAILED: {err:?}");
                } else if let Err(err) = http_get() {
                    log::error!("# HTTP GET FAILED: {err:?}");
                }
                println!("# NETWORK SYNCED");
                None
            }
        }
    }
}

impl PlatformModule for FetchAndUart {
    fn name(&self) -> &'static str {
        "fetch-and-uart"
    }

    fn subscriptions(&self) -> &'static [SystemEventId] {
        &[SystemEventId::DbQueryResult]
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
        let PlatformEvent::DbQueryResult(msg) = event else {
            return None;
        };
        let result = fetch_and_uart::update(self.model, *msg);
        self.model = result.next;
        self.execute(&result.command)
    }
}

// HTTP GET against the slirp gateway. Wi-Fi is already up (held by the module).
fn http_get() -> Result<(), Box<dyn std::error::Error>> {
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

    Ok(())
}
