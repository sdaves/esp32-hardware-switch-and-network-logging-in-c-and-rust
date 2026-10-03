#include "domain.h"

Model fetch_and_uart_init(void)
{
    return (Model){ .synced = false, .led_on = false };
}

UpdateResult fetch_and_uart_update(Model model, Msg msg)
{
    UpdateResult result = {
        .next = model,
        .command = { .type = CMD_NONE, .led_on = model.led_on },
    };

    switch (msg.type) {
    case MSG_DB_QUERY_RESULT_READY:
        if (msg.success) {
            result.command.type = CMD_SEND_UART;
        }
        break;
    case MSG_UART_TX_DONE:
        result.command.type = CMD_SYNC_NETWORK;
        result.command.led_on = model.led_on;
        result.next.synced = true;
        break;
    case MSG_LED_TOGGLED:
        result.next.led_on = msg.led_on;
        result.command.type = CMD_SYNC_NETWORK;
        result.command.led_on = msg.led_on;
        break;
    }

    return result;
}
