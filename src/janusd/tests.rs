use super::adapters::{redact_text, validate_tool_args};
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
        git_ssh_auth_sock: Some("/var/run/janus/ssh-agent.sock".to_string()),
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
    assert_eq!(
        env_map.get("SSH_AUTH_SOCK"),
        Some(&"/var/run/janus/ssh-agent.sock".to_string())
    );
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

    let denied = authorize_connect_token_for_host(&state, &session.token, "github.com", 443).await;
    assert!(denied.is_err());
}

#[tokio::test]
async fn connect_allows_postgres_wire_on_5432_only() {
    let cfg = Arc::new(test_config());
    let session = test_session(vec![CAP_POSTGRES_WIRE]);
    let mut session_map = HashMap::new();
    session_map.insert(session.id.clone(), session.clone());

    let state = AppState {
        config: cfg,
        sessions: Arc::new(RwLock::new(session_map)),
        http_client: Client::builder().build().expect("client"),
        started_at: Utc::now(),
    };

    let ok = authorize_connect_token_for_host(&state, &session.token, "github.com", 5432).await;
    assert!(ok.is_ok());

    let denied = authorize_connect_token_for_host(&state, &session.token, "github.com", 6379).await;
    assert!(denied.is_err());
}

#[tokio::test]
async fn connect_allows_redis_capability_on_6379() {
    let cfg = Arc::new(test_config());
    let session = test_session(vec![CAP_REDIS]);
    let mut session_map = HashMap::new();
    session_map.insert(session.id.clone(), session.clone());

    let state = AppState {
        config: cfg,
        sessions: Arc::new(RwLock::new(session_map)),
        http_client: Client::builder().build().expect("client"),
        started_at: Utc::now(),
    };

    let ok = authorize_connect_token_for_host(&state, &session.token, "github.com", 6379).await;
    assert!(ok.is_ok());
}
