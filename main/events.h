#ifndef EVENTS_H
#define EVENTS_H

#include <stdbool.h>
#include <stdint.h>

/* Cross-feature event payloads. Neutral by design: no feature's Msg/Cmd
 * types appear here, so producer and consumer each include this header and
 * never reference each other. Event ids live in registry.h (SystemEventId);
 * this header carries only what travels on the bus. */

/* Self-describing payload tag. A subscriber may receive payloads for several
 * events on one queue, so each neutral payload starts with a tag drawn from
 * this shared vocabulary rather than a feature's own enum. Values start well
 * above any feature's own MsgType range so a raw field can be discriminated
 * safely. */
typedef enum {
    EVENT_PAYLOAD_LED_TOGGLED = 0x100,
} EventPayloadTag;

typedef struct {
    EventPayloadTag tag;
    bool on;
} LedToggledEvent;

#endif
