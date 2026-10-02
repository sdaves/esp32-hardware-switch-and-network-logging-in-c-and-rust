#include "domain.h"

Model fetch_and_save_init(void)
{
    return (Model){ .saved = false };
}

UpdateResult fetch_and_save_update(Model model, Msg msg)
{
    UpdateResult result = {
        .next = model,
        .command = { .type = CMD_NONE },
    };

    switch (msg.type) {
    case MSG_HTTP_RESPONSE_READY:
        result.next.saved = msg.success;
        if (msg.success) {
            result.command.type = CMD_SAVE_ISO8601_FILE;
        }
        break;
    }

    return result;
}
