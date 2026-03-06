# Implementation Loop

Use this loop for non-trivial implementation tasks.
It is baseline-independent so it can be inherited by any Jim-managed project.

## Loop

1. Research first.
2. Write/update `plan.md` before coding.
3. Mark constraints explicitly (APIs, paths, style, tests).
4. Implement in small increments.
5. Validate after each increment.
6. If drift occurs, revert to plan and re-approach.
7. Record decision note and evolution event.

## Required Artifacts Per Non-Trivial Change

- Plan file (or plan section in task notes)
- Decision note or ADR
- Evolution event with evidence links
- Validation output summary
- Meaningful commit for completed slice (unless explicitly skipped by user)

## Drift Recovery

- Stop current implementation branch of thought.
- Identify which instruction/constraint was violated.
- Patch plan first, then patch code.
- Log correction in evolution event.
