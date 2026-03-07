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

const CAP_HTTP_PROXY: &str = "http_proxy";
const CAP_GIT_HTTP: &str = "git_http";
const CAP_GIT_SSH: &str = "git_ssh";
const CAP_POSTGRES_QUERY: &str = "postgres_query";
const CAP_DEPLOY_KUBECTL: &str = "deploy_kubectl";
const CAP_DEPLOY_HELM: &str = "deploy_helm";
const CAP_DEPLOY_TERRAFORM: &str = "deploy_terraform";

const KNOWN_CAPABILITIES: [&str; 7] = [
    CAP_HTTP_PROXY,
    CAP_GIT_HTTP,
    CAP_GIT_SSH,
    CAP_POSTGRES_QUERY,
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

#[derive(Parser, Debug)]
#[command(
    name = "janusd",
    version,
    about = "Janus host-side secret broker daemon",
    long_about = "Janus runs on the host and keeps upstream credentials host-side.\n\
Sandboxed LLM agents get only short-lived capability sessions (tokens, proxy wiring, policy scopes), not raw secrets.\n\
How it works:\n\
  - control plane: local Unix socket API for host-managed sessions and typed adapters\n\
  - data plane: HTTP(S) proxy and Git-over-HTTP credential injection\n\
  - adapters: typed Postgres/deployment endpoints for protocols not fully proxy-mediated\n\
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
struct PostgresDefaults {
    host: Option<String>,
    port: Option<String>,
    user: Option<String>,
    database: Option<String>,
    password: Option<String>,
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
    postgres: PostgresDefaults,
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
struct PostgresQueryRequest {
    session_id: String,
    sql: String,
    database: Option<String>,
    timeout_seconds: Option<u64>,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let proxy_task = tokio::spawn(async move { run_proxy_server(proxy_state).await });
    let control_task = tokio::spawn(async move { run_control_server(control_state).await });

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

        let postgres = PostgresDefaults {
            host: env_non_empty("JANUS_POSTGRES_HOST"),
            port: env_non_empty("JANUS_POSTGRES_PORT"),
            user: env_non_empty("JANUS_POSTGRES_USER"),
            database: env_non_empty("JANUS_POSTGRES_DATABASE"),
            password: env_non_empty("JANUS_POSTGRES_PASSWORD"),
        };

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
            postgres,
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

async fn run_control_server(state: AppState) -> anyhow::Result<()> {
    if Path::new(&state.config.control_socket).exists() {
        std::fs::remove_file(&state.config.control_socket)?;
    }

    let listener = UnixListener::bind(&state.config.control_socket)?;
    std::fs::set_permissions(
        &state.config.control_socket,
        std::fs::Permissions::from_mode(0o600),
    )?;

    let app = Router::new()
        .route("/health", get(api_health))
        .route("/v1/health", get(api_health))
        .route("/v1/config", get(api_config))
        .route(
            "/v1/sessions",
            post(api_create_session).get(api_list_sessions),
        )
        .route("/v1/sessions/{id}", delete(api_delete_session))
        .route("/v1/postgres/query", post(api_postgres_query))
        .route("/v1/deploy/kubectl", post(api_deploy_kubectl))
        .route("/v1/deploy/helm", post(api_deploy_helm))
        .route("/v1/deploy/terraform", post(api_deploy_terraform))
        .with_state(state.clone());

    info!(socket = %state.config.control_socket.display(), "control API listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api_health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let uptime = (Utc::now() - state.started_at).num_seconds().max(0);
    let generic_targets = state
        .config
        .allowed_hosts
        .iter()
        .map(|host| format!("https://{host}/*"))
        .collect::<Vec<String>>();
    let git_targets = state
        .config
        .git_hosts
        .iter()
        .map(|host| format!("/git/{host}/* -> https://{host}/*"))
        .collect::<Vec<String>>();
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "uptimeSeconds": uptime,
            "proxyBind": state.config.proxy_bind.to_string(),
            "controlSocket": state.config.control_socket,
            "capabilities": state.config.default_capabilities,
            "proxyableEndpoints": {
                "genericForward": generic_targets,
                "gitHttpRoutes": git_targets,
                "typedAdapters": [
                    "/v1/postgres/query",
                    "/v1/deploy/kubectl",
                    "/v1/deploy/helm",
                    "/v1/deploy/terraform"
                ]
            }
        })),
    )
}

