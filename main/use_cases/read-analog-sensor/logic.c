#include "domain.h"

Model analog_sensor_init(void)
{
    return (Model){ .last_millivolts = 0, .in_error = false };
}

UpdateResult analog_sensor_update(Model model, Msg msg)
{
    UpdateResult result = {
        .next = model,
        .command = { .type = CMD_NONE, .millivolts = 0 },
    };

    switch (msg.type) {
    case MSG_ANALOG_SAMPLE_READY:
        result.next.last_millivolts = msg.millivolts;
        result.next.in_error = msg.millivolts < 0;
        result.command.millivolts = msg.millivolts;
        result.command.type = result.next.in_error ? CMD_POST_ANALOG_ERROR : CMD_POST_ANALOG;
        break;
    }

    return result;
}
