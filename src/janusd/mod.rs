use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use clap::Parser;
use http::header::{AUTHORIZATION, HOST, PROXY_AUTHORIZATION};
use http::{Method, Request, Response, StatusCode, Uri};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade;
use hyper_util::rt::TokioIo;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream, UnixListener};
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};
use tracing::{info, warn};
use uuid::Uuid;

const CAP_HTTP_PROXY: &str = crate::protocols::http_proxy::CAPABILITY;
const CAP_GIT_HTTP: &str = crate::protocols::git_http::CAPABILITY;
const CAP_GIT_SSH: &str = crate::protocols::git_ssh::CAPABILITY;
const CAP_POSTGRES_WIRE: &str = crate::protocols::postgres_wire::CAPABILITY;
const CAP_MYSQL_WIRE: &str = crate::protocols::mysql_wire::CAPABILITY;
const CAP_REDIS: &str = crate::protocols::redis::CAPABILITY;
const CAP_MONGODB: &str = crate::protocols::mongodb::CAPABILITY;
const CAP_AMQP: &str = crate::protocols::amqp::CAPABILITY;
const CAP_KAFKA: &str = crate::protocols::kafka::CAPABILITY;
const CAP_NATS: &str = crate::protocols::nats::CAPABILITY;
const CAP_MQTT: &str = crate::protocols::mqtt::CAPABILITY;
const CAP_LDAP: &str = crate::protocols::ldap::CAPABILITY;
const CAP_SFTP: &str = crate::protocols::sftp::CAPABILITY;
const CAP_SMB: &str = crate::protocols::smb::CAPABILITY;
const CAP_DEPLOY_KUBECTL: &str = "deploy_kubectl";
const CAP_DEPLOY_HELM: &str = "deploy_helm";
const CAP_DEPLOY_TERRAFORM: &str = "deploy_terraform";

const KNOWN_CAPABILITIES: [&str; 17] = [
    CAP_HTTP_PROXY,
    CAP_GIT_HTTP,
    CAP_GIT_SSH,
    CAP_POSTGRES_WIRE,
    CAP_MYSQL_WIRE,
    CAP_REDIS,
    CAP_MONGODB,
    CAP_AMQP,
    CAP_KAFKA,
    CAP_NATS,
    CAP_MQTT,
    CAP_LDAP,
    CAP_SFTP,
    CAP_SMB,
    CAP_DEPLOY_KUBECTL,
    CAP_DEPLOY_HELM,
    CAP_DEPLOY_TERRAFORM,
];

const KUBECTL_VERBS: [&str; 8] = [
    "get", "describe", "logs", "apply", "delete", "rollout", "patch", "exec",
];
const HELM_VERBS: [&str; 8] = [
    "list",
    "status",
    "install",
    "upgrade",
    "uninstall",
    "repo",
    "template",
    "lint",
];
const TERRAFORM_VERBS: [&str; 7] = [
    "init", "plan", "apply", "destroy", "output", "validate", "fmt",
];

const KUBECTL_FORBIDDEN_FLAGS: [&str; 6] = [
    "--token",
    "--username",
    "--password",
    "--client-key",
    "--client-certificate",
    "--kubeconfig",
];
const HELM_FORBIDDEN_FLAGS: [&str; 5] = [
    "--kube-token",
    "--kubeconfig",
    "--username",
    "--password",
    "--pass-credentials",
];
const TERRAFORM_FORBIDDEN_FLAGS: [&str; 2] = ["-var", "-var-file"];

mod adapters;
mod control;
mod proxy;

#[cfg(test)]
mod tests;

#[derive(Parser, Debug)]
#[command(
    name = "janusd",
    version,
    about = "Janus host-side secret broker daemon",
    long_about = "Janus runs on the host and keeps upstream credentials host-side.\n\
Sandboxed LLM agents get only short-lived capability sessions (tokens, proxy wiring, policy scopes), not raw secrets.\n\
How it works:\n\
  - control plane: local Unix socket API for host-managed sessions and typed adapters\n\
  - data plane: HTTP(S) proxy, Git-over-HTTP credential injection, and Git-over-SSH transport/auth wiring\n\
  - adapters: typed deployment endpoints for operations not fully proxy-mediated\n\
Why this is safer:\n\
  - no generic remote shell endpoint\n\
  - capability checks on every request\n\
  - per-session host allowlist\n\
  - secrets never returned by API and command output is redacted\n\
Run with no arguments for defaults."
)]
struct Cli {
    #[arg(long, help = "Disable startup banner")]
    no_banner: bool,
}

