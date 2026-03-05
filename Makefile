SHELL := /usr/bin/env bash
.DEFAULT_GOAL := start

.PHONY: start

start:
	bun run src/mcp-server.ts --workspace "$(CURDIR)" --client host
