#include "domain.h"

void read_and_insert_init_use_case_hardware(void)
{
}

void execute_read_and_insert_hardware(const Cmd *command)
{
    if (!command || command->type == CMD_NONE) {
        return;
    }
}
