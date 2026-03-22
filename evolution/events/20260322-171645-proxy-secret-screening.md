# Evolution Event: proxy secret screening

## Timestamp
2026-03-22T17:16:45+00:00

## Trigger
User requested evaluating a proxy-based integration so Codex can stay untouched and upgrades remain seamless.

## Change
Implemented leakwall-style secret screening in `codex-responses-api-proxy`, removed the earlier Codex-core request scrubbing, and added a standalone GitHub Actions workflow that prints the raw prompt, sanitized forwarded prompt, and mock LLM echo.

## Decision Link
- ADR:
- Task decision: [TASK-PROXY-SECRET-SCREENING-proxy-secret-screening.md](../../docs/decisions/TASK-PROXY-SECRET-SCREENING-proxy-secret-screening.md)

## Validation Evidence
- `codex-rs/responses-api-proxy/src/screening.rs`
- `codex-rs/responses-api-proxy/src/lib.rs`
- `.github/workflows/proxy-secret-screening.yml`
- `cargo test -p codex-responses-api-proxy sanitizes_leakwall_style_prompt_fixture -- --nocapture`
- `cargo test -p codex-secrets`
- `cargo test -p codex-api --test clients`

## Outcome
Success

## Follow-up
- Configure Codex to use `codex-responses-api-proxy` as a custom Responses provider when screening is desired.
