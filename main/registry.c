#include "registry.h"

#include <string.h>

#define REGISTRY_MAX_MODULES 16

static const UseCaseModule *s_modules[REGISTRY_MAX_MODULES];
static size_t s_module_count;

void registry_add_module(const UseCaseModule *module)
{
    if (!module || s_module_count >= REGISTRY_MAX_MODULES) {
        return;
    }
    s_modules[s_module_count++] = module;
}

size_t registry_module_count(void)
{
    return s_module_count;
}

const UseCaseModule *const *registry_modules(void)
{
    return s_modules;
}

typedef struct {
    bool in_use;
    SystemEventId event;
    QueueHandle_t queue;
    size_t message_size;
} Subscription;

static Subscription s_subscriptions[EVENT_BUS_MAX_SUBSCRIBERS];
static size_t s_subscriber_count;

void event_bus_subscribe(SystemEventId event, QueueHandle_t queue, size_t message_size)
{
    if (!queue || message_size == 0 || message_size > EVENT_BUS_MAX_MSG_SIZE) {
        return;
    }
    if (event < 0 || event >= EVENT_ID_COUNT) {
        return;
    }
    if (s_subscriber_count >= EVENT_BUS_MAX_SUBSCRIBERS) {
        return;
    }
    s_subscriptions[s_subscriber_count].in_use = true;
    s_subscriptions[s_subscriber_count].event = event;
    s_subscriptions[s_subscriber_count].queue = queue;
    s_subscriptions[s_subscriber_count].message_size = message_size;
    s_subscriber_count++;
}

int event_bus_publish(SystemEventId event, const void *message, size_t message_size)
{
    if (!message || message_size == 0 || message_size > EVENT_BUS_MAX_MSG_SIZE) {
        return -1;
    }
    if (event < 0 || event >= EVENT_ID_COUNT) {
        return -1;
    }

    unsigned char block[EVENT_BUS_MAX_MSG_SIZE];
    int delivered = 0;

    for (size_t i = 0; i < s_subscriber_count; ++i) {
        Subscription *sub = &s_subscriptions[i];
        if (!sub->in_use || sub->event != event) {
            continue;
        }
        if (sub->message_size != message_size) {
            continue;
        }
        memcpy(block, message, message_size);
        if (xQueueSend(sub->queue, block, 0) == pdTRUE) {
            delivered++;
        }
    }

    return delivered;
}