#[derive(Clone)]
struct Config {
    proxy_bind: SocketAddr,
    control_socket: PathBuf,
    default_ttl_seconds: u64,
    default_capabilities: Vec<String>,
    allowed_hosts: Vec<String>,
    git_hosts: Vec<String>,
    git_username: String,
    git_password: Option<String>,
    git_ssh_auth_sock: Option<String>,
    kubeconfig_path: Option<String>,
    show_banner: bool,
}

#[derive(Clone, Serialize)]
struct Session {
    id: String,
    token: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    allowed_hosts: Vec<String>,
    capabilities: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    http_client: Client,
    started_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    ttl_seconds: Option<u64>,
    allowed_hosts: Option<Vec<String>>,
    capabilities: Option<Vec<String>>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    session_id: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    capabilities: Vec<String>,
    env: HashMap<String, String>,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct SessionView {
    id: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    allowed_hosts: Vec<String>,
    capabilities: Vec<String>,
}

#[derive(Deserialize)]
struct DeployRunRequest {
    session_id: String,
    args: Vec<String>,
    cwd: Option<String>,
    timeout_seconds: Option<u64>,
}

#[derive(Serialize)]
struct CommandResponse {
    command: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

type ProxyBody = Full<Bytes>;
type ApiResult<T> = Result<(StatusCode, Json<T>), (StatusCode, Json<Value>)>;

pub(crate) async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let mut parsed = Config::from_env()?;
    if cli.no_banner {
        parsed.show_banner = false;
    }

    let config = Arc::new(parsed);
    let state = AppState {
        config: config.clone(),
        sessions: Arc::new(RwLock::new(HashMap::new())),
        http_client: Client::builder().build()?,
        started_at: Utc::now(),
    };

    if config.show_banner {
        print_startup_banner(&config);
    }

    let proxy_state = state.clone();
    let control_state = state.clone();

    let proxy_task = tokio::spawn(async move { proxy::run_proxy_server(proxy_state).await });
    let control_task =
        tokio::spawn(async move { control::run_control_server(control_state).await });

    let (proxy_res, control_res) = tokio::join!(proxy_task, control_task);
    proxy_res??;
    control_res??;

    Ok(())
}

impl Config {
    fn from_env() -> anyhow::Result<Self> {
        let proxy_bind = env::var("JANUS_PROXY_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9080".to_string())
            .parse::<SocketAddr>()?;

        let control_socket = PathBuf::from(
            env::var("JANUS_CONTROL_SOCKET")
                .unwrap_or_else(|_| "/tmp/janusd-control.sock".to_string()),
        );

        let default_ttl_seconds = env::var("JANUS_DEFAULT_TTL_SECONDS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(3600)
            .clamp(60, 86_400);

        let default_capabilities = normalize_capabilities(parse_list_env(
            "JANUS_DEFAULT_CAPABILITIES",
            &[CAP_HTTP_PROXY, CAP_GIT_HTTP],
        ))
        .map_err(anyhow::Error::msg)?;

        let git_hosts = parse_list_env("JANUS_GIT_HTTP_HOSTS", &["github.com"]);
        let allowed_hosts = parse_list_env(
            "JANUS_ALLOWED_HOSTS",
            &["github.com", "api.github.com", "gitlab.com"],
        );

        let git_username =
            env::var("JANUS_GIT_HTTP_USERNAME").unwrap_or_else(|_| "x-access-token".to_string());
        let git_password = env::var("JANUS_GIT_HTTP_PASSWORD")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                env::var("JANUS_GIT_HTTP_TOKEN")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            });
        let git_ssh_auth_sock =
            env_non_empty("JANUS_GIT_SSH_AUTH_SOCK").or_else(|| env_non_empty("SSH_AUTH_SOCK"));

        let kubeconfig_path = env_non_empty("JANUS_KUBECONFIG");
        let show_banner = env::var("JANUS_NO_BANNER").unwrap_or_default() != "1";

        Ok(Self {
            proxy_bind,
            control_socket,
            default_ttl_seconds,
            default_capabilities,
            allowed_hosts,
            git_hosts,
            git_username,
            git_password,
            git_ssh_auth_sock,
            kubeconfig_path,
            show_banner,
        })
    }
}

