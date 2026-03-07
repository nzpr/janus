use std::collections::HashSet;
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use reqwest::blocking::Client;
use serde_json::{json, Value};

#[derive(Parser, Debug)]
#[command(
    name = "janus-mcp",
    version,
    about = "Read-only MCP server for Janus capability discovery",
    long_about = "janus-mcp is a host-side MCP companion for sandboxed LLMs.\n\
It provides safe metadata only (health, capabilities, policy summary) and never exposes host secrets, session tokens, or control-socket paths."
)]
struct Cli {
    #[arg(
        long,
        default_value = "/tmp/janusd-control.sock",
        help = "Path to Janus control socket"
    )]
    control_socket: PathBuf,
}

struct App {
    client: Client,
}

#[derive(Clone, Copy)]
struct ProtocolSpec {
    capability: &'static str,
    ports: &'static [u16],
}

const PROTOCOL_CATALOG: [ProtocolSpec; 14] = [
    ProtocolSpec {
        capability: "http_proxy",
        ports: &[],
    },
    ProtocolSpec {
        capability: "git_http",
        ports: &[],
    },
    ProtocolSpec {
        capability: "git_ssh",
        ports: &[22],
    },
    ProtocolSpec {
        capability: "postgres_wire",
        ports: &[5432],
    },
    ProtocolSpec {
        capability: "mysql_wire",
        ports: &[3306],
    },
    ProtocolSpec {
        capability: "redis",
        ports: &[6379],
    },
    ProtocolSpec {
        capability: "mongodb",
        ports: &[27017],
    },
    ProtocolSpec {
        capability: "amqp",
        ports: &[5672],
    },
    ProtocolSpec {
        capability: "kafka",
        ports: &[9092],
    },
    ProtocolSpec {
        capability: "nats",
        ports: &[4222],
    },
    ProtocolSpec {
        capability: "mqtt",
        ports: &[1883, 8883],
    },
    ProtocolSpec {
        capability: "ldap",
        ports: &[389, 636],
    },
    ProtocolSpec {
        capability: "sftp",
        ports: &[22],
    },
    ProtocolSpec {
        capability: "smb",
        ports: &[445],
    },
];

const RESOURCE_CATALOG: [&str; 4] = [
    "postgres_query",
    "deploy_kubectl",
    "deploy_helm",
    "deploy_terraform",
];

fn main() -> Result<()> {
    let cli = Cli::parse();

    let control_socket = env::var("JANUS_CONTROL_SOCKET")
        .ok()
        .map(PathBuf::from)
        .unwrap_or(cli.control_socket);

    let client = Client::builder()
        .unix_socket(control_socket.clone())
        .build()
        .context("failed to build unix-socket HTTP client")?;

    let app = App { client };

    run_stdio_server(app)
}

fn run_stdio_server(app: App) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    loop {
        let message = match read_message(&mut reader)? {
            Some(value) => value,
            None => break,
        };

        if let Some(response) = handle_message(&app, &message) {
            write_message(&mut writer, &response)?;
        }
    }

    Ok(())
}

fn handle_message(app: &App, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(|v| v.as_str())?;

    if id.is_none() {
        return None;
    }

    let id = id.unwrap_or(Value::Null);
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));

    let response = match method {
        "initialize" => Ok(handle_initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(handle_tools_list()),
        "tools/call" => handle_tools_call(app, &params),
        "resources/list" => Ok(handle_resources_list()),
        "resources/read" => handle_resources_read(app, &params),
        "prompts/list" => Ok(json!({"prompts": []})),
        _ => Err(anyhow!("method not found: {method}")),
    };

    Some(match response {
        Ok(result) => json!({"jsonrpc":"2.0", "id": id, "result": result}),
        Err(error) => json!({
            "jsonrpc":"2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": error.to_string()
            }
        }),
    })
}

