#include <assert.h>
#include <stdio.h>

#include "domain.h"

int main(void)
{
    Model initial = fetch_and_uart_init();
    assert(initial.synced == false);

    UpdateResult failed = fetch_and_uart_update(
        initial, (Msg){ .type = MSG_DB_QUERY_RESULT_READY, .success = false });
    assert(failed.command.type == CMD_NONE);
    assert(failed.next.synced == false);

    UpdateResult send = fetch_and_uart_update(
        initial, (Msg){ .type = MSG_DB_QUERY_RESULT_READY, .success = true });
    assert(send.command.type == CMD_SEND_UART);
    assert(send.next.synced == false);

    UpdateResult synced = fetch_and_uart_update(send.next, (Msg){ .type = MSG_UART_TX_DONE });
    assert(synced.command.type == CMD_SYNC_NETWORK);
    assert(synced.next.synced == true);

    printf("test_fetch_and_uart: PASS\n");
    return 0;
}
