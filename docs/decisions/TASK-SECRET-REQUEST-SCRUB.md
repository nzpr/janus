# Decision: Port leakwall-style secret screening into Codex request sanitization

## Task
TASK-SECRET-REQUEST-SCRUB

## Date
2026-03-22

## Context
Codex was sending prompt and tool payload strings to the Responses endpoint with only a narrow regex redactor available elsewhere in the codebase. The requested behavior was to mirror leakwall's secret screening model inside Codex itself, without adopting leakwall's proxy layer.

## Options Considered
- Add a proxy or transport shim around Codex requests.
- Port leakwall-style screening into Codex request serialization.
- Keep the existing limited regex redaction.

## Decision
Port leakwall-inspired request screening into `codex-secrets` and apply it to serialized Responses HTTP and websocket payloads in `codex-api`.

## Reasoning
Applying sanitization at the serialized request boundary keeps the integration small and upstream-merge-friendly while still covering both transport paths. Reusing leakwall's screening ideas inside `codex-secrets` avoids introducing proxy behavior and allows regression tests to seed `.env` and git metadata directly.

## Consequences
Expected impacts and risks.
- Outbound request strings are redacted before transport for both HTTP and websocket Responses calls.
- Local secret discovery now influences request payload sanitization, so redaction can affect prompts that intentionally contain local credentials.
- The implementation is intentionally limited to request sanitization; allowlisting and proxy-only leakwall behaviors are not carried over.
- CI logs now print the intended prompt, sanitized outbound prompt, and echoed assistant response for the regression test.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: `evolution/events/2026-03-22-secret-request-scrub.md`
- Evidence (files/tests):
  - `codex-rs/secrets/src/sanitizer.rs`
  - `codex-rs/codex-api/src/endpoint/responses.rs`
  - `codex-rs/codex-api/src/endpoint/responses_websocket.rs`
  - `codex-rs/codex-api/tests/clients.rs`
  - `.github/workflows/rust-ci.yml`