fn env_non_empty(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_list_env(name: &str, defaults: &[&str]) -> Vec<String> {
    let raw = env::var(name).unwrap_or_else(|_| defaults.join(","));
    raw.split(',')
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .collect::<Vec<String>>()
}

fn normalize_capabilities(raw: Vec<String>) -> Result<Vec<String>, String> {
    let known = KNOWN_CAPABILITIES
        .iter()
        .copied()
        .collect::<HashSet<&str>>();

    let mut normalized = HashSet::new();
    for capability in raw {
        let cap = capability.trim().to_lowercase();
        if cap.is_empty() {
            continue;
        }
        if !known.contains(cap.as_str()) {
            return Err(format!("unknown capability: {cap}"));
        }
        normalized.insert(cap);
    }

    if normalized.is_empty() {
        return Err("capabilities resolved to empty set".to_string());
    }

    let mut capabilities = normalized.into_iter().collect::<Vec<String>>();
    capabilities.sort();
    Ok(capabilities)
}

fn session_has_capability(session: &Session, capability: &str) -> bool {
    session.capabilities.iter().any(|entry| entry == capability)
}

fn print_startup_banner(config: &Config) {
    eprintln!("     _    _    _   _ _   _ ____");
    eprintln!("    | |  / \\  | \\ | | | | / ___|");
    eprintln!(" _  | | / _ \\ |  \\| | | | \\___ \\");
    eprintln!("| |_| |/ ___ \\| |\\  | |_| |___) |");
    eprintln!(" \\___//_/   \\_\\_| \\_|\\___/|____/");
    eprintln!("status: online");
    eprintln!("proxy: {}", config.proxy_bind);
    eprintln!("control: {}", config.control_socket.display());
    if let Some(sock) = &config.git_ssh_auth_sock {
        eprintln!("git ssh auth sock: {sock}");
    }
    eprintln!("quick use:");
    eprintln!(
        "  curl --unix-socket {} -s http://localhost/v1/health",
        config.control_socket.display()
    );
    eprintln!(
        "  curl --unix-socket {} -s -X POST http://localhost/v1/sessions",
        config.control_socket.display()
    );
    eprintln!("  apply returned env map to sandbox runtime");
    eprintln!("for more info: janusd --help");
}

async fn get_session_for_capability(
    state: &AppState,
    session_id: &str,
    capability: &str,
) -> Result<Session, (StatusCode, Json<Value>)> {
    cleanup_expired_sessions(state).await;

    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(session_id).cloned()
    }
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "unknown session_id"})),
        )
    })?;

    if !session_has_capability(&session, capability) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": format!("session missing capability: {capability}")})),
        ));
    }

    Ok(session)
}

fn build_session_env(config: &Config, session: &Session) -> HashMap<String, String> {
    let mut env_map = HashMap::new();

    if session_has_capability(session, CAP_HTTP_PROXY) {
        let proxy_url = format!("http://janus:{}@{}", session.token, config.proxy_bind);
        env_map.insert("HTTP_PROXY".to_string(), proxy_url.clone());
        env_map.insert("HTTPS_PROXY".to_string(), proxy_url.clone());
        env_map.insert("ALL_PROXY".to_string(), proxy_url);
        env_map.insert("NO_PROXY".to_string(), "127.0.0.1,localhost".to_string());
    }

    env_map.insert("JANUS_SESSION_ID".to_string(), session.id.clone());

    if session_has_capability(session, CAP_GIT_HTTP) {
        let mut entries: Vec<(String, String)> = Vec::new();
        for host in &config.git_hosts {
            if !is_host_allowed_for_session(host, session) {
                continue;
            }
            entries.push((
                format!(
                    "url.http://janus:{}@{}/git/{}/.insteadof",
                    session.token, config.proxy_bind, host
                ),
                format!("https://{host}/"),
            ));
        }

        if !entries.is_empty() {
            env_map.insert("GIT_CONFIG_COUNT".to_string(), entries.len().to_string());
            for (idx, (key, value)) in entries.into_iter().enumerate() {
                env_map.insert(format!("GIT_CONFIG_KEY_{idx}"), key);
                env_map.insert(format!("GIT_CONFIG_VALUE_{idx}"), value);
            }
            env_map.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
        }
    }
    if session_has_capability(session, CAP_GIT_SSH) {
        env_map.insert(
            "GIT_SSH_COMMAND".to_string(),
            build_git_ssh_command(config, session),
        );
        if let Some(sock) = &config.git_ssh_auth_sock {
            env_map.insert("SSH_AUTH_SOCK".to_string(), sock.clone());
        }
        env_map.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
    }

    env_map
}

