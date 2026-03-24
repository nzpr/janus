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
Implement leakwall-style screening in `codex-responses-api-proxy`, remove the previously added Codex-core request scrubbing, and allow additional secret values to be pushed into the proxy over an optional Unix socket.

## Reasoning
The proxy is already an optional boundary around `/v1/responses` traffic, so adding screening there keeps the main Codex codepath closer to upstream and makes future Codex upgrades easier. The implementation can stay self-contained in the proxy crate: sanitize the JSON body before forwarding, keep transport behavior intact, and allow another privileged local process to replace a supplemental secret list over a Unix socket. The tradeoff is that users must explicitly run and configure the proxy, and direct provider calls outside the proxy are not protected.

## Consequences
- Codex itself can remain closer to upstream.
- Screening logic lives in the proxy crate rather than `codex-api`.
- Protection only applies when Codex is configured to use the proxy.
- The proxy test surface becomes the main place to validate outbound filtering behavior.
- Supplemental secrets can be supplied at runtime without restarting the proxy or rewriting env/files.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260322-171645-proxy-secret-screening.md](../../evolution/events/20260322-171645-proxy-secret-screening.md)
- Evidence (files/tests):
  - `codex-rs/responses-api-proxy/src/screening.rs`
  - `codex-rs/responses-api-proxy/src/lib.rs`
  - `codex-rs/responses-api-proxy/src/secret_socket.rs`
  - `codex-rs/responses-api-proxy/README.md`
  - `codex-rs/responses-api-proxy/npm/package.json`
  - `codex-rs/responses-api-proxy/npm/README.md`
  - `README-proxy.md`
  - `cargo test -p codex-responses-api-proxy`
