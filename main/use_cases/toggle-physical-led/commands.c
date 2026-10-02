#include "domain.h"

#include <stdio.h>

#ifdef ESP_PLATFORM
#include "driver/gpio.h"
#define TOGGLE_LED_GPIO    GPIO_NUM_2
#define TOGGLE_BUTTON_GPIO GPIO_NUM_0
#endif

void init_use_case_hardware(void)
{
#ifdef ESP_PLATFORM
    gpio_reset_pin(TOGGLE_LED_GPIO);
    gpio_set_direction(TOGGLE_LED_GPIO, GPIO_MODE_OUTPUT);

    gpio_config_t btn = {
        .pin_bit_mask = 1ULL << TOGGLE_BUTTON_GPIO,
        .mode         = GPIO_MODE_INPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type    = GPIO_INTR_DISABLE,
    };
    gpio_config(&btn);
#endif
}

bool toggle_button_pressed(void)
{
#ifdef ESP_PLATFORM
    return gpio_get_level(TOGGLE_BUTTON_GPIO) == 0;
#else
    return false;
#endif
}

void execute_toggle_led_hardware(const Cmd *command)
{
    if (!command || command->type != CMD_TOGGLE_LED) {
        return;
    }
#ifdef ESP_PLATFORM
    gpio_set_level(TOGGLE_LED_GPIO, command->level ? 1 : 0);
#endif
    printf("# LED %s (gpio %d)\n", command->level ? "ON" : "OFF", (int)command->pin);
}
