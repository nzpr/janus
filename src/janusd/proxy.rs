use super::*;

pub(super) async fn run_proxy_server(state: AppState) -> anyhow::Result<()> {
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
        let required_capability = describe_connect_capability_requirement(port);
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

fn describe_connect_capability_requirement(port: u16) -> String {
    let capabilities = capabilities_for_connect_port(port);
    if capabilities.is_empty() {
        CAP_HTTP_PROXY.to_string()
    } else {
        format!("{CAP_HTTP_PROXY}|{}", capabilities.join("|"))
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
