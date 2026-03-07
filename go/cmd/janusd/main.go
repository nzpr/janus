package main

import (
	"bufio"
	"bytes"
	"context"
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
)

const (
	capHTTPProxy       = "http_proxy"
	capGitHTTP         = "git_http"
	capGitSSH          = "git_ssh"
	capPostgresQuery   = "postgres_query"
	capDeployKubectl   = "deploy_kubectl"
	capDeployHelm      = "deploy_helm"
	capDeployTerraform = "deploy_terraform"
)

var knownCapabilities = map[string]struct{}{
	capHTTPProxy:       {},
	capGitHTTP:         {},
	capGitSSH:          {},
	capPostgresQuery:   {},
	capDeployKubectl:   {},
	capDeployHelm:      {},
	capDeployTerraform: {},
}

var kubectlVerbs = map[string]struct{}{
	"get": {}, "describe": {}, "logs": {}, "apply": {}, "delete": {}, "rollout": {}, "patch": {}, "exec": {},
}
var helmVerbs = map[string]struct{}{
	"list": {}, "status": {}, "install": {}, "upgrade": {}, "uninstall": {}, "repo": {}, "template": {}, "lint": {},
}
var terraformVerbs = map[string]struct{}{
	"init": {}, "plan": {}, "apply": {}, "destroy": {}, "output": {}, "validate": {}, "fmt": {},
}

var kubectlForbiddenFlags = []string{"--token", "--username", "--password", "--client-key", "--client-certificate", "--kubeconfig"}
var helmForbiddenFlags = []string{"--kube-token", "--kubeconfig", "--username", "--password", "--pass-credentials"}
var terraformForbiddenFlags = []string{"-var", "-var-file"}

type postgresDefaults struct {
	Host     string
	Port     string
	User     string
	Database string
	Password string
}

type config struct {
	ProxyBind           string
	ControlSocket       string
	DefaultTTLSeconds   int64
	DefaultCapabilities []string
	AllowedHosts        []string
	GitHosts            []string
	GitUsername         string
	GitPassword         string
	Postgres            postgresDefaults
	KubeconfigPath      string
	ShowBanner          bool
}

type session struct {
	ID           string    `json:"id"`
	Token        string    `json:"-"`
	CreatedAt    time.Time `json:"created_at"`
	ExpiresAt    time.Time `json:"expires_at"`
	AllowedHosts []string  `json:"allowed_hosts"`
	Capabilities []string  `json:"capabilities"`
}

type app struct {
	cfg        config
	sessions   map[string]session
	sessionsM  sync.RWMutex
	httpClient *http.Client
	startedAt  time.Time
}

type createSessionRequest struct {
	TTLSeconds   *int64   `json:"ttl_seconds"`
	AllowedHosts []string `json:"allowed_hosts"`
	Capabilities []string `json:"capabilities"`
}

type createSessionResponse struct {
	SessionID    string            `json:"session_id"`
	CreatedAt    time.Time         `json:"created_at"`
	ExpiresAt    time.Time         `json:"expires_at"`
	Capabilities []string          `json:"capabilities"`
	Env          map[string]string `json:"env"`
	Notes        []string          `json:"notes"`
}

type commandResponse struct {
	Command  string `json:"command"`
	ExitCode int    `json:"exit_code"`
	Stdout   string `json:"stdout"`
	Stderr   string `json:"stderr"`
}

type postgresQueryRequest struct {
	SessionID      string `json:"session_id"`
	SQL            string `json:"sql"`
	Database       string `json:"database"`
	TimeoutSeconds *int64 `json:"timeout_seconds"`
}

type deployRunRequest struct {
	SessionID      string   `json:"session_id"`
	Args           []string `json:"args"`
	CWD            string   `json:"cwd"`
	TimeoutSeconds *int64   `json:"timeout_seconds"`
}

func main() {
	noBanner := flag.Bool("no-banner", false, "disable startup banner")
	flag.Parse()

	cfg, err := loadConfig()
	if err != nil {
		log.Fatalf("load config: %v", err)
	}
	if *noBanner {
		cfg.ShowBanner = false
	}

	a := &app{
		cfg:      cfg,
		sessions: make(map[string]session),
		httpClient: &http.Client{Transport: &http.Transport{
			Proxy: nil,
		}},
		startedAt: time.Now().UTC(),
	}

	if cfg.ShowBanner {
		printStartupBanner(cfg)
	}

	errCh := make(chan error, 2)
	go func() { errCh <- a.runControlServer() }()
	go func() { errCh <- a.runProxyServer() }()

	if runErr := <-errCh; runErr != nil {
		log.Fatal(runErr)
	}
}

