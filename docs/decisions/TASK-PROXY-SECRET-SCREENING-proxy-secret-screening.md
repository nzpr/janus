# Decision: proxy secret screening

## Task
TASK-PROXY-SECRET-SCREENING

## Date
2026-03-22

## Context
The user prefers to keep the Codex client itself unchanged to preserve seamless upstream upgrades. The alternative is to add leakwall-style secret discovery, scanning, and redaction to `codex-responses-api-proxy`, then route Codex through that proxy when protection is desired.

## Options Considered
- Keep secret screening inside Codex request serialization.
- Move secret screening into `codex-responses-api-proxy`.

## Decision
Implement leakwall-style screening in `codex-responses-api-proxy` and remove the previously added Codex-core request scrubbing.

## Reasoning
The proxy is already an optional boundary around `/v1/responses` traffic, so adding screening there keeps the main Codex codepath closer to upstream and makes future Codex upgrades easier. The implementation can stay self-contained in the proxy crate: sanitize the JSON body before forwarding, keep transport behavior intact, and expose a standalone CI workflow that shows raw prompt, sanitized forwarded prompt, and mock LLM echo. The tradeoff is that users must explicitly run and configure the proxy, and direct provider calls outside the proxy are not protected.

## Consequences
- Codex itself can remain closer to upstream.
- Screening logic lives in the proxy crate rather than `codex-api`.
- Protection only applies when Codex is configured to use the proxy.
- The proxy test surface becomes the main place to validate outbound filtering behavior.
- GitHub Actions shows a separate `proxy-secret-screening` workflow entry instead of hiding the regression inside `rust-ci`.
- The proxy can be released independently via a `proxy-v*` GitHub release workflow without changing Codex's main release process.
- The same `proxy-release` workflow can also stage and publish the npm wrapper package using the proxy's own vendored binaries.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260322-171645-proxy-secret-screening.md](../../evolution/events/20260322-171645-proxy-secret-screening.md)
- Evidence (files/tests):
  - `codex-rs/responses-api-proxy/src/screening.rs`
  - `codex-rs/responses-api-proxy/src/lib.rs`
  - `codex-rs/responses-api-proxy/README.md`
  - `.github/workflows/proxy-secret-screening.yml`
  - `.github/workflows/proxy-release.yml`
  - `codex-cli/scripts/build_npm_package.py`
  - GitHub secret `NPM_TOKEN`
  - `cargo test -p codex-responses-api-proxy sanitizes_leakwall_style_prompt_fixture -- --nocapture`
