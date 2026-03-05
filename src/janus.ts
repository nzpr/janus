#!/usr/bin/env bun

import { spawnSync } from "child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import http from "http";
import http2 from "http2";
import https from "https";
import os from "os";
import path from "path";

type SecretGrantTransport = "http" | "grpc" | "database" | "filesystem" | "ssh" | "custom";

type SecretGrant = {
  id: string;
  enabled?: boolean;
  provider?: "host_env";
  sourceEnv: string;
  sourceEnvFallbacks?: string[];
  transport: SecretGrantTransport;
  adapter: string;
  targetHost?: string;
  targetHostEnv?: string;
  targetPort?: number;
  targetPortEnv?: string;
  targetDatabase?: string;
  targetDatabaseEnv?: string;
  fileName?: string;
  fileMode?: string;
  outputEnv?: string;
  outputPrefix?: string;
  authScheme?: "bearer" | "token" | "basic";
  username?: string;
  usernameEnv?: string;
};

type SecretGrantFile = {
  version: number;
  grants: SecretGrant[];
};

type GitRewriteRule = {
  from: string;
  to: string;
};

type SecretBrokerRuntime = {
  env: Record<string, string>;
  activeGrantIds: string[];
  skipped: string[];
  stop: () => Promise<void>;
};

type GitHttpTarget = {
  host: string;
  rewritePrefixes: string[];
};

type HostPortTarget = {
  host: string;
  port: number;
  authority: string;
};

type Command = "run" | "plan" | "serve" | "help";
type ClientScope = "container" | "host";

type ParsedArgs = {
  command: Command;
  workspace: string;
  grantsPath?: string;
  clientScope: ClientScope;
  instanceId: string;
  commandArgs: string[];
};

const defaultSecretGrantsRelativePath = path.join(".janus", "secret-grants.json");
const legacySecretGrantsRelativePath = path.join(".jim", "secret-grants.json");

function printHelp(): void {
  console.log(`Janus Secret Broker

Usage:
  bun src/janus.ts run [options] -- <command...>
  bun src/janus.ts plan [options]
  bun src/janus.ts serve [options]
  bun src/janus.ts help

Options:
  --workspace <dir>   Workspace root to inspect remotes (default: cwd)
  --grants <path>     Secret grants file (default: .janus/secret-grants.json, fallback: .jim/secret-grants.json)
  --client <scope>    Rewrite target host scope: container|host (default: container)
  --instance <id>     Instance identifier for logs/metadata (default: $USER or "janus")
  --                  End option parsing and pass the rest to command

Host credentials:
  JANUS_GIT_HTTP_USERNAME
  JANUS_GIT_HTTP_PASSWORD (fallback: JANUS_GIT_HTTP_TOKEN)
  Optional allow-list override: JANUS_GIT_HTTP_HOSTS=host1,host2

Adapters:
  http/git_http_auth
  grpc/grpc_header_auth
  ssh/ssh_key_command (host client scope only)
  database/postgres_pgpass (host client scope only)
  filesystem/file_materialize (host client scope only)
`);
}

function parseArgs(argv: string[]): ParsedArgs {
  const args = [...argv];
  let command: Command = "run";
  if (
    args[0] === "run" ||
    args[0] === "plan" ||
    args[0] === "serve" ||
    args[0] === "help" ||
    args[0] === "--help" ||
    args[0] === "-h"
  ) {
    const first = args.shift()!;
    command = first === "--help" || first === "-h" ? "help" : (first as Command);
  }

  let workspace = process.cwd();
  let grantsPath: string | undefined;
  let clientScope: ClientScope = "container";
  let instanceId = process.env.USER?.trim() || "janus";
  const commandArgs: string[] = [];

  while (args.length > 0) {
    const arg = args.shift()!;
    if (arg === "--") {
      commandArgs.push(...args);
      break;
    }

    if (arg === "--workspace") {
      const value = args.shift();
      if (!value) {
        throw new Error("Missing value for --workspace");
      }
      workspace = path.resolve(value);
      continue;
    }

    if (arg.startsWith("--workspace=")) {
      workspace = path.resolve(arg.replace("--workspace=", ""));
      continue;
    }

    if (arg === "--grants") {
      const value = args.shift();
      if (!value) {
        throw new Error("Missing value for --grants");
      }
      grantsPath = value;
      continue;
    }

    if (arg.startsWith("--grants=")) {
      grantsPath = arg.replace("--grants=", "");
      continue;
    }

    if (arg === "--client") {
      const value = args.shift();
      if (value !== "container" && value !== "host") {
        throw new Error("Invalid value for --client. Use container or host.");
      }
      clientScope = value;
      continue;
    }

    if (arg === "--instance") {
      const value = args.shift();
      if (!value) {
        throw new Error("Missing value for --instance");
      }
      instanceId = value;
      continue;
    }

    if (arg.startsWith("--client=")) {
      const value = arg.replace("--client=", "");
      if (value !== "container" && value !== "host") {
        throw new Error("Invalid value for --client. Use container or host.");
      }
      clientScope = value;
      continue;
    }

    if (arg.startsWith("--instance=")) {
      instanceId = arg.replace("--instance=", "");
      continue;
    }

    commandArgs.push(arg);
  }

  return { command, workspace, grantsPath, clientScope, instanceId, commandArgs };
}