func loadConfig() (config, error) {
	proxyBind := getenvDefault("JANUS_PROXY_BIND", "127.0.0.1:9080")
	controlSocket := getenvDefault("JANUS_CONTROL_SOCKET", "/tmp/janusd-control.sock")
	ttl := parseIntEnv("JANUS_DEFAULT_TTL_SECONDS", 3600)
	if ttl < 60 {
		ttl = 60
	}
	if ttl > 86400 {
		ttl = 86400
	}

	defaultCaps, err := normalizeCapabilities(parseListEnv("JANUS_DEFAULT_CAPABILITIES", []string{capHTTPProxy, capGitHTTP}))
	if err != nil {
		return config{}, err
	}

	cfg := config{
		ProxyBind:           proxyBind,
		ControlSocket:       controlSocket,
		DefaultTTLSeconds:   ttl,
		DefaultCapabilities: defaultCaps,
		AllowedHosts:        parseListEnv("JANUS_ALLOWED_HOSTS", []string{"github.com", "api.github.com", "gitlab.com"}),
		GitHosts:            parseListEnv("JANUS_GIT_HTTP_HOSTS", []string{"github.com"}),
		GitUsername:         getenvDefault("JANUS_GIT_HTTP_USERNAME", "x-access-token"),
		GitPassword:         nonEmpty(getenvDefault("JANUS_GIT_HTTP_PASSWORD", getenvDefault("JANUS_GIT_HTTP_TOKEN", ""))),
		Postgres: postgresDefaults{
			Host:     nonEmpty(os.Getenv("JANUS_POSTGRES_HOST")),
			Port:     nonEmpty(os.Getenv("JANUS_POSTGRES_PORT")),
			User:     nonEmpty(os.Getenv("JANUS_POSTGRES_USER")),
			Database: nonEmpty(os.Getenv("JANUS_POSTGRES_DATABASE")),
			Password: nonEmpty(os.Getenv("JANUS_POSTGRES_PASSWORD")),
		},
		KubeconfigPath: nonEmpty(os.Getenv("JANUS_KUBECONFIG")),
		ShowBanner:     os.Getenv("JANUS_NO_BANNER") != "1",
	}

	return cfg, nil
}

func printStartupBanner(cfg config) {
	fmt.Fprintln(os.Stderr, "     _    _    _   _ _   _ ____")
	fmt.Fprintln(os.Stderr, "    | |  / \\  | \\ | | | | / ___|")
	fmt.Fprintln(os.Stderr, " _  | | / _ \\ |  \\| | | | \\___ \\")
	fmt.Fprintln(os.Stderr, "| |_| |/ ___ \\| |\\  | |_| |___) |")
	fmt.Fprintln(os.Stderr, " \\___//_/   \\_\\_| \\_|\\___/|____/")
	fmt.Fprintln(os.Stderr, "status: online")
	fmt.Fprintf(os.Stderr, "proxy: %s\n", cfg.ProxyBind)
	fmt.Fprintf(os.Stderr, "control: %s\n", cfg.ControlSocket)
	fmt.Fprintln(os.Stderr, "quick use:")
	fmt.Fprintf(os.Stderr, "  curl --unix-socket %s -s -X POST http://localhost/v1/sessions\n", cfg.ControlSocket)
	fmt.Fprintln(os.Stderr, "  apply returned env map to sandbox runtime")
	fmt.Fprintln(os.Stderr, "for more info: janusd --help")
}

