#include "domain.h"
#include "registry.h"

static Model local_model;
static QueueHandle_t local_queue;

static int s_stable_level = 1;
static int s_candidate_level = 1;
static int s_stable_ticks = 0;

static void local_poll_timer_tick(void)
{
    int level = fetch_and_uart_button_pressed() ? 0 : 1;
    if (level == s_candidate_level) {
        if (level != s_stable_level && ++s_stable_ticks >= 2) {
            s_stable_level = level;
            s_stable_ticks = 0;
            if (level == 0) {
                Msg query = { .type = MSG_DB_QUERY_RESULT_READY, .success = true };
                event_bus_publish(EVENT_DB_QUERY_RESULT, &query, sizeof(Msg));
            }
        }
    } else {
        s_candidate_level = level;
        s_stable_ticks = 0;
    }
}

static void local_process_queue_item(const void *item)
{
    const Msg *msg = (const Msg *)item;
    UpdateResult result = fetch_and_uart_update(local_model, *msg);
    local_model = result.next;
    execute_fetch_and_uart_hardware(&result.command);
    if (result.command.type == CMD_SEND_UART) {
        Msg done = { .type = MSG_UART_TX_DONE };
        event_bus_publish(EVENT_DB_QUERY_RESULT, &done, sizeof(Msg));
    }
}

static void local_subscriptions(QueueHandle_t my_queue)
{
    local_queue = my_queue;
    local_model = fetch_and_uart_init();
    event_bus_subscribe(EVENT_DB_QUERY_RESULT, my_queue, sizeof(Msg));
}

static const UseCaseModule self_module = {
    .name = "fetch-and-uart",
    .queue_length = 8,
    .message_size = sizeof(Msg),
    .init_hardware = fetch_and_uart_init_use_case_hardware,
    .process_queue_item = local_process_queue_item,
    .poll_timer_tick = local_poll_timer_tick,
    .wire_subscriptions = local_subscriptions,
};

USE_CASE_REGISTER(fetch_and_uart, self_module);