async fn api_config(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "proxyBind": state.config.proxy_bind.to_string(),
            "controlSocket": state.config.control_socket,
            "defaultTtlSeconds": state.config.default_ttl_seconds,
            "allowedHosts": state.config.allowed_hosts,
            "gitHosts": state.config.git_hosts,
            "defaultCapabilities": state.config.default_capabilities,
            "knownCapabilities": KNOWN_CAPABILITIES,
            "supports": {
                "proxy": [CAP_HTTP_PROXY, CAP_GIT_HTTP, CAP_GIT_SSH],
                "typedAdapters": [CAP_POSTGRES_QUERY, CAP_DEPLOY_KUBECTL, CAP_DEPLOY_HELM, CAP_DEPLOY_TERRAFORM]
            }
        })),
    )
}

async fn api_create_session(
    State(state): State<AppState>,
    Json(payload): Json<CreateSessionRequest>,
) -> ApiResult<CreateSessionResponse> {
    let ttl = payload
        .ttl_seconds
        .unwrap_or(state.config.default_ttl_seconds)
        .clamp(60, 86_400);

    let allowed_hosts = payload
        .allowed_hosts
        .unwrap_or_else(|| state.config.allowed_hosts.clone())
        .into_iter()
        .map(|h| h.trim().to_lowercase())
        .filter(|h| !h.is_empty())
        .collect::<Vec<String>>();

    if allowed_hosts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "allowed_hosts resolved to empty set"})),
        ));
    }

    let requested_caps = payload
        .capabilities
        .unwrap_or_else(|| state.config.default_capabilities.clone());
    let capabilities = match normalize_capabilities(requested_caps) {
        Ok(value) => value,
        Err(reason) => {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": reason}))));
        }
    };

    let now = Utc::now();
    let session = Session {
        id: Uuid::new_v4().to_string(),
        token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        created_at: now,
        expires_at: now + chrono::Duration::seconds(ttl as i64),
        allowed_hosts,
        capabilities: capabilities.clone(),
    };

    let env = build_session_env(&state.config, &session);

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session.id.clone(), session.clone());
    }

    info!(
        event = "session_created",
        session_id = %session.id,
        capabilities = %session.capabilities.join(","),
        expires_at = %session.expires_at,
        "audit"
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            session_id: session.id,
            created_at: session.created_at,
            expires_at: session.expires_at,
            capabilities,
            env,
            notes: vec![
                "Session carries capability token only; upstream credentials remain host-side."
                    .to_string(),
                "Control socket is not exposed in session env.".to_string(),
            ],
        }),
    ))
}

async fn api_list_sessions(State(state): State<AppState>) -> (StatusCode, Json<Vec<SessionView>>) {
    cleanup_expired_sessions(&state).await;
    let sessions = state.sessions.read().await;
    let mut list = sessions
        .values()
        .map(|s| SessionView {
            id: s.id.clone(),
            created_at: s.created_at,
            expires_at: s.expires_at,
            allowed_hosts: s.allowed_hosts.clone(),
            capabilities: s.capabilities.clone(),
        })
        .collect::<Vec<SessionView>>();
    list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    (StatusCode::OK, Json(list))
}

async fn api_delete_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> (StatusCode, Json<Value>) {
    let mut sessions = state.sessions.write().await;
    let removed = sessions.remove(&id).is_some();
    if removed {
        info!(event = "session_deleted", session_id = %id, "audit");
        (StatusCode::OK, Json(json!({"ok": true})))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "session not found"})),
        )
    }
}

