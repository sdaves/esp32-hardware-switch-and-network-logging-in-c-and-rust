#ifndef TOGGLE_PHYSICAL_LED_DOMAIN_H
#define TOGGLE_PHYSICAL_LED_DOMAIN_H

#include <stdbool.h>
#include <stdint.h>

typedef enum {
    MSG_BUTTON_DOWN = 0,
} MsgType;

typedef struct {
    MsgType type;
} Msg;

typedef enum {
    CMD_NONE = 0,
    CMD_TOGGLE_LED,
} CmdType;

typedef struct {
    CmdType type;
    uint8_t pin;
    uint8_t level;
} Cmd;

typedef struct {
    bool led_on;
} Model;

typedef struct {
    Model next;
    Cmd command;
} UpdateResult;

Model toggle_led_init(void);
UpdateResult toggle_led_update(Model model, Msg msg);

void init_use_case_hardware(void);
void execute_toggle_led_hardware(const Cmd *command);
bool toggle_button_pressed(void);

#endif
