#include <stdbool.h>
#include <stdio.h>

#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "esp_timer.h"

#include "registry.h"

#define PLATFORM_POLL_PERIOD_MS 100

static const UseCaseModule *s_modules[USE_CASE_MAX_MODULES];
static QueueHandle_t s_module_queues[USE_CASE_MAX_MODULES];
static size_t s_module_count;

static void platform_timer_tick(void *arg)
{
    (void)arg;
    for (size_t i = 0; i < s_module_count; ++i) {
        if (s_modules[i] && s_modules[i]->poll_timer_tick) {
            s_modules[i]->poll_timer_tick();
        }
    }
}

void app_main(void)
{
    const UseCaseModule *const *registered = registry_modules();
    s_module_count = registry_module_count();
    if (s_module_count > USE_CASE_MAX_MODULES) {
        s_module_count = USE_CASE_MAX_MODULES;
    }

    printf("Platform Engine Initializing: Found %u Autonomous Modules.\n",
           (unsigned)s_module_count);

    for (size_t i = 0; i < s_module_count; ++i) {
        const UseCaseModule *module = registered[i];
        s_modules[i] = module;
        if (!module || module->message_size == 0 ||
            module->message_size > EVENT_BUS_MAX_MSG_SIZE) {
            printf("Platform Engine: module %u has an invalid message size, skipped.\n",
                   (unsigned)i);
            continue;
        }
        uint32_t depth = module->queue_length ? module->queue_length : 8;
        s_module_queues[i] = xQueueCreate(depth, module->message_size);
    }

    for (size_t i = 0; i < s_module_count; ++i) {
        if (s_modules[i] && s_modules[i]->init_hardware) {
            s_modules[i]->init_hardware();
        }
    }

    for (size_t i = 0; i < s_module_count; ++i) {
        if (s_modules[i] && s_modules[i]->wire_subscriptions && s_module_queues[i]) {
            s_modules[i]->wire_subscriptions(s_module_queues[i]);
        }
    }

    const esp_timer_create_args_t timer_args = {
        .callback = platform_timer_tick,
        .name = "platform_tick",
    };
    esp_timer_handle_t poll_timer = NULL;
    if (esp_timer_create(&timer_args, &poll_timer) == ESP_OK) {
        esp_timer_start_periodic(poll_timer, PLATFORM_POLL_PERIOD_MS * 1000);
    }

    unsigned char item[EVENT_BUS_MAX_MSG_SIZE];
    while (true) {
        bool idle = true;
        for (size_t i = 0; i < s_module_count; ++i) {
            if (!s_modules[i] || !s_module_queues[i]) {
                continue;
            }
            while (xQueueReceive(s_module_queues[i], item, 0) == pdTRUE) {
                idle = false;
                if (s_modules[i]->process_queue_item) {
                    s_modules[i]->process_queue_item(item);
                }
            }
        }
        if (idle) {
            vTaskDelay(pdMS_TO_TICKS(10));
        }
    }
}
