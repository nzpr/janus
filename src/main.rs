use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
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
use tracing::{info, warn};
use uuid::Uuid;

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
    allowed_hosts: Vec<String>,
    git_hosts: Vec<String>,
    git_username: String,
    git_password: Option<String>,
    exec_allowlist: HashSet<String>,
    postgres: PostgresDefaults,
    show_banner: bool,
}

#[derive(Clone, Serialize)]
struct Session {
    id: String,
    token: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    allowed_hosts: Vec<String>,
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
}

#[derive(Serialize)]
struct CreateSessionResponse {
    session_id: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    env: HashMap<String, String>,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct SessionView {
    id: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    allowed_hosts: Vec<String>,
}

#[derive(Deserialize)]
struct ExecRequest {
    session_id: String,
    command: String,
    args: Option<Vec<String>>,
    cwd: Option<String>,
}

#[derive(Serialize)]
struct ExecResponse {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

type ProxyBody = Full<Bytes>;

type ApiResult<T> = Result<(StatusCode, Json<T>), (StatusCode, Json<Value>)>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let config = Arc::new(Config::from_env()?);
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

        let exec_allowlist = parse_list_env(
            "JANUS_EXEC_ALLOWLIST",
            &["git", "psql", "kubectl", "helm", "terraform", "ssh"],
        )
        .into_iter()
        .collect::<HashSet<String>>();

        let postgres = PostgresDefaults {
            host: env_non_empty("JANUS_POSTGRES_HOST"),
            port: env_non_empty("JANUS_POSTGRES_PORT"),
            user: env_non_empty("JANUS_POSTGRES_USER"),
            database: env_non_empty("JANUS_POSTGRES_DATABASE"),
            password: env_non_empty("JANUS_POSTGRES_PASSWORD"),
        };

        let show_banner = env::var("JANUS_NO_BANNER").unwrap_or_default() != "1";

        Ok(Self {
            proxy_bind,
            control_socket,
            default_ttl_seconds,
            allowed_hosts,
            git_hosts,
            git_username,
            git_password,
            exec_allowlist,
            postgres,
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

fn print_startup_banner(config: &Config) {
    eprintln!("JANUS HOST DAEMON");
    eprintln!("status: online");
    eprintln!("proxy: {}", config.proxy_bind);
    eprintln!("control: {}", config.control_socket.display());
    eprintln!("quick use:");
    eprintln!(
        "  1) create session: curl --unix-socket {} -s -X POST http://localhost/v1/sessions",
        config.control_socket.display()
    );
    eprintln!("  2) apply returned env to sandboxed client");
    eprintln!("  3) for host tooling use /v1/exec with session_id");
    eprintln!("more info: README.md");
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
        .route("/v1/config", get(api_config))
        .route(
            "/v1/sessions",
            post(api_create_session).get(api_list_sessions),
        )
        .route("/v1/sessions/{id}", delete(api_delete_session))
        .route("/v1/exec", post(api_exec))
        .with_state(state.clone());

    info!(socket = %state.config.control_socket.display(), "control API listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api_health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let uptime = (Utc::now() - state.started_at).num_seconds().max(0);
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "uptimeSeconds": uptime,
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
            "execAllowlist": state.config.exec_allowlist,
            "supports": ["http_proxy", "git_http", "host_exec"],
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

    let now = Utc::now();
    let session = Session {
        id: Uuid::new_v4().to_string(),
        token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        created_at: now,
        expires_at: now + chrono::Duration::seconds(ttl as i64),
        allowed_hosts,
    };

    let env = build_session_env(&state.config, &session);

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session.id.clone(), session.clone());
    }

    Ok((
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            session_id: session.id,
            created_at: session.created_at,
            expires_at: session.expires_at,
            env,
            notes: vec![
                "Session carries capability token only; upstream credentials remain host-side.".to_string(),
                "Use /v1/exec for host-native tools (psql/kubectl/terraform/ssh) when direct protocol proxying is unavailable.".to_string(),
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
        (StatusCode::OK, Json(json!({"ok": true})))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "session not found"})),
        )
    }
}

async fn api_exec(
    State(state): State<AppState>,
    Json(payload): Json<ExecRequest>,
) -> ApiResult<ExecResponse> {
    cleanup_expired_sessions(&state).await;

    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&payload.session_id).cloned()
    };

    let session = match session {
        Some(session) => session,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "unknown session_id"})),
            ));
        }
    };

    if !state
        .config
        .exec_allowlist
        .contains(&payload.command.to_lowercase())
    {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "command is not allowed by JANUS_EXEC_ALLOWLIST"})),
        ));
    }

    let mut cmd = Command::new(&payload.command);
    for arg in payload.args.unwrap_or_default() {
        cmd.arg(arg);
    }

    if let Some(cwd) = payload.cwd {
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

    for (key, value) in build_session_env(&state.config, &session) {
        cmd.env(key, value);
    }

    if payload.command == "psql" {
        if let Some(host) = &state.config.postgres.host {
            cmd.env("PGHOST", host);
        }
        if let Some(port) = &state.config.postgres.port {
            cmd.env("PGPORT", port);
        }
        if let Some(user) = &state.config.postgres.user {
            cmd.env("PGUSER", user);
        }
        if let Some(database) = &state.config.postgres.database {
            cmd.env("PGDATABASE", database);
        }
        if let Some(password) = &state.config.postgres.password {
            cmd.env("PGPASSWORD", password);
        }
    }

    let output = cmd.output().await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to run command: {error}")})),
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(ExecResponse {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }),
    ))
}

