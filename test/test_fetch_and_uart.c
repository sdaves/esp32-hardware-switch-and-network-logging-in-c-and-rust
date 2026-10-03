#include <assert.h>
#include <stdio.h>

#include "domain.h"

int main(void)
{
    Model initial = fetch_and_uart_init();
    assert(initial.synced == false);
    assert(initial.led_on == false);

    UpdateResult failed = fetch_and_uart_update(
        initial, (Msg){ .type = MSG_DB_QUERY_RESULT_READY, .success = false });
    assert(failed.command.type == CMD_NONE);
    assert(failed.next.synced == false);

    UpdateResult send = fetch_and_uart_update(
        initial, (Msg){ .type = MSG_DB_QUERY_RESULT_READY, .success = true });
    assert(send.command.type == CMD_SEND_UART);
    assert(send.next.synced == false);

    UpdateResult synced = fetch_and_uart_update(
        send.next, (Msg){ .type = MSG_UART_TX_DONE, .success = false, .led_on = false });
    assert(synced.command.type == CMD_SYNC_NETWORK);
    assert(synced.command.led_on == false);
    assert(synced.next.synced == true);

    UpdateResult led_on = fetch_and_uart_update(
        initial, (Msg){ .type = MSG_LED_TOGGLED, .success = false, .led_on = true });
    assert(led_on.command.type == CMD_SYNC_NETWORK);
    assert(led_on.command.led_on == true);
    assert(led_on.next.led_on == true);

    UpdateResult led_off = fetch_and_uart_update(
        led_on.next, (Msg){ .type = MSG_LED_TOGGLED, .success = false, .led_on = false });
    assert(led_off.command.type == CMD_SYNC_NETWORK);
    assert(led_off.command.led_on == false);
    assert(led_off.next.led_on == false);

    UpdateResult net_after_on = fetch_and_uart_update(
        led_on.next, (Msg){ .type = MSG_UART_TX_DONE, .success = false, .led_on = false });
    assert(net_after_on.command.type == CMD_SYNC_NETWORK);
    assert(net_after_on.command.led_on == true);

    printf("test_fetch_and_uart: PASS\n");
    return 0;
}