function getBrokerHost(scope: ClientScope): string {
  if (scope === "host") {
    return "127.0.0.1";
  }
  return process.platform === "linux" ? "172.17.0.1" : "host.docker.internal";
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .replace(/-+$/, "")
    .slice(0, 64);
}

function parseDelimitedValues(raw: string | undefined): string[] {
  if (!raw) {
    return [];
  }

  return raw
    .split(/[,\s]+/g)
    .map((value) => value.trim())
    .filter(Boolean);
}

function resolveEnvValue(primary: string | undefined, fallbacks: string[] = []): string | undefined {
  const candidates = [primary, ...fallbacks].filter((value): value is string => Boolean(value));
  for (const name of candidates) {
    const value = process.env[name]?.trim();
    if (value) {
      return value;
    }
  }
  return undefined;
}

function legacyJimFallbacks(envName: string | undefined): string[] {
  if (!envName) {
    return [];
  }
  if (envName.startsWith("JANUS_")) {
    return [envName.replace(/^JANUS_/, "JIM_")];
  }
  return [];
}

function envSafeSegment(value: string): string {
  const normalized = value
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, "_")
    .replace(/^_+/, "")
    .replace(/_+$/, "");
  return normalized || "GRANT";
}

function parseOctalMode(raw: string | undefined, fallback: number): number {
  if (!raw) {
    return fallback;
  }

  if (!/^[0-7]{3,4}$/.test(raw)) {
    throw new Error(`Invalid octal file mode: ${raw}`);
  }
  return Number.parseInt(raw, 8);
}

function createRuntimeTempDir(grantId: string, label: string): string {
  const safeGrant = slugify(grantId) || "grant";
  return mkdtempSync(path.join(os.tmpdir(), `janus-${safeGrant}-${label}-`));
}

function parseHostPortCandidate(candidate: string, defaultPort: number): HostPortTarget | undefined {
  const trimmed = candidate.trim();
  if (!trimmed) {
    return undefined;
  }

  const asUrl = trimmed.includes("://") ? trimmed : `scheme://${trimmed}`;
  try {
    const parsed = new URL(asUrl);
    if (!parsed.hostname) {
      return undefined;
    }
    const host = parsed.hostname.toLowerCase();
    const port = parsed.port ? Number(parsed.port) : defaultPort;
    if (!Number.isFinite(port) || port <= 0 || port > 65535) {
      return undefined;
    }
    const authority = parsed.port ? `${host}:${port}` : `${host}:${defaultPort}`;
    return { host, port, authority };
  } catch {
    return undefined;
  }
}

function resolvePortOverride(grant: SecretGrant, defaultPort: number): number {
  const raw = resolveEnvValue(grant.targetPortEnv, legacyJimFallbacks(grant.targetPortEnv));
  const candidate = raw?.trim() || (typeof grant.targetPort === "number" ? String(grant.targetPort) : "");
  if (!candidate) {
    return defaultPort;
  }

  const parsed = Number(candidate);
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 65535) {
    throw new Error(`Grant ${grant.id} has invalid target port: ${candidate}`);
  }
  return parsed;
}

function resolveExplicitTargets(grant: SecretGrant, defaultPort: number): HostPortTarget[] {
  const hostValues =
    grant.targetHost?.trim() ||
    resolveEnvValue(grant.targetHostEnv, legacyJimFallbacks(grant.targetHostEnv)) ||
    "";
  const hosts = parseDelimitedValues(hostValues)
    .map((value) => parseHostPortCandidate(value, defaultPort))
    .filter((value): value is HostPortTarget => Boolean(value));

  const deduped = new Map<string, HostPortTarget>();
  for (const host of hosts) {
    deduped.set(host.authority, host);
  }
  return Array.from(deduped.values());
}

