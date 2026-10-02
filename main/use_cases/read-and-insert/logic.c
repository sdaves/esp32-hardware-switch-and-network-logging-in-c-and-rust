#include "domain.h"

Model read_and_insert_init(void)
{
    return (Model){ .rows_logged = 0 };
}

UpdateResult read_and_insert_update(Model model, Msg msg)
{
    UpdateResult result = {
        .next = model,
        .command = { .type = CMD_NONE, .millivolts = 0 },
    };

    switch (msg.type) {
    case MSG_ADC_ROW_READY:
        result.next.rows_logged = model.rows_logged + 1;
        result.command.type = CMD_INSERT_DB_ROW;
        result.command.millivolts = msg.millivolts;
        break;
    }

    return result;
}