fn build_git_ssh_command(config: &Config, session: &Session) -> String {
    let (proxy_host, proxy_port) = proxy_dial_host_port(config.proxy_bind);
    let proxy_auth = BASE64.encode(format!("janus:{}", session.token));
    let proxy_script = format!(
        r#"set -euo pipefail; host="%h"; port="%p"; exec 3<>/dev/tcp/{proxy_host}/{proxy_port}; printf "CONNECT $host:$port HTTP/1.1\r\nHost: $host:$port\r\nProxy-Authorization: Basic {proxy_auth}\r\n\r\n" >&3; IFS= read -r status <&3 || exit 1; case "$status" in *" 200 "*) ;; *) echo "janus proxy connect failed: $status" >&2; exit 1;; esac; cr=$(printf "\r"); while IFS= read -r line <&3; do if [ -z "$line" ] || [ "$line" = "$cr" ]; then break; fi; done; cat <&3 & bg=$!; cat >&3; wait "$bg" || true"#
    );
    let proxy_command = format!("/bin/bash -lc {}", shell_single_quote(&proxy_script));
    format!("ssh -o ProxyCommand={}", shell_single_quote(&proxy_command))
}

fn proxy_dial_host_port(proxy_bind: SocketAddr) -> (String, u16) {
    let host = match proxy_bind.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        IpAddr::V6(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        ip => ip.to_string(),
    };
    (host, proxy_bind.port())
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'"'"'"#))
}

fn extract_token(headers: &http::HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(PROXY_AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_basic_token)
    {
        return Some(value);
    }

    if let Some(value) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_basic_token)
    {
        return Some(value);
    }

    headers
        .get("x-janus-token")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

fn parse_basic_token(value: &str) -> Option<String> {
    if !value.starts_with("Basic ") {
        return None;
    }

    let payload = value.trim_start_matches("Basic ").trim();
    let decoded = BASE64.decode(payload).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let mut parts = decoded.splitn(2, ':');
    let _user = parts.next()?;
    let password = parts.next()?.trim();
    if password.is_empty() {
        None
    } else {
        Some(password.to_string())
    }
}

async fn authorize_token_for_host_and_capability(
    state: &AppState,
    token: &str,
    host: &str,
    capability: &str,
) -> Result<Session, String> {
    cleanup_expired_sessions(state).await;

    let sessions = state.sessions.read().await;
    let session = sessions
        .values()
        .find(|session| session.token == token)
        .cloned()
        .ok_or_else(|| "unknown or expired session token".to_string())?;

    if !session_has_capability(&session, capability) {
        return Err(format!("session missing capability: {capability}"));
    }

    if !is_host_allowed_for_session(host, &session) {
        return Err(format!("host not allowed by session policy: {host}"));
    }

    Ok(session)
}

async fn authorize_connect_token_for_host(
    state: &AppState,
    token: &str,
    host: &str,
    port: u16,
) -> Result<(), String> {
    match authorize_token_for_host_and_capability(state, token, host, CAP_HTTP_PROXY).await {
        Ok(_) => Ok(()),
        Err(proxy_reason) => {
            if !is_missing_capability_error(&proxy_reason) {
                return Err(proxy_reason);
            }
            let capabilities = capabilities_for_connect_port(port);
            if capabilities.is_empty() {
                return Err(proxy_reason);
            }
            for capability in &capabilities {
                match authorize_token_for_host_and_capability(state, token, host, capability).await
                {
                    Ok(_) => return Ok(()),
                    Err(reason) => {
                        if !is_missing_capability_error(&reason) {
                            return Err(reason);
                        }
                    }
                }
            }
            Err(format!(
                "session missing capability for CONNECT port {port}: requires one of {}",
                capabilities.join(",")
            ))
        }
    }
}

fn is_missing_capability_error(reason: &str) -> bool {
    reason.starts_with("session missing capability:")
}

fn capabilities_for_connect_port(port: u16) -> Vec<&'static str> {
    crate::protocols::connect_capabilities_for_port(port)
}

async fn cleanup_expired_sessions(state: &AppState) {
    let mut sessions = state.sessions.write().await;
    let now = Utc::now();
    sessions.retain(|_, session| session.expires_at > now);
}

fn is_host_allowed_for_session(host: &str, session: &Session) -> bool {
    session
        .allowed_hosts
        .iter()
        .any(|allowed| host_matches(host, allowed))
}

fn host_matches(host: &str, allowed: &str) -> bool {
    let host = normalize_host(host);
    let allowed = normalize_host(allowed);
    host == allowed || host.ends_with(&format!(".{allowed}"))
}

fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_lowercase()
}

fn proxy_error(status: StatusCode, message: &str) -> Response<ProxyBody> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::from(message.to_string())))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("internal proxy error"))))
}