fn handle_initialize(params: &Value) -> Value {
    let client_protocol = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("2025-03-26");

    json!({
        "protocolVersion": client_protocol,
        "capabilities": {
            "tools": {
                "listChanged": false
            },
            "resources": {
                "listChanged": false,
                "subscribe": false
            },
            "prompts": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "janus-mcp",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Read-only Janus metadata MCP. Discovery uses only janusd public APIs (/health, /v1/config). janusd must be started externally."
    })
}

fn handle_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "janus.health",
                "description": "Return Janus daemon health status.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            },
            {
                "name": "janus.capabilities",
                "description": "Return safe Janus capability and policy summary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            },
            {
                "name": "janus.safety",
                "description": "Explain Janus secret-isolation model and constraints.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            },
            {
                "name": "janus.discovery",
                "description": "Return protocol/resource availability and gaps using Janus public discovery APIs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        ]
    })
}

fn handle_resources_list() -> Value {
    json!({
        "resources": [
            {
                "uri": "janus://discovery/protocols",
                "name": "Janus Protocol Availability",
                "description": "Protocol capabilities available and unavailable on this Janus server.",
                "mimeType": "application/json"
            },
            {
                "uri": "janus://discovery/resources",
                "name": "Janus Resource Availability",
                "description": "Typed adapters/capabilities available and unavailable on this Janus server.",
                "mimeType": "application/json"
            },
            {
                "uri": "janus://discovery/summary",
                "name": "Janus Discovery Summary",
                "description": "Combined protocol/resource/discovery summary for agent planning.",
                "mimeType": "application/json"
            }
        ]
    })
}

fn handle_tools_call(app: &App, params: &Value) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("tools/call requires name"))?;

    let payload = match name {
        "janus.health" => tool_janus_health(app)?,
        "janus.capabilities" => tool_janus_capabilities(app)?,
        "janus.discovery" => tool_janus_discovery(app)?,
        "janus.safety" => tool_janus_safety(),
        _ => return Err(anyhow!("unknown tool: {name}")),
    };

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&payload)?
            }
        ],
        "structuredContent": payload
    }))
}

fn handle_resources_read(app: &App, params: &Value) -> Result<Value> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("resources/read requires uri"))?;

    let discovery = read_discovery(app)?;
    let payload = match uri {
        "janus://discovery/protocols" => discovery
            .get("protocols")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "janus://discovery/resources" => discovery
            .get("resources")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "janus://discovery/summary" => discovery,
        _ => return Err(anyhow!("unknown resource uri: {uri}")),
    };

    Ok(json!({
        "contents": [
            {
                "uri": uri,
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&payload)?
            }
        ]
    }))
}

fn tool_janus_health(app: &App) -> Result<Value> {
    let raw = read_control_json(app, "/health")?;
    Ok(json!({
        "status": raw.get("status").cloned().unwrap_or(Value::String("unknown".to_string())),
        "uptimeSeconds": raw.get("uptimeSeconds").cloned().unwrap_or(Value::Null)
    }))
}

fn tool_janus_capabilities(app: &App) -> Result<Value> {
    let raw = read_control_json(app, "/v1/config")?;

    Ok(json!({
        "proxyBind": raw.get("proxyBind").cloned().unwrap_or(Value::Null),
        "defaultTtlSeconds": raw.get("defaultTtlSeconds").cloned().unwrap_or(Value::Null),
        "defaultCapabilities": raw.get("defaultCapabilities").cloned().unwrap_or(json!([])),
        "knownCapabilities": raw.get("knownCapabilities").cloned().unwrap_or(json!([])),
        "supports": raw.get("supports").cloned().unwrap_or(json!({})),
        "allowedHosts": raw.get("allowedHosts").cloned().unwrap_or(json!([])),
        "gitHosts": raw.get("gitHosts").cloned().unwrap_or(json!([])),
        "notes": [
            "control socket path intentionally hidden",
            "session/token endpoints are intentionally unavailable via MCP"
        ]
    }))
}

fn tool_janus_discovery(app: &App) -> Result<Value> {
    read_discovery(app)
}

fn tool_janus_safety() -> Value {
    json!({
        "model": "strict_host_broker",
        "guarantees": [
            "upstream credentials remain host-side",
            "MCP surface is read-only metadata",
            "no session creation/token issuance via MCP",
            "no control socket path exposure",
            "all protected operations enforced by Janus capability checks",
            "janusd policy evaluation is deterministic and non-LLM"
        ],
        "operator_requirements": [
            "run janusd externally on host",
            "janus-mcp does not start janusd",
            "keep sandbox unable to access host control socket path",
            "issue session env from host supervisor, not from MCP"
        ]
    })
}

fn read_discovery(app: &App) -> Result<Value> {
    let health = read_control_json(app, "/health")?;
    let config = read_control_json(app, "/v1/config")?;
    Ok(build_discovery_from_config(&health, &config))
}

