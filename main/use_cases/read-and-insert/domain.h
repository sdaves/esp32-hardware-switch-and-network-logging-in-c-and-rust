#ifndef READ_AND_INSERT_DOMAIN_H
#define READ_AND_INSERT_DOMAIN_H

#include <stdbool.h>
#include <stdint.h>

typedef enum {
    MSG_ADC_ROW_READY = 0,
} MsgType;

typedef struct {
    MsgType type;
    int32_t millivolts;
} Msg;

typedef enum {
    CMD_NONE = 0,
    CMD_INSERT_DB_ROW,
} CmdType;

typedef struct {
    CmdType type;
    int32_t millivolts;
} Cmd;

typedef struct {
    uint32_t rows_logged;
} Model;

typedef struct {
    Model next;
    Cmd command;
} UpdateResult;

Model read_and_insert_init(void);
UpdateResult read_and_insert_update(Model model, Msg msg);

void read_and_insert_init_use_case_hardware(void);
void execute_read_and_insert_hardware(const Cmd *command);

#endif
