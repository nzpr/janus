SHELL := /usr/bin/env bash
.DEFAULT_GOAL := run

.PHONY: build run start

build:
	cargo build --release --bin janusd --bin janus-mcp

run:
	cargo run --bin janusd

start: run
