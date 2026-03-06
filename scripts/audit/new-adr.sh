#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <slug>"
  exit 1
fi

slug="$1"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
adr_dir="$root/docs/adr"
index_file="$adr_dir/INDEX.md"
mkdir -p "$adr_dir"

last_num=$(ls "$adr_dir" 2>/dev/null | rg '^[0-9]{4}-.*\.md$' | sed -E 's/^([0-9]{4})-.*/\1/' | sort | tail -n1 || true)
if [[ -z "${last_num:-}" ]]; then
  next_num=1
else
  next_num=$((10#$last_num + 1))
fi

num=$(printf "%04d" "$next_num")
file="$adr_dir/${num}-${slug}.md"
date=$(date +%F)
title="${slug//-/ }"

cat > "$file" <<TEMPLATE
# ADR-${num}: ${slug//-/ }

## Status
Proposed

## Date
${date}

## Context

## Decision

## Options Considered
- 

## Consequences
### Positive
- 

### Negative
- 

## References
- Related task(s):
- Related decision notes:
- Related evolution events:
- Source links:
TEMPLATE

if [[ ! -f "$index_file" ]]; then
  cat > "$index_file" <<INDEX
# ADR Index

| Date | ADR | File | Summary |
|------|-----|------|---------|
INDEX
fi

link="[${num}-${slug}.md](./${num}-${slug}.md)"
if ! rg -qF "$link" "$index_file"; then
  echo "| ${date} | ADR-${num} | ${link} | ${title} |" >> "$index_file"
fi

echo "$file"
