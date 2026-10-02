CC ?= gcc
CFLAGS := -std=c11 -Wall -Wextra -Wswitch-enum -Werror -DPLATFORM_HOST_TEST
INC := -I../main -I../main/use_cases/toggle-physical-led
UC5_INC := -I../main -I../main/use_cases/fetch-and-uart

.PHONY: all clean

all: test_toggle_led test_event_bus test_fetch_and_uart
	./test_toggle_led
	./test_event_bus
	./test_fetch_and_uart

test_toggle_led: test_toggle_led.o toggle_logic.o
	$(CC) $(CFLAGS) $^ -o $@

test_event_bus: test_event_bus.o mock_registry.o
	$(CC) $(CFLAGS) $^ -o $@

test_fetch_and_uart: test_fetch_and_uart.o fetch_and_uart_logic.o
	$(CC) $(CFLAGS) $^ -o $@

test_toggle_led.o: test_toggle_led.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

toggle_logic.o: ../main/use_cases/toggle-physical-led/logic.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

test_event_bus.o: test_event_bus.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

test_fetch_and_uart.o: test_fetch_and_uart.c
	$(CC) $(CFLAGS) $(UC5_INC) -c $< -o $@

fetch_and_uart_logic.o: ../main/use_cases/fetch-and-uart/logic.c
	$(CC) $(CFLAGS) $(UC5_INC) -c $< -o $@

mock_registry.o: mock_registry.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

clean:
	rm -f *.o test_toggle_led test_event_bus test_fetch_and_uart
