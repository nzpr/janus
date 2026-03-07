package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"
)

type app struct {
	client *http.Client
}

type protocolSpec struct {
	Capability string
	Ports      []int
}

var protocolCatalog = []protocolSpec{
	{Capability: "http_proxy", Ports: []int{}},
	{Capability: "git_http", Ports: []int{}},
	{Capability: "git_ssh", Ports: []int{22}},
	{Capability: "postgres_wire", Ports: []int{5432}},
	{Capability: "mysql_wire", Ports: []int{3306}},
	{Capability: "redis", Ports: []int{6379}},
	{Capability: "mongodb", Ports: []int{27017}},
	{Capability: "amqp", Ports: []int{5672}},
	{Capability: "kafka", Ports: []int{9092}},
	{Capability: "nats", Ports: []int{4222}},
	{Capability: "mqtt", Ports: []int{1883, 8883}},
	{Capability: "ldap", Ports: []int{389, 636}},
	{Capability: "sftp", Ports: []int{22}},
	{Capability: "smb", Ports: []int{445}},
}

var resourceCatalog = []string{
	"postgres_query",
	"deploy_kubectl",
	"deploy_helm",
	"deploy_terraform",
}

func main() {
	controlSocket := flag.String("control-socket", "/tmp/janusd-control.sock", "Path to Janus control socket")
	flag.Parse()

	if envSocket := strings.TrimSpace(os.Getenv("JANUS_CONTROL_SOCKET")); envSocket != "" {
		*controlSocket = envSocket
	}

	transport := &http.Transport{
		DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
			var d net.Dialer
			return d.DialContext(ctx, "unix", *controlSocket)
		},
	}
	client := &http.Client{Transport: transport, Timeout: 10 * time.Second}

	a := app{client: client}
	if err := runStdioServer(a); err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		os.Exit(1)
	}
}

func runStdioServer(a app) error {
	reader := bufio.NewReader(os.Stdin)
	writer := bufio.NewWriter(os.Stdout)
	defer writer.Flush()

	for {
		msg, err := readMessage(reader)
		if err != nil {
			if errors.Is(err, io.EOF) {
				return nil
			}
			return err
		}
		if msg == nil {
			return nil
		}
		resp := handleMessage(a, msg)
		if resp == nil {
			continue
		}
		if err := writeMessage(writer, resp); err != nil {
			return err
		}
	}
}

func handleMessage(a app, msg map[string]any) map[string]any {
	id, hasID := msg["id"]
	method, _ := msg["method"].(string)
	if method == "" || !hasID {
		return nil
	}

	params := map[string]any{}
	if raw, ok := msg["params"].(map[string]any); ok {
		params = raw
	}

	result, err := handleMethod(a, method, params)
	if err != nil {
		return map[string]any{
			"jsonrpc": "2.0",
			"id":      id,
			"error": map[string]any{
				"code":    -32601,
				"message": err.Error(),
			},
		}
	}

	return map[string]any{"jsonrpc": "2.0", "id": id, "result": result}
}

func handleMethod(a app, method string, params map[string]any) (map[string]any, error) {
	switch method {
	case "initialize":
		protocolVersion, _ := params["protocolVersion"].(string)
		if strings.TrimSpace(protocolVersion) == "" {
			protocolVersion = "2025-03-26"
		}
		return map[string]any{
			"protocolVersion": protocolVersion,
			"capabilities": map[string]any{
				"tools":     map[string]any{"listChanged": false},
				"resources": map[string]any{"listChanged": false, "subscribe": false},
				"prompts":   map[string]any{"listChanged": false},
			},
			"serverInfo":   map[string]any{"name": "janus-mcp", "version": "0.1.0-go"},
			"instructions": "Read-only Janus metadata MCP. Discovery uses only janusd public APIs (/health, /v1/config). janusd must be started externally.",
		}, nil
	case "ping":
		return map[string]any{}, nil
	case "tools/list":
		return map[string]any{"tools": []any{
			map[string]any{"name": "janus.health", "description": "Return Janus daemon health status.", "inputSchema": emptySchema()},
			map[string]any{"name": "janus.capabilities", "description": "Return safe Janus capability and policy summary.", "inputSchema": emptySchema()},
			map[string]any{"name": "janus.discovery", "description": "Return protocol/resource availability and gaps using Janus public discovery APIs.", "inputSchema": emptySchema()},
			map[string]any{"name": "janus.safety", "description": "Explain Janus secret-isolation model and constraints.", "inputSchema": emptySchema()},
		}}, nil
	case "tools/call":
		name, _ := params["name"].(string)
		if strings.TrimSpace(name) == "" {
			return nil, errors.New("tools/call requires name")
		}
		payload, err := handleToolCall(a, name)
		if err != nil {
			return nil, err
		}
		pretty, _ := json.MarshalIndent(payload, "", "  ")
		return map[string]any{
			"content":           []any{map[string]any{"type": "text", "text": string(pretty)}},
			"structuredContent": payload,
		}, nil
	case "resources/list":
		return map[string]any{"resources": []any{
			map[string]any{
				"uri":         "janus://discovery/protocols",
				"name":        "Janus Protocol Availability",
				"description": "Protocol capabilities available and unavailable on this Janus server.",
				"mimeType":    "application/json",
			},
			map[string]any{
				"uri":         "janus://discovery/resources",
				"name":        "Janus Resource Availability",
				"description": "Typed adapters/capabilities available and unavailable on this Janus server.",
				"mimeType":    "application/json",
			},
			map[string]any{
				"uri":         "janus://discovery/summary",
				"name":        "Janus Discovery Summary",
				"description": "Combined protocol/resource/discovery summary for agent planning.",
				"mimeType":    "application/json",
			},
		}}, nil
	case "resources/read":
		return handleResourceRead(a, params)
	case "prompts/list":
		return map[string]any{"prompts": []any{}}, nil
	default:
		return nil, fmt.Errorf("method not found: %s", method)
	}
}

