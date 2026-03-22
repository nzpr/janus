# Evolution Event: Secret request scrubber parity with leakwall

## Date
2026-03-22

## Task
TASK-SECRET-REQUEST-SCRUB

## Summary
Codex now redacts leakwall-style discovered and pattern-matched secrets from Responses request payloads before they are sent over HTTP or websocket transports.

## Changes
- Added leakwall-inspired secret discovery and recursive JSON string sanitization in `codex-secrets`.
- Sanitized serialized request payloads in both `codex-api` Responses transport paths.
- Added regression tests for direct request-body scrubbing and a `.env`/git-remote seeded prompt fixture that echoes the sanitized prompt back through the mock LLM path.
- Added a dedicated CI job for the secret redaction regression test.

## Evidence
- `cargo test -p codex-secrets`
- `cargo test -p codex-api stream_request_redacts_leakwall_style_prompt_fixture -- --nocapture`

## Links
- Related decision: `docs/decisions/TASK-SECRET-REQUEST-SCRUB.md`
