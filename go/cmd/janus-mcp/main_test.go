package main

import (
	"fmt"
	"slices"
	"strings"
	"testing"
)

func TestParseContentLengthMessage(t *testing.T) {
	body := `{"jsonrpc":"2.0"}`
	raw := fmt.Sprintf("Content-Length: %d\r\n\r\n%s", len(body), body)
	msg, err := _readMessageForTest(raw)
	if err != nil {
		t.Fatalf("read message: %v", err)
	}
	if msg["jsonrpc"] != "2.0" {
		t.Fatalf("unexpected payload: %#v", msg)
	}
}

func TestToolsListContainsOnlySafeTools(t *testing.T) {
	result, err := handleMethod(app{}, "tools/list", map[string]any{})
	if err != nil {
		t.Fatalf("tools/list failed: %v", err)
	}
	toolsRaw, ok := result["tools"].([]any)
	if !ok {
		t.Fatalf("missing tools array: %v", result)
	}
	names := make([]string, 0, len(toolsRaw))
	for _, tItem := range toolsRaw {
		tool, _ := tItem.(map[string]any)
		name, _ := tool["name"].(string)
		names = append(names, name)
	}
	raw := strings.Join(names, ",")
	if !strings.Contains(raw, "janus.health") || !strings.Contains(raw, "janus.capabilities") || !strings.Contains(raw, "janus.discovery") || !strings.Contains(raw, "janus.safety") {
		t.Fatalf("missing expected tools: %v", result)
	}
	if strings.Contains(raw, "secret") || strings.Contains(raw, "session") {
		t.Fatalf("tool list should not expose secret/session endpoints: %v", result)
	}
}

func TestResourcesListContainsDiscoveryResources(t *testing.T) {
	result, err := handleMethod(app{}, "resources/list", map[string]any{})
	if err != nil {
		t.Fatalf("resources/list failed: %v", err)
	}
	resourcesRaw, ok := result["resources"].([]any)
	if !ok {
		t.Fatalf("missing resources array: %v", result)
	}
	uris := make([]string, 0, len(resourcesRaw))
	for _, item := range resourcesRaw {
		resource, _ := item.(map[string]any)
		uri, _ := resource["uri"].(string)
		uris = append(uris, uri)
	}
	expected := []string{
		"janus://discovery/protocols",
		"janus://discovery/resources",
		"janus://discovery/summary",
	}
	for _, uri := range expected {
		if !slices.Contains(uris, uri) {
			t.Fatalf("missing resource %q in %v", uri, uris)
		}
	}
}

func TestSafetyToolExplainsNoSecretAPIs(t *testing.T) {
	payload, err := handleToolCall(app{}, "janus.safety")
	if err != nil {
		t.Fatalf("janus.safety failed: %v", err)
	}
	raw := fmt.Sprintf("%v", payload)
	if !strings.Contains(raw, "no session creation/token issuance via MCP") {
		t.Fatalf("missing guardrail statement: %v", payload)
	}
	if !strings.Contains(raw, "deterministic and non-LLM") {
		t.Fatalf("missing deterministic model statement: %v", payload)
	}
}

func TestBuildDiscoveryFromConfig(t *testing.T) {
	health := map[string]any{
		"status":        "ok",
		"uptimeSeconds": int64(123),
	}
	config := map[string]any{
		"knownCapabilities":   []any{"http_proxy", "git_http", "git_ssh", "postgres_wire", "postgres_query"},
		"defaultCapabilities": []any{"http_proxy", "git_http"},
		"supports": map[string]any{
			"proxy":         []any{"http_proxy", "git_http", "git_ssh", "postgres_wire"},
			"typedAdapters": []any{"postgres_query"},
		},
		"discovery": map[string]any{
			"publicEndpoints": []any{"/health", "/v1/config"},
		},
		"executionModel": map[string]any{
			"deterministic": true,
			"llmDriven":     false,
			"notes":         []any{"deterministic policy only"},
		},
	}

	discovery := buildDiscoveryFromConfig(health, config)

	source, _ := discovery["source"].(map[string]any)
	if source["mode"] != "public_api_only" {
		t.Fatalf("unexpected source mode: %v", source["mode"])
	}

	executionModel, _ := discovery["executionModel"].(map[string]any)
	if executionModel["deterministic"] != true || executionModel["llmDriven"] != false {
		t.Fatalf("unexpected execution model: %v", executionModel)
	}

	protocols, _ := discovery["protocols"].([]any)
	gitSSH := findCapability(protocols, "git_ssh")
	if gitSSH["available"] != true || gitSSH["defaultEnabled"] != false {
		t.Fatalf("unexpected git_ssh protocol state: %v", gitSSH)
	}
	mysql := findCapability(protocols, "mysql_wire")
	if mysql["available"] != false {
		t.Fatalf("expected mysql_wire unavailable: %v", mysql)
	}

	resources, _ := discovery["resources"].([]any)
	postgresQuery := findCapability(resources, "postgres_query")
	if postgresQuery["available"] != true {
		t.Fatalf("expected postgres_query available: %v", postgresQuery)
	}
	terraform := findCapability(resources, "deploy_terraform")
	if terraform["available"] != false {
		t.Fatalf("expected deploy_terraform unavailable: %v", terraform)
	}

	unavailableProtocols := toStringSlice(discovery["unavailableProtocols"])
	if !slices.Contains(unavailableProtocols, "mysql_wire") {
		t.Fatalf("missing mysql_wire in unavailableProtocols: %v", unavailableProtocols)
	}
	unavailableResources := toStringSlice(discovery["unavailableResources"])
	if !slices.Contains(unavailableResources, "deploy_terraform") {
		t.Fatalf("missing deploy_terraform in unavailableResources: %v", unavailableResources)
	}
}

func findCapability(items []any, capability string) map[string]any {
	for _, item := range items {
		typed, _ := item.(map[string]any)
		if typed["capability"] == capability {
			return typed
		}
	}
	return map[string]any{}
}