function normalizeGitHostCandidate(candidate: string): string | undefined {
  const trimmed = candidate.trim();
  if (!trimmed) {
    return undefined;
  }

  const scpLike = /^[^@]+@([^:/]+)(?::\d+)?(?::|\/|$)/.exec(trimmed);
  if (scpLike) {
    return scpLike[1].toLowerCase();
  }

  if (trimmed.includes("://")) {
    try {
      const parsed = new URL(trimmed);
      if (!parsed.host) {
        return undefined;
      }
      return parsed.host.toLowerCase();
    } catch {
      return undefined;
    }
  }

  return trimmed.replace(/\/.*$/, "").toLowerCase();
}

function parseRemoteRewriteInfo(remoteUrl: string): GitHttpTarget | undefined {
  const scpLike = /^([^@]+)@([^:/]+):/.exec(remoteUrl);
  if (scpLike) {
    return {
      host: scpLike[2].toLowerCase(),
      rewritePrefixes: [`${scpLike[1]}@${scpLike[2]}:`]
    };
  }

  try {
    const parsed = new URL(remoteUrl);
    if (!parsed.host) {
      return undefined;
    }
    const host = parsed.host.toLowerCase();

    if (parsed.protocol === "http:" || parsed.protocol === "https:" || parsed.protocol === "git:") {
      return {
        host,
        rewritePrefixes: [`${parsed.protocol}//${parsed.host}/`]
      };
    }

    if (parsed.protocol === "ssh:") {
      const userPrefix = parsed.username ? `${parsed.username}@` : "";
      return {
        host,
        rewritePrefixes: [`ssh://${userPrefix}${parsed.host}/`]
      };
    }
  } catch {
    return undefined;
  }

  return undefined;
}

function discoverGitRemoteTargets(cwd: string): Map<string, Set<string>> {
  const result = spawnSync("git", ["remote", "-v"], { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    return new Map();
  }

  const discovered = new Map<string, Set<string>>();
  for (const line of result.stdout.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) {
      continue;
    }

    const columns = trimmed.split(/\s+/);
    if (columns.length < 2) {
      continue;
    }

    const parsed = parseRemoteRewriteInfo(columns[1]);
    if (!parsed) {
      continue;
    }

    const existing = discovered.get(parsed.host) ?? new Set<string>();
    for (const prefix of parsed.rewritePrefixes) {
      existing.add(prefix);
    }
    discovered.set(parsed.host, existing);
  }

  for (const [host, prefixes] of discovered.entries()) {
    prefixes.add(`https://${host}/`);
    prefixes.add(`http://${host}/`);
  }

  return discovered;
}

function buildAuthorizationHeader(secret: string, authScheme: SecretGrant["authScheme"], username: string): string {
  if (authScheme === "token") {
    return `token ${secret}`;
  }

  if (authScheme === "basic") {
    const encoded = Buffer.from(`${username}:${secret}`).toString("base64");
    return `Basic ${encoded}`;
  }

  return `Bearer ${secret}`;
}

function defaultSecretGrantFile(): SecretGrantFile {
  return {
    version: 1,
    grants: [
      {
        id: "default-git-http-auth",
        provider: "host_env",
        sourceEnv: "JANUS_GIT_HTTP_PASSWORD",
        sourceEnvFallbacks: ["JANUS_GIT_HTTP_TOKEN", "JIM_GIT_HTTP_PASSWORD", "JIM_GIT_HTTP_TOKEN"],
        transport: "http",
        adapter: "git_http_auth",
        targetHostEnv: "JANUS_GIT_HTTP_HOSTS",
        authScheme: "basic",
        usernameEnv: "JANUS_GIT_HTTP_USERNAME",
        enabled: true
      }
    ]
  };
}

function validateSecretGrantFile(input: unknown, sourcePath: string): SecretGrantFile {
  if (!input || typeof input !== "object") {
    throw new Error(`Invalid secret grants file: ${sourcePath}`);
  }

  const parsed = input as { version?: unknown; grants?: unknown };
  if (parsed.version !== 1) {
    throw new Error(`Unsupported secret grants version in ${sourcePath}: ${String(parsed.version)}`);
  }

  if (!Array.isArray(parsed.grants)) {
    throw new Error(`Invalid secret grants \"grants\" array in ${sourcePath}`);
  }

  const grants: SecretGrant[] = [];
  for (const grant of parsed.grants) {
    if (!grant || typeof grant !== "object") {
      throw new Error(`Invalid grant entry in ${sourcePath}`);
    }

    const entry = grant as SecretGrant;
    if (!entry.id || !entry.sourceEnv || !entry.transport || !entry.adapter) {
      throw new Error(`Grant entries require id/sourceEnv/transport/adapter in ${sourcePath}`);
    }
    grants.push(entry);
  }

  return {
    version: 1,
    grants
  };
}

