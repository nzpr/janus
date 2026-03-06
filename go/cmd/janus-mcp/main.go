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
			"instructions": "Read-only Janus metadata MCP. No secret/session/token APIs are exposed.",
		}, nil
	case "ping":
		return map[string]any{}, nil
	case "tools/list":
		return map[string]any{"tools": []any{
			map[string]any{"name": "janus.health", "description": "Return Janus daemon health status.", "inputSchema": emptySchema()},
			map[string]any{"name": "janus.capabilities", "description": "Return safe Janus capability and policy summary.", "inputSchema": emptySchema()},
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
		return map[string]any{"resources": []any{}}, nil
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
	case "janus.safety":
		return map[string]any{
			"model": "strict_host_broker",
			"guarantees": []string{
				"upstream credentials remain host-side",
				"MCP surface is read-only metadata",
				"no session creation/token issuance via MCP",
				"no control socket path exposure",
				"all protected operations enforced by Janus capability checks",
			},
			"operator_requirements": []string{
				"run janusd on host",
				"keep sandbox unable to access host control socket path",
				"issue session env from host supervisor, not from MCP",
			},
		}, nil
	default:
		return nil, fmt.Errorf("unknown tool: %s", name)
	}
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
