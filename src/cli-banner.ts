type Rgb = {
  r: number;
  g: number;
  b: number;
};

type JanusServeBannerInput = {
  instanceId: string;
  workspace: string;
  clientScope: "container" | "host";
  activeGrantIds: string[];
  skipped: string[];
};

type JanusMcpBannerInput = {
  mcpServerPath: string;
  toolNames: string[];
};

const ansiReset = "\u001b[0m";

function shouldShowBanner(): boolean {
  return process.env.JANUS_NO_BANNER !== "1";
}

function supportsColor(): boolean {
  if (process.env.NO_COLOR) {
    return false;
  }
  if (process.env.FORCE_COLOR === "0") {
    return false;
  }
  return Boolean(process.stderr.isTTY);
}

function style(text: string, code: string, enabled: boolean): string {
  if (!enabled) {
    return text;
  }
  return `\u001b[${code}m${text}${ansiReset}`;
}

function colorRgb(text: string, rgb: Rgb, enabled: boolean): string {
  if (!enabled) {
    return text;
  }
  return `\u001b[38;2;${rgb.r};${rgb.g};${rgb.b}m${text}${ansiReset}`;
}

function gradient(text: string, from: Rgb, to: Rgb, enabled: boolean): string {
  if (!enabled || text.length === 0) {
    return text;
  }

  const chars = [...text];
  if (chars.length === 1) {
    return colorRgb(chars[0], from, enabled);
  }

  return chars
    .map((char, index) => {
      const ratio = index / (chars.length - 1);
      const rgb: Rgb = {
        r: Math.round(from.r + (to.r - from.r) * ratio),
        g: Math.round(from.g + (to.g - from.g) * ratio),
        b: Math.round(from.b + (to.b - from.b) * ratio)
      };
      return colorRgb(char, rgb, enabled);
    })
    .join("");
}

function stripAnsi(input: string): string {
  return input.replace(/\u001b\[[0-9;]*m/g, "");
}

function padRightAnsi(input: string, width: number): string {
  const visible = stripAnsi(input).length;
  const padding = Math.max(0, width - visible);
  return `${input}${" ".repeat(padding)}`;
}

function renderBox(lines: string[], enabled: boolean): string {
  const width = Math.max(...lines.map((line) => stripAnsi(line).length), 24);
  const border = style(`+${"-".repeat(width + 2)}+`, "90", enabled);
  const body = lines.map((line) => `${style("|", "90", enabled)} ${padRightAnsi(line, width)} ${style("|", "90", enabled)}`);
  return [border, ...body, border].join("\n");
}

function renderTitle(enabled: boolean, mode: string): string {
  const word = style("JANUS", "1;97", enabled);
  const top = gradient("      _  __    _    _   _ _   _ ____", { r: 0, g: 255, b: 180 }, { r: 0, g: 170, b: 255 }, enabled);
  const mid = gradient("     | |/ /   / \\  | \\ | | | | / ___|", { r: 0, g: 255, b: 180 }, { r: 0, g: 170, b: 255 }, enabled);
  const low = gradient("  _  | ' /   / _ \\ |  \\| | | | \\___ \\", { r: 0, g: 255, b: 180 }, { r: 0, g: 170, b: 255 }, enabled);
  const bot = gradient(" | |_| . \\  / ___ \\| |\\  | |_| |___) |", { r: 0, g: 255, b: 180 }, { r: 0, g: 170, b: 255 }, enabled);
  const fin = gradient("  \\___/ \\_\\/_/   \\_\\_| \\_|\\___/|____/ ", { r: 0, g: 255, b: 180 }, { r: 0, g: 170, b: 255 }, enabled);
  const modeLine = style(`[${mode}]`, "1;36", enabled);
  return [word, top, mid, low, bot, fin, modeLine].join("\n");
}

function renderSectionHeader(label: string, enabled: boolean): string {
  return style(label, "1;97", enabled);
}

export function printJanusServeStartupBanner(input: JanusServeBannerInput): void {
  if (!shouldShowBanner()) {
    return;
  }
  const color = supportsColor();
  const okColor = (value: string): string => style(value, "1;32", color);
  const warnColor = (value: string): string => style(value, "1;33", color);
  const dim = (value: string): string => style(value, "2", color);

  const lines: string[] = [
    renderSectionHeader("status", color) + `  ${okColor("proxy service online")}`,
    `instance: ${input.instanceId}`,
    `workspace: ${input.workspace}`,
    `scope: ${input.clientScope}`,
    `active grants: ${okColor(String(input.activeGrantIds.length))}`,
    `skipped grants: ${input.skipped.length > 0 ? warnColor(String(input.skipped.length)) : okColor("0")}`,
    "",
    renderSectionHeader("what started", color),
    "- Janus transport adapters are live for this process.",
    "- Env bundle was emitted on stdout for caller integration.",
    "",
    renderSectionHeader("quick use", color),
    "- Inspect plan: bun run src/janus.ts plan",
    "- Run command via broker: bun run src/janus.ts run -- <command...>",
    "- Stop service: Ctrl+C",
    "",
    dim("set JANUS_NO_BANNER=1 to disable this startup banner")
  ];

  process.stderr.write(`${renderTitle(color, "HOST PROXY SERVICE")}\n${renderBox(lines, color)}\n`);
}

export function printJanusMcpStartupBanner(input: JanusMcpBannerInput): void {
  if (!shouldShowBanner()) {
    return;
  }
  const color = supportsColor();
  const okColor = (value: string): string => style(value, "1;32", color);
  const dim = (value: string): string => style(value, "2", color);

  const configJsonLines = JSON.stringify(
    {
      mcpServers: {
        janus: {
          command: "bun",
          args: ["run", input.mcpServerPath]
        }
      }
    },
    null,
    2
  ).split("\n");

  const lines: string[] = [
    renderSectionHeader("status", color) + `  ${okColor("mcp server ready (stdio)")}`,
    renderSectionHeader("registered tools", color),
    ...input.toolNames.map((tool) => `- ${tool}`),
    "",
    renderSectionHeader("defaults", color),
    "- grants path: .janus/secret-grants.json",
    "- legacy grants fallback: .jim/secret-grants.json",
    "- git user env: JANUS_GIT_HTTP_USERNAME",
    "- git secret env: JANUS_GIT_HTTP_PASSWORD (fallback JANUS_GIT_HTTP_TOKEN)",
    "- workspace source: current working directory where server is started",
    "- client mode: host",
    "",
    renderSectionHeader("quick use", color),
    "- This process IS the MCP server (host-side).",
    "- Put this JSON into Claude/Codex MCP config:",
    ...configJsonLines,
    "- Normal flow: janus_plan -> janus_session_start.",
    "- No separate manual janus serve start is required.",
    "",
    dim("set JANUS_NO_BANNER=1 to disable this startup banner")
  ];

  process.stderr.write(`${renderTitle(color, "MCP CONTROL PLANE")}\n${renderBox(lines, color)}\n`);
}
