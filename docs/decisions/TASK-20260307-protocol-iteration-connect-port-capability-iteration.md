# Decision: connect port capability iteration

## Task
TASK-20260307-protocol-iteration

## Date
2026-03-07

## Context
User asked to proceed with protocol iteration after the all-protocol rollout plan.
Janus already supported `http_proxy` and `git_ssh`, but non-HTTP protocol capabilities were not yet enforceable at the proxy layer.

## Options Considered
- Keep relying only on broad `http_proxy` for all protocol traffic.
- Implement full protocol-aware adapters for every planned protocol immediately.
- Add a first iteration: CONNECT port-to-capability policy for planned protocol capabilities (chosen).

## Decision
Implement a capability-scoped CONNECT policy map in both Go and Rust daemons:
- `postgres_wire` -> `5432`
- `mysql_wire` -> `3306`
- `redis` -> `6379`
- `mongodb` -> `27017`
- `amqp` -> `5672`
- `kafka` -> `9092`
- `nats` -> `4222`
- `mqtt` -> `1883`, `8883`
- `ldap` -> `389`, `636`
- `sftp` -> `22`
- `smb` -> `445`

If `http_proxy` is missing, CONNECT auth now checks mapped protocol capabilities for the target port.

## Reasoning
- Delivers immediate policy granularity without waiting for full protocol adapter implementations.
- Preserves backward compatibility (`http_proxy` still works as before).
- Keeps rollout aligned across Go and Rust implementations.

## Consequences
- Protocol capabilities are currently port-scoped only (not deep protocol semantic enforcement yet).
- Further phases are needed for protocol-level limits, validation, and richer observability.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-162829-connect-port-capability-iteration.md](../../evolution/events/20260307-162829-connect-port-capability-iteration.md)
- Evidence (files/tests):
  - `go/cmd/janusd/main.go`
  - `go/cmd/janusd/main_test.go`
  - `src/main.rs`
  - `README.md`
  - `go/README.md`
  - `cd go && go test ./...`
  - `cargo test -q`