async fn api_postgres_query(
    State(state): State<AppState>,
    Json(payload): Json<PostgresQueryRequest>,
) -> ApiResult<CommandResponse> {
    let session =
        get_session_for_capability(&state, &payload.session_id, CAP_POSTGRES_QUERY).await?;

    let sql = payload.sql.trim();
    if sql.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "sql cannot be empty"})),
        ));
    }
    if sql.len() > 100_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "sql exceeds 100000 characters"})),
        ));
    }

    let mut args = vec![
        "-X".to_string(),
        "-v".to_string(),
        "ON_ERROR_STOP=1".to_string(),
        "-P".to_string(),
        "pager=off".to_string(),
    ];

    if let Some(database) = payload.database.as_ref().and_then(|d| non_empty_string(d)) {
        args.push("-d".to_string());
        args.push(database);
    }
    args.push("-c".to_string());
    args.push(sql.to_string());

    let mut extra_env = HashMap::new();
    if let Some(host) = &state.config.postgres.host {
        extra_env.insert("PGHOST".to_string(), host.clone());
    }
    if let Some(port) = &state.config.postgres.port {
        extra_env.insert("PGPORT".to_string(), port.clone());
    }
    if let Some(user) = &state.config.postgres.user {
        extra_env.insert("PGUSER".to_string(), user.clone());
    }
    if let Some(database) = &state.config.postgres.database {
        extra_env.insert("PGDATABASE".to_string(), database.clone());
    }
    if let Some(password) = &state.config.postgres.password {
        extra_env.insert("PGPASSWORD".to_string(), password.clone());
    }

    let timeout_seconds = payload.timeout_seconds.unwrap_or(60).clamp(1, 600);
    info!(
        event = "adapter_postgres_query",
        session_id = %session.id,
        timeout_seconds,
        "audit"
    );

    execute_host_command(
        &state,
        &session,
        "psql",
        &args,
        None,
        timeout_seconds,
        extra_env,
    )
    .await
}

async fn api_deploy_kubectl(
    State(state): State<AppState>,
    Json(payload): Json<DeployRunRequest>,
) -> ApiResult<CommandResponse> {
    run_deploy_tool(
        state,
        payload,
        "kubectl",
        CAP_DEPLOY_KUBECTL,
        &KUBECTL_VERBS,
        &KUBECTL_FORBIDDEN_FLAGS,
    )
    .await
}

async fn api_deploy_helm(
    State(state): State<AppState>,
    Json(payload): Json<DeployRunRequest>,
) -> ApiResult<CommandResponse> {
    run_deploy_tool(
        state,
        payload,
        "helm",
        CAP_DEPLOY_HELM,
        &HELM_VERBS,
        &HELM_FORBIDDEN_FLAGS,
    )
    .await
}

async fn api_deploy_terraform(
    State(state): State<AppState>,
    Json(payload): Json<DeployRunRequest>,
) -> ApiResult<CommandResponse> {
    run_deploy_tool(
        state,
        payload,
        "terraform",
        CAP_DEPLOY_TERRAFORM,
        &TERRAFORM_VERBS,
        &TERRAFORM_FORBIDDEN_FLAGS,
    )
    .await
}

async fn run_deploy_tool(
    state: AppState,
    payload: DeployRunRequest,
    command: &str,
    capability: &str,
    allowed_verbs: &[&str],
    forbidden_flags: &[&str],
) -> ApiResult<CommandResponse> {
    let session = get_session_for_capability(&state, &payload.session_id, capability).await?;

    validate_tool_args(command, &payload.args, allowed_verbs, forbidden_flags)
        .map_err(|reason| (StatusCode::BAD_REQUEST, Json(json!({"error": reason}))))?;

    let mut extra_env = HashMap::new();
    if (command == "kubectl" || command == "helm") && state.config.kubeconfig_path.is_some() {
        extra_env.insert(
            "KUBECONFIG".to_string(),
            state.config.kubeconfig_path.clone().unwrap_or_default(),
        );
    }

    let timeout_seconds = payload.timeout_seconds.unwrap_or(600).clamp(1, 3600);

    info!(
        event = "adapter_deploy_command",
        session_id = %session.id,
        capability,
        command,
        timeout_seconds,
        "audit"
    );

    execute_host_command(
        &state,
        &session,
        command,
        &payload.args,
        payload.cwd.as_deref(),
        timeout_seconds,
        extra_env,
    )
    .await
}

