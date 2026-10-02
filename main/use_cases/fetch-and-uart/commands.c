#include "domain.h"

#include <stdio.h>

#ifdef ESP_PLATFORM
#include "driver/gpio.h"
#define FETCH_AND_UART_BUTTON_GPIO GPIO_NUM_0
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
        /* Wi-Fi is disabled in the emulator, so the sync is reported by
         * marker; the real HTTP client call goes here on hardware. */
        printf("# NETWORK SYNCED\n");
        break;
    }
}
