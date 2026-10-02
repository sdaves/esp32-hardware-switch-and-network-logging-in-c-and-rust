#include "domain.h"
#include "registry.h"

static Model local_model;
static QueueHandle_t local_queue;

static void local_process_queue_item(const void *item)
{
    const Msg *msg = (const Msg *)item;
    UpdateResult result = fetch_and_save_update(local_model, *msg);
    local_model = result.next;
    execute_fetch_and_save_hardware(&result.command);
}

static void local_subscriptions(QueueHandle_t my_queue)
{
    local_queue = my_queue;
    local_model = fetch_and_save_init();
    event_bus_subscribe(EVENT_HTTP_RESPONSE_RECEIVED, my_queue, sizeof(Msg));
}

static const UseCaseModule self_module = {
    .name = "fetch-and-save",
    .queue_length = 8,
    .message_size = sizeof(Msg),
    .init_hardware = fetch_and_save_init_use_case_hardware,
    .process_queue_item = local_process_queue_item,
    .poll_timer_tick = NULL,
    .wire_subscriptions = local_subscriptions,
};

USE_CASE_REGISTER(fetch_and_save, self_module);
