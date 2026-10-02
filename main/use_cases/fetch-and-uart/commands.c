#include "domain.h"

#include <stdio.h>
#include <string.h>

#ifdef ESP_PLATFORM
#include "driver/gpio.h"

#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"

#include "esp_event.h"
#include "esp_http_client.h"
#include "esp_log.h"
#include "esp_netif.h"
#include "esp_wifi.h"
#include "nvs_flash.h"

#define FETCH_AND_UART_BUTTON_GPIO GPIO_NUM_4

/* The Velxio QEMU ESP32 machine broadcasts a hardcoded open AP, and only the
 * slirp network (192.168.4.0/24) is reachable from the guest. Hardcode both so
 * an uploaded prebuilt .bin associates without any in-editor SSID rewrite. */
#define FETCH_AND_UART_WIFI_SSID "Espressif"
#define FETCH_AND_UART_WIFI_PASS ""
#define FETCH_AND_UART_URL       "http://192.168.4.2:8000/"

#define WIFI_CONNECTED_BIT BIT0
#define WIFI_FAIL_BIT      BIT1

static EventGroupHandle_t s_wifi_event_group;
#endif

void fetch_and_uart_init_use_case_hardware(void)
{
#ifdef ESP_PLATFORM
    gpio_config_t btn = {
        .pin_bit_mask = 1ULL << FETCH_AND_UART_BUTTON_GPIO,
        .mode         = GPIO_MODE_INPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type    = GPIO_INTR_DISABLE,
    };
    gpio_config(&btn);
#endif
}

#ifdef ESP_PLATFORM
static void wifi_event_handler(void *arg, esp_event_base_t base,
                               int32_t id, void *data)
{
    (void)arg;
    if (base == WIFI_EVENT && id == WIFI_EVENT_STA_START) {
        esp_wifi_connect();
    } else if (base == WIFI_EVENT && id == WIFI_EVENT_STA_DISCONNECTED) {
        esp_wifi_connect();
    } else if (base == IP_EVENT && id == IP_EVENT_STA_GOT_IP) {
        xEventGroupSetBits(s_wifi_event_group, WIFI_CONNECTED_BIT);
    }
}

/* Bring up the Wi-Fi stack exactly once. Re-running esp_netif_init /
 * esp_netif_create_default_wifi_sta on a later press returns NULL for the
 * already-created netif and asserts inside IDF, resetting the chip. */
static bool wifi_init_once(void)
{
    static bool s_wifi_ready = false;
    if (s_wifi_ready) {
        return true;
    }

    s_wifi_event_group = xEventGroupCreate();

    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        nvs_flash_erase();
        err = nvs_flash_init();
    }

    esp_netif_init();
    esp_event_loop_create_default();
    esp_netif_create_default_wifi_sta();

    wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
    esp_wifi_init(&cfg);

    esp_event_handler_instance_t any_id;
    esp_event_handler_instance_t got_ip;
    esp_event_handler_instance_register(WIFI_EVENT, ESP_EVENT_ANY_ID,
                                        &wifi_event_handler, NULL, &any_id);
    esp_event_handler_instance_register(IP_EVENT, IP_EVENT_STA_GOT_IP,
                                        &wifi_event_handler, NULL, &got_ip);

    wifi_config_t wifi_config = { 0 };
    strncpy((char *)wifi_config.sta.ssid, FETCH_AND_UART_WIFI_SSID,
            sizeof(wifi_config.sta.ssid) - 1);
    strncpy((char *)wifi_config.sta.password, FETCH_AND_UART_WIFI_PASS,
            sizeof(wifi_config.sta.password) - 1);
    wifi_config.sta.threshold.authmode = WIFI_AUTH_OPEN;

    esp_wifi_set_mode(WIFI_MODE_STA);
    esp_wifi_set_config(WIFI_IF_STA, &wifi_config);
    esp_wifi_start();

    s_wifi_ready = true;
    return true;
}

static bool wifi_wait_connected(void)
{
    EventBits_t bits = xEventGroupWaitBits(s_wifi_event_group,
                                           WIFI_CONNECTED_BIT | WIFI_FAIL_BIT,
                                           pdFALSE, pdFALSE,
                                           pdMS_TO_TICKS(15000));
    return (bits & WIFI_CONNECTED_BIT) != 0;
}

static void http_get_sample(void)
{
    esp_http_client_config_t config = {
        .url = FETCH_AND_UART_URL,
        .timeout_ms = 5000,
    };
    esp_http_client_handle_t client = esp_http_client_init(&config);
    if (!client) {
        printf("# HTTP INIT FAILED\n");
        return;
    }

    esp_err_t err = esp_http_client_open(client, 0);
    if (err == ESP_OK) {
        esp_http_client_fetch_headers(client);
        int status = esp_http_client_get_status_code(client);
        int total = esp_http_client_get_content_length(client);
        char buf[64];
        int read = esp_http_client_read(client, buf, sizeof(buf) - 1);
        if (read < 0) {
            read = 0;
        }
        buf[read] = '\0';
        printf("# HTTP GET %s -> status %d, len %d\n",
               FETCH_AND_UART_URL, status, total);
        if (read > 0) {
            printf("# HTTP BODY[%d]: %s\n", read, buf);
        }
    } else {
        printf("# HTTP GET FAILED (%d)\n", (int)err);
    }
    esp_http_client_close(client);
    esp_http_client_cleanup(client);
}
#endif

bool fetch_and_uart_button_pressed(void)
{
#ifdef ESP_PLATFORM
    return gpio_get_level(FETCH_AND_UART_BUTTON_GPIO) == 0;
#else
    return false;
#endif
}

void execute_fetch_and_uart_hardware(const Cmd *command)
{
    if (!command) {
        return;
    }

    switch (command->type) {
    case CMD_NONE:
        break;
    case CMD_SEND_UART:
        /* The console printf is a real UART0 transmit and is the observable
         * effect on the emulator bridge; a driver-based write goes here on
         * hardware. */
        printf("# UART SENT\n");
        break;
    case CMD_SYNC_NETWORK:
#ifdef ESP_PLATFORM
        if (wifi_init_once() && wifi_wait_connected()) {
            printf("# WIFI CONNECTED (%s)\n", FETCH_AND_UART_WIFI_SSID);
            http_get_sample();
        } else {
            printf("# WIFI CONNECT FAILED (%s)\n", FETCH_AND_UART_WIFI_SSID);
        }
#endif
        printf("# NETWORK SYNCED\n");
        break;
    }
}
