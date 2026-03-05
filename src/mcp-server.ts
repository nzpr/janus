#!/usr/bin/env bun

import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "child_process";
import path from "path";
import { fileURLToPath } from "url";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod/v4";
import { printJanusMcpStartupBanner } from "./cli-banner";

type ClientScope = "container" | "host";

type McpServerConfig = {
  workspace: string;
  grantsPath?: string;
  clientScope: ClientScope;
  janusScriptPath: string;
  instancePrefix: string;
};

type ServeSnapshot = {
  instanceId: string;
  workspace: string;
  clientScope: ClientScope;
  activeGrantIds: string[];
  skipped: string[];
  env: Record<string, string>;
};

type JanusSession = {
  id: string;
  child: ChildProcessWithoutNullStreams;
  startedAt: string;
  snapshot: ServeSnapshot;
  stderrTail: string;
};

const maxStderrTailLength = 8192;
const sessionStartupTimeoutMs = 12_000;

function printHelp(): void {
  console.error(`Janus MCP Server

Usage:
  bun src/mcp-server.ts [options]

Options:
  --workspace <dir>       Default workspace passed to Janus tools (default: cwd)
  --grants <path>         Default grants file path
  --client <scope>        Default client scope: container|host (default: container)
  --janus-script <path>   Path to janus.ts (default: sibling src/janus.ts)
  --instance-prefix <id>  Prefix for generated session ids (default: "janus")
  -h, --help              Show this help

UI:
  JANUS_NO_BANNER=1 disables startup banner output
`);
}

function parseServerArgs(argv: string[]): McpServerConfig {
  const moduleDir = path.dirname(fileURLToPath(import.meta.url));
  const args = [...argv];
  const config: McpServerConfig = {
    workspace: process.cwd(),
    clientScope: "container",
    janusScriptPath: path.resolve(moduleDir, "janus.ts"),
    instancePrefix: "janus"
  };

  while (args.length > 0) {
    const arg = args.shift()!;
    if (arg === "-h" || arg === "--help") {
      printHelp();
      process.exit(0);
    }

    if (arg === "--workspace") {
      const value = args.shift();
      if (!value) {
        throw new Error("Missing value for --workspace");
      }
      config.workspace = path.resolve(value);
      continue;
    }

    if (arg.startsWith("--workspace=")) {
      config.workspace = path.resolve(arg.replace("--workspace=", ""));
      continue;
    }

    if (arg === "--grants") {
      const value = args.shift();
      if (!value) {
        throw new Error("Missing value for --grants");
      }
      config.grantsPath = value;
      continue;
    }

    if (arg.startsWith("--grants=")) {
      config.grantsPath = arg.replace("--grants=", "");
      continue;
    }

    if (arg === "--client") {
      const value = args.shift();
      if (value !== "container" && value !== "host") {
        throw new Error("Invalid value for --client. Use container or host.");
      }
      config.clientScope = value;
      continue;
    }

    if (arg.startsWith("--client=")) {
      const value = arg.replace("--client=", "");
      if (value !== "container" && value !== "host") {
        throw new Error("Invalid value for --client. Use container or host.");
      }
      config.clientScope = value;
      continue;
    }

    if (arg === "--janus-script") {
      const value = args.shift();
      if (!value) {
        throw new Error("Missing value for --janus-script");
      }
      config.janusScriptPath = path.resolve(value);
      continue;
    }

    if (arg.startsWith("--janus-script=")) {
      config.janusScriptPath = path.resolve(arg.replace("--janus-script=", ""));
      continue;
    }

    if (arg === "--instance-prefix") {
      const value = args.shift();
      if (!value) {
        throw new Error("Missing value for --instance-prefix");
      }
      config.instancePrefix = value.trim();
      continue;
    }

    if (arg.startsWith("--instance-prefix=")) {
      config.instancePrefix = arg.replace("--instance-prefix=", "").trim();
      continue;
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!config.instancePrefix) {
    config.instancePrefix = "janus";
  }
  return config;
}

function textResult(text: string, isError = false): { content: Array<{ type: "text"; text: string }>; isError?: boolean } {
  if (isError) {
    return { isError: true, content: [{ type: "text", text }] };
  }
  return { content: [{ type: "text", text }] };
}

function jsonResult(payload: unknown, isError = false): { content: Array<{ type: "text"; text: string }>; isError?: boolean } {
  return textResult(JSON.stringify(payload, null, 2), isError);
}

function buildJanusArgs(
  janusScriptPath: string,
  command: "plan" | "serve",
  options: { workspace: string; grantsPath?: string; clientScope: ClientScope; instanceId?: string }
): string[] {
  const args = ["run", janusScriptPath, command, "--workspace", options.workspace, "--client", options.clientScope];
  if (options.grantsPath) {
    args.push("--grants", options.grantsPath);
  }
  if (command === "serve" && options.instanceId) {
    args.push("--instance", options.instanceId);
  }
  return args;
}

function parseFirstJsonObject(input: string): { value?: unknown; consumed: number } {
  const start = input.indexOf("{");
  if (start < 0) {
    return { consumed: 0 };
  }

  let depth = 0;
  let inString = false;
  let escaped = false;

  for (let i = start; i < input.length; i += 1) {
    const ch = input[i];

    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (ch === "\\") {
        escaped = true;
      } else if (ch === "\"") {
        inString = false;
      }
      continue;
    }

    if (ch === "\"") {
      inString = true;
      continue;
    }

    if (ch === "{") {
      depth += 1;
      continue;
    }

    if (ch === "}") {
      depth -= 1;
      if (depth === 0) {
        const candidate = input.slice(start, i + 1);
        try {
          return {
            value: JSON.parse(candidate) as unknown,
            consumed: i + 1
          };
        } catch {
          return { consumed: 0 };
        }
      }
    }
  }

  return { consumed: 0 };
}

function normalizeStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((entry): entry is string => typeof entry === "string");
}

function normalizeStringRecord(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object") {
    return {};
  }
  const source = value as Record<string, unknown>;
  const output: Record<string, string> = {};
  for (const [key, entry] of Object.entries(source)) {
    if (typeof entry === "string") {
      output[key] = entry;
    }
  }
  return output;
}

function normalizeServeSnapshot(value: unknown): ServeSnapshot {
  if (!value || typeof value !== "object") {
    throw new Error("Invalid Janus serve output: expected JSON object");
  }
  const payload = value as Record<string, unknown>;
  const instanceId = typeof payload.instanceId === "string" ? payload.instanceId : "janus";
  const workspace = typeof payload.workspace === "string" ? payload.workspace : "";
  const clientScope = payload.clientScope === "host" ? "host" : "container";

  if (!workspace) {
    throw new Error("Invalid Janus serve output: missing workspace");
  }

  return {
    instanceId,
    workspace,
    clientScope,
    activeGrantIds: normalizeStringArray(payload.activeGrantIds),
    skipped: normalizeStringArray(payload.skipped),
    env: normalizeStringRecord(payload.env)
  };
}

async function waitForServeSnapshot(child: ChildProcessWithoutNullStreams): Promise<ServeSnapshot> {
  return await new Promise<ServeSnapshot>((resolve, reject) => {
    let stdoutBuffer = "";
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error(`Timed out waiting for Janus serve startup (${sessionStartupTimeoutMs}ms)`));
    }, sessionStartupTimeoutMs);

    const onStdout = (chunk: Buffer): void => {
      stdoutBuffer += chunk.toString("utf8");
      const parsed = parseFirstJsonObject(stdoutBuffer);
      if (!parsed.value) {
        return;
      }
      cleanup();
      try {
        resolve(normalizeServeSnapshot(parsed.value));
      } catch (error) {
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    };

    const onExit = (code: number | null, signal: NodeJS.Signals | null): void => {
      cleanup();
      reject(new Error(`Janus serve exited before startup completed (code=${String(code)}, signal=${String(signal)})`));
    };

    const cleanup = (): void => {
      clearTimeout(timer);
      child.stdout.off("data", onStdout);
      child.off("exit", onExit);
    };

    child.stdout.on("data", onStdout);
    child.once("exit", onExit);
  });
}

async function terminateChild(child: ChildProcessWithoutNullStreams): Promise<void> {
  if (child.exitCode !== null || child.killed) {
    return;
  }

  await new Promise<void>((resolve) => {
    const timer = setTimeout(() => {
      if (child.exitCode === null) {
        child.kill("SIGKILL");
      }
    }, 4_000);

    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });

    child.kill("SIGTERM");
  });
}

function runJanusPlan(
  config: McpServerConfig,
  options: { workspace: string; grantsPath?: string; clientScope: ClientScope }
): { ok: true; output: unknown } | { ok: false; error: string } {
  const args = buildJanusArgs(config.janusScriptPath, "plan", options);
  const result = spawnSync(process.execPath, args, {
    env: process.env,
    encoding: "utf8"
  });

  if (result.error) {
    return { ok: false, error: result.error.message };
  }
  if (result.status !== 0) {
    return { ok: false, error: (result.stderr || result.stdout || "").trim() || `janus plan exited with ${String(result.status)}` };
  }

  try {
    return { ok: true, output: JSON.parse(result.stdout) as unknown };
  } catch {
    return { ok: false, error: `Failed to parse Janus plan output as JSON: ${result.stdout}` };
  }
}

