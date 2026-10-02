#ifndef FETCH_AND_SAVE_DOMAIN_H
#define FETCH_AND_SAVE_DOMAIN_H

#include <stdbool.h>
#include <stdint.h>

typedef enum {
    MSG_HTTP_RESPONSE_READY = 0,
} MsgType;

typedef struct {
    MsgType type;
    bool success;
} Msg;

typedef enum {
    CMD_NONE = 0,
    CMD_SAVE_ISO8601_FILE,
} CmdType;

typedef struct {
    CmdType type;
} Cmd;

typedef struct {
    bool saved;
} Model;

typedef struct {
    Model next;
    Cmd command;
} UpdateResult;

Model fetch_and_save_init(void);
UpdateResult fetch_and_save_update(Model model, Msg msg);

void fetch_and_save_init_use_case_hardware(void);
void execute_fetch_and_save_hardware(const Cmd *command);

#endif
