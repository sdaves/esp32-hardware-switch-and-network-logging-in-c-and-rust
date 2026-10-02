#include "domain.h"

void fetch_and_uart_init_use_case_hardware(void)
{
}

void execute_fetch_and_uart_hardware(const Cmd *command)
{
    if (!command || command->type == CMD_NONE) {
        return;
    }
}
