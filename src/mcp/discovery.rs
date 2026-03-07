use super::*;
use std::collections::HashSet;

const RESOURCE_CATALOG: [&str; 4] = [
    "postgres_query",
    "deploy_kubectl",
    "deploy_helm",
    "deploy_terraform",
];

pub(super) fn handle_resources_list() -> Value {
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

pub(super) fn handle_resources_read(app: &App, params: &Value) -> Result<Value> {
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

pub(super) fn tool_janus_discovery(app: &App) -> Result<Value> {
    read_discovery(app)
}

fn read_discovery(app: &App) -> Result<Value> {
    let health = read_control_json(app, "/health")?;
    let config = read_control_json(app, "/v1/config")?;
    Ok(build_discovery_from_config(&health, &config))
}

pub(super) fn build_discovery_from_config(health: &Value, config: &Value) -> Value {
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

    let protocol_catalog = crate::protocols::all();
    let mut protocols = Vec::with_capacity(protocol_catalog.len());
    let mut unavailable_protocols = Vec::new();
    for spec in protocol_catalog {
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
