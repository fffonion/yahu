SHELL := /usr/bin/env bash
PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
SERVICE_DIR ?= $(HOME)/.config/systemd/user
SERVICE_NAME := yahu.service
BINARY := yahu
WORKSPACE ?= $(HOME)/workspace
API_URL ?= http://127.0.0.1:8642
PORT ?= 9642
HOST ?= 127.0.0.1

.PHONY: assets build run install uninstall service-install service-enable service-stop system-service-install system-service-enable system-service-stop clean check

assets:
	bun install
	bun run build

build: assets
	cargo build --release

check: assets
	cargo check

run: build
	cargo run -- --insecure --host $(HOST) --port $(PORT) --api-url $(API_URL) --workspace $(WORKSPACE)

TARGET_DIR := $(shell cargo metadata --format-version=1 --no-deps 2>/dev/null | grep -o '"target_directory":"[^"]*"' | cut -d'"' -f4 || echo target)

install: build
	install -d "$(BINDIR)"
	install -m 0755 "$(TARGET_DIR)/release/$(BINARY)" "$(BINDIR)/$(BINARY)"
	install -d "$(PREFIX)/share/yet-another-hermes-ui/scripts"
	install -m 0755 scripts/png-to-ios-heic.sh "$(PREFIX)/share/yet-another-hermes-ui/scripts/png-to-ios-heic.sh"
	@echo "Installed $(BINDIR)/$(BINARY)"

uninstall: service-stop
	rm -f "$(BINDIR)/$(BINARY)"
	rm -f "$(SERVICE_DIR)/$(SERVICE_NAME)"
	-systemctl --user daemon-reload

service-install: install
	install -d "$(SERVICE_DIR)"
	install -m 0644 deploy/$(SERVICE_NAME) "$(SERVICE_DIR)/$(SERVICE_NAME)"
	systemctl --user daemon-reload
	@echo "Edit $(SERVICE_DIR)/$(SERVICE_NAME) to set HERMES_API_KEY and HERMES_WEBUI_AUTH_KEY."

service-enable: service-install
	systemctl --user enable --now $(SERVICE_NAME)
	systemctl --user status $(SERVICE_NAME) --no-pager

service-stop:
	-systemctl --user stop $(SERVICE_NAME)
	-systemctl --user disable $(SERVICE_NAME)

system-service-install: install
	sudo install -m 0644 deploy/yahu.system.service /etc/systemd/system/$(SERVICE_NAME)
	sudo systemctl daemon-reload

system-service-enable: system-service-install
	sudo systemctl enable --now $(SERVICE_NAME)
	sudo systemctl status $(SERVICE_NAME) --no-pager

system-service-stop:
	-sudo systemctl stop $(SERVICE_NAME)
	-sudo systemctl disable $(SERVICE_NAME)

clean:
	rm -rf dist node_modules
	cargo clean
