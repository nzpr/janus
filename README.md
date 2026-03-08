# Janus

Janus is a host-side broker for sandboxed LLM agents.

Primary goal:
- keep secrets out of the LLM jail,
- enforce scoped network access (capabilities + host allowlist + TTL),
- let normal developer tools (for example `psql`, `git`) work through Janus.

## What Runs Where

| Component | Trust level | Runs where | Purpose |
|---|---|---|---|
| `janusd` | trusted | host | control API + data-plane proxy/tunnel enforcement |
| `janus-mcp` | read-only | jail or host | capability/resource discovery for LLM |
| `janus-tunnel` | trusted sidecar | jail sidecar container | generic local TCP -> Janus CONNECT bridge |
| `janus-pg-sidecar` | trusted sidecar | jail sidecar container | PostgreSQL auth sidecar (LLM process has no DB password) |
| LLM agent | untrusted | jail container | code/tool execution only |

## Supported Protocols

| Capability | Ports | Usable from jailed LLM by |
|---|---|---|
| `http_proxy` | any HTTP(S) | proxy env (`HTTP_PROXY`/`HTTPS_PROXY`) |
| `git_http` | 443 | Git HTTP rewrite env from session |
| `git_ssh` | 22 | auto `GIT_SSH_COMMAND` from session |
| `postgres_wire` | 5432 | `janus-pg-sidecar` (preferred) or `janus-tunnel` |
| `mysql_wire` | 3306 | `janus-tunnel` |
| `redis` | 6379 | `janus-tunnel` |
| `mongodb` | 27017 | `janus-tunnel` |
| `amqp` | 5672 | `janus-tunnel` |
| `kafka` | 9092 | `janus-tunnel` |
| `nats` | 4222 | `janus-tunnel` |
| `mqtt` | 1883, 8883 | `janus-tunnel` |
| `ldap` | 389, 636 | `janus-tunnel` |
| `sftp` | 22 | `janus-tunnel` |
| `smb` | 445 | `janus-tunnel` |

## Quickstart (Jailed LLM)

### 1) Host: configure and start Janus

```bash
cd /workspace
cp .env.example .env
# edit .env: set host allowlist, capabilities, and host-side secrets
set -a
. ./.env
set +a
make start
make health
```

Required for jailed MCP discovery:
- set `JANUS_DISCOVERY_BIND` in `.env` (example `127.0.0.1:9181`).

### 2) Jail: start MCP companion

```bash
export JANUS_PUBLIC_BASE_URL=http://host.docker.internal:9181
janus-mcp
```

### 3) Host: create a session

```bash
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/sessions \
  -H 'content-type: application/json' \
  -d '{
    "ttl_seconds": 3600,
    "allowed_hosts": ["postgres.internal","github.com","codeberg.org"],
    "capabilities": ["git_http","git_ssh","postgres_wire"]
  }'
```

The response includes an `env` map (session-scoped runtime variables).

### 4) Inject session env into trusted sidecar/runtime, not into LLM process

Strict model (recommended):
- LLM process gets **no Janus token** and no DB password.
- Sidecar process gets session env and protocol secrets (if needed).

## Zero-Secret PostgreSQL Pattern

Use `janus-pg-sidecar` in trusted sidecar container/process.

### Sidecar startup

```bash
# trusted sidecar env
export JANUS_CONNECT_PROXY_URL='http://janus:<session-token>@127.0.0.1:9080'
export JANUS_PG_PASSWORD='replace-me'

janus-pg-sidecar \
  --target-host postgres.internal \
  --upstream-user app_user \
  --upstream-db app_db \
  --listen 127.0.0.1:15432
```

### LLM container usage (no DB password)

```bash
psql "host=127.0.0.1 port=15432 dbname=app_db user=app_user"
```

## Generic Wire Protocol Pattern

For non-Postgres wire protocols, run `janus-tunnel` in trusted sidecar.

```bash
export JANUS_CONNECT_PROXY_URL='http://janus:<session-token>@127.0.0.1:9080'
janus-tunnel --protocol redis --target-host redis.internal --listen 127.0.0.1:16379
```

Then point client to local sidecar endpoint (`127.0.0.1:16379`).

## Control/Data/Discovery Interfaces

| Plane | Interface | Default |
|---|---|---|
| control | Unix socket | `/tmp/janusd-control.sock` |
| discovery | HTTP (optional) | disabled unless `JANUS_DISCOVERY_BIND` set |
| data | HTTP proxy + CONNECT | `127.0.0.1:9080` |

## Session Env Behavior

Session env returned by `POST /v1/sessions` includes:
- `JANUS_CONNECT_PROXY_URL` when any CONNECT-capable protocol is granted.
- `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` only when `http_proxy` is granted.
- `GIT_SSH_COMMAND` when `git_ssh` is granted.
- Git HTTP rewrite keys when `git_http` is granted.

## Docker Build/Deploy

```bash
cp .env.docker.example .env
PROXY_PORT=9080 DISCOVERY_PORT=9181 make deploy
make logs
make health
```

Stop:

```bash
make stop
```

## Security Notes

- Never mount host control socket into untrusted LLM container.
- Keep control API host-only.
- Keep session TTL short.
- Scope `capabilities` and `allowed_hosts` narrowly.
- Put protocol secrets (for example `JANUS_PG_PASSWORD`) only in trusted sidecar, not LLM process.

## MCP Example Config

```json
{
  "mcpServers": {
    "janus": {
      "command": "janus-mcp",
      "args": [],
      "env": {
        "JANUS_PUBLIC_BASE_URL": "http://host.docker.internal:9181"
      }
    }
  }
}
```

## Environment Files

- `.env.example`: host run baseline (now includes all-protocol capability example).
- `.env.docker.example`: Docker deployment baseline.

## License

MIT. See [LICENSE](./LICENSE).