func (a *app) runControlServer() error {
	_ = os.Remove(a.cfg.ControlSocket)
	if err := os.MkdirAll(filepath.Dir(a.cfg.ControlSocket), 0o755); err != nil {
		return fmt.Errorf("mkdir control socket dir: %w", err)
	}
	ln, err := net.Listen("unix", a.cfg.ControlSocket)
	if err != nil {
		return fmt.Errorf("listen unix socket: %w", err)
	}
	if err := os.Chmod(a.cfg.ControlSocket, 0o600); err != nil {
		return fmt.Errorf("chmod control socket: %w", err)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/health", a.handleHealth)
	mux.HandleFunc("/v1/config", a.handleConfig)
	mux.HandleFunc("/v1/sessions", a.handleSessions)
	mux.HandleFunc("/v1/sessions/", a.handleSessionDelete)
	mux.HandleFunc("/v1/postgres/query", a.handlePostgresQuery)
	mux.HandleFunc("/v1/deploy/kubectl", a.handleDeployKubectl)
	mux.HandleFunc("/v1/deploy/helm", a.handleDeployHelm)
	mux.HandleFunc("/v1/deploy/terraform", a.handleDeployTerraform)

	server := &http.Server{Handler: mux}
	log.Printf("control API listening: %s", a.cfg.ControlSocket)
	return server.Serve(ln)
}

func (a *app) runProxyServer() error {
	server := &http.Server{Addr: a.cfg.ProxyBind, Handler: http.HandlerFunc(a.handleProxy)}
	log.Printf("proxy listening: %s", a.cfg.ProxyBind)
	return server.ListenAndServe()
}

func (a *app) handleProxy(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodConnect {
		a.handleConnect(w, r)
		return
	}
	a.handleForward(w, r)
}

func (a *app) handleConnect(w http.ResponseWriter, r *http.Request) {
	host, port := splitHostPort(r.Host, 443)
	token := extractToken(r)
	if token == "" {
		http.Error(w, "missing proxy token", http.StatusProxyAuthRequired)
		return
	}
	if _, err := a.authorizeConnectToken(token, host, port); err != nil {
		http.Error(w, err.Error(), http.StatusForbidden)
		return
	}

	hj, ok := w.(http.Hijacker)
	if !ok {
		http.Error(w, "hijacking not supported", http.StatusInternalServerError)
		return
	}

	clientConn, brw, err := hj.Hijack()
	if err != nil {
		http.Error(w, "hijack failed", http.StatusInternalServerError)
		return
	}
	defer func() {
		_ = brw.Flush()
	}()

	upstream, err := net.Dial("tcp", net.JoinHostPort(host, strconv.Itoa(port)))
	if err != nil {
		_ = clientConn.Close()
		return
	}

	_, _ = clientConn.Write([]byte("HTTP/1.1 200 Connection Established\r\n\r\n"))

	go func() {
		_, _ = io.Copy(upstream, clientConn)
		_ = upstream.Close()
	}()
	_, _ = io.Copy(clientConn, upstream)
	_ = upstream.Close()
	_ = clientConn.Close()
}

func (a *app) authorizeConnectToken(token, host string, port int) (session, error) {
	if s, proxyErr := a.authorizeTokenForHostAndCapability(token, host, capHTTPProxy); proxyErr == nil {
		return s, nil
	} else if port == 22 {
		return a.authorizeTokenForHostAndCapability(token, host, capGitSSH)
	} else {
		return session{}, proxyErr
	}
}

func (a *app) handleForward(w http.ResponseWriter, r *http.Request) {
	token := extractToken(r)
	if token == "" {
		http.Error(w, "missing proxy token", http.StatusProxyAuthRequired)
		return
	}

	if gitHost, gitPath, ok := parseGitRoute(r.URL); ok {
		if _, err := a.authorizeTokenForHostAndCapability(token, gitHost, capGitHTTP); err != nil {
			http.Error(w, err.Error(), http.StatusForbidden)
			return
		}
		a.forwardGitRequest(w, r, gitHost, gitPath)
		return
	}

	targetURL, host, err := deriveForwardTarget(r)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	if _, err := a.authorizeTokenForHostAndCapability(token, host, capHTTPProxy); err != nil {
		http.Error(w, err.Error(), http.StatusForbidden)
		return
	}
	a.forwardGenericRequest(w, r, targetURL)
}

func (a *app) forwardGitRequest(w http.ResponseWriter, r *http.Request, host, pathAndQuery string) {
	if !matchAnyHost(host, a.cfg.GitHosts) {
		http.Error(w, "git host is not enabled in JANUS_GIT_HTTP_HOSTS", http.StatusForbidden)
		return
	}
	if a.cfg.GitPassword == "" {
		http.Error(w, "missing JANUS_GIT_HTTP_PASSWORD on host", http.StatusServiceUnavailable)
		return
	}

	fullURL := fmt.Sprintf("https://%s/%s", host, strings.TrimPrefix(pathAndQuery, "/"))
	body, err := io.ReadAll(r.Body)
	if err != nil {
		http.Error(w, fmt.Sprintf("request body error: %v", err), http.StatusBadRequest)
		return
	}

	upReq, err := http.NewRequest(r.Method, fullURL, bytes.NewReader(body))
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	copyForwardHeaders(upReq.Header, r.Header)
	auth := base64.StdEncoding.EncodeToString([]byte(a.cfg.GitUsername + ":" + a.cfg.GitPassword))
	upReq.Header.Set("Authorization", "Basic "+auth)

	resp, err := a.httpClient.Do(upReq)
	if err != nil {
		http.Error(w, fmt.Sprintf("upstream request failed: %v", err), http.StatusBadGateway)
		return
	}
	defer resp.Body.Close()

	copyResponse(w, resp)
}

func (a *app) forwardGenericRequest(w http.ResponseWriter, r *http.Request, targetURL string) {
	body, err := io.ReadAll(r.Body)
	if err != nil {
		http.Error(w, fmt.Sprintf("request body error: %v", err), http.StatusBadRequest)
		return
	}

	upReq, err := http.NewRequest(r.Method, targetURL, bytes.NewReader(body))
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	copyForwardHeaders(upReq.Header, r.Header)

	resp, err := a.httpClient.Do(upReq)
	if err != nil {
		http.Error(w, fmt.Sprintf("upstream request failed: %v", err), http.StatusBadGateway)
		return
	}
	defer resp.Body.Close()

	copyResponse(w, resp)
}

func (a *app) handleHealth(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		methodNotAllowed(w)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"status":        "ok",
		"uptimeSeconds": int64(time.Since(a.startedAt).Seconds()),
	})
}

