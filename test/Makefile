CC ?= gcc
CFLAGS := -std=c11 -Wall -Wextra -Wswitch-enum -Werror -DPLATFORM_HOST_TEST
INC := -I../main -I../main/use_cases/toggle-physical-led

.PHONY: all clean

all: test_toggle_led test_event_bus
	./test_toggle_led
	./test_event_bus

test_toggle_led: test_toggle_led.o toggle_logic.o
	$(CC) $(CFLAGS) $^ -o $@

test_event_bus: test_event_bus.o mock_registry.o
	$(CC) $(CFLAGS) $^ -o $@

test_toggle_led.o: test_toggle_led.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

toggle_logic.o: ../main/use_cases/toggle-physical-led/logic.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

test_event_bus.o: test_event_bus.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

mock_registry.o: mock_registry.c
	$(CC) $(CFLAGS) $(INC) -c $< -o $@

clean:
	rm -f *.o test_toggle_led test_event_bus