func handleToolCall(a app, name string) (map[string]any, error) {
	switch name {
	case "janus.health":
		raw, err := readControlJSON(a.client, "/health")
		if err != nil {
			return nil, err
		}
		return map[string]any{
			"status":        raw["status"],
			"uptimeSeconds": raw["uptimeSeconds"],
		}, nil
	case "janus.capabilities":
		raw, err := readControlJSON(a.client, "/v1/config")
		if err != nil {
			return nil, err
		}
		return map[string]any{
			"proxyBind":           raw["proxyBind"],
			"defaultTtlSeconds":   raw["defaultTtlSeconds"],
			"defaultCapabilities": raw["defaultCapabilities"],
			"knownCapabilities":   raw["knownCapabilities"],
			"supports":            raw["supports"],
			"allowedHosts":        raw["allowedHosts"],
			"gitHosts":            raw["gitHosts"],
			"notes": []string{
				"control socket path intentionally hidden",
				"session/token endpoints are intentionally unavailable via MCP",
			},
		}, nil
	case "janus.discovery":
		return readDiscovery(a)
	case "janus.safety":
		return map[string]any{
			"model": "strict_host_broker",
			"guarantees": []string{
				"upstream credentials remain host-side",
				"MCP surface is read-only metadata",
				"no session creation/token issuance via MCP",
				"no control socket path exposure",
				"all protected operations enforced by Janus capability checks",
				"janusd policy evaluation is deterministic and non-LLM",
			},
			"operator_requirements": []string{
				"run janusd externally on host",
				"janus-mcp does not start janusd",
				"keep sandbox unable to access host control socket path",
				"issue session env from host supervisor, not from MCP",
			},
		}, nil
	default:
		return nil, fmt.Errorf("unknown tool: %s", name)
	}
}

func handleResourceRead(a app, params map[string]any) (map[string]any, error) {
	uri, _ := params["uri"].(string)
	if strings.TrimSpace(uri) == "" {
		return nil, errors.New("resources/read requires uri")
	}
	discovery, err := readDiscovery(a)
	if err != nil {
		return nil, err
	}

	var payload any
	switch uri {
	case "janus://discovery/protocols":
		payload = discovery["protocols"]
	case "janus://discovery/resources":
		payload = discovery["resources"]
	case "janus://discovery/summary":
		payload = discovery
	default:
		return nil, fmt.Errorf("unknown resource uri: %s", uri)
	}
	text, _ := json.MarshalIndent(payload, "", "  ")
	return map[string]any{
		"contents": []any{
			map[string]any{
				"uri":      uri,
				"mimeType": "application/json",
				"text":     string(text),
			},
		},
	}, nil
}

func readDiscovery(a app) (map[string]any, error) {
	health, err := readControlJSON(a.client, "/health")
	if err != nil {
		return nil, err
	}
	config, err := readControlJSON(a.client, "/v1/config")
	if err != nil {
		return nil, err
	}
	return buildDiscoveryFromConfig(health, config), nil
}

