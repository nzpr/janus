# Decision: protocol rollout plan all protocols

## Task
TASK-20260307-protocol-rollout

## Date
2026-03-07

## Context
User changed scope from a Postgres-only rollout to a portfolio rollout for all likely protocols needed by production and R&D workloads.
Janus currently covers HTTP proxying, Git HTTP/SSH mediation, and typed host adapters, but lacks a unified protocol expansion roadmap.

## Options Considered
- Expand protocols ad hoc per request with no global ordering.
- Build a generic raw TCP tunnel for everything first.
- Use a staged, risk-tiered protocol roadmap with capability-specific controls (chosen).

## Decision
Adopt a 4-wave rollout plan across protocol families, with each protocol added behind explicit capability gates and policy controls.

Wave 0 (foundation):
- Introduce shared model for non-HTTP protocols: capability name, host+port policy, session-level limits, redaction strategy, and audit schema.
- Add generic integration-test harness for stateful protocol proxies.

Wave 1 (highest value, moderate complexity):
- `postgres_wire`
- `mysql_wire`
- `redis`

Wave 2 (common app/data workloads):
- `mongodb`
- `amqp` (RabbitMQ)
- `kafka`

Wave 3 (infra and enterprise integrations):
- `nats`
- `mqtt`
- `ldap`
- `sftp` (or SSH file-transfer mediation policy)

Wave 4 (specialized/high-risk protocols):
- `smb`
- Other legacy/internal protocols added only with explicit demand and threat model sign-off.

Per-protocol acceptance gate (must pass before GA):
- Capability-scoped auth and allowlist enforcement.
- Protocol-specific abuse controls (timeouts, request limits, connection caps).
- Security review checklist and threat notes.
- Unit + integration tests with reproducible fixtures.
- Observability: per-capability usage, deny reasons, and redaction validation.

## Reasoning
- Provides predictable sequencing and avoids reactive one-off protocol work.
- Maintains Janus security posture by preventing uncontrolled raw tunnel expansion.
- Balances immediate developer value with progressively higher-risk protocol additions.

## Consequences
- More up-front design work before new protocol implementation begins.
- Requires sustained CI investment for protocol integration fixtures.
- Some protocols may be delayed if they fail security/operational gates.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-161307-protocol-rollout-plan-all-protocols.md](../../evolution/events/20260307-161307-protocol-rollout-plan-all-protocols.md)
- Evidence (files/tests):
  - `README.md` capability/safety sections
  - `go/cmd/janusd/main.go`
  - `src/main.rs`