fn validate_tool_args(
    command: &str,
    args: &[String],
    allowed_verbs: &[&str],
    forbidden_flags: &[&str],
) -> Result<(), String> {
    if args.is_empty() {
        return Err(format!("{command} args cannot be empty"));
    }

    let first = args[0].trim().to_lowercase();
    if first.starts_with('-') {
        return Err(format!(
            "{command} requires explicit verb as first argument (flags-first is denied)"
        ));
    }

    if !allowed_verbs.iter().any(|verb| *verb == first) {
        return Err(format!(
            "{command} verb '{first}' is not allowed; allowed: {}",
            allowed_verbs.join(",")
        ));
    }

    for arg in args {
        let normalized = arg.trim().to_lowercase();
        for forbidden in forbidden_flags {
            if normalized == *forbidden || normalized.starts_with(&format!("{forbidden}=")) {
                return Err(format!("{command} argument '{arg}' is forbidden"));
            }
        }
    }

    Ok(())
}

async fn execute_host_command(
    state: &AppState,
    session: &Session,
    command: &str,
    args: &[String],
    cwd: Option<&str>,
    timeout_seconds: u64,
    extra_env: HashMap<String, String>,
) -> ApiResult<CommandResponse> {
    let mut cmd = Command::new(command);
    cmd.kill_on_drop(true);
    cmd.args(args);

    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    cmd.env_clear();
    if let Some(path) = env_non_empty("PATH") {
        cmd.env("PATH", path);
    }
    if let Some(home) = env_non_empty("HOME") {
        cmd.env("HOME", home);
    }
    if let Some(lang) = env_non_empty("LANG") {
        cmd.env("LANG", lang);
    }

    cmd.env("JANUS_SESSION_ID", session.id.clone());

    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    let output = match timeout(Duration::from_secs(timeout_seconds), cmd.output()).await {
        Ok(result) => result,
        Err(_) => {
            return Err((
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({"error": format!("{command} timed out after {timeout_seconds}s")})),
            ));
        }
    }
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to run {command}: {error}")})),
        )
    })?;

    let stdout = redact_text(
        state,
        session,
        String::from_utf8_lossy(&output.stdout).to_string(),
    );
    let stderr = redact_text(
        state,
        session,
        String::from_utf8_lossy(&output.stderr).to_string(),
    );

    Ok((
        StatusCode::OK,
        Json(CommandResponse {
            command: command.to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout,
            stderr,
        }),
    ))
}

fn redact_text(state: &AppState, session: &Session, input: String) -> String {
    let mut redacted = input;

    let mut secrets = vec![session.token.clone()];
    if let Some(secret) = &state.config.git_password {
        secrets.push(secret.clone());
    }
    if let Some(secret) = &state.config.postgres.password {
        secrets.push(secret.clone());
    }

    for secret in secrets {
        if secret.trim().is_empty() || secret.len() < 4 {
            continue;
        }
        redacted = redacted.replace(&secret, "[REDACTED]");
    }

    redacted
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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
        env_map.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
    }

    env_map
}

fn build_git_ssh_command(config: &Config, session: &Session) -> String {
    let (proxy_host, proxy_port) = proxy_dial_host_port(config.proxy_bind);
    let proxy_auth = BASE64.encode(format!("janus:{}", session.token));
    let proxy_script = format!(
        r#"set -euo pipefail; host="%h"; port="%p"; exec 3<>/dev/tcp/{proxy_host}/{proxy_port}; printf "CONNECT %s:%s HTTP/1.1\r\nHost: %s:%s\r\nProxy-Authorization: Basic {proxy_auth}\r\n\r\n" "$host" "$port" "$host" "$port" >&3; IFS= read -r status <&3 || exit 1; case "$status" in *" 200 "*) ;; *) echo "janus proxy connect failed: $status" >&2; exit 1;; esac; cr=$(printf "\r"); while IFS= read -r line <&3; do if [ -z "$line" ] || [ "$line" = "$cr" ]; then break; fi; done; cat <&3 & bg=$!; cat >&3; wait "$bg" || true"#
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

async fn run_proxy_server(state: AppState) -> anyhow::Result<()> {
    let listener = TcpListener::bind(state.config.proxy_bind).await?;
    info!(bind = %state.config.proxy_bind, "proxy listening");

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let service = service_fn(move |req| proxy_entry(req, state.clone(), addr));
            if let Err(error) = http1::Builder::new()
                .serve_connection(io, service)
                .with_upgrades()
                .await
            {
                warn!(peer = %addr, %error, "proxy connection error");
            }
        });
    }
}

