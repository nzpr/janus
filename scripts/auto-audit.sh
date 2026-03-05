#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <adr|task> <slug> [TASK-ID]"
  exit 1
fi

scope="$1"
slug="$2"
task_id="${3:-}"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

case "$scope" in
  adr)
    decision_file="$($root/scripts/new-adr.sh "$slug")"
    ;;
  task)
    if [[ -z "$task_id" ]]; then
      echo "task scope requires TASK-ID: $0 task <slug> <TASK-ID>"
      exit 1
    fi
    decision_file="$($root/scripts/new-decision.sh "$task_id" "$slug")"
    ;;
  *)
    echo "invalid scope: $scope (expected adr|task)"
    exit 1
    ;;
esac

event_file="$($root/scripts/log-evolution.sh "$slug")"

decision_base="$(basename "$decision_file")"
event_base="$(basename "$event_file")"

if [[ "$scope" == "adr" ]]; then
  sed -i "s#^- ADR:#- ADR: [${decision_base}](../../docs/adr/${decision_base})#" "$event_file"
else
  sed -i "s#^- Task decision:#- Task decision: [${decision_base}](../../docs/decisions/${decision_base})#" "$event_file"
fi

sed -i "s#^- Related evolution events:#- Related evolution events: [${event_base}](../../evolution/events/${event_base})#" "$decision_file"
sed -i "s#^- Related evolution event:#- Related evolution event: [${event_base}](../../evolution/events/${event_base})#" "$decision_file"

echo "decision=$decision_file"
echo "event=$event_file"
