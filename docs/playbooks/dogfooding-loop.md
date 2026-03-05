# Jim Dogfooding Loop

Use this process to run project work with Jim and feed cross-project improvements back to Jim.

## 1. Bootstrap every project

Install the contract once per repo:

```bash
bun /path/to/jim.ts init --path <project-repo> --contract-version v1
```

Or run normal Jim mode; it auto-initializes when missing:

```bash
bun /path/to/jim.ts
```

## 2. Run normal project delivery

- Keep decisions in `docs/decisions/`.
- Keep architecture choices in `docs/adr/`.
- Keep evolution evidence in `evolution/events/`.

## 3. Mark reusable improvements

When you discover a process/tooling improvement that should be reused in other repos, tag it with `jim-improvement` in decision/event notes.

Example marker line:

```markdown
- jim-improvement: add a preflight script for flaky integration tests.
```

## 4. Harvest improvements back into Jim

Run harvest from project to Jim repo:

```bash
bun /path/to/jim.ts harvest --source <project-repo> --jim-repo <jim-repo> --tag jim-improvement
```

Or accept the publish prompt after a normal Jim session ends.

This writes a timestamped report in `docs/harvest/` inside Jim.

## 5. Promote improvements into contract

For each harvested item:

1. Classify as global contract change or project-local pattern.
2. Record decision/evolution evidence in Jim repo.
3. Update `contract/<version>/` and `jim.ts` if needed.
4. Ask projects to run `jim upgrade`.

## 6. Verify continuously

- In project repos: `bun /path/to/jim.ts verify-contract --path . --contract-version v1 --strict-path`
- In Jim repo: `bun ./jim.ts verify-contract --contract-version v1`