async fn proxy_entry(
    req: Request<Incoming>,
    state: AppState,
    peer: SocketAddr,
) -> Result<Response<ProxyBody>, Infallible> {
    let response = if req.method() == Method::CONNECT {
        proxy_connect(req, state, peer).await
    } else {
        proxy_forward(req, state, peer).await
    };
    Ok(response)
}

async fn proxy_connect(
    req: Request<Incoming>,
    state: AppState,
    peer: SocketAddr,
) -> Response<ProxyBody> {
    let target = match req.uri().authority().map(|a| a.as_str().to_string()) {
        Some(value) => value,
        None => {
            warn!(
                event = "proxy_request_rejected",
                peer = %peer,
                method = "CONNECT",
                reason = "CONNECT requires host:port authority",
                "audit"
            );
            return proxy_error(
                StatusCode::BAD_REQUEST,
                "CONNECT requires host:port authority",
            );
        }
    };

    let (host, port) = parse_host_and_port(&target, 443);

    let token = match extract_token(req.headers()) {
        Some(token) => token,
        None => {
            warn!(
                event = "proxy_auth_failed",
                peer = %peer,
                method = "CONNECT",
                target_host = %host,
                reason = "missing proxy token",
                credentials_present = false,
                "audit"
            );
            return proxy_error(
                StatusCode::PROXY_AUTHENTICATION_REQUIRED,
                "missing proxy token",
            );
        }
    };

    if let Err(reason) = authorize_connect_token_for_host(&state, &token, &host, port).await {
        let required_capability = if port == 22 {
            format!("{CAP_HTTP_PROXY}|{CAP_GIT_SSH}")
        } else {
            CAP_HTTP_PROXY.to_string()
        };
        warn!(
            event = "proxy_auth_failed",
            peer = %peer,
            method = "CONNECT",
            target_host = %host,
            capability = %required_capability,
            reason = %reason,
            credentials_present = true,
            "audit"
        );
        return proxy_error(StatusCode::FORBIDDEN, &reason);
    }

    tokio::spawn(async move {
        match upgrade::on(req).await {
            Ok(upgraded) => {
                let mut upgraded = TokioIo::new(upgraded);
                match TcpStream::connect((host.as_str(), port)).await {
                    Ok(mut upstream) => {
                        if let Err(error) = copy_bidirectional(&mut upgraded, &mut upstream).await {
                            warn!(%error, "CONNECT tunnel copy failed");
                        }
                    }
                    Err(error) => {
                        warn!(%error, "CONNECT upstream dial failed");
                    }
                }
            }
            Err(error) => {
                warn!(%error, "CONNECT upgrade failed");
            }
        }
    });

    proxy_error(StatusCode::OK, "")
}

