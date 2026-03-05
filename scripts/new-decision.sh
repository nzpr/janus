#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <TASK-ID> <slug>"
  exit 1
fi

task_id="$1"
slug="$2"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dec_dir="$root/docs/decisions"
index_file="$dec_dir/INDEX.md"
date=$(date +%F)
file="$dec_dir/${task_id}-${slug}.md"
title="${slug//-/ }"
mkdir -p "$dec_dir"

cat > "$file" <<TEMPLATE
# Decision: ${slug//-/ }

## Task
${task_id}

## Date
${date}

## Context

## Options Considered
- 

## Decision

## Reasoning

## Consequences

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event:
- Evidence (files/tests):
TEMPLATE

if [[ ! -f "$index_file" ]]; then
  cat > "$index_file" <<INDEX
# Decision Notes Index

| Date | Task | File | Summary |
|------|------|------|---------|
INDEX
fi

link="[${task_id}-${slug}.md](./${task_id}-${slug}.md)"
if ! rg -qF "$link" "$index_file"; then
  echo "| ${date} | ${task_id} | ${link} | ${title} |" >> "$index_file"
fi

echo "$file"