func (a *app) handleConfig(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		methodNotAllowed(w)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"proxyBind":           a.cfg.ProxyBind,
		"controlSocket":       a.cfg.ControlSocket,
		"defaultTtlSeconds":   a.cfg.DefaultTTLSeconds,
		"allowedHosts":        a.cfg.AllowedHosts,
		"gitHosts":            a.cfg.GitHosts,
		"defaultCapabilities": a.cfg.DefaultCapabilities,
		"knownCapabilities":   sortedKnownCapabilities(),
		"supports": map[string]any{
			"proxy":         []string{capHTTPProxy, capGitHTTP, capGitSSH},
			"typedAdapters": []string{capPostgresQuery, capDeployKubectl, capDeployHelm, capDeployTerraform},
		},
	})
}

func (a *app) handleSessions(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodPost:
		a.handleCreateSession(w, r)
	case http.MethodGet:
		a.handleListSessions(w)
	default:
		methodNotAllowed(w)
	}
}

func (a *app) handleCreateSession(w http.ResponseWriter, r *http.Request) {
	var req createSessionRequest
	if err := decodeJSON(r.Body, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}

	ttl := a.cfg.DefaultTTLSeconds
	if req.TTLSeconds != nil {
		ttl = *req.TTLSeconds
	}
	if ttl < 60 {
		ttl = 60
	}
	if ttl > 86400 {
		ttl = 86400
	}

	allowedHosts := req.AllowedHosts
	if len(allowedHosts) == 0 {
		allowedHosts = append([]string{}, a.cfg.AllowedHosts...)
	}
	for i := range allowedHosts {
		allowedHosts[i] = strings.ToLower(strings.TrimSpace(allowedHosts[i]))
	}
	allowedHosts = filterNonEmpty(allowedHosts)
	if len(allowedHosts) == 0 {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": "allowed_hosts resolved to empty set"})
		return
	}

	requestedCaps := req.Capabilities
	if len(requestedCaps) == 0 {
		requestedCaps = append([]string{}, a.cfg.DefaultCapabilities...)
	}
	caps, err := normalizeCapabilities(requestedCaps)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}

	token, err := randomToken(32)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": "failed to generate token"})
		return
	}

	now := time.Now().UTC()
	s := session{
		ID:           uuid.NewString(),
		Token:        token,
		CreatedAt:    now,
		ExpiresAt:    now.Add(time.Duration(ttl) * time.Second),
		AllowedHosts: allowedHosts,
		Capabilities: caps,
	}

	a.sessionsM.Lock()
	a.cleanupExpiredSessionsLocked(now)
	a.sessions[s.ID] = s
	a.sessionsM.Unlock()

	writeJSON(w, http.StatusCreated, createSessionResponse{
		SessionID:    s.ID,
		CreatedAt:    s.CreatedAt,
		ExpiresAt:    s.ExpiresAt,
		Capabilities: s.Capabilities,
		Env:          buildSessionEnv(a.cfg, s),
		Notes: []string{
			"Session carries capability token only; upstream credentials remain host-side.",
			"Control socket is not exposed in session env.",
		},
	})
}

func (a *app) handleListSessions(w http.ResponseWriter) {
	now := time.Now().UTC()
	a.sessionsM.Lock()
	a.cleanupExpiredSessionsLocked(now)
	list := make([]session, 0, len(a.sessions))
	for _, s := range a.sessions {
		list = append(list, s)
	}
	a.sessionsM.Unlock()

	sort.Slice(list, func(i, j int) bool { return list[i].CreatedAt.Before(list[j].CreatedAt) })
	for i := range list {
		list[i].Token = ""
	}
	writeJSON(w, http.StatusOK, list)
}