function loadSecretGrantFile(cwd: string, overridePath?: string): SecretGrantFile {
  let configuredPath: string;
  if (overridePath) {
    configuredPath = path.resolve(cwd, overridePath);
  } else {
    const janusPath = path.join(cwd, defaultSecretGrantsRelativePath);
    const legacyPath = path.join(cwd, legacySecretGrantsRelativePath);
    configuredPath = existsSync(janusPath) ? janusPath : legacyPath;
  }

  if (!existsSync(configuredPath)) {
    return defaultSecretGrantFile();
  }

  const raw = readFileSync(configuredPath, "utf8");
  return validateSecretGrantFile(JSON.parse(raw) as unknown, configuredPath);
}

function resolveGrantSecret(grant: SecretGrant): { value?: string; source?: string } {
  const sources = [grant.sourceEnv, ...(grant.sourceEnvFallbacks ?? [])];
  for (const source of sources) {
    const value = process.env[source]?.trim();
    if (value) {
      return { value, source };
    }
  }
  return {};
}

function resolveGitHttpTargets(grant: SecretGrant, cwd: string): GitHttpTarget[] {
  const hostValues =
    grant.targetHost?.trim() ||
    resolveEnvValue(grant.targetHostEnv, legacyJimFallbacks(grant.targetHostEnv)) ||
    "";
  const explicitHostValues = parseDelimitedValues(
    hostValues
  )
    .map((value) => normalizeGitHostCandidate(value))
    .filter((value): value is string => Boolean(value));

  const discoveredTargets = discoverGitRemoteTargets(cwd);
  const targetHosts = explicitHostValues.length > 0 ? explicitHostValues : Array.from(discoveredTargets.keys());

  const targets: GitHttpTarget[] = [];
  for (const host of targetHosts) {
    const rewritePrefixes = new Set(discoveredTargets.get(host) ?? []);
    rewritePrefixes.add(`https://${host}/`);
    rewritePrefixes.add(`http://${host}/`);
    targets.push({ host, rewritePrefixes: Array.from(rewritePrefixes) });
  }

  return targets;
}

async function startGitHttpAuthAdapter(
  grant: SecretGrant,
  secret: string,
  targetHost: string,
  rewritePrefixes: string[],
  brokerHost: string
): Promise<{ rewriteRules: GitRewriteRule[]; stop: () => Promise<void> }> {
  const username = resolveGrantUsername(grant);
  if ((grant.authScheme ?? "basic") === "basic" && !username) {
    throw new Error(`Grant ${grant.id} requires usernameEnv/username for basic auth.`);
  }

  const authorizationValue = buildAuthorizationHeader(secret, grant.authScheme, username);
  const pathPrefix = `/git/${slugify(`${grant.id}-${targetHost}`)}`;
  let upstreamHostname = targetHost;
  let upstreamPort = 443;
  try {
    const parsedHost = new URL(`https://${targetHost}`);
    upstreamHostname = parsedHost.hostname;
    upstreamPort = parsedHost.port ? Number(parsedHost.port) : 443;
  } catch {
    upstreamHostname = targetHost;
    upstreamPort = 443;
  }

  const server = http.createServer((req, res) => {
    try {
      if (!req.url) {
        res.writeHead(400);
        res.end("missing request URL");
        return;
      }

      const incoming = new URL(req.url, "http://localhost");
      if (!incoming.pathname.startsWith(pathPrefix)) {
        res.writeHead(404);
        res.end("unknown broker route");
        return;
      }

      const suffixPath = incoming.pathname.slice(pathPrefix.length);
      const upstreamPath = `${suffixPath}${incoming.search}`;

      const upstreamHeaders: Record<string, string> = {};
      for (const [name, value] of Object.entries(req.headers)) {
        if (!value) {
          continue;
        }
        const lower = name.toLowerCase();
        if (lower === "host" || lower === "authorization" || lower === "content-length") {
          continue;
        }
        upstreamHeaders[name] = Array.isArray(value) ? value.join(",") : String(value);
      }

      upstreamHeaders.Authorization = authorizationValue;

      const upstreamReq = https.request(
        {
          protocol: "https:",
          hostname: upstreamHostname,
          port: upstreamPort,
          method: req.method || "GET",
          path: upstreamPath,
          headers: upstreamHeaders
        },
        (upstreamRes) => {
          res.writeHead(upstreamRes.statusCode ?? 502, upstreamRes.headers as http.OutgoingHttpHeaders);
          upstreamRes.pipe(res);
        }
      );

      upstreamReq.on("error", (error) => {
        res.writeHead(502);
        res.end(`upstream error: ${error.message}`);
      });

      req.pipe(upstreamReq);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      res.writeHead(500);
      res.end(`broker error: ${message}`);
    }
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve());
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    await new Promise<void>((resolve) => server.close(() => resolve()));
    throw new Error("Failed to determine broker listen address.");
  }

  const proxyBase = `http://${brokerHost}:${address.port}${pathPrefix}/`;
  return {
    rewriteRules: rewritePrefixes.map((prefix) => ({ from: prefix, to: proxyBase })),
    stop: async () =>
      await new Promise<void>((resolve, reject) => {
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve();
        });
      })
  };
}

