#include "domain.h"

Model toggle_led_init(void)
{
    return (Model){ .led_on = false };
}

UpdateResult toggle_led_update(Model model, Msg msg)
{
    UpdateResult result = {
        .next = model,
        .command = { .type = CMD_NONE, .pin = 0, .level = 0 },
    };

    switch (msg.type) {
    case MSG_BUTTON_DOWN:
        result.next.led_on = !model.led_on;
        result.command.type = CMD_TOGGLE_LED;
        result.command.pin = 2;
        result.command.level = result.next.led_on ? 1 : 0;
        break;
    }

    return result;
}
