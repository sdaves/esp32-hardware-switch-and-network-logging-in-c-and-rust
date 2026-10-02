#include <assert.h>
#include <stdio.h>

#include "domain.h"

int main(void)
{
    Model initial = toggle_led_init();
    assert(initial.led_on == false);

    UpdateResult first = toggle_led_update(initial, (Msg){ .type = MSG_BUTTON_DOWN });
    assert(first.command.type == CMD_TOGGLE_LED);
    assert(first.command.pin == 2);
    assert(first.command.level == 1);
    assert(first.next.led_on == true);

    UpdateResult second = toggle_led_update(first.next, (Msg){ .type = MSG_BUTTON_DOWN });
    assert(second.command.type == CMD_TOGGLE_LED);
    assert(second.command.level == 0);
    assert(second.next.led_on == false);

    printf("test_toggle_led: PASS\n");
    return 0;
}
