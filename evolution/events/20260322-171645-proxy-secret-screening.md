# Evolution Event: proxy secret screening

## Timestamp
2026-03-22T17:16:45+00:00

## Trigger
User requested evaluating a proxy-based integration so Codex can stay untouched and upgrades remain seamless.

## Change
Implemented secret redaction in `codex-responses-api-proxy`, removed the earlier Codex-core request scrubbing, and later extended the proxy so another local process can replace the full redaction list over an optional Unix socket before requests are forwarded upstream. Follow-up corrections taught the socket parser to extract values from env-style payloads, made literal redaction avoid clobbering identifier names that merely contain a secret substring, and then removed the proxy's implicit file/environment/git/regex discovery so only explicit socket-provided values are redacted.

## Decision Link
- ADR:
- Task decision: [TASK-PROXY-SECRET-SCREENING-proxy-secret-screening.md](../../docs/decisions/TASK-PROXY-SECRET-SCREENING-proxy-secret-screening.md)

## Validation Evidence
- `codex-rs/responses-api-proxy/src/screening.rs`
- `codex-rs/responses-api-proxy/src/lib.rs`
- `codex-rs/responses-api-proxy/src/secret_socket.rs`
- `codex-rs/responses-api-proxy/README.md`
- `codex-rs/responses-api-proxy/npm/package.json`
- `codex-rs/responses-api-proxy/npm/README.md`
- `README-proxy.md`
- `cargo test -p codex-responses-api-proxy`
- `just bazel-lock-update`
- `just bazel-lock-check`
- `just fix -p codex-responses-api-proxy`
- `just fmt`

## Outcome
Success

## Follow-up
- Configure Codex to use `codex-responses-api-proxy` as a custom Responses provider when screening is desired.
- Install `libcap` development files in the build environment before running `argument-comment-lint`, or document that check as environment-dependent for this crate.
