#include "domain.h"
#include "registry.h"

static Model local_model;
static QueueHandle_t local_queue;

/* Edge detection with release re-arm: fire once on the high->low transition
 * (button pressed, active-low) and latch until the button returns high. This
 * catches even a single-tick tap, where the previous 2-consecutive-tick
 * debounce missed presses shorter than ~200 ms. */
static int s_last_level = 1;
static bool s_armed = true;

static void local_poll_timer_tick(void)
{
    int level = toggle_button_pressed() ? 0 : 1;
    if (level == 0 && s_last_level == 1 && s_armed) {
        s_armed = false;
        Msg press = { .type = MSG_BUTTON_DOWN };
        event_bus_publish(EVENT_HARDWARE_ALERT_TRIGGERED, &press, sizeof(Msg));
    } else if (level == 1) {
        s_armed = true;
    }
    s_last_level = level;
}

static void local_process_queue_item(const void *item)
{
    const Msg *msg = (const Msg *)item;
    UpdateResult result = toggle_led_update(local_model, *msg);
    local_model = result.next;
    execute_toggle_led_hardware(&result.command);
}

static void local_subscriptions(QueueHandle_t my_queue)
{
    local_queue = my_queue;
    local_model = toggle_led_init();
    event_bus_subscribe(EVENT_HARDWARE_ALERT_TRIGGERED, my_queue, sizeof(Msg));
}

static const UseCaseModule self_module = {
    .name = "toggle-physical-led",
    .queue_length = 8,
    .message_size = sizeof(Msg),
    .init_hardware = init_use_case_hardware,
    .process_queue_item = local_process_queue_item,
    .poll_timer_tick = local_poll_timer_tick,
    .wire_subscriptions = local_subscriptions,
};

USE_CASE_REGISTER(toggle_physical_led, self_module);
