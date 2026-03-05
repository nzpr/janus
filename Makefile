SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

WORKSPACE ?= $(CURDIR)
CLIENT ?= container
INSTANCE ?= $(USER)
INSTANCE_PREFIX ?= janus
GRANTS ?=
CMD ?=

.PHONY: help install plan plan-host run serve serve-host mcp mcp-host check

help:
	@echo "Targets:"
	@echo "  install                          Install dependencies with bun"
	@echo "  plan [CLIENT=... GRANTS=...]     Show resolved grants/env keys"
	@echo "  plan-host                        Shortcut for CLIENT=host plan"
	@echo "  run CMD='<command>'              Run command via Janus broker"
	@echo "  serve [INSTANCE=... CLIENT=...]  Run Janus proxy service"
	@echo "  serve-host [INSTANCE=...]        Shortcut for CLIENT=host serve"
	@echo "  mcp [CLIENT=... GRANTS=...]      Run Janus MCP server (stdio)"
	@echo "  mcp-host                         Shortcut for CLIENT=host mcp"
	@echo "  check                            Basic smoke checks for Janus + MCP"

install:
	bun install

plan:
	@args=(--workspace "$(WORKSPACE)" --client "$(CLIENT)"); \
	if [[ -n "$(GRANTS)" ]]; then args+=(--grants "$(GRANTS)"); fi; \
	bun run src/janus.ts plan "$${args[@]}"

plan-host:
	@$(MAKE) plan CLIENT=host GRANTS="$(GRANTS)" WORKSPACE="$(WORKSPACE)"

run:
	@if [[ -z "$(CMD)" ]]; then \
		echo "CMD is required, e.g. make run CMD='git ls-remote origin'"; \
		exit 1; \
	fi
	@args=(--workspace "$(WORKSPACE)" --client "$(CLIENT)" --instance "$(INSTANCE)"); \
	if [[ -n "$(GRANTS)" ]]; then args+=(--grants "$(GRANTS)"); fi; \
	bun run src/janus.ts run "$${args[@]}" -- bash -lc "$(CMD)"

serve:
	@instance="$${INSTANCE:-$${USER:-janus}}"; \
	args=(--workspace "$(WORKSPACE)" --client "$(CLIENT)" --instance "$$instance"); \
	if [[ -n "$(GRANTS)" ]]; then args+=(--grants "$(GRANTS)"); fi; \
	bun run src/janus.ts serve "$${args[@]}"

serve-host:
	@$(MAKE) serve CLIENT=host INSTANCE="$(INSTANCE)" GRANTS="$(GRANTS)" WORKSPACE="$(WORKSPACE)"

mcp:
	@args=(--workspace "$(WORKSPACE)" --client "$(CLIENT)" --instance-prefix "$(INSTANCE_PREFIX)"); \
	if [[ -n "$(GRANTS)" ]]; then args+=(--grants "$(GRANTS)"); fi; \
	bun run src/mcp-server.ts "$${args[@]}"

mcp-host:
	@$(MAKE) mcp CLIENT=host GRANTS="$(GRANTS)" WORKSPACE="$(WORKSPACE)"

check:
	bun run src/janus.ts help > /dev/null
	bun run src/janus.ts plan > /dev/null
	bun run src/mcp-server.ts --help > /dev/null
	@echo "checks passed"
