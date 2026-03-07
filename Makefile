SHELL := /usr/bin/env bash
.DEFAULT_GOAL := run

.PHONY: build run start health

build:
	cargo build --release --bin janusd --bin janus-mcp

run:
	cargo run --bin janusd

start: run

health:
	@socket="$${JANUS_CONTROL_SOCKET:-/tmp/janusd-control.sock}"; \
	curl --silent --show-error --fail --unix-socket "$$socket" http://localhost/v1/health
