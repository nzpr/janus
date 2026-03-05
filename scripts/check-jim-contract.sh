#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <path-to-jim-ts> [repo-root] [contract-version]"
  exit 1
fi

jim_cli="$1"
repo_root="${2:-$(pwd)}"
contract_version="${3:-v1}"

if [[ ! -f "$jim_cli" ]]; then
  echo "jim.ts not found at $jim_cli"
  exit 1
fi

bun "$jim_cli" verify-contract --contract-version "$contract_version"
bun "$jim_cli" verify-contract --path "$repo_root" --contract-version "$contract_version" --strict-path