func (a *app) handleSessionDelete(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodDelete {
		methodNotAllowed(w)
		return
	}
	id := strings.TrimPrefix(r.URL.Path, "/v1/sessions/")
	if id == "" || strings.Contains(id, "/") {
		writeJSON(w, http.StatusNotFound, map[string]any{"error": "session not found"})
		return
	}
	a.sessionsM.Lock()
	_, ok := a.sessions[id]
	if ok {
		delete(a.sessions, id)
	}
	a.sessionsM.Unlock()
	if !ok {
		writeJSON(w, http.StatusNotFound, map[string]any{"error": "session not found"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"ok": true})
}

func (a *app) handlePostgresQuery(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		methodNotAllowed(w)
		return
	}
	var req postgresQueryRequest
	if err := decodeJSON(r.Body, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}

	s, err := a.getSessionForCapability(req.SessionID, capPostgresQuery)
	if err != nil {
		writeJSON(w, err.status, map[string]any{"error": err.message})
		return
	}

	sql := strings.TrimSpace(req.SQL)
	if sql == "" {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": "sql cannot be empty"})
		return
	}
	if len(sql) > 100000 {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": "sql exceeds 100000 characters"})
		return
	}

	args := []string{"-X", "-v", "ON_ERROR_STOP=1", "-P", "pager=off"}
	if db := strings.TrimSpace(req.Database); db != "" {
		args = append(args, "-d", db)
	}
	args = append(args, "-c", sql)

	env := map[string]string{}
	if a.cfg.Postgres.Host != "" {
		env["PGHOST"] = a.cfg.Postgres.Host
	}
	if a.cfg.Postgres.Port != "" {
		env["PGPORT"] = a.cfg.Postgres.Port
	}
	if a.cfg.Postgres.User != "" {
		env["PGUSER"] = a.cfg.Postgres.User
	}
	if a.cfg.Postgres.Database != "" {
		env["PGDATABASE"] = a.cfg.Postgres.Database
	}
	if a.cfg.Postgres.Password != "" {
		env["PGPASSWORD"] = a.cfg.Postgres.Password
	}

	timeoutSec := int64(60)
	if req.TimeoutSeconds != nil {
		timeoutSec = *req.TimeoutSeconds
	}
	if timeoutSec < 1 {
		timeoutSec = 1
	}
	if timeoutSec > 600 {
		timeoutSec = 600
	}

	resp, err := a.executeHostCommand(s, "psql", args, "", timeoutSec, env)
	if err != nil {
		writeJSON(w, err.status, map[string]any{"error": err.message})
		return
	}
	writeJSON(w, http.StatusOK, resp)
}

func (a *app) handleDeployKubectl(w http.ResponseWriter, r *http.Request) {
	a.handleDeployTool(w, r, "kubectl", capDeployKubectl, kubectlVerbs, kubectlForbiddenFlags)
}
func (a *app) handleDeployHelm(w http.ResponseWriter, r *http.Request) {
	a.handleDeployTool(w, r, "helm", capDeployHelm, helmVerbs, helmForbiddenFlags)
}
func (a *app) handleDeployTerraform(w http.ResponseWriter, r *http.Request) {
	a.handleDeployTool(w, r, "terraform", capDeployTerraform, terraformVerbs, terraformForbiddenFlags)
}

func (a *app) handleDeployTool(w http.ResponseWriter, r *http.Request, command, capability string, allowedVerbs map[string]struct{}, forbiddenFlags []string) {
	if r.Method != http.MethodPost {
		methodNotAllowed(w)
		return
	}
	var req deployRunRequest
	if err := decodeJSON(r.Body, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}

	s, e := a.getSessionForCapability(req.SessionID, capability)
	if e != nil {
		writeJSON(w, e.status, map[string]any{"error": e.message})
		return
	}
	if err := validateToolArgs(command, req.Args, allowedVerbs, forbiddenFlags); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}

	timeoutSec := int64(600)
	if req.TimeoutSeconds != nil {
		timeoutSec = *req.TimeoutSeconds
	}
	if timeoutSec < 1 {
		timeoutSec = 1
	}
	if timeoutSec > 3600 {
		timeoutSec = 3600
	}

	env := map[string]string{}
	if (command == "kubectl" || command == "helm") && a.cfg.KubeconfigPath != "" {
		env["KUBECONFIG"] = a.cfg.KubeconfigPath
	}

	resp, cmdErr := a.executeHostCommand(s, command, req.Args, strings.TrimSpace(req.CWD), timeoutSec, env)
	if cmdErr != nil {
		writeJSON(w, cmdErr.status, map[string]any{"error": cmdErr.message})
		return
	}
	writeJSON(w, http.StatusOK, resp)
}

type apiError struct {
	status  int
	message string
}

func (a *app) getSessionForCapability(sessionID, capability string) (session, *apiError) {
	now := time.Now().UTC()
	a.sessionsM.Lock()
	a.cleanupExpiredSessionsLocked(now)
	s, ok := a.sessions[sessionID]
	a.sessionsM.Unlock()
	if !ok {
		return session{}, &apiError{status: http.StatusNotFound, message: "unknown session_id"}
	}
	if !sessionHasCapability(s, capability) {
		return session{}, &apiError{status: http.StatusForbidden, message: fmt.Sprintf("session missing capability: %s", capability)}
	}
	return s, nil
}

