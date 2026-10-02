#include <assert.h>
#include <stdio.h>

#include "registry.h"

typedef struct {
    int value;
} SmallMsg;

typedef struct {
    long long value;
} LargeMsg;

int main(void)
{
    QueueHandle_t box_a = mock_bus_create_mailbox(sizeof(SmallMsg));
    QueueHandle_t box_b = mock_bus_create_mailbox(sizeof(SmallMsg));
    assert(box_a != NULL && box_b != NULL);

    event_bus_subscribe(EVENT_HARDWARE_ALERT_TRIGGERED, box_a, sizeof(SmallMsg));
    event_bus_subscribe(EVENT_HARDWARE_ALERT_TRIGGERED, box_b, sizeof(SmallMsg));

    SmallMsg outgoing = { .value = 7 };
    int delivered = event_bus_publish(EVENT_HARDWARE_ALERT_TRIGGERED, &outgoing,
                                      sizeof(SmallMsg));
    assert(delivered == 2);
    assert(mock_bus_has_message(box_a));
    assert(mock_bus_has_message(box_b));

    const SmallMsg *received = (const SmallMsg *)mock_bus_read(box_a);
    assert(received != NULL);
    assert(received->value == 7);
    assert(!mock_bus_has_message(box_a));

    QueueHandle_t box_c = mock_bus_create_mailbox(sizeof(LargeMsg));
    assert(box_c != NULL);
    event_bus_subscribe(EVENT_SENSOR_READING_READY, box_c, sizeof(LargeMsg));
    event_bus_publish(EVENT_SENSOR_READING_READY, &outgoing, sizeof(SmallMsg));
    assert(!mock_bus_has_message(box_c));

    assert(event_bus_publish(EVENT_HARDWARE_ALERT_TRIGGERED, NULL, sizeof(SmallMsg)) == -1);

    printf("test_event_bus: PASS\n");
    return 0;
}
