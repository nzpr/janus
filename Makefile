SHELL := /usr/bin/env bash
.DEFAULT_GOAL := run

IMAGE ?= janusd:latest
CONTAINER ?= janusd
PROXY_PORT ?= 9080
DISCOVERY_PORT ?=
SOCKET_DIR ?= /tmp/janus
JANUS_ENV_FILE ?= .env
DOCKER_SOCKET_PATH ?= /var/run/janus/janusd-control.sock
HOST_SOCKET_PATH ?= $(SOCKET_DIR)/janusd-control.sock

.PHONY: build run start health docker-build deploy stop logs

build:
	cargo build --release --bin janusd --bin janus-mcp --bin janus-tunnel --bin janus-pg-sidecar

run:
	cargo run --bin janusd

start: run

health:
	@socket="$${JANUS_CONTROL_SOCKET:-}"; \
	if [ -z "$$socket" ]; then \
	  if [ -S "$(HOST_SOCKET_PATH)" ]; then socket="$(HOST_SOCKET_PATH)"; else socket="/tmp/janusd-control.sock"; fi; \
	fi; \
	curl --silent --show-error --fail --unix-socket "$$socket" http://localhost/v1/health

docker-build:
	docker build -t $(IMAGE) .

deploy: docker-build
	@set -euo pipefail; \
	mkdir -p "$(SOCKET_DIR)"; \
	docker rm -f "$(CONTAINER)" >/dev/null 2>&1 || true; \
	env_flag=""; \
	if [ -f "$(JANUS_ENV_FILE)" ]; then env_flag="--env-file $(JANUS_ENV_FILE)"; fi; \
	discovery_flags=""; \
	if [ -n "$(DISCOVERY_PORT)" ]; then \
	  discovery_flags="-p $(DISCOVERY_PORT):9181 -e JANUS_DISCOVERY_BIND=0.0.0.0:9181"; \
	fi; \
	docker run -d --name "$(CONTAINER)" \
	  -p "$(PROXY_PORT):9080" \
	  $$discovery_flags \
	  -v "$(SOCKET_DIR):/var/run/janus" \
	  $$env_flag \
	  -e JANUS_PROXY_BIND=0.0.0.0:9080 \
	  -e JANUS_CONTROL_SOCKET=$(DOCKER_SOCKET_PATH) \
	  "$(IMAGE)" >/dev/null; \
	echo "deployed $(CONTAINER) ($(IMAGE))"; \
	echo "proxy: 127.0.0.1:$(PROXY_PORT)"; \
	if [ -n "$(DISCOVERY_PORT)" ]; then echo "public discovery: 127.0.0.1:$(DISCOVERY_PORT)"; fi; \
	echo "control socket: $(HOST_SOCKET_PATH)"

stop:
	docker rm -f "$(CONTAINER)"

logs:
	docker logs -f "$(CONTAINER)"
