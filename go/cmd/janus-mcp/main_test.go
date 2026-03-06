package main

import (
	"fmt"
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
	if !strings.Contains(raw, "janus.health") || !strings.Contains(raw, "janus.capabilities") || !strings.Contains(raw, "janus.safety") {
		t.Fatalf("missing expected tools: %v", result)
	}
	if strings.Contains(raw, "secret") || strings.Contains(raw, "session") {
		t.Fatalf("tool list should not expose secret/session endpoints: %v", result)
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
}