func buildDiscoveryFromConfig(health map[string]any, config map[string]any) map[string]any {
	known := toStringSlice(config["knownCapabilities"])
	defaultCaps := toStringSlice(config["defaultCapabilities"])
	knownSet := makeSet(known)
	defaultSet := makeSet(defaultCaps)

	supports, _ := config["supports"].(map[string]any)
	proxySet := makeSet(toStringSlice(supports["proxy"]))
	typedSet := makeSet(toStringSlice(supports["typedAdapters"]))

	protocols := make([]any, 0, len(protocolCatalog))
	unavailableProtocols := make([]string, 0)
	for _, spec := range protocolCatalog {
		available := has(knownSet, spec.Capability) && (spec.Capability == "http_proxy" || has(proxySet, spec.Capability))
		if !available {
			unavailableProtocols = append(unavailableProtocols, spec.Capability)
		}
		protocols = append(protocols, map[string]any{
			"capability":     spec.Capability,
			"ports":          spec.Ports,
			"available":      available,
			"defaultEnabled": has(defaultSet, spec.Capability),
		})
	}

	resources := make([]any, 0, len(resourceCatalog))
	unavailableResources := make([]string, 0)
	for _, capability := range resourceCatalog {
		available := has(knownSet, capability) && has(typedSet, capability)
		if !available {
			unavailableResources = append(unavailableResources, capability)
		}
		resources = append(resources, map[string]any{
			"capability":     capability,
			"available":      available,
			"defaultEnabled": has(defaultSet, capability),
		})
	}

	discoveryInfo, _ := config["discovery"].(map[string]any)
	executionModel, _ := config["executionModel"].(map[string]any)

	publicEndpoints := toStringSlice(discoveryInfo["publicEndpoints"])
	if len(publicEndpoints) == 0 {
		publicEndpoints = []string{"/health", "/v1/config"}
	}

	return map[string]any{
		"source": map[string]any{
			"mode":                "public_api_only",
			"queriedEndpoints":    []string{"/health", "/v1/config"},
			"advertisedEndpoints": publicEndpoints,
		},
		"daemon": map[string]any{
			"status":        health["status"],
			"uptimeSeconds": health["uptimeSeconds"],
		},
		"executionModel": map[string]any{
			"deterministic": valueOr(executionModel["deterministic"], true),
			"llmDriven":     valueOr(executionModel["llmDriven"], false),
			"notes":         valueOr(executionModel["notes"], []any{}),
		},
		"protocols":            protocols,
		"resources":            resources,
		"unavailableProtocols": unavailableProtocols,
		"unavailableResources": unavailableResources,
		"guidance": []string{
			"If required protocol/resource is unavailable, ask operator to enable/update Janus server capability set.",
			"If capability exists but is not default-enabled, request session issuance with explicit capability and allowed_hosts.",
		},
	}
}

func toStringSlice(value any) []string {
	switch typed := value.(type) {
	case []string:
		out := make([]string, 0, len(typed))
		for _, v := range typed {
			if s := strings.TrimSpace(v); s != "" {
				out = append(out, s)
			}
		}
		return out
	case []any:
		out := make([]string, 0, len(typed))
		for _, v := range typed {
			if s, ok := v.(string); ok {
				if trimmed := strings.TrimSpace(s); trimmed != "" {
					out = append(out, trimmed)
				}
			}
		}
		return out
	default:
		return nil
	}
}

func makeSet(values []string) map[string]struct{} {
	out := make(map[string]struct{}, len(values))
	for _, v := range values {
		out[v] = struct{}{}
	}
	return out
}

func has(set map[string]struct{}, key string) bool {
	_, ok := set[key]
	return ok
}

func valueOr(v any, fallback any) any {
	if v == nil {
		return fallback
	}
	return v
}

func readControlJSON(client *http.Client, path string) (map[string]any, error) {
	req, err := http.NewRequest(http.MethodGet, "http://localhost"+path, nil)
	if err != nil {
		return nil, err
	}
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed request to %s: %w", path, err)
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("janusd returned %d for %s: %s", resp.StatusCode, path, strings.TrimSpace(string(body)))
	}
	var out map[string]any
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, fmt.Errorf("invalid JSON from janusd endpoint %s", path)
	}
	return out, nil
}

func readMessage(reader *bufio.Reader) (map[string]any, error) {
	contentLength := -1
	for {
		line, err := reader.ReadString('\n')
		if err != nil {
			if errors.Is(err, io.EOF) {
				return nil, io.EOF
			}
			return nil, err
		}
		trimmed := strings.TrimRight(line, "\r\n")
		if trimmed == "" {
			break
		}
		if strings.HasPrefix(strings.ToLower(trimmed), "content-length:") {
			raw := strings.TrimSpace(trimmed[len("content-length:"):])
			v, convErr := strconv.Atoi(raw)
			if convErr != nil {
				return nil, fmt.Errorf("invalid Content-Length value")
			}
			contentLength = v
		}
	}
	if contentLength < 0 {
		return nil, fmt.Errorf("missing Content-Length header")
	}
	payload := make([]byte, contentLength)
	if _, err := io.ReadFull(reader, payload); err != nil {
		return nil, fmt.Errorf("failed reading MCP payload")
	}
	var out map[string]any
	if err := json.Unmarshal(payload, &out); err != nil {
		return nil, fmt.Errorf("invalid JSON payload")
	}
	return out, nil
}

func writeMessage(writer *bufio.Writer, msg map[string]any) error {
	payload, err := json.Marshal(msg)
	if err != nil {
		return err
	}
	header := fmt.Sprintf("Content-Length: %d\r\n\r\n", len(payload))
	if _, err := writer.WriteString(header); err != nil {
		return err
	}
	if _, err := writer.Write(payload); err != nil {
		return err
	}
	return writer.Flush()
}

func emptySchema() map[string]any {
	return map[string]any{
		"type":                 "object",
		"properties":           map[string]any{},
		"additionalProperties": false,
	}
}

func _readMessageForTest(raw string) (map[string]any, error) {
	reader := bufio.NewReader(bytes.NewBufferString(raw))
	return readMessage(reader)
}