fn build_session_env(config: &Config, session: &Session) -> HashMap<String, String> {
    let mut env_map = HashMap::new();
    let proxy_url = format!("http://janus:{}@{}", session.token, config.proxy_bind);
    env_map.insert("HTTP_PROXY".to_string(), proxy_url.clone());
    env_map.insert("HTTPS_PROXY".to_string(), proxy_url.clone());
    env_map.insert("ALL_PROXY".to_string(), proxy_url.clone());
    env_map.insert("NO_PROXY".to_string(), "127.0.0.1,localhost".to_string());
    env_map.insert("JANUS_SESSION_ID".to_string(), session.id.clone());
    env_map.insert(
        "JANUS_CONTROL_SOCKET".to_string(),
        config.control_socket.display().to_string(),
    );

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

    env_map
}

async fn run_proxy_server(state: AppState) -> anyhow::Result<()> {
    let listener = TcpListener::bind(state.config.proxy_bind).await?;
    info!(bind = %state.config.proxy_bind, "proxy listening");

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let service = service_fn(move |req| proxy_entry(req, state.clone()));
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
) -> Result<Response<ProxyBody>, Infallible> {
    let response = if req.method() == Method::CONNECT {
        proxy_connect(req, state).await
    } else {
        proxy_forward(req, state).await
    };
    Ok(response)
}

async fn proxy_connect(req: Request<Incoming>, state: AppState) -> Response<ProxyBody> {
    let target = match req.uri().authority().map(|a| a.as_str().to_string()) {
        Some(value) => value,
        None => {
            return proxy_error(
                StatusCode::BAD_REQUEST,
                "CONNECT requires host:port authority",
            )
        }
    };

    let (host, port) = parse_host_and_port(&target, 443);

    let token = match extract_token(req.headers()) {
        Some(token) => token,
        None => {
            return proxy_error(
                StatusCode::PROXY_AUTHENTICATION_REQUIRED,
                "missing proxy token",
            )
        }
    };

    if let Err(reason) = authorize_token_for_host(&state, &token, &host).await {
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

async fn proxy_forward(req: Request<Incoming>, state: AppState) -> Response<ProxyBody> {
    let token = match extract_token(req.headers()) {
        Some(token) => token,
        None => {
            return proxy_error(
                StatusCode::PROXY_AUTHENTICATION_REQUIRED,
                "missing proxy token",
            )
        }
    };

    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    if let Some((git_host, git_path)) = parse_git_route(&uri) {
        if let Err(reason) = authorize_token_for_host(&state, &token, &git_host).await {
            return proxy_error(StatusCode::FORBIDDEN, &reason);
        }
        return forward_git_request(method, headers, req.into_body(), git_host, git_path, state)
            .await;
    }

    let (url, host) = match derive_forward_target(&uri, &headers) {
        Ok(value) => value,
        Err(reason) => return proxy_error(StatusCode::BAD_REQUEST, &reason),
    };

    if let Err(reason) = authorize_token_for_host(&state, &token, &host).await {
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
) -> Response<ProxyBody> {
    if !state
        .config
        .git_hosts
        .iter()
        .any(|entry| host_matches(&host, entry))
    {
        return proxy_error(
            StatusCode::FORBIDDEN,
            "git host is not enabled in JANUS_GIT_HTTP_HOSTS",
        );
    }

    let password = match &state.config.git_password {
        Some(value) => value,
        None => {
            return proxy_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "missing JANUS_GIT_HTTP_PASSWORD on host",
            )
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
        Ok(response) => reqwest_to_proxy_response(response).await,
        Err(error) => proxy_error(
            StatusCode::BAD_GATEWAY,
            &format!("upstream request failed: {error}"),
        ),
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

async fn authorize_token_for_host(state: &AppState, token: &str, host: &str) -> Result<(), String> {
    cleanup_expired_sessions(state).await;

    let sessions = state.sessions.read().await;
    let session = sessions
        .values()
        .find(|session| session.token == token)
        .ok_or_else(|| "unknown or expired session token".to_string())?;

    if !is_host_allowed_for_session(host, session) {
        return Err(format!("host not allowed by session policy: {host}"));
    }

    Ok(())
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
