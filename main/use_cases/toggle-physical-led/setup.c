#include "domain.h"
#include "registry.h"

static Model local_model;
static QueueHandle_t local_queue;

static int s_stable_level = 1;
static int s_candidate_level = 1;
static int s_stable_ticks = 0;

static void local_poll_timer_tick(void)
{
    int level = toggle_button_pressed() ? 0 : 1;
    if (level == s_candidate_level) {
        if (level != s_stable_level && ++s_stable_ticks >= 2) {
            s_stable_level = level;
            s_stable_ticks = 0;
            if (level == 0) {
                Msg press = { .type = MSG_BUTTON_DOWN };
                event_bus_publish(EVENT_HARDWARE_ALERT_TRIGGERED, &press, sizeof(Msg));
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
