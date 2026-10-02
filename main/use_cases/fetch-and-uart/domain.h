#ifndef FETCH_AND_UART_DOMAIN_H
#define FETCH_AND_UART_DOMAIN_H

#include <stdbool.h>
#include <stdint.h>

typedef enum {
    MSG_DB_QUERY_RESULT_READY = 0,
    MSG_UART_TX_DONE,
} MsgType;

typedef struct {
    MsgType type;
    bool success;
} Msg;

typedef enum {
    CMD_NONE = 0,
    CMD_SEND_UART,
    CMD_SYNC_NETWORK,
} CmdType;

typedef struct {
    CmdType type;
} Cmd;

typedef struct {
    bool synced;
} Model;

typedef struct {
    Model next;
    Cmd command;
} UpdateResult;

Model fetch_and_uart_init(void);
UpdateResult fetch_and_uart_update(Model model, Msg msg);

void fetch_and_uart_init_use_case_hardware(void);
void execute_fetch_and_uart_hardware(const Cmd *command);
bool fetch_and_uart_button_pressed(void);

#endif
