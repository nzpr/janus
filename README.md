# Janus

Janus is a **host-side MCP server** for secret-brokered protocol access.
Published repository: `https://github.com/nzpr/janus`

If your LLM runs in a sandbox, run Janus on the host and connect via MCP tools. You typically do **not** start `janus serve` manually.

## MCP-First Setup

1. Clone the published repository on the host:

```bash
git clone https://github.com/nzpr/janus /opt/janus
cd /opt/janus
```

2. Install dependencies:

```bash
bun install
```

3. Export host secrets (no grants file required):

```bash
export JANUS_GIT_HTTP_PASSWORD=your-token-or-password
# optional overrides:
# export JANUS_GIT_HTTP_USERNAME=your-github-username
# export JANUS_GIT_HTTP_HOSTS=github.com
```

By default Janus uses a built-in Git HTTP auth grant with username `x-access-token`.

4. Add Janus as MCP server in your client config:

```json
{
  "mcpServers": {
    "janus": {
      "command": "bun",
      "args": [
        "run",
        "/opt/janus/src/mcp-server.ts"
      ]
    }
  }
}
```

That is the main integration path.

## MCP Tools

- `janus_plan`
- `janus_session_start`
- `janus_session_list`
- `janus_session_get`
- `janus_session_stop`

Recommended usage flow:
1. Call `janus_plan`
2. Call `janus_session_start`
3. Use `janus_session_get` or `janus_session_list` to inspect
4. Call `janus_session_stop` when done

## One Make Command

For manual host startup/debugging:

```bash
make start
```

This runs:

```bash
bun run src/mcp-server.ts
```

## Why It Is Safer

- Secrets stay on host env/runtime, not in prompt/tool args.
- Janus injects auth at runtime/proxy boundaries.
- MCP responses return session/connection metadata, not raw secret values.
- File materialization adapters use restrictive file modes and cleanup on shutdown.

Disable startup banners when needed:

```bash
export JANUS_NO_BANNER=1
```

## Notes

- Convention mode works without `.janus/secret-grants.json`; that file is only for advanced custom policy.
- Legacy fallbacks supported: `.jim/secret-grants.json` and corresponding `JIM_*` env names.
- Standalone non-MCP runtime (`src/janus.ts`) is available for local debugging, but MCP mode is the intended agent path.

## License And Warranty

Licensed under MIT. See [LICENSE](./LICENSE).

This software is provided **"AS IS"**, without warranty of any kind, express or implied.
