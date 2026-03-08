# Janus

Janus is a host-side broker for sandboxed LLM agents.

What Janus does:
- keeps control and policy on host,
- issues short-lived scoped sessions,
- enforces egress through proxy/tunnels,
- exposes read-only discovery to LLM via MCP.

## Runtime Model

| Component | Trust | Location | Role |
|---|---|---|---|
| `janusd` | trusted | host | control API + data-plane enforcement |
| `janus-mcp` | read-only | jail or host | capability/resource discovery |
| `janus-pg-sidecar` | trusted | sidecar | Postgres auth bridge (no DB secret in LLM process) |
| `janus-tunnel` | trusted | sidecar | generic CONNECT bridge for wire protocols |
| LLM process | untrusted | jail | normal tooling/code execution |

## Protocol Status

| Capability | Ports | Ready path |
|---|---|---|
| `http_proxy` | any HTTP(S) | direct (`HTTP_PROXY`/`HTTPS_PROXY`) |
| `git_http` | 443 | direct (session git rewrite env) |
| `git_ssh` | 22 | direct (`GIT_SSH_COMMAND` from session) |
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

## Fast Start (Jailed LLM)

### 1) Host: start Janus

```bash
cd /workspace
cp .env.example .env
# edit .env: allowlist/capabilities/secrets
set -a
. ./.env
set +a
make start
make health
```

Required for jailed MCP:
- set `JANUS_DISCOVERY_BIND` in `.env` (example `127.0.0.1:9181`).

### 2) Jail: start MCP

```bash
export JANUS_PUBLIC_BASE_URL=http://host.docker.internal:9181
janus-mcp
```

### 3) Host: create scoped session

```bash
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/sessions \
  -H 'content-type: application/json' \
  -d '{
    "ttl_seconds": 3600,
    "allowed_hosts": ["postgres.internal","github.com"],
    "capabilities": ["git_http","git_ssh","postgres_wire"]
  }'
```

### 4) Inject returned session env into trusted sidecar/runtime

Recommended strict model:
- LLM process has no Janus token and no protocol secrets.
- Trusted sidecar has session env and protocol secrets.

## PostgreSQL (Zero Secret in LLM Process)

### Sidecar

```bash
export JANUS_CONNECT_PROXY_URL='http://janus:<session-token>@127.0.0.1:9080'
export JANUS_PG_PASSWORD='replace-me'

janus-pg-sidecar \
  --target-host postgres.internal \
  --upstream-user app_user \
  --upstream-db app_db \
  --listen 127.0.0.1:15432
```

### LLM process

```bash
psql "host=127.0.0.1 port=15432 dbname=app_db user=app_user"
```

## Generic Wire Protocol Pattern

```bash
export JANUS_CONNECT_PROXY_URL='http://janus:<session-token>@127.0.0.1:9080'
janus-tunnel --protocol redis --target-host redis.internal --listen 127.0.0.1:16379
```

Then point client to the local endpoint (`127.0.0.1:16379`).

## Interfaces

| Plane | Interface | Default |
|---|---|---|
| control | Unix socket | `/tmp/janusd-control.sock` |
| discovery | HTTP (optional) | disabled unless `JANUS_DISCOVERY_BIND` |
| data | HTTP proxy + CONNECT | `127.0.0.1:9080` |

## Session Env Notes

`POST /v1/sessions` may return:
- `JANUS_CONNECT_PROXY_URL` (CONNECT-capable sessions)
- `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` (`http_proxy` capability)
- `GIT_SSH_COMMAND` (`git_ssh` capability)
- Git HTTP rewrite keys (`git_http` capability)

## Docker

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

## Security Checklist

- do not mount host control socket into untrusted LLM container,
- keep control API host-only,
- keep sessions short-lived,
- keep `capabilities` and `allowed_hosts` narrow,
- keep protocol secrets in trusted sidecar only.

## MCP Config Example

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

## Files

- `.env.example`: host baseline.
- `.env.docker.example`: Docker baseline.

## License

MIT. See [LICENSE](./LICENSE).
