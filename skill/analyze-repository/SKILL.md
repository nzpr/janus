# Analyze Repository Skill

Use this skill when you need evidence-backed answers about where or how something is implemented in a repository.

## Goals

- Produce verifiable findings with file/line/commit evidence.
- Keep analysis scoped to the user query.
- Return report path and key confidence signals.

## Default Command

```bash
bun ./jim.ts repo-intel --repo <path-or-url> --query "<question>"
```

## Useful Options

- `--mode auto|conceptual|implementation|history|comprehensive`
- `--profile default|strict`
- `--connector web|docs|code` (repeatable)
- `--no-verify` for remote repo URL when shallow clone verification is not needed
- `--max-evidence <n>`
- `--out <path>`

## Execution Rules

1. Prefer local repo path when available.
2. Use `--profile strict` when user asks for high confidence.
3. Keep `--max-evidence` bounded to avoid noisy outputs.
4. Return the generated report path and a concise summary.

## Output Expectations

- report path
- mode/profile used
- confidence/disciplined findings summary
- top evidence bullets
