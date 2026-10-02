#include "registry.h"

#include <string.h>

#define MOCK_MAX_BOXES 8

typedef struct {
    bool created;
    size_t message_size;
    bool has_message;
    unsigned char data[EVENT_BUS_MAX_MSG_SIZE];
} MockMailbox;

typedef struct {
    bool in_use;
    SystemEventId event;
    QueueHandle_t queue;
    size_t message_size;
} MockSubscription;

static MockMailbox s_mailboxes[MOCK_MAX_BOXES];
static MockSubscription s_subscriptions[MOCK_MAX_BOXES];
static size_t s_subscription_count;

QueueHandle_t mock_bus_create_mailbox(size_t message_size)
{
    if (message_size == 0 || message_size > EVENT_BUS_MAX_MSG_SIZE) {
        return NULL;
    }
    for (size_t i = 0; i < MOCK_MAX_BOXES; ++i) {
        if (!s_mailboxes[i].created) {
            s_mailboxes[i].created = true;
            s_mailboxes[i].message_size = message_size;
            s_mailboxes[i].has_message = false;
            return (QueueHandle_t)&s_mailboxes[i];
        }
    }
    return NULL;
}

void event_bus_subscribe(SystemEventId event, QueueHandle_t queue, size_t message_size)
{
    if (!queue || s_subscription_count >= MOCK_MAX_BOXES) {
        return;
    }
    s_subscriptions[s_subscription_count].in_use = true;
    s_subscriptions[s_subscription_count].event = event;
    s_subscriptions[s_subscription_count].queue = queue;
    s_subscriptions[s_subscription_count].message_size = message_size;
    s_subscription_count++;
}

int event_bus_publish(SystemEventId event, const void *message, size_t message_size)
{
    if (!message || message_size == 0) {
        return -1;
    }

    int delivered = 0;
    for (size_t i = 0; i < s_subscription_count; ++i) {
        MockSubscription *sub = &s_subscriptions[i];
        if (!sub->in_use || sub->event != event) {
            continue;
        }
        if (sub->message_size != message_size) {
            continue;
        }
        MockMailbox *box = (MockMailbox *)sub->queue;
        memcpy(box->data, message, message_size);
        box->has_message = true;
        delivered++;
    }
    return delivered;
}

bool mock_bus_has_message(QueueHandle_t queue)
{
    MockMailbox *box = (MockMailbox *)queue;
    return box != NULL && box->has_message;
}

const void *mock_bus_read(QueueHandle_t queue)
{
    MockMailbox *box = (MockMailbox *)queue;
    if (!box || !box->has_message) {
        return NULL;
    }
    box->has_message = false;
    return box->data;
}
