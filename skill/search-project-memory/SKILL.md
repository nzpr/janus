# Search Project Memory Skill

Use this skill when you need prior context, decisions, or references from project memory.

## Goals

- Retrieve relevant memory quickly.
- Prefer indexed search when available.
- Fall back cleanly to markdown search when index storage is unavailable.

## Default Command

```bash
bun ./scripts/intel/memory.ts query --cwd <repo-root> -- "<query>"
```

## Optional Prep

When indexed retrieval is configured and you need fresh results:

```bash
bun ./scripts/intel/memory.ts sync --cwd <repo-root>
```

## Useful Options

- `--top-k <n>`
- `--fts-k <n>`
- `--vector-k <n>`
- `--rrf-k <n>`
- `--provider hash|openai`
- `--model <embedding-model>`
- `--db-path <repo-local-pglite-path>`

## Execution Rules

1. Run query first; do not force sync unless needed.
2. If query output says indexed memory unavailable, continue with markdown fallback result.
3. Surface path + snippet matches in response.
4. Keep output compact and relevant to current task.

## Output Expectations

- memory mode used (indexed or markdown fallback)
- top matched paths and snippets
- any actionable prior decisions found