function resolveGrantUsername(grant: SecretGrant): string {
  return resolveEnvValue(grant.usernameEnv, legacyJimFallbacks(grant.usernameEnv)) || grant.username || "";
}

async function startGrpcHeaderAuthAdapter(
  grant: SecretGrant,
  secret: string,
  target: HostPortTarget,
  brokerHost: string
): Promise<{ env: Record<string, string>; stop: () => Promise<void> }> {
  const username = resolveGrantUsername(grant);
  if ((grant.authScheme ?? "bearer") === "basic" && !username) {
    throw new Error(`Grant ${grant.id} requires usernameEnv/username for basic auth.`);
  }
  const authorizationValue = buildAuthorizationHeader(secret, grant.authScheme ?? "bearer", username);
  const upstreamAuthority = `${target.host}:${target.port}`;
  let upstreamSession: http2.ClientHttp2Session | undefined;

  const getUpstreamSession = (): http2.ClientHttp2Session => {
    if (!upstreamSession || upstreamSession.closed || upstreamSession.destroyed) {
      upstreamSession = http2.connect(`https://${upstreamAuthority}`);
      upstreamSession.on("error", () => {
        // upstream connection errors are surfaced on per-stream handlers.
      });
    }
    return upstreamSession;
  };

  const server = http2.createServer();
  server.on("stream", (downstream, headers) => {
    const upstreamHeaders: http2.OutgoingHttpHeaders = {};
    for (const [name, value] of Object.entries(headers)) {
      if (value === undefined) {
        continue;
      }
      const lower = name.toLowerCase();
      if (lower === ":authority" || lower === ":scheme" || lower === "authorization") {
        continue;
      }
      upstreamHeaders[name] = value;
    }
    upstreamHeaders[":authority"] = upstreamAuthority;
    upstreamHeaders[":scheme"] = "https";
    upstreamHeaders.authorization = authorizationValue;

    const upstream = getUpstreamSession().request(upstreamHeaders);

    upstream.on("response", (responseHeaders) => {
      try {
        downstream.respond(responseHeaders);
      } catch {
        downstream.close(http2.constants.NGHTTP2_INTERNAL_ERROR);
      }
    });

    upstream.on("trailers", (trailers) => {
      try {
        downstream.sendTrailers(trailers);
      } catch {
        // ignore trailer forwarding failures; stream data still completes.
      }
    });

    upstream.on("data", (chunk) => {
      downstream.write(chunk);
    });

    upstream.on("end", () => {
      downstream.end();
    });

    upstream.on("error", () => {
      downstream.close(http2.constants.NGHTTP2_INTERNAL_ERROR);
    });

    downstream.on("data", (chunk) => {
      upstream.write(chunk);
    });

    downstream.on("end", () => {
      upstream.end();
    });

    downstream.on("close", () => {
      if (!upstream.closed) {
        upstream.close();
      }
    });
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve());
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    server.close();
    if (upstreamSession && !upstreamSession.closed) {
      upstreamSession.close();
    }
    throw new Error("Failed to determine gRPC broker listen address.");
  }

  const segment = envSafeSegment(grant.outputPrefix || grant.id);
  const endpoint = `${brokerHost}:${address.port}`;
  const env: Record<string, string> = {
    [`JANUS_GRPC_ENDPOINT_${segment}`]: endpoint,
    [`JANUS_GRPC_TARGET_${segment}`]: upstreamAuthority
  };
  if (grant.outputEnv) {
    env[grant.outputEnv] = endpoint;
  }

  return {
    env,
    stop: async () => {
      if (upstreamSession && !upstreamSession.closed) {
        upstreamSession.close();
      }
      await new Promise<void>((resolve, reject) => {
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve();
        });
      });
    }
  };
}