async fn proxy_forward(
    req: Request<Incoming>,
    state: AppState,
    peer: SocketAddr,
) -> Response<ProxyBody> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    let token = match extract_token(req.headers()) {
        Some(token) => token,
        None => {
            warn!(
                event = "proxy_auth_failed",
                peer = %peer,
                method = %method,
                route = %uri,
                reason = "missing proxy token",
                credentials_present = false,
                "audit"
            );
            return proxy_error(
                StatusCode::PROXY_AUTHENTICATION_REQUIRED,
                "missing proxy token",
            );
        }
    };

    if let Some((git_host, git_path)) = parse_git_route(&uri) {
        if let Err(reason) =
            authorize_token_for_host_and_capability(&state, &token, &git_host, CAP_GIT_HTTP).await
        {
            warn!(
                event = "proxy_auth_failed",
                peer = %peer,
                method = %method,
                route = %uri,
                target_host = %git_host,
                capability = CAP_GIT_HTTP,
                reason = %reason,
                credentials_present = true,
                "audit"
            );
            return proxy_error(StatusCode::FORBIDDEN, &reason);
        }
        return forward_git_request(
            method,
            headers,
            req.into_body(),
            git_host,
            git_path,
            state,
            peer,
        )
        .await;
    }

    let (url, host) = match derive_forward_target(&uri, &headers) {
        Ok(value) => value,
        Err(reason) => {
            warn!(
                event = "proxy_request_rejected",
                peer = %peer,
                method = %method,
                route = %uri,
                reason = %reason,
                "audit"
            );
            return proxy_error(StatusCode::BAD_REQUEST, &reason);
        }
    };

    if let Err(reason) =
        authorize_token_for_host_and_capability(&state, &token, &host, CAP_HTTP_PROXY).await
    {
        warn!(
            event = "proxy_auth_failed",
            peer = %peer,
            method = %method,
            route = %uri,
            target_host = %host,
            capability = CAP_HTTP_PROXY,
            reason = %reason,
            credentials_present = true,
            "audit"
        );
        return proxy_error(StatusCode::FORBIDDEN, &reason);
    }

    forward_generic_request(method, headers, req.into_body(), url, state).await
}

async fn forward_git_request(
    method: Method,
    headers: http::HeaderMap,
    body: Incoming,
    host: String,
    path_and_query: String,
    state: AppState,
    peer: SocketAddr,
) -> Response<ProxyBody> {
    if !state
        .config
        .git_hosts
        .iter()
        .any(|entry| host_matches(&host, entry))
    {
        warn!(
            event = "proxy_request_rejected",
            peer = %peer,
            target_host = %host,
            reason = "git host is not enabled in JANUS_GIT_HTTP_HOSTS",
            "audit"
        );
        return proxy_error(
            StatusCode::FORBIDDEN,
            "git host is not enabled in JANUS_GIT_HTTP_HOSTS",
        );
    }

    let password = match &state.config.git_password {
        Some(value) => value,
        None => {
            warn!(
                event = "proxy_upstream_unavailable",
                peer = %peer,
                target_host = %host,
                reason = "missing JANUS_GIT_HTTP_PASSWORD on host",
                "audit"
            );
            return proxy_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "missing JANUS_GIT_HTTP_PASSWORD on host",
            );
        }
    };

    let auth = format!(
        "Basic {}",
        BASE64.encode(format!("{}:{}", state.config.git_username, password))
    );

    let url = format!("https://{host}/{path_and_query}");

    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            return proxy_error(
                StatusCode::BAD_REQUEST,
                &format!("request body error: {error}"),
            )
        }
    };

    let mut request_builder = state.http_client.request(method, &url);
    for (name, value) in headers.iter() {
        let key = name.as_str().to_lowercase();
        if key == "host"
            || key == "proxy-authorization"
            || key == "authorization"
            || key == "connection"
            || key == "proxy-connection"
            || key == "content-length"
        {
            continue;
        }
        if let Ok(v) = value.to_str() {
            request_builder = request_builder.header(name.as_str(), v);
        }
    }

    request_builder = request_builder.header("authorization", auth);

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes.clone());
    }

    match request_builder.send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_client_error() || status.is_server_error() {
                warn!(
                    event = "proxy_upstream_response_error",
                    peer = %peer,
                    target_host = %host,
                    status = %status,
                    route = %path_and_query,
                    "audit"
                );
            }
            reqwest_to_proxy_response(response).await
        }
        Err(error) => {
            warn!(
                event = "proxy_upstream_request_failed",
                peer = %peer,
                target_host = %host,
                error = %error,
                "audit"
            );
            proxy_error(
                StatusCode::BAD_GATEWAY,
                &format!("upstream request failed: {error}"),
            )
        }
    }
}