fn build_discovery_from_config(health: &Value, config: &Value) -> Value {
    let known = to_string_vec(config.get("knownCapabilities").unwrap_or(&Value::Null));
    let defaults = to_string_vec(config.get("defaultCapabilities").unwrap_or(&Value::Null));
    let known_set: HashSet<String> = known.into_iter().collect();
    let default_set: HashSet<String> = defaults.into_iter().collect();

    let supports = config.get("supports").unwrap_or(&Value::Null);
    let proxy_set: HashSet<String> = to_string_vec(supports.get("proxy").unwrap_or(&Value::Null))
        .into_iter()
        .collect();
    let typed_set: HashSet<String> =
        to_string_vec(supports.get("typedAdapters").unwrap_or(&Value::Null))
            .into_iter()
            .collect();

    let mut protocols = Vec::with_capacity(PROTOCOL_CATALOG.len());
    let mut unavailable_protocols = Vec::new();
    for spec in PROTOCOL_CATALOG {
        let available = known_set.contains(spec.capability)
            && (spec.capability == "http_proxy" || proxy_set.contains(spec.capability));
        if !available {
            unavailable_protocols.push(spec.capability.to_string());
        }
        protocols.push(json!({
            "capability": spec.capability,
            "ports": spec.ports,
            "available": available,
            "defaultEnabled": default_set.contains(spec.capability),
        }));
    }

    let mut resources = Vec::with_capacity(RESOURCE_CATALOG.len());
    let mut unavailable_resources = Vec::new();
    for capability in RESOURCE_CATALOG {
        let available = known_set.contains(capability) && typed_set.contains(capability);
        if !available {
            unavailable_resources.push(capability.to_string());
        }
        resources.push(json!({
            "capability": capability,
            "available": available,
            "defaultEnabled": default_set.contains(capability),
        }));
    }

    let discovery = config.get("discovery").unwrap_or(&Value::Null);
    let mut public_endpoints =
        to_string_vec(discovery.get("publicEndpoints").unwrap_or(&Value::Null));
    if public_endpoints.is_empty() {
        public_endpoints = vec!["/health".to_string(), "/v1/config".to_string()];
    }

    let execution_model = config.get("executionModel").unwrap_or(&Value::Null);
    let deterministic = execution_model
        .get("deterministic")
        .cloned()
        .unwrap_or_else(|| json!(true));
    let llm_driven = execution_model
        .get("llmDriven")
        .cloned()
        .unwrap_or_else(|| json!(false));
    let notes = execution_model
        .get("notes")
        .cloned()
        .unwrap_or_else(|| json!([]));

    json!({
        "source": {
            "mode": "public_api_only",
            "queriedEndpoints": ["/health", "/v1/config"],
            "advertisedEndpoints": public_endpoints,
        },
        "daemon": {
            "status": health.get("status").cloned().unwrap_or(Value::Null),
            "uptimeSeconds": health.get("uptimeSeconds").cloned().unwrap_or(Value::Null),
        },
        "executionModel": {
            "deterministic": deterministic,
            "llmDriven": llm_driven,
            "notes": notes,
        },
        "protocols": protocols,
        "resources": resources,
        "unavailableProtocols": unavailable_protocols,
        "unavailableResources": unavailable_resources,
        "guidance": [
            "If required protocol/resource is unavailable, ask operator to enable/update Janus server capability set.",
            "If capability exists but is not default-enabled, request session issuance with explicit capability and allowed_hosts."
        ]
    })
}

