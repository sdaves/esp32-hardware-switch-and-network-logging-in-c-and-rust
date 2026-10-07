# Top-level command surface for the ESP32 TEA platform.
#
# Firmware builds and emulator runs execute on the host (Docker + ESP-IDF) and
# are driven from the devcontainer through the host listener (see AGENTS.md §6).
# `make listener` starts that listener; every other remote target talks to it
# over HTTP and streams the log back.

LISTENER ?= http://host.docker.internal:2222
NAME     ?= uc1_button_toggle
CMD      ?=

CURL := curl -sS
LONG := --max-time 2400

.DEFAULT_GOAL := help

.PHONY: help listener up down restart logs ps build test native-test scenario exec clean check-listener

help:
	@echo "ESP32 TEA platform - top-level targets"
	@echo ""
	@echo "  make listener            Start the host build/test listener (run on the host)"
	@echo "  make up                  Pull + start the Velxio stack (docker compose up --build -d)"
	@echo "  make down                Stop the Velxio stack"
	@echo "  make restart             down + up"
	@echo "  make logs                Show Velxio container logs"
	@echo "  make ps                  Show Velxio container status"
	@echo "  make build               Build firmware (idf.py build + 4 MB merge-bin)"
	@echo "  make test                Native tests + firmware build + all emulator scenarios"
	@echo "  make native-test         Desktop C unit tests only (make -C test)"
	@echo "  make scenario NAME=...   Run one emulator scenario (default: $(NAME))"
	@echo "  make exec CMD='...'      Run an arbitrary shell command in the Velxio container"
	@echo "  make clean               Remove native binaries and ESP-IDF build output"
	@echo ""
	@echo "  Listener: $(LISTENER)   (override with LISTENER=http://...)"
	@echo "  On the host itself: make LISTENER=http://localhost:2222 <target>"

check-listener:
	@$(CURL) -fsS --max-time 3 $(LISTENER)/health >/dev/null 2>&1 || { \
		echo "ERROR: host listener not reachable at $(LISTENER)"; \
		echo "Start it on the host with:  make listener"; \
		echo "On the host itself:         make LISTENER=http://localhost:2222 <target>"; \
		exit 1; }

listener:
	python3 .devcontainer/docker-build-listener.py

up: check-listener
	@$(CURL) $(LONG) $(LISTENER)/build

down: check-listener
	@$(CURL) $(LISTENER)/down

restart: down up

logs: check-listener
	@$(CURL) $(LISTENER)/logs

ps: check-listener
	@$(CURL) $(LISTENER)/ps

build: check-listener
	@$(CURL) $(LONG) $(LISTENER)/build-firmware

test: check-listener
	@$(CURL) $(LONG) $(LISTENER)/test

native-test:
	$(MAKE) -C test

scenario: check-listener
	@$(CURL) -G --data-urlencode 'cmd=cd /workspace && ./scripts/scenario.sh $(NAME)' $(LISTENER)/exec

exec: check-listener
	@$(CURL) -G --data-urlencode 'cmd=$(CMD)' $(LISTENER)/exec

clean:
	-$(MAKE) -C test clean
	rm -rf build sdkconfig sdkconfig.old
	@echo "cleaned"
