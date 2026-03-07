use super::*;

pub(super) async fn api_postgres_query(
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

pub(super) async fn api_deploy_kubectl(
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

pub(super) async fn api_deploy_helm(
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

pub(super) async fn api_deploy_terraform(
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

pub(super) fn validate_tool_args(
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

pub(super) fn redact_text(state: &AppState, session: &Session, input: String) -> String {
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