fn to_string_vec(value: &Value) -> Vec<String> {
    match value {
        Value::Array(values) => values
            .iter()
            .filter_map(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn read_control_json(app: &App, path: &str) -> Result<Value> {
    let url = format!("http://localhost{path}");
    let response = app
        .client
        .get(&url)
        .send()
        .with_context(|| format!("failed request to {path}"))?;

    let status = response.status();
    let text = response.text().context("failed reading response body")?;

    if !status.is_success() {
        return Err(anyhow!(
            "janusd returned {} for {}: {}",
            status.as_u16(),
            path,
            text
        ));
    }

    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON from janusd endpoint {path}"))?;

    Ok(value)
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .context("failed reading MCP header line")?;

        if bytes == 0 {
            return Ok(None);
        }

        let line_trimmed = line.trim_end_matches(['\r', '\n']);
        if line_trimmed.is_empty() {
            break;
        }

        if let Some(rest) = line_trimmed.strip_prefix("Content-Length:") {
            let parsed = rest
                .trim()
                .parse::<usize>()
                .context("invalid Content-Length value")?;
            content_length = Some(parsed);
        }
    }

    let len = content_length.ok_or_else(|| anyhow!("missing Content-Length header"))?;
    let mut body = vec![0_u8; len];
    reader
        .read_exact(&mut body)
        .context("failed reading MCP payload")?;

    let json: Value = serde_json::from_slice(&body).context("invalid JSON payload")?;
    Ok(Some(json))
}

fn write_message(writer: &mut impl Write, message: &Value) -> Result<()> {
    let payload = serde_json::to_vec(message).context("failed to serialize MCP response")?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())
        .context("failed writing MCP header")?;
    writer
        .write_all(&payload)
        .context("failed writing MCP payload")?;
    writer.flush().context("failed flushing MCP output")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_length_message() {
        let body = b"{\"jsonrpc\":\"2.0\"}";
        let mut raw = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        raw.extend_from_slice(body);
        let mut reader = BufReader::new(&raw[..]);
        let message = read_message(&mut reader)
            .expect("read ok")
            .expect("message");
        assert_eq!(message["jsonrpc"], "2.0");
    }

    #[test]
    fn tools_list_contains_only_safe_tools() {
        let tools = handle_tools_list();
        let names: Vec<String> = tools["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .filter_map(|tool| tool.get("name").and_then(|v| v.as_str()))
            .map(|v| v.to_string())
            .collect();

        assert!(names.contains(&"janus.health".to_string()));
        assert!(names.contains(&"janus.capabilities".to_string()));
        assert!(names.contains(&"janus.discovery".to_string()));
        assert!(names.contains(&"janus.safety".to_string()));
        assert!(!names.iter().any(|name| name.contains("session")));
        assert!(!names.iter().any(|name| name.contains("secret")));
    }

    #[test]
    fn resources_list_contains_discovery_resources() {
        let resources = handle_resources_list();
        let uris: Vec<String> = resources["resources"]
            .as_array()
            .expect("resources array")
            .iter()
            .filter_map(|resource| resource.get("uri").and_then(|v| v.as_str()))
            .map(|v| v.to_string())
            .collect();

        assert!(uris.contains(&"janus://discovery/protocols".to_string()));
        assert!(uris.contains(&"janus://discovery/resources".to_string()));
        assert!(uris.contains(&"janus://discovery/summary".to_string()));
    }

    #[test]
    fn safety_tool_mentions_no_secret_apis() {
        let safety = tool_janus_safety();
        let text = serde_json::to_string(&safety).expect("serialize");
        assert!(text.contains("read-only metadata"));
        assert!(text.contains("no session creation/token issuance via MCP"));
        assert!(text.contains("deterministic and non-LLM"));
    }

    #[test]
    fn build_discovery_from_config_classifies_availability() {
        let health = json!({
            "status": "ok",
            "uptimeSeconds": 123
        });
        let config = json!({
            "knownCapabilities": ["http_proxy", "git_http", "git_ssh", "postgres_wire", "postgres_query"],
            "defaultCapabilities": ["http_proxy", "git_http"],
            "supports": {
                "proxy": ["http_proxy", "git_http", "git_ssh", "postgres_wire"],
                "typedAdapters": ["postgres_query"]
            },
            "discovery": {
                "publicEndpoints": ["/health", "/v1/config"]
            },
            "executionModel": {
                "deterministic": true,
                "llmDriven": false,
                "notes": ["deterministic policy only"]
            }
        });

        let discovery = build_discovery_from_config(&health, &config);
        assert_eq!(discovery["source"]["mode"], "public_api_only");
        assert_eq!(discovery["executionModel"]["deterministic"], true);
        assert_eq!(discovery["executionModel"]["llmDriven"], false);

        let protocols = discovery["protocols"].as_array().expect("protocols array");
        let git_ssh = protocols
            .iter()
            .find(|item| item["capability"] == "git_ssh")
            .expect("git_ssh entry");
        assert_eq!(git_ssh["available"], true);
        assert_eq!(git_ssh["defaultEnabled"], false);

        let mysql = protocols
            .iter()
            .find(|item| item["capability"] == "mysql_wire")
            .expect("mysql entry");
        assert_eq!(mysql["available"], false);

        let resources = discovery["resources"].as_array().expect("resources array");
        let postgres_query = resources
            .iter()
            .find(|item| item["capability"] == "postgres_query")
            .expect("postgres_query entry");
        assert_eq!(postgres_query["available"], true);
        let deploy_terraform = resources
            .iter()
            .find(|item| item["capability"] == "deploy_terraform")
            .expect("deploy_terraform entry");
        assert_eq!(deploy_terraform["available"], false);
    }
}