async function startSshKeyCommandAdapter(
  grant: SecretGrant,
  secret: string,
  clientScope: ClientScope
): Promise<{ env: Record<string, string>; stop: () => Promise<void> } | { skip: string }> {
  if (clientScope !== "host") {
    return { skip: `${grant.id}:requires-client-host:ssh_key_command` };
  }

  const runtimeDir = createRuntimeTempDir(grant.id, "ssh");
  const keyPath = path.join(runtimeDir, "id_janus");
  const secretValue = secret.endsWith("\n") ? secret : `${secret}\n`;
  writeFileSync(keyPath, secretValue, { mode: 0o600 });
  chmodSync(keyPath, 0o600);

  const segment = envSafeSegment(grant.outputPrefix || grant.id);
  const sshCommand = `ssh -i "${keyPath}" -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new`;
  const env: Record<string, string> = {
    GIT_SSH_COMMAND: sshCommand,
    [`JANUS_SSH_KEY_PATH_${segment}`]: keyPath
  };
  if (grant.outputEnv) {
    env[grant.outputEnv] = keyPath;
  }

  return {
    env,
    stop: async () => {
      rmSync(runtimeDir, { recursive: true, force: true });
    }
  };
}

function escapePgPassToken(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/:/g, "\\:");
}

async function startPostgresPgpassAdapter(
  grant: SecretGrant,
  secret: string,
  clientScope: ClientScope
): Promise<{ env: Record<string, string>; activeGrantIds: string[]; stop: () => Promise<void> } | { skip: string }> {
  if (clientScope !== "host") {
    return { skip: `${grant.id}:requires-client-host:postgres_pgpass` };
  }

  const username = resolveGrantUsername(grant);
  if (!username) {
    throw new Error(`Grant ${grant.id} requires usernameEnv/username for postgres_pgpass.`);
  }

  const defaultPort = resolvePortOverride(grant, 5432);
  const targets = resolveExplicitTargets(grant, defaultPort);
  if (targets.length === 0) {
    return { skip: `${grant.id}:missing-target-host:${grant.targetHostEnv ?? "targetHost"}` };
  }

  const database =
    grant.targetDatabase?.trim() ||
    resolveEnvValue(grant.targetDatabaseEnv, legacyJimFallbacks(grant.targetDatabaseEnv)) ||
    "postgres";

  const runtimeDir = createRuntimeTempDir(grant.id, "pgpass");
  const pgpassPath = path.join(runtimeDir, ".pgpass");
  const pgLines = targets.map(
    (target) =>
      `${escapePgPassToken(target.host)}:${target.port}:${escapePgPassToken(database)}:${escapePgPassToken(username)}:${escapePgPassToken(secret)}`
  );
  writeFileSync(pgpassPath, `${pgLines.join("\n")}\n`, { mode: 0o600 });
  chmodSync(pgpassPath, 0o600);

  const primary = targets[0];
  const segment = envSafeSegment(grant.outputPrefix || grant.id);
  const env: Record<string, string> = {
    PGPASSFILE: pgpassPath,
    PGHOST: primary.host,
    PGPORT: String(primary.port),
    PGUSER: username,
    PGDATABASE: database,
    [`JANUS_POSTGRES_PGPASSFILE_${segment}`]: pgpassPath
  };
  if (grant.outputEnv) {
    env[grant.outputEnv] = pgpassPath;
  }

  return {
    env,
    activeGrantIds: targets.map((target) => `${grant.id}@${target.authority}`),
    stop: async () => {
      rmSync(runtimeDir, { recursive: true, force: true });
    }
  };
}

async function startFilesystemMaterializeAdapter(
  grant: SecretGrant,
  secret: string,
  clientScope: ClientScope
): Promise<{ env: Record<string, string>; stop: () => Promise<void> } | { skip: string }> {
  if (clientScope !== "host") {
    return { skip: `${grant.id}:requires-client-host:file_materialize` };
  }

  const runtimeDir = createRuntimeTempDir(grant.id, "file");
  const mode = parseOctalMode(grant.fileMode, 0o600);
  const fileName = grant.fileName?.trim() || `${slugify(grant.id) || "secret"}.secret`;
  const secretPath = path.join(runtimeDir, fileName);
  writeFileSync(secretPath, secret, { mode });
  chmodSync(secretPath, mode);

  const segment = envSafeSegment(grant.outputPrefix || grant.id);
  const envKey = grant.outputEnv || `JANUS_FILE_PATH_${segment}`;
  const env: Record<string, string> = {
    [envKey]: secretPath
  };

  return {
    env,
    stop: async () => {
      rmSync(runtimeDir, { recursive: true, force: true });
    }
  };
}

