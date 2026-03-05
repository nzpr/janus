SHELL := /usr/bin/env bash
.DEFAULT_GOAL := start

.PHONY: start

start:
	cargo run --bin janusd
