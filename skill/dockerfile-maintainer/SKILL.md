# Dockerfile Maintainer Skill

Use this skill when creating or modifying Dockerfiles, Docker build scripts, or image module
layouts.

## Goals

- Minimize cache invalidation blast radius.
- Keep Docker hierarchy modular and composable.
- Make rebuild cost predictable when adding/updating one toolchain.
- Preserve reproducibility and safety.

## Default Approach

1. Understand current stage graph and module boundaries.
2. Identify which exact stage should change.
3. Prefer stage-scoped `COPY` for scripts/assets over broad tree copy.
4. Use per-module/per-concern `RUN` layers for heavyweight installs.
5. Keep frequently changing inputs as late as possible.
6. Validate with explicit checks and document cache implications.

## Cache Rules

- Avoid broad `COPY docker/...` in early base stages when only subset is needed.
- Split heavyweight installers into separate `RUN` layers when independent.
- Do not group unrelated language/tool installs in one `RUN`.
- Keep build args ordered by volatility (stable first, volatile later).
- Keep cleanup in same `RUN` as install for layer hygiene, but do not over-bundle unrelated work.

## Validation Checklist

- `bash -n` for touched shell module scripts.
- Build help/targets still resolve (`build.sh --help` / `make` targets).
- If possible, dry-run or partial build to confirm affected-stage-only invalidation.
- Update docs/decision records when cache behavior or hierarchy changes.

## Output Expectations

- Clearly state:
  - changed stages/modules,
  - expected cache impact,
  - required rebuild command.
- Keep change scoped and reversible.
