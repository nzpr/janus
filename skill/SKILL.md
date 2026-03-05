# Talk-Only Audit Skill

Use this skill when the user wants to stay in pure conversation mode and still keep auditable evolution records.

## Intent

- User only talks.
- Agent runs audit scripts when needed.
- Agent keeps ADR/decision/event records linked and current.

## Trigger Detection

Run audit automation when any of these occur:
- Architecture-level choice with cross-task impact.
- Task-level tradeoff or implementation direction is selected.
- Explicit request to preserve rationale or handoff context.

## Command Mapping

1. Architecture decision:
   - `bash ./scripts/auto-audit.sh adr <slug>`
2. Task decision:
   - `bash ./scripts/auto-audit.sh task <slug> <TASK-ID>`
3. If task id is unknown:
   - use `TASK-ADHOC` and continue.

## Post-Command Requirements

After command execution:
1. Fill the generated decision file with:
   - context
   - options
   - chosen decision
   - reasoning
   - consequences
2. Fill generated evolution event with:
   - trigger
   - change
   - validation evidence
   - outcome
3. In the response, report created file paths and a one-line summary.

## Behavior Rule

Do not ask the user to run scripts manually.
Run scripts directly via tools, unless the environment blocks execution.