function mergeEnv(target: Record<string, string>, source: Record<string, string>): void {
  for (const [key, value] of Object.entries(source)) {
    target[key] = value;
  }
}

async function startSecretBroker(
  cwd: string,
  overridePath: string | undefined,
  clientScope: ClientScope
): Promise<SecretBrokerRuntime> {
  const grantsFile = loadSecretGrantFile(cwd, overridePath);
  const env: Record<string, string> = {};
  const stopHandlers: Array<() => Promise<void>> = [];
  const activeGrantIds: string[] = [];
  const skipped: string[] = [];
  const gitRewriteRules: GitRewriteRule[] = [];
  const brokerHost = getBrokerHost(clientScope);

  for (const grant of grantsFile.grants) {
    if (grant.enabled === false) {
      skipped.push(`${grant.id}:disabled`);
      continue;
    }

    if ((grant.provider ?? "host_env") !== "host_env") {
      skipped.push(`${grant.id}:unsupported-provider:${String(grant.provider)}`);
      continue;
    }

    const resolvedSecret = resolveGrantSecret(grant);
    if (!resolvedSecret.value) {
      const sources = [grant.sourceEnv, ...(grant.sourceEnvFallbacks ?? [])].join(",");
      skipped.push(`${grant.id}:missing-env:${sources}`);
      continue;
    }

    if (grant.transport === "http" && grant.adapter === "git_http_auth") {
      const targets = resolveGitHttpTargets(grant, cwd);
      if (targets.length === 0) {
        skipped.push(`${grant.id}:missing-target-host:${grant.targetHostEnv ?? "git-remotes"}`);
        continue;
      }

      for (const target of targets) {
        const adapter = await startGitHttpAuthAdapter(
          grant,
          resolvedSecret.value,
          target.host,
          target.rewritePrefixes,
          brokerHost
        );
        gitRewriteRules.push(...adapter.rewriteRules);
        stopHandlers.push(adapter.stop);
        activeGrantIds.push(`${grant.id}@${target.host}`);
      }
      continue;
    }

    if (grant.transport === "grpc" && grant.adapter === "grpc_header_auth") {
      const defaultPort = resolvePortOverride(grant, 443);
      const targets = resolveExplicitTargets(grant, defaultPort);
      if (targets.length === 0) {
        skipped.push(`${grant.id}:missing-target-host:${grant.targetHostEnv ?? "targetHost"}`);
        continue;
      }

      for (const target of targets) {
        const adapter = await startGrpcHeaderAuthAdapter(grant, resolvedSecret.value, target, brokerHost);
        mergeEnv(env, adapter.env);
        stopHandlers.push(adapter.stop);
        activeGrantIds.push(`${grant.id}@${target.authority}`);
      }
      continue;
    }

    if (grant.transport === "ssh" && grant.adapter === "ssh_key_command") {
      const adapter = await startSshKeyCommandAdapter(grant, resolvedSecret.value, clientScope);
      if ("skip" in adapter) {
        skipped.push(adapter.skip);
        continue;
      }
      mergeEnv(env, adapter.env);
      stopHandlers.push(adapter.stop);
      activeGrantIds.push(grant.id);
      continue;
    }

    if (grant.transport === "database" && grant.adapter === "postgres_pgpass") {
      const adapter = await startPostgresPgpassAdapter(grant, resolvedSecret.value, clientScope);
      if ("skip" in adapter) {
        skipped.push(adapter.skip);
        continue;
      }
      mergeEnv(env, adapter.env);
      stopHandlers.push(adapter.stop);
      activeGrantIds.push(...adapter.activeGrantIds);
      continue;
    }

    if (grant.transport === "filesystem" && grant.adapter === "file_materialize") {
      const adapter = await startFilesystemMaterializeAdapter(grant, resolvedSecret.value, clientScope);
      if ("skip" in adapter) {
        skipped.push(adapter.skip);
        continue;
      }
      mergeEnv(env, adapter.env);
      stopHandlers.push(adapter.stop);
      activeGrantIds.push(grant.id);
      continue;
    }

    skipped.push(`${grant.id}:unsupported-adapter:${grant.adapter}:${grant.transport}`);
  }

  if (gitRewriteRules.length > 0) {
    env.GIT_TERMINAL_PROMPT = "0";
    env.GIT_ASKPASS = "true";

    let configIndex = 0;
    for (const rule of gitRewriteRules) {
      env[`GIT_CONFIG_KEY_${configIndex}`] = `url.${rule.to}.insteadOf`;
      env[`GIT_CONFIG_VALUE_${configIndex}`] = rule.from;
      configIndex += 1;
    }
    env.GIT_CONFIG_COUNT = String(configIndex);
  }

  return {
    env,
    activeGrantIds,
    skipped,
    stop: async () => {
      for (const stop of stopHandlers.reverse()) {
        try {
          await stop();
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          console.error(`janus shutdown warning: ${message}`);
        }
      }
    }
  };
}

