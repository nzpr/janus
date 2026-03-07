use super::*;

pub(super) async fn run_control_server(state: AppState) -> anyhow::Result<()> {
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
        .route("/v1/deploy/kubectl", post(adapters::api_deploy_kubectl))
        .route("/v1/deploy/helm", post(adapters::api_deploy_helm))
        .route("/v1/deploy/terraform", post(adapters::api_deploy_terraform))
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
                "gitSshAuthSockConfigured": state.config.git_ssh_auth_sock.is_some(),
                "typedAdapters": [
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
            "gitSshAuthSock": state.config.git_ssh_auth_sock,
            "defaultCapabilities": state.config.default_capabilities,
            "knownCapabilities": KNOWN_CAPABILITIES,
            "discovery": {
                "publicEndpoints": ["/health", "/v1/config"]
            },
            "executionModel": {
                "deterministic": true,
                "llmDriven": false,
                "notes": [
                    "janusd is a deterministic policy broker",
                    "no LLM inference or stochastic policy path in janusd"
                ]
            },
            "supports": {
                "proxy": crate::protocols::proxy_capabilities(),
                "typedAdapters": [CAP_DEPLOY_KUBECTL, CAP_DEPLOY_HELM, CAP_DEPLOY_TERRAFORM]
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