async fn forward_generic_request(
    method: Method,
    headers: http::HeaderMap,
    body: Incoming,
    url: String,
    state: AppState,
) -> Response<ProxyBody> {
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            return proxy_error(
                StatusCode::BAD_REQUEST,
                &format!("request body error: {error}"),
            )
        }
    };

    let mut request_builder = state.http_client.request(method, &url);
    for (name, value) in headers.iter() {
        let key = name.as_str().to_lowercase();
        if key == "host"
            || key == "proxy-authorization"
            || key == "authorization"
            || key == "connection"
            || key == "proxy-connection"
            || key == "content-length"
        {
            continue;
        }
        if let Ok(v) = value.to_str() {
            request_builder = request_builder.header(name.as_str(), v);
        }
    }

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes);
    }

    match request_builder.send().await {
        Ok(response) => reqwest_to_proxy_response(response).await,
        Err(error) => proxy_error(
            StatusCode::BAD_GATEWAY,
            &format!("upstream request failed: {error}"),
        ),
    }
}

async fn reqwest_to_proxy_response(response: reqwest::Response) -> Response<ProxyBody> {
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return proxy_error(
                StatusCode::BAD_GATEWAY,
                &format!("failed to read upstream response body: {error}"),
            )
        }
    };

    let mut builder = Response::builder().status(status);
    for (name, value) in headers.iter() {
        let key = name.as_str().to_lowercase();
        if key == "transfer-encoding" || key == "connection" {
            continue;
        }
        builder = builder.header(name, value);
    }

    builder.body(Full::new(body_bytes)).unwrap_or_else(|_| {
        proxy_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to build response",
        )
    })
}

fn parse_git_route(uri: &Uri) -> Option<(String, String)> {
    let path = uri.path();
    if !path.starts_with("/git/") {
        return None;
    }

    let suffix = path.trim_start_matches("/git/");
    let mut segments = suffix.splitn(2, '/');
    let host = segments.next()?.trim().to_lowercase();
    let rest = segments.next().unwrap_or("");
    let path_and_query = if let Some(query) = uri.query() {
        format!("{rest}?{query}")
    } else {
        rest.to_string()
    };

    if host.is_empty() {
        return None;
    }

    Some((host, path_and_query))
}

fn derive_forward_target(uri: &Uri, headers: &http::HeaderMap) -> Result<(String, String), String> {
    if let Some(host) = uri.host() {
        let normalized_host = normalize_host(host);
        return Ok((uri.to_string(), normalized_host));
    }

    let host_header = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing host header".to_string())?;
    let host = normalize_host(host_header.split(':').next().unwrap_or(host_header));

    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    Ok((format!("http://{host}{path}"), host))
}

