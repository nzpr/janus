#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <slug>"
  exit 1
fi

slug="$1"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
events_dir="$root/evolution/events"
mkdir -p "$events_dir"
index_file="$events_dir/INDEX.md"

ts=$(date +%Y%m%d-%H%M%S)
file="$events_dir/${ts}-${slug}.md"
date_iso=$(date -Iseconds)

cat > "$file" <<TEMPLATE
# Evolution Event: ${slug//-/ }

## Timestamp
${date_iso}

## Trigger

## Change

## Decision Link
- ADR:
- Task decision:

## Validation Evidence
- 

## Outcome
Improved | Neutral | Regressed

## Follow-up
- 
TEMPLATE

if [[ ! -f "$index_file" ]]; then
  cat > "$index_file" <<INDEX
# Evolution Events Index

| Timestamp | File | Summary |
|-----------|------|---------|
INDEX
fi

link="[${ts}-${slug}.md](./${ts}-${slug}.md)"
if ! rg -qF "$link" "$index_file"; then
  echo "| ${date_iso} | ${link} | ${slug//-/ } |" >> "$index_file"
fi

echo "$file"
