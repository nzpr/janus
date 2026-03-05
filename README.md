# Janus

Janus is a host-side secret broker and proxy for agent workloads.

It lets LLM agents use authenticated protocols (Git HTTP, gRPC, SSH, Postgres, file-based secrets) without putting raw secrets directly in agent prompts or tool payloads.

## Quick Start

1. Install dependencies:

```bash
bun install
```

2. Configure grants (default path: `.janus/secret-grants.json`):

```bash
mkdir -p .janus
cat > .janus/secret-grants.json <<'JSON'
{
  "version": 1,
  "grants": [
    {
      "id": "default-git-http-auth",
      "provider": "host_env",
      "sourceEnv": "JANUS_GIT_HTTP_PASSWORD",
      "sourceEnvFallbacks": ["JANUS_GIT_HTTP_TOKEN"],
      "transport": "http",
      "adapter": "git_http_auth",
      "targetHostEnv": "JANUS_GIT_HTTP_HOSTS",
      "authScheme": "basic",
      "usernameEnv": "JANUS_GIT_HTTP_USERNAME",
      "enabled": true
    }
  ]
}
JSON
```

3. Provide secrets on host:

```bash
export JANUS_GIT_HTTP_USERNAME=your-bot-user
export JANUS_GIT_HTTP_PASSWORD=your-token-or-password
export JANUS_GIT_HTTP_HOSTS=github.com,gitlab.com
```

4. Check broker plan:

```bash
bun run src/janus.ts plan
```

5. Start Janus proxy service:

```bash
bun run src/janus.ts serve --instance "$USER"
```

## Provide Secrets

Janus reads secrets from host environment variables specified by each grant.

- Default grant uses `JANUS_GIT_HTTP_USERNAME`, `JANUS_GIT_HTTP_PASSWORD` (or `JANUS_GIT_HTTP_TOKEN`).
- You can define additional grants in `.janus/secret-grants.json` for:
  - `grpc/grpc_header_auth`
  - `ssh/ssh_key_command`
  - `database/postgres_pgpass`
  - `filesystem/file_materialize`

Legacy fallbacks are supported during migration:
- `.jim/secret-grants.json` path fallback
- `JIM_*` env fallback for corresponding `JANUS_*` names

## Why This Is Safer

Janus is safer than passing credentials directly into agent contexts because:

- Secrets stay in host env / host runtime, not in MCP tool arguments.
- Protocol adapters inject auth at proxy/runtime boundaries.
- Host file materialization adapters use restrictive file modes (`0600`) and cleanup on shutdown.
- MCP returns connection/session metadata, not raw secret values.

Important caveat:
- Janus reduces exposure but does not replace host hardening. Protect host env, process access, logs, and filesystem permissions.

## Run As MCP Server

Start MCP server (stdio):

```bash
bun run src/mcp-server.ts --workspace "$PWD" --client host
```

Available MCP tools:
- `janus_plan`
- `janus_session_start`
- `janus_session_list`
- `janus_session_get`
- `janus_session_stop`

### MCP Client Config Example

Use absolute paths:

```json
{
  "mcpServers": {
    "janus": {
      "command": "bun",
      "args": [
        "run",
        "/ABS/PATH/TO/janus/src/mcp-server.ts",
        "--workspace",
        "/ABS/PATH/TO/workspace",
        "--client",
        "host"
      ]
    }
  }
}
```

If your MCP client supports env injection, set required `JANUS_*` vars there.

## Useful Commands

```bash
bun run src/janus.ts help
bun run src/janus.ts plan
bun run src/janus.ts run -- <command...>
bun run src/janus.ts serve --instance <id>
bun run src/mcp-server.ts --help
```

or

```bash
make help
make plan
make run CMD="git ls-remote origin"
make serve INSTANCE="$USER"
make mcp
```

## License And Warranty

This project is licensed under the MIT License.

This software is provided **"AS IS"**, without warranty of any kind, express or implied, including but not limited to merchantability, fitness for a particular purpose, and noninfringement. See [LICENSE](./LICENSE) for full terms.
