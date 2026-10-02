#include "domain.h"
#include "registry.h"

static Model local_model;
static QueueHandle_t local_queue;

static void local_process_queue_item(const void *item)
{
    const Msg *msg = (const Msg *)item;
    UpdateResult result = analog_sensor_update(local_model, *msg);
    local_model = result.next;
    execute_analog_sensor_hardware(&result.command);
}

static void local_subscriptions(QueueHandle_t my_queue)
{
    local_queue = my_queue;
    local_model = analog_sensor_init();
    event_bus_subscribe(EVENT_SENSOR_READING_READY, my_queue, sizeof(Msg));
}

static const UseCaseModule self_module = {
    .name = "read-analog-sensor",
    .queue_length = 8,
    .message_size = sizeof(Msg),
    .init_hardware = analog_sensor_init_use_case_hardware,
    .process_queue_item = local_process_queue_item,
    .poll_timer_tick = NULL,
    .wire_subscriptions = local_subscriptions,
};

USE_CASE_REGISTER(read_analog_sensor, self_module);