fn parse_host_and_port(authority: &str, default_port: u16) -> (String, u16) {
    let mut host = authority.to_string();
    let mut port = default_port;

    if let Some(idx) = authority.rfind(':') {
        let maybe_port = &authority[idx + 1..];
        if let Ok(parsed) = maybe_port.parse::<u16>() {
            host = authority[..idx].to_string();
            port = parsed;
        }
    }

    (normalize_host(&host), port)
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
            if port == 22 {
                authorize_token_for_host_and_capability(state, token, host, CAP_GIT_SSH)
                    .await
                    .map(|_| ())
            } else {
                Err(proxy_reason)
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            proxy_bind: "127.0.0.1:9080".parse().expect("valid socket"),
            control_socket: PathBuf::from("/tmp/janusd-control.sock"),
            default_ttl_seconds: 3600,
            default_capabilities: vec![CAP_HTTP_PROXY.to_string(), CAP_GIT_HTTP.to_string()],
            allowed_hosts: vec!["github.com".to_string()],
            git_hosts: vec!["github.com".to_string()],
            git_username: "x-access-token".to_string(),
            git_password: Some("ghp_secret_token".to_string()),
            postgres: PostgresDefaults {
                host: Some("db.internal".to_string()),
                port: Some("5432".to_string()),
                user: Some("janus".to_string()),
                database: Some("app".to_string()),
                password: Some("pg_secret_password".to_string()),
            },
            kubeconfig_path: None,
            show_banner: false,
        }
    }

    fn test_session(capabilities: Vec<&str>) -> Session {
        Session {
            id: "session-1".to_string(),
            token: "token-secret-value".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            allowed_hosts: vec!["github.com".to_string()],
            capabilities: capabilities.into_iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn normalize_capabilities_dedups_and_sorts() {
        let out = normalize_capabilities(vec![
            CAP_GIT_HTTP.to_string(),
            CAP_HTTP_PROXY.to_string(),
            CAP_GIT_HTTP.to_string(),
        ])
        .expect("normalize works");
        assert_eq!(
            out,
            vec![CAP_GIT_HTTP.to_string(), CAP_HTTP_PROXY.to_string()]
        );
    }

    #[test]
    fn normalize_capabilities_rejects_unknown() {
        let err = normalize_capabilities(vec!["unknown_cap".to_string()]).expect_err("must fail");
        assert!(err.contains("unknown capability"));
    }

    #[test]
    fn build_session_env_excludes_control_socket() {
        let cfg = test_config();
        let session = test_session(vec![CAP_HTTP_PROXY, CAP_GIT_HTTP]);
        let env_map = build_session_env(&cfg, &session);
        assert!(!env_map.contains_key("JANUS_CONTROL_SOCKET"));
    }

    #[test]
    fn build_session_env_scopes_proxy_vars_to_http_capability() {
        let cfg = test_config();
        let session = test_session(vec![CAP_GIT_HTTP]);
        let env_map = build_session_env(&cfg, &session);
        assert!(!env_map.contains_key("HTTP_PROXY"));
        assert!(env_map.contains_key("GIT_CONFIG_COUNT"));
    }

    #[test]
    fn build_session_env_includes_git_ssh_command() {
        let cfg = test_config();
        let session = test_session(vec![CAP_GIT_SSH]);
        let env_map = build_session_env(&cfg, &session);
        let cmd = env_map
            .get("GIT_SSH_COMMAND")
            .expect("GIT_SSH_COMMAND must exist");
        assert!(cmd.contains("ProxyCommand="));
        assert!(cmd.contains("/dev/tcp/127.0.0.1/9080"));
        assert!(!cmd.contains("token-secret-value"));
    }

    #[test]
    fn host_matches_supports_subdomains() {
        assert!(host_matches("api.github.com", "github.com"));
        assert!(host_matches("github.com", "github.com"));
        assert!(!host_matches("github.com.evil.com", "github.com"));
    }

    #[test]
    fn redact_text_removes_known_secrets() {
        let cfg = Arc::new(test_config());
        let session = test_session(vec![CAP_HTTP_PROXY]);
        let state = AppState {
            config: cfg,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            http_client: Client::builder().build().expect("client"),
            started_at: Utc::now(),
        };

        let text = "token-secret-value ghp_secret_token pg_secret_password".to_string();
        let redacted = redact_text(&state, &session, text);
        assert!(!redacted.contains("token-secret-value"));
        assert!(!redacted.contains("ghp_secret_token"));
        assert!(!redacted.contains("pg_secret_password"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn validate_tool_args_rejects_forbidden_flags() {
        let args = vec!["apply".to_string(), "--token=abc".to_string()];
        let result = validate_tool_args("kubectl", &args, &KUBECTL_VERBS, &KUBECTL_FORBIDDEN_FLAGS);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connect_allows_git_ssh_on_port_22_only() {
        let cfg = Arc::new(test_config());
        let session = test_session(vec![CAP_GIT_SSH]);
        let mut session_map = HashMap::new();
        session_map.insert(session.id.clone(), session.clone());

        let state = AppState {
            config: cfg,
            sessions: Arc::new(RwLock::new(session_map)),
            http_client: Client::builder().build().expect("client"),
            started_at: Utc::now(),
        };

        let ok = authorize_connect_token_for_host(&state, &session.token, "github.com", 22).await;
        assert!(ok.is_ok());

        let denied =
            authorize_connect_token_for_host(&state, &session.token, "github.com", 443).await;
        assert!(denied.is_err());
    }
}