async function runPlan(parsed: ParsedArgs): Promise<void> {
  const broker = await startSecretBroker(parsed.workspace, parsed.grantsPath, parsed.clientScope);
  try {
    console.log(
      JSON.stringify(
        {
          instanceId: parsed.instanceId,
          workspace: parsed.workspace,
          clientScope: parsed.clientScope,
          activeGrantIds: broker.activeGrantIds,
          skipped: broker.skipped,
          envKeys: Object.keys(broker.env).sort()
        },
        null,
        2
      )
    );
  } finally {
    await broker.stop();
  }
}

async function runServe(parsed: ParsedArgs): Promise<void> {
  const broker = await startSecretBroker(parsed.workspace, parsed.grantsPath, parsed.clientScope);
  const keepAlive = setInterval(() => {
    // keep process alive even when no proxy listeners are active.
  }, 60_000);
  let shuttingDown = false;
  const onSigInt = (): void => {
    void shutdown("SIGINT");
  };
  const onSigTerm = (): void => {
    void shutdown("SIGTERM");
  };

  const shutdown = async (signal: string): Promise<void> => {
    if (shuttingDown) {
      return;
    }
    shuttingDown = true;
    console.error(`Janus shutting down (${signal})`);
    process.off("SIGINT", onSigInt);
    process.off("SIGTERM", onSigTerm);
    clearInterval(keepAlive);
    await broker.stop();
    process.exit(0);
  };

  process.on("SIGINT", onSigInt);
  process.on("SIGTERM", onSigTerm);

  console.log(
    JSON.stringify(
      {
        instanceId: parsed.instanceId,
        workspace: parsed.workspace,
        clientScope: parsed.clientScope,
        activeGrantIds: broker.activeGrantIds,
        skipped: broker.skipped,
        env: broker.env
      },
      null,
      2
    )
  );
  if (broker.activeGrantIds.length === 0) {
    console.error("Janus serve warning: no active grants; waiting for configuration updates/restart.");
  }
  await new Promise<void>(() => {
    // keep adapters and proxy listeners alive until signal.
  });
}

async function runExec(parsed: ParsedArgs): Promise<void> {
  const broker = await startSecretBroker(parsed.workspace, parsed.grantsPath, parsed.clientScope);
  if (broker.activeGrantIds.length > 0) {
    console.error(`Janus active grants: ${broker.activeGrantIds.join(", ")}`);
  }
  if (broker.skipped.length > 0) {
    console.error(`Janus skipped grants: ${broker.skipped.join(", ")}`);
  }

  if (parsed.commandArgs.length === 0) {
    console.log(
      JSON.stringify(
        {
          activeGrantIds: broker.activeGrantIds,
          skipped: broker.skipped,
          env: broker.env
        },
        null,
        2
      )
    );
    await broker.stop();
    return;
  }

  try {
    const [command, ...args] = parsed.commandArgs;
    const result = spawnSync(command, args, {
      cwd: parsed.workspace,
      stdio: "inherit",
      env: {
        ...process.env,
        ...broker.env
      }
    });

    if (result.error) {
      throw result.error;
    }

    if (typeof result.status === "number") {
      process.exit(result.status);
    }

    process.exit(1);
  } finally {
    await broker.stop();
  }
}

async function main(): Promise<void> {
  const parsed = parseArgs(process.argv.slice(2));
  if (parsed.command === "help") {
    printHelp();
    return;
  }

  if (parsed.command === "plan") {
    await runPlan(parsed);
    return;
  }

  if (parsed.command === "serve") {
    await runServe(parsed);
    return;
  }

  await runExec(parsed);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
