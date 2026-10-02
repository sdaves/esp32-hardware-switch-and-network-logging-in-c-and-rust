#include "domain.h"

void analog_sensor_init_use_case_hardware(void)
{
}

void execute_analog_sensor_hardware(const Cmd *command)
{
    if (!command || command->type == CMD_NONE) {
        return;
    }
}
