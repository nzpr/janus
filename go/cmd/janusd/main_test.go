package main

import (
	"strings"
	"testing"
	"time"
)

func testConfig() config {
	return config{
		ProxyBind:           "127.0.0.1:9080",
		ControlSocket:       "/tmp/janusd-control.sock",
		DefaultTTLSeconds:   3600,
		DefaultCapabilities: []string{capHTTPProxy, capGitHTTP},
		AllowedHosts:        []string{"github.com"},
		GitHosts:            []string{"github.com"},
		GitUsername:         "x-access-token",
		GitPassword:         "ghp_secret_token",
		GitSSHAuthSock:      "/var/run/janus/ssh-agent.sock",
		Postgres: postgresDefaults{
			Host:     "db.internal",
			Port:     "5432",
			User:     "janus",
			Database: "app",
			Password: "pg_secret_password",
		},
	}
}

func testSession(capabilities []string) session {
	return session{
		ID:           "session-1",
		Token:        "token-secret-value",
		CreatedAt:    time.Now().UTC(),
		ExpiresAt:    time.Now().UTC().Add(time.Hour),
		AllowedHosts: []string{"github.com"},
		Capabilities: capabilities,
	}
}

func TestNormalizeCapabilitiesDedupAndSort(t *testing.T) {
	out, err := normalizeCapabilities([]string{capGitHTTP, capHTTPProxy, capGitHTTP})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(out, ",") != "git_http,http_proxy" {
		t.Fatalf("unexpected capabilities: %#v", out)
	}
}

func TestNormalizeCapabilitiesRejectUnknown(t *testing.T) {
	if _, err := normalizeCapabilities([]string{"unknown_cap"}); err == nil {
		t.Fatal("expected unknown capability error")
	}
}

func TestBuildSessionEnvExcludesControlSocket(t *testing.T) {
	env := buildSessionEnv(testConfig(), testSession([]string{capHTTPProxy, capGitHTTP}))
	if _, ok := env["JANUS_CONTROL_SOCKET"]; ok {
		t.Fatal("JANUS_CONTROL_SOCKET must not be exposed in session env")
	}
}

func TestBuildSessionEnvScopesProxyToHTTPCapability(t *testing.T) {
	env := buildSessionEnv(testConfig(), testSession([]string{capGitHTTP}))
	if _, ok := env["HTTP_PROXY"]; ok {
		t.Fatal("HTTP_PROXY should not exist without http_proxy capability")
	}
	if _, ok := env["GIT_CONFIG_COUNT"]; !ok {
		t.Fatal("expected git rewrite config entries")
	}
}

func TestBuildSessionEnvIncludesGitSSHCommand(t *testing.T) {
	s := testSession([]string{capGitSSH})
	env := buildSessionEnv(testConfig(), s)
	cmd, ok := env["GIT_SSH_COMMAND"]
	if !ok {
		t.Fatal("expected GIT_SSH_COMMAND for git_ssh capability")
	}
	if !strings.Contains(cmd, "ProxyCommand=") {
		t.Fatalf("missing ProxyCommand in GIT_SSH_COMMAND: %s", cmd)
	}
	if !strings.Contains(cmd, "/dev/tcp/127.0.0.1/9080") {
		t.Fatalf("expected proxy dial target in GIT_SSH_COMMAND: %s", cmd)
	}
	if strings.Contains(cmd, s.Token) {
		t.Fatalf("expected token not to appear in plain text: %s", cmd)
	}
	if env["SSH_AUTH_SOCK"] != "/var/run/janus/ssh-agent.sock" {
		t.Fatalf("expected SSH_AUTH_SOCK in session env, got %q", env["SSH_AUTH_SOCK"])
	}
}

func TestAuthorizeConnectTokenAllowsGitSSHOnlyOnSSHPort(t *testing.T) {
	s := testSession([]string{capGitSSH})
	a := &app{
		cfg:      testConfig(),
		sessions: map[string]session{s.ID: s},
	}
	if _, err := a.authorizeConnectToken(s.Token, "github.com", 22); err != nil {
		t.Fatalf("expected git_ssh capability to authorize CONNECT on port 22: %v", err)
	}
	if _, err := a.authorizeConnectToken(s.Token, "github.com", 443); err == nil {
		t.Fatal("expected CONNECT on non-SSH port to require http_proxy capability")
	}
}

func TestHostMatchesSubdomains(t *testing.T) {
	if !hostMatches("api.github.com", "github.com") {
		t.Fatal("api.github.com should match github.com")
	}
	if hostMatches("github.com.evil.com", "github.com") {
		t.Fatal("github.com.evil.com should not match github.com")
	}
}

func TestRedactTextRemovesSecrets(t *testing.T) {
	a := &app{cfg: testConfig()}
	s := testSession([]string{capHTTPProxy})
	out := a.redactText(s, "token-secret-value ghp_secret_token pg_secret_password")
	if strings.Contains(out, "token-secret-value") || strings.Contains(out, "ghp_secret_token") || strings.Contains(out, "pg_secret_password") {
		t.Fatalf("redaction failed: %s", out)
	}
}
