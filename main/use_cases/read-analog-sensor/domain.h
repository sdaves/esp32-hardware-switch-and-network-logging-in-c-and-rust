#ifndef READ_ANALOG_SENSOR_DOMAIN_H
#define READ_ANALOG_SENSOR_DOMAIN_H

#include <stdbool.h>
#include <stdint.h>

typedef enum {
    MSG_ANALOG_SAMPLE_READY = 0,
} MsgType;

typedef struct {
    MsgType type;
    int32_t millivolts;
} Msg;

typedef enum {
    CMD_NONE = 0,
    CMD_POST_ANALOG,
    CMD_POST_ANALOG_ERROR,
} CmdType;

typedef struct {
    CmdType type;
    int32_t millivolts;
} Cmd;

typedef struct {
    int32_t last_millivolts;
    bool in_error;
} Model;

typedef struct {
    Model next;
    Cmd command;
} UpdateResult;

Model analog_sensor_init(void);
UpdateResult analog_sensor_update(Model model, Msg msg);

void analog_sensor_init_use_case_hardware(void);
void execute_analog_sensor_hardware(const Cmd *command);

#endif