func (a *app) executeHostCommand(s session, command string, args []string, cwd string, timeoutSeconds int64, extraEnv map[string]string) (commandResponse, *apiError) {
	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(timeoutSeconds)*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, command, args...)
	if cwd != "" {
		cmd.Dir = cwd
	}

	envList := []string{}
	for _, k := range []string{"PATH", "HOME", "LANG"} {
		if v := strings.TrimSpace(os.Getenv(k)); v != "" {
			envList = append(envList, k+"="+v)
		}
	}
	envList = append(envList, "JANUS_SESSION_ID="+s.ID)
	for k, v := range extraEnv {
		envList = append(envList, k+"="+v)
	}
	cmd.Env = envList

	var stdoutBuf, stderrBuf bytes.Buffer
	cmd.Stdout = &stdoutBuf
	cmd.Stderr = &stderrBuf
	err := cmd.Run()

	if ctx.Err() == context.DeadlineExceeded {
		return commandResponse{}, &apiError{status: http.StatusGatewayTimeout, message: fmt.Sprintf("%s timed out after %ds", command, timeoutSeconds)}
	}

	exitCode := 0
	if err != nil {
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) {
			exitCode = exitErr.ExitCode()
		} else {
			return commandResponse{}, &apiError{status: http.StatusInternalServerError, message: fmt.Sprintf("failed to run %s: %v", command, err)}
		}
	}

	stdout := a.redactText(s, stdoutBuf.String())
	stderr := a.redactText(s, stderrBuf.String())

	return commandResponse{Command: command, ExitCode: exitCode, Stdout: stdout, Stderr: stderr}, nil
}

func (a *app) redactText(s session, text string) string {
	secrets := []string{s.Token, a.cfg.GitPassword, a.cfg.Postgres.Password}
	out := text
	for _, secret := range secrets {
		secret = strings.TrimSpace(secret)
		if len(secret) < 4 {
			continue
		}
		out = strings.ReplaceAll(out, secret, "[REDACTED]")
	}
	return out
}

func (a *app) authorizeTokenForHostAndCapability(token, host, capability string) (session, error) {
	now := time.Now().UTC()
	a.sessionsM.Lock()
	a.cleanupExpiredSessionsLocked(now)
	var found session
	ok := false
	for _, s := range a.sessions {
		if s.Token == token {
			found = s
			ok = true
			break
		}
	}
	a.sessionsM.Unlock()
	if !ok {
		return session{}, errors.New("unknown or expired session token")
	}
	if !sessionHasCapability(found, capability) {
		return session{}, fmt.Errorf("session missing capability: %s", capability)
	}
	if !isHostAllowedForSession(host, found) {
		return session{}, fmt.Errorf("host not allowed by session policy: %s", host)
	}
	return found, nil
}

func (a *app) cleanupExpiredSessionsLocked(now time.Time) {
	for id, s := range a.sessions {
		if !s.ExpiresAt.After(now) {
			delete(a.sessions, id)
		}
	}
}

func parseGitRoute(u *url.URL) (string, string, bool) {
	path := u.Path
	if !strings.HasPrefix(path, "/git/") {
		return "", "", false
	}
	suffix := strings.TrimPrefix(path, "/git/")
	parts := strings.SplitN(suffix, "/", 2)
	if len(parts) == 0 || strings.TrimSpace(parts[0]) == "" {
		return "", "", false
	}
	host := normalizeHost(parts[0])
	rest := ""
	if len(parts) > 1 {
		rest = parts[1]
	}
	if u.RawQuery != "" {
		rest = rest + "?" + u.RawQuery
	}
	return host, rest, true
}

func deriveForwardTarget(r *http.Request) (string, string, error) {
	if r.URL.IsAbs() && r.URL.Host != "" {
		return r.URL.String(), normalizeHost(r.URL.Hostname()), nil
	}
	hostHeader := strings.TrimSpace(r.Host)
	if hostHeader == "" {
		hostHeader = strings.TrimSpace(r.Header.Get("Host"))
	}
	if hostHeader == "" {
		return "", "", errors.New("missing host header")
	}
	host := normalizeHost(strings.Split(hostHeader, ":")[0])
	path := r.URL.RequestURI()
	if path == "" {
		path = "/"
	}
	return "http://" + host + path, host, nil
}

func splitHostPort(authority string, defaultPort int) (string, int) {
	host := strings.TrimSpace(authority)
	port := defaultPort
	if h, p, err := net.SplitHostPort(authority); err == nil {
		host = h
		if parsed, convErr := strconv.Atoi(p); convErr == nil {
			port = parsed
		}
	} else if strings.Contains(authority, ":") {
		idx := strings.LastIndex(authority, ":")
		if idx > -1 && idx+1 < len(authority) {
			if parsed, convErr := strconv.Atoi(authority[idx+1:]); convErr == nil {
				host = authority[:idx]
				port = parsed
			}
		}
	}
	return normalizeHost(host), port
}

