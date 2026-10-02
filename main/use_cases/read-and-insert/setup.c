#include "domain.h"
#include "registry.h"

static Model local_model;
static QueueHandle_t local_queue;

static void local_process_queue_item(const void *item)
{
    const Msg *msg = (const Msg *)item;
    UpdateResult result = read_and_insert_update(local_model, *msg);
    local_model = result.next;
    execute_read_and_insert_hardware(&result.command);
}

static void local_subscriptions(QueueHandle_t my_queue)
{
    local_queue = my_queue;
    local_model = read_and_insert_init();
    event_bus_subscribe(EVENT_DB_ROW_READY, my_queue, sizeof(Msg));
}

static const UseCaseModule self_module = {
    .name = "read-and-insert",
    .queue_length = 8,
    .message_size = sizeof(Msg),
    .init_hardware = read_and_insert_init_use_case_hardware,
    .process_queue_item = local_process_queue_item,
    .poll_timer_tick = NULL,
    .wire_subscriptions = local_subscriptions,
};

USE_CASE_REGISTER(read_and_insert, self_module);
