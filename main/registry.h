#ifndef REGISTRY_H
#define REGISTRY_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef PLATFORM_HOST_TEST
typedef void *QueueHandle_t;
#else
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#endif

#define EVENT_BUS_MAX_SUBSCRIBERS 16
#define EVENT_BUS_MAX_MSG_SIZE 256
#define USE_CASE_MAX_MODULES 16

typedef enum {
    EVENT_HARDWARE_ALERT_TRIGGERED = 0,
    EVENT_SENSOR_READING_READY,
    EVENT_HTTP_RESPONSE_RECEIVED,
    EVENT_DB_ROW_READY,
    EVENT_DB_QUERY_RESULT,
    EVENT_ID_COUNT,
} SystemEventId;

typedef void (*UseCaseInitHardwareFn)(void);
typedef void (*UseCaseProcessQueueItemFn)(const void *item);
typedef void (*UseCasePollTimerTickFn)(void);
typedef void (*UseCaseWireSubscriptionsFn)(QueueHandle_t queue);

typedef struct UseCaseModule {
    const char *name;
    uint32_t queue_length;
    size_t message_size;
    UseCaseInitHardwareFn init_hardware;
    UseCaseProcessQueueItemFn process_queue_item;
    UseCasePollTimerTickFn poll_timer_tick;
    UseCaseWireSubscriptionsFn wire_subscriptions;
} UseCaseModule;

void event_bus_subscribe(SystemEventId event, QueueHandle_t queue, size_t message_size);
int event_bus_publish(SystemEventId event, const void *message, size_t message_size);

void registry_add_module(const UseCaseModule *module);
size_t registry_module_count(void);
const UseCaseModule *const *registry_modules(void);

#ifdef PLATFORM_HOST_TEST
QueueHandle_t mock_bus_create_mailbox(size_t message_size);
bool mock_bus_has_message(QueueHandle_t queue);
const void *mock_bus_read(QueueHandle_t queue);
#define USE_CASE_REGISTER(module)
#else
/* Autonomous plugin registration.
 *
 * Each plugin's setup.c declares its module and calls USE_CASE_REGISTER,
 * which emits a constructor that appends the module to the platform
 * registry at boot. No central list and no ordering, so adding or removing
 * a feature needs no edit to main.c.
 *
 * The constructor is given external linkage and is force-referenced by the
 * component's link options (-u), because IDF builds with --gc-sections and
 * would otherwise discard both the .ctors slot and the constructor body. */
#define USE_CASE_REGISTER(tag, module) \
    __attribute__((constructor)) void _uc_register_##tag(void) \
    { \
        registry_add_module(&(module)); \
    }
#endif

#endif