func extractToken(r *http.Request) string {
	for _, header := range []string{"Proxy-Authorization", "Authorization"} {
		if token := parseBasicToken(r.Header.Get(header)); token != "" {
			return token
		}
	}
	if token := strings.TrimSpace(r.Header.Get("x-janus-token")); token != "" {
		return token
	}
	return ""
}

func parseBasicToken(v string) string {
	v = strings.TrimSpace(v)
	if !strings.HasPrefix(v, "Basic ") {
		return ""
	}
	decoded, err := base64.StdEncoding.DecodeString(strings.TrimSpace(strings.TrimPrefix(v, "Basic ")))
	if err != nil {
		return ""
	}
	parts := strings.SplitN(string(decoded), ":", 2)
	if len(parts) != 2 {
		return ""
	}
	pwd := strings.TrimSpace(parts[1])
	return pwd
}

func copyForwardHeaders(dst, src http.Header) {
	for k, values := range src {
		lk := strings.ToLower(k)
		if lk == "host" || lk == "proxy-authorization" || lk == "authorization" || lk == "connection" || lk == "proxy-connection" || lk == "content-length" {
			continue
		}
		for _, v := range values {
			dst.Add(k, v)
		}
	}
}

func copyResponse(w http.ResponseWriter, resp *http.Response) {
	for k, values := range resp.Header {
		lk := strings.ToLower(k)
		if lk == "transfer-encoding" || lk == "connection" {
			continue
		}
		for _, v := range values {
			w.Header().Add(k, v)
		}
	}
	w.WriteHeader(resp.StatusCode)
	_, _ = io.Copy(w, resp.Body)
}

func buildSessionEnv(cfg config, s session) map[string]string {
	env := map[string]string{}

	if sessionHasCapability(s, capHTTPProxy) {
		proxyURL := fmt.Sprintf("http://janus:%s@%s", s.Token, cfg.ProxyBind)
		env["HTTP_PROXY"] = proxyURL
		env["HTTPS_PROXY"] = proxyURL
		env["ALL_PROXY"] = proxyURL
		env["NO_PROXY"] = "127.0.0.1,localhost"
	}
	env["JANUS_SESSION_ID"] = s.ID

	if sessionHasCapability(s, capGitHTTP) {
		type pair struct{ key, value string }
		entries := []pair{}
		for _, host := range cfg.GitHosts {
			if !isHostAllowedForSession(host, s) {
				continue
			}
			entries = append(entries, pair{
				key:   fmt.Sprintf("url.http://janus:%s@%s/git/%s/.insteadof", s.Token, cfg.ProxyBind, host),
				value: "https://" + host + "/",
			})
		}
		if len(entries) > 0 {
			env["GIT_CONFIG_COUNT"] = strconv.Itoa(len(entries))
			for i, p := range entries {
				env[fmt.Sprintf("GIT_CONFIG_KEY_%d", i)] = p.key
				env[fmt.Sprintf("GIT_CONFIG_VALUE_%d", i)] = p.value
			}
			env["GIT_TERMINAL_PROMPT"] = "0"
		}
	}
	if sessionHasCapability(s, capGitSSH) {
		env["GIT_SSH_COMMAND"] = buildGitSSHCommand(cfg, s)
		env["GIT_TERMINAL_PROMPT"] = "0"
	}

	return env
}

func buildGitSSHCommand(cfg config, s session) string {
	proxyHost, proxyPort := proxyDialHostPort(cfg.ProxyBind)
	proxyAuth := base64.StdEncoding.EncodeToString([]byte("janus:" + s.Token))
	proxyScript := fmt.Sprintf(
		`set -euo pipefail; host="%%h"; port="%%p"; exec 3<>/dev/tcp/%s/%d; printf "CONNECT %%s:%%s HTTP/1.1\r\nHost: %%s:%%s\r\nProxy-Authorization: Basic %s\r\n\r\n" "$host" "$port" "$host" "$port" >&3; IFS= read -r status <&3 || exit 1; case "$status" in *" 200 "*) ;; *) echo "janus proxy connect failed: $status" >&2; exit 1;; esac; cr=$(printf "\r"); while IFS= read -r line <&3; do if [ -z "$line" ] || [ "$line" = "$cr" ]; then break; fi; done; cat <&3 & bg=$!; cat >&3; wait "$bg" || true`,
		proxyHost,
		proxyPort,
		proxyAuth,
	)
	proxyCommand := "/bin/bash -lc " + shellSingleQuote(proxyScript)
	return "ssh -o ProxyCommand=" + shellSingleQuote(proxyCommand)
}

func proxyDialHostPort(proxyBind string) (string, int) {
	host, port := splitHostPort(proxyBind, 9080)
	if host == "" || host == "0.0.0.0" || host == "::" {
		host = "127.0.0.1"
	}
	return host, port
}