async function main(): Promise<void> {
  const config = parseServerArgs(process.argv.slice(2));
  const server = new McpServer({
    name: "janus-mcp-server",
    version: "0.1.0"
  });

  const sessions = new Map<string, JanusSession>();
  const registeredToolNames = [
    "janus_plan",
    "janus_session_start",
    "janus_session_list",
    "janus_session_get",
    "janus_session_stop"
  ];

  const planInputSchema = {
    workspace: z.string().optional(),
    grantsPath: z.string().optional(),
    clientScope: z.enum(["container", "host"]).optional()
  };

  const startInputSchema = {
    sessionId: z.string().optional(),
    instanceId: z.string().optional(),
    workspace: z.string().optional(),
    grantsPath: z.string().optional(),
    clientScope: z.enum(["container", "host"]).optional()
  };

  const stopInputSchema = {
    sessionId: z.string()
  };

  const getInputSchema = {
    sessionId: z.string()
  };

  server.registerTool(
    "janus_plan",
    {
      description: "Run Janus plan and return resolved grants, skipped reasons, and exported env keys.",
      inputSchema: planInputSchema
    },
    async (args) => {
      const workspace = args.workspace ? path.resolve(args.workspace) : config.workspace;
      const grantsPath = args.grantsPath || config.grantsPath;
      const clientScope = args.clientScope || config.clientScope;

      const result = runJanusPlan(config, {
        workspace,
        grantsPath,
        clientScope
      });

      if (!result.ok) {
        return textResult(result.error, true);
      }
      return jsonResult({
        workspace,
        grantsPath,
        clientScope,
        plan: result.output
      });
    }
  );

  server.registerTool(
    "janus_session_start",
    {
      description: "Start a long-lived Janus serve session and return the session snapshot + env bundle.",
      inputSchema: startInputSchema
    },
    async (args) => {
      const sessionId = args.sessionId?.trim() || `${config.instancePrefix}-${crypto.randomUUID()}`;
      if (sessions.has(sessionId)) {
        return textResult(`Session already exists: ${sessionId}`, true);
      }

      const workspace = args.workspace ? path.resolve(args.workspace) : config.workspace;
      const grantsPath = args.grantsPath || config.grantsPath;
      const clientScope = args.clientScope || config.clientScope;
      const instanceId = args.instanceId || sessionId;
      const janusArgs = buildJanusArgs(config.janusScriptPath, "serve", {
        workspace,
        grantsPath,
        clientScope,
        instanceId
      });

      const child = spawn(process.execPath, janusArgs, {
        env: process.env,
        stdio: ["ignore", "pipe", "pipe"]
      });

      let stderrTail = "";
      child.stderr.on("data", (chunk: Buffer) => {
        stderrTail += chunk.toString("utf8");
        if (stderrTail.length > maxStderrTailLength) {
          stderrTail = stderrTail.slice(-maxStderrTailLength);
        }
      });

      try {
        const snapshot = await waitForServeSnapshot(child);
        const session: JanusSession = {
          id: sessionId,
          child,
          startedAt: new Date().toISOString(),
          snapshot,
          stderrTail
        };
        sessions.set(sessionId, session);
        child.once("exit", () => {
          sessions.delete(sessionId);
        });

        return jsonResult({
          sessionId,
          pid: child.pid,
          startedAt: session.startedAt,
          snapshot
        });
      } catch (error) {
        await terminateChild(child);
        const message = error instanceof Error ? error.message : String(error);
        return jsonResult(
          {
            error: message,
            stderrTail
          },
          true
        );
      }
    }
  );

  server.registerTool("janus_session_list", {
    description: "List currently running Janus serve sessions managed by this MCP server."
  }, async () => {
    const payload = Array.from(sessions.values()).map((session) => ({
      sessionId: session.id,
      pid: session.child.pid,
      startedAt: session.startedAt,
      workspace: session.snapshot.workspace,
      clientScope: session.snapshot.clientScope,
      activeGrantIds: session.snapshot.activeGrantIds,
      skipped: session.snapshot.skipped
    }));

    return jsonResult({ sessions: payload });
  });

  server.registerTool(
    "janus_session_get",
    {
      description: "Get details (including exported env bundle) for one running Janus session.",
      inputSchema: getInputSchema
    },
    async (args) => {
      const session = sessions.get(args.sessionId);
      if (!session) {
        return textResult(`Unknown session: ${args.sessionId}`, true);
      }
      return jsonResult({
        sessionId: session.id,
        pid: session.child.pid,
        startedAt: session.startedAt,
        snapshot: session.snapshot,
        stderrTail: session.stderrTail
      });
    }
  );

  server.registerTool(
    "janus_session_stop",
    {
      description: "Stop one Janus serve session managed by this MCP server.",
      inputSchema: stopInputSchema
    },
    async (args) => {
      const session = sessions.get(args.sessionId);
      if (!session) {
        return textResult(`Unknown session: ${args.sessionId}`, true);
      }
      await terminateChild(session.child);
      sessions.delete(args.sessionId);
      return jsonResult({
        sessionId: args.sessionId,
        stopped: true
      });
    }
  );

  const stopAllSessions = async (): Promise<void> => {
    const running = Array.from(sessions.values());
    sessions.clear();
    for (const session of running) {
      try {
        await terminateChild(session.child);
      } catch {
        // best-effort cleanup
      }
    }
  };

  process.on("SIGINT", () => {
    void stopAllSessions().finally(() => {
      process.exit(0);
    });
  });
  process.on("SIGTERM", () => {
    void stopAllSessions().finally(() => {
      process.exit(0);
    });
  });

  const transport = new StdioServerTransport();
  await server.connect(transport);
  printJanusMcpStartupBanner({
    workspace: config.workspace,
    clientScope: config.clientScope,
    janusScriptPath: config.janusScriptPath,
    instancePrefix: config.instancePrefix,
    toolNames: registeredToolNames
  });
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