func shellSingleQuote(value string) string {
	return "'" + strings.ReplaceAll(value, "'", `'"'"'`) + "'"
}

func sessionHasCapability(s session, capability string) bool {
	for _, c := range s.Capabilities {
		if c == capability {
			return true
		}
	}
	return false
}

func normalizeCapabilities(raw []string) ([]string, error) {
	seen := map[string]struct{}{}
	for _, r := range raw {
		cap := strings.ToLower(strings.TrimSpace(r))
		if cap == "" {
			continue
		}
		if _, ok := knownCapabilities[cap]; !ok {
			return nil, fmt.Errorf("unknown capability: %s", cap)
		}
		seen[cap] = struct{}{}
	}
	if len(seen) == 0 {
		return nil, errors.New("capabilities resolved to empty set")
	}
	out := make([]string, 0, len(seen))
	for k := range seen {
		out = append(out, k)
	}
	sort.Strings(out)
	return out, nil
}

func isHostAllowedForSession(host string, s session) bool {
	for _, allowed := range s.AllowedHosts {
		if hostMatches(host, allowed) {
			return true
		}
	}
	return false
}

func hostMatches(host, allowed string) bool {
	h := normalizeHost(host)
	a := normalizeHost(allowed)
	return h == a || strings.HasSuffix(h, "."+a)
}

func normalizeHost(host string) string {
	host = strings.TrimSpace(strings.ToLower(host))
	host = strings.TrimSuffix(host, ".")
	return host
}

func validateToolArgs(command string, args []string, allowedVerbs map[string]struct{}, forbiddenFlags []string) error {
	if len(args) == 0 {
		return fmt.Errorf("%s args cannot be empty", command)
	}
	first := strings.ToLower(strings.TrimSpace(args[0]))
	if strings.HasPrefix(first, "-") {
		return fmt.Errorf("%s requires explicit verb as first argument (flags-first is denied)", command)
	}
	if _, ok := allowedVerbs[first]; !ok {
		verbs := make([]string, 0, len(allowedVerbs))
		for v := range allowedVerbs {
			verbs = append(verbs, v)
		}
		sort.Strings(verbs)
		return fmt.Errorf("%s verb '%s' is not allowed; allowed: %s", command, first, strings.Join(verbs, ","))
	}
	for _, arg := range args {
		n := strings.ToLower(strings.TrimSpace(arg))
		for _, forbidden := range forbiddenFlags {
			if n == forbidden || strings.HasPrefix(n, forbidden+"=") {
				return fmt.Errorf("%s argument '%s' is forbidden", command, arg)
			}
		}
	}
	return nil
}

func sortedKnownCapabilities() []string {
	out := make([]string, 0, len(knownCapabilities))
	for k := range knownCapabilities {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

func matchAnyHost(host string, allowed []string) bool {
	for _, h := range allowed {
		if hostMatches(host, h) {
			return true
		}
	}
	return false
}

func parseListEnv(name string, defaults []string) []string {
	raw := os.Getenv(name)
	if strings.TrimSpace(raw) == "" {
		return append([]string{}, defaults...)
	}
	parts := strings.Split(raw, ",")
	out := make([]string, 0, len(parts))
	for _, p := range parts {
		v := strings.ToLower(strings.TrimSpace(p))
		if v != "" {
			out = append(out, v)
		}
	}
	if len(out) == 0 {
		return append([]string{}, defaults...)
	}
	return out
}

func filterNonEmpty(values []string) []string {
	out := make([]string, 0, len(values))
	for _, v := range values {
		if strings.TrimSpace(v) != "" {
			out = append(out, v)
		}
	}
	return out
}

func randomToken(size int) (string, error) {
	b := make([]byte, size)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return hex.EncodeToString(b), nil
}

func writeJSON(w http.ResponseWriter, status int, payload any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(payload)
}

func decodeJSON(r io.Reader, out any) error {
	if r == nil {
		return nil
	}
	dec := json.NewDecoder(bufio.NewReader(r))
	dec.DisallowUnknownFields()
	if err := dec.Decode(out); err != nil {
		if errors.Is(err, io.EOF) {
			return nil
		}
		return err
	}
	return nil
}

func methodNotAllowed(w http.ResponseWriter) {
	writeJSON(w, http.StatusMethodNotAllowed, map[string]any{"error": "method not allowed"})
}

func getenvDefault(name, fallback string) string {
	v := strings.TrimSpace(os.Getenv(name))
	if v == "" {
		return fallback
	}
	return v
}

func parseIntEnv(name string, fallback int64) int64 {
	raw := strings.TrimSpace(os.Getenv(name))
	if raw == "" {
		return fallback
	}
	v, err := strconv.ParseInt(raw, 10, 64)
	if err != nil {
		return fallback
	}
	return v
}

func nonEmpty(v string) string {
	return strings.TrimSpace(v)
}
