use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;

use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clap::Parser;
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use rand::distr::{Alphanumeric, SampleString};
use reqwest::Url;
use sha2::{Digest, Sha256};
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PROTOCOL_VERSION_3: i32 = 196_608;
const SSL_REQUEST_CODE: i32 = 808_771_03;
const CANCEL_REQUEST_CODE: i32 = 808_771_02;

#[derive(Parser, Debug)]
#[command(
    name = "janus-pg-sidecar",
    version,
    about = "PostgreSQL auth sidecar for jailed LLMs (no DB secret in jail process)"
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:15432", help = "Local listen address")]
    listen: SocketAddr,
    #[arg(long, help = "Upstream Postgres host")]
    target_host: String,
    #[arg(long, default_value_t = 5432, help = "Upstream Postgres port")]
    target_port: u16,
    #[arg(
        long,
        help = "Optional upstream username (fallback: startup packet user)"
    )]
    upstream_user: Option<String>,
    #[arg(
        long,
        help = "Optional upstream database (fallback: startup packet db/user)"
    )]
    upstream_db: Option<String>,
    #[arg(
        long,
        help = "Optional upstream password (fallback env JANUS_PG_PASSWORD)"
    )]
    upstream_password: Option<String>,
}

#[derive(Clone, Debug)]
struct ProxyConfig {
    host: String,
    port: u16,
    token: String,
    source_env: String,
}

#[derive(Clone, Debug)]
struct UpstreamTarget {
    host: String,
    port: u16,
}

#[derive(Debug, Clone)]
struct UpstreamAuthConfig {
    user: String,
    database: String,
    password: String,
}

#[derive(Debug)]
struct StartupMessage {
    protocol: i32,
    params: HashMap<String, String>,
}

#[derive(Debug)]
struct BackendMessage {
    kind: u8,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct ScramState {
    client_first_bare: String,
    expected_server_signature: Vec<u8>,
}

pub(crate) async fn run() -> Result<()> {
    let cli = Cli::parse();
    let target = UpstreamTarget {
        host: cli.target_host.trim().to_string(),
        port: cli.target_port,
    };
    let proxy = proxy_from_env()?;

    let listener = TcpListener::bind(cli.listen)
        .await
        .with_context(|| format!("failed binding {}", cli.listen))?;
    let local = listener.local_addr().context("failed reading local addr")?;

    eprintln!(
        "janus-pg-sidecar listening on {} -> {}:{} via janus proxy {}:{} ({})",
        local, target.host, target.port, proxy.host, proxy.port, proxy.source_env
    );
    eprintln!(
        "run psql with no password against sidecar: psql -h {} -p {} -U <user> <db>",
        local.ip(),
        local.port()
    );

    loop {
        let (client, peer) = listener.accept().await.context("accept failed")?;
        let target = target.clone();
        let proxy = proxy.clone();
        let configured_user = cli.upstream_user.clone();
        let configured_db = cli.upstream_db.clone();
        let configured_password = cli
            .upstream_password
            .clone()
            .or_else(|| env_non_empty("JANUS_PG_PASSWORD"));

        tokio::spawn(async move {
            if let Err(error) = handle_client(
                client,
                &target,
                &proxy,
                configured_user,
                configured_db,
                configured_password,
            )
            .await
            {
                eprintln!("janus-pg-sidecar client {peer} failed: {error}");
            }
        });
    }
}

async fn handle_client(
    mut client: TcpStream,
    target: &UpstreamTarget,
    proxy: &ProxyConfig,
    configured_user: Option<String>,
    configured_db: Option<String>,
    configured_password: Option<String>,
) -> Result<()> {
    let startup = read_client_startup(&mut client).await?;
    if startup.protocol != PROTOCOL_VERSION_3 {
        bail!("unsupported startup protocol: {}", startup.protocol);
    }

    let auth = resolve_upstream_auth(
        startup.params,
        configured_user,
        configured_db,
        configured_password,
    )?;

    let mut upstream = connect_via_proxy(proxy, target).await?;
    let startup_packet = build_startup_packet(&auth.user, &auth.database);
    upstream
        .write_all(&startup_packet)
        .await
        .context("failed sending startup to upstream")?;

    let prebuffer = authenticate_upstream(&mut upstream, &auth).await?;
    for msg in prebuffer {
        client
            .write_all(&msg)
            .await
            .context("failed writing startup response to client")?;
    }

    let _ = copy_bidirectional(&mut client, &mut upstream)
        .await
        .context("copy failed")?;

    Ok(())
}

fn resolve_upstream_auth(
    startup_params: HashMap<String, String>,
    configured_user: Option<String>,
    configured_db: Option<String>,
    configured_password: Option<String>,
) -> Result<UpstreamAuthConfig> {
    let user = configured_user
        .or_else(|| startup_params.get("user").cloned())
        .ok_or_else(|| {
            anyhow!("missing user: set --upstream-user or pass user in startup packet")
        })?;

    let database = configured_db
        .or_else(|| startup_params.get("database").cloned())
        .unwrap_or_else(|| user.clone());

    let password = configured_password.ok_or_else(|| {
        anyhow!("missing upstream password: set JANUS_PG_PASSWORD or --upstream-password")
    })?;

    Ok(UpstreamAuthConfig {
        user,
        database,
        password,
    })
}

async fn read_client_startup(stream: &mut TcpStream) -> Result<StartupMessage> {
    loop {
        let packet = read_startup_packet(stream).await?;
        let code = i32::from_be_bytes(packet[0..4].try_into().expect("length checked"));

        if code == SSL_REQUEST_CODE {
            stream
                .write_all(b"N")
                .await
                .context("failed writing SSL denial")?;
            continue;
        }

        if code == CANCEL_REQUEST_CODE {
            bail!("cancel request is not supported by sidecar startup handler");
        }

        let params = parse_startup_params(&packet[4..])?;
        return Ok(StartupMessage {
            protocol: code,
            params,
        });
    }
}

async fn read_startup_packet(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut len_buf = [0_u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .context("failed reading startup length")?;
    let len = i32::from_be_bytes(len_buf) as usize;
    if !(8..=8192).contains(&len) {
        bail!("invalid startup packet length: {len}");
    }

    let mut rest = vec![0_u8; len - 4];
    stream
        .read_exact(&mut rest)
        .await
        .context("failed reading startup packet body")?;
    Ok(rest)
}

fn parse_startup_params(mut bytes: &[u8]) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    while !bytes.is_empty() {
        if bytes[0] == 0 {
            break;
        }
        let (key, rem) = read_cstr(bytes)?;
        let (value, rem2) = read_cstr(rem)?;
        out.insert(key, value);
        bytes = rem2;
    }
    Ok(out)
}

fn read_cstr(bytes: &[u8]) -> Result<(String, &[u8])> {
    let pos = bytes
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(|| anyhow!("invalid cstr payload"))?;
    let value = String::from_utf8(bytes[..pos].to_vec()).context("invalid utf8 in startup cstr")?;
    Ok((value, &bytes[pos + 1..]))
}

fn build_startup_packet(user: &str, database: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&PROTOCOL_VERSION_3.to_be_bytes());

    write_cstr(&mut payload, "user");
    write_cstr(&mut payload, user);
    write_cstr(&mut payload, "database");
    write_cstr(&mut payload, database);
    write_cstr(&mut payload, "client_encoding");
    write_cstr(&mut payload, "UTF8");
    payload.push(0);

    let len = (payload.len() + 4) as i32;
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&payload);
    out
}

fn write_cstr(target: &mut Vec<u8>, value: &str) {
    target.extend_from_slice(value.as_bytes());
    target.push(0);
}

async fn connect_via_proxy(proxy: &ProxyConfig, target: &UpstreamTarget) -> Result<TcpStream> {
    let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port))
        .await
        .with_context(|| format!("failed dialing janus proxy {}:{}", proxy.host, proxy.port))?;

    let auth = BASE64.encode(format!("janus:{}", proxy.token));
    let request = format!(
        "CONNECT {h}:{p} HTTP/1.1\r\nHost: {h}:{p}\r\nProxy-Authorization: Basic {auth}\r\n\r\n",
        h = target.host,
        p = target.port,
        auth = auth
    );

    stream
        .write_all(request.as_bytes())
        .await
        .context("failed writing CONNECT request")?;

    let status = read_connect_status(&mut stream).await?;
    if !status.contains(" 200 ") {
        bail!("proxy CONNECT rejected: {status}");
    }

    Ok(stream)
}

async fn read_connect_status(stream: &mut TcpStream) -> Result<String> {
    const MAX_HEADER: usize = 16 * 1024;
    let mut buffer = Vec::with_capacity(1024);
    let mut temp = [0_u8; 1024];

    loop {
        let n = stream
            .read(&mut temp)
            .await
            .context("failed reading CONNECT response")?;
        if n == 0 {
            bail!("proxy closed connection before CONNECT response");
        }
        buffer.extend_from_slice(&temp[..n]);
        if buffer.len() > MAX_HEADER {
            bail!("CONNECT response too large");
        }
        if let Some(idx) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = &buffer[..idx + 4];
            let line = head
                .split(|b| *b == b'\n')
                .next()
                .ok_or_else(|| anyhow!("missing CONNECT status line"))?;
            return Ok(String::from_utf8_lossy(line).trim().to_string());
        }
    }
}

async fn authenticate_upstream(
    stream: &mut TcpStream,
    auth: &UpstreamAuthConfig,
) -> Result<Vec<Vec<u8>>> {
    let mut forward_to_client = Vec::new();
    let mut scram_state: Option<ScramState> = None;

    loop {
        let msg = read_backend_message(stream).await?;
        match msg.kind {
            b'R' => {
                let code =
                    i32::from_be_bytes(msg.payload[0..4].try_into().context("invalid auth msg")?);
                match code {
                    0 => {
                        forward_to_client.push(encode_backend_message(&msg));
                    }
                    3 => {
                        let mut payload = auth.password.as_bytes().to_vec();
                        payload.push(0);
                        write_frontend_message(stream, b'p', &payload).await?;
                    }
                    5 => {
                        if msg.payload.len() < 8 {
                            bail!("invalid md5 auth message");
                        }
                        let salt: [u8; 4] = msg.payload[4..8]
                            .try_into()
                            .map_err(|_| anyhow!("invalid md5 salt"))?;
                        let pwd = pg_md5_password(&auth.user, &auth.password, &salt);
                        let mut payload = pwd.as_bytes().to_vec();
                        payload.push(0);
                        write_frontend_message(stream, b'p', &payload).await?;
                    }
                    10 => {
                        let server_methods = parse_auth_sasl_methods(&msg.payload[4..])?;
                        if !server_methods.iter().any(|m| m == "SCRAM-SHA-256") {
                            bail!("upstream SASL does not advertise SCRAM-SHA-256");
                        }
                        let (client_first, state) =
                            build_scram_initial(&auth.user, &auth.password)?;
                        scram_state = Some(state);
                        write_sasl_initial_response(stream, "SCRAM-SHA-256", &client_first).await?;
                    }
                    11 => {
                        let state = scram_state
                            .take()
                            .ok_or_else(|| anyhow!("received SASLContinue without SCRAM state"))?;
                        let server_first = String::from_utf8(msg.payload[4..].to_vec())
                            .context("invalid SASLContinue payload")?;
                        let (client_final, next_state) =
                            build_scram_final(state, &auth.password, &server_first)?;
                        scram_state = Some(next_state);
                        write_frontend_message(stream, b'p', client_final.as_bytes()).await?;
                    }
                    12 => {
                        let state = scram_state
                            .take()
                            .ok_or_else(|| anyhow!("received SASLFinal without SCRAM state"))?;
                        let server_final = String::from_utf8(msg.payload[4..].to_vec())
                            .context("invalid SASLFinal payload")?;
                        verify_scram_server_final(&state, &server_final)?;
                    }
                    other => {
                        bail!("unsupported PostgreSQL auth code: {other}");
                    }
                }
            }
            b'E' => {
                forward_to_client.push(encode_backend_message(&msg));
                for payload in &forward_to_client {
                    stream.flush().await.ok();
                    // error gets forwarded to client later by caller before returning
                    let _ = payload;
                }
                bail!("upstream returned error during auth");
            }
            _ => {
                let raw = encode_backend_message(&msg);
                let is_ready = msg.kind == b'Z';
                forward_to_client.push(raw);
                if is_ready {
                    break;
                }
            }
        }
    }

    Ok(forward_to_client)
}

async fn read_backend_message(stream: &mut TcpStream) -> Result<BackendMessage> {
    let mut kind = [0_u8; 1];
    stream
        .read_exact(&mut kind)
        .await
        .context("failed reading backend message kind")?;

    let mut len_buf = [0_u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .context("failed reading backend message length")?;
    let len = i32::from_be_bytes(len_buf);
    if len < 4 || len > 1_048_576 {
        bail!("invalid backend message length: {len}");
    }

    let mut payload = vec![0_u8; (len - 4) as usize];
    stream
        .read_exact(&mut payload)
        .await
        .context("failed reading backend message payload")?;

    Ok(BackendMessage {
        kind: kind[0],
        payload,
    })
}

fn encode_backend_message(msg: &BackendMessage) -> Vec<u8> {
    let mut out = Vec::with_capacity(msg.payload.len() + 5);
    out.push(msg.kind);
    out.extend_from_slice(&((msg.payload.len() + 4) as i32).to_be_bytes());
    out.extend_from_slice(&msg.payload);
    out
}

async fn write_frontend_message(stream: &mut TcpStream, kind: u8, payload: &[u8]) -> Result<()> {
    let mut out = Vec::with_capacity(payload.len() + 5);
    out.push(kind);
    out.extend_from_slice(&((payload.len() + 4) as i32).to_be_bytes());
    out.extend_from_slice(payload);
    stream
        .write_all(&out)
        .await
        .with_context(|| format!("failed writing frontend message {}", kind as char))
}

fn pg_md5_password(user: &str, password: &str, salt: &[u8; 4]) -> String {
    let first = md5::compute(format!("{}{}", password, user));
    let first_hex = format!("{:x}", first);
    let mut second_input = first_hex.into_bytes();
    second_input.extend_from_slice(salt);
    let second = md5::compute(second_input);
    format!("md5{:x}", second)
}

fn parse_auth_sasl_methods(mut bytes: &[u8]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        if bytes[0] == 0 {
            break;
        }
        let (value, rem) = read_cstr(bytes)?;
        out.push(value);
        bytes = rem;
    }
    Ok(out)
}

fn build_scram_initial(user: &str, password: &str) -> Result<(String, ScramState)> {
    let nonce = Alphanumeric.sample_string(&mut rand::rng(), 24);
    let escaped_user = user.replace('=', "=3D").replace(',', "=2C");
    let client_first_bare = format!("n={escaped_user},r={nonce}");
    let client_first = format!("n,,{client_first_bare}");

    let dummy_state = ScramState {
        client_first_bare,
        expected_server_signature: derive_server_signature(password.as_bytes(), b"", 1, "")?,
    };

    Ok((client_first, dummy_state))
}

fn build_scram_final(
    state: ScramState,
    password: &str,
    server_first: &str,
) -> Result<(String, ScramState)> {
    let attrs = parse_scram_attrs(server_first)?;
    let nonce = attrs
        .get("r")
        .ok_or_else(|| anyhow!("SCRAM missing nonce r"))?
        .to_string();
    if !nonce.starts_with(
        state
            .client_first_bare
            .split(",r=")
            .nth(1)
            .ok_or_else(|| anyhow!("SCRAM client nonce missing"))?,
    ) {
        bail!("SCRAM nonce mismatch");
    }

    let salt = BASE64
        .decode(
            attrs
                .get("s")
                .ok_or_else(|| anyhow!("SCRAM missing salt s"))?,
        )
        .context("SCRAM invalid salt base64")?;
    let iter = attrs
        .get("i")
        .ok_or_else(|| anyhow!("SCRAM missing iteration i"))?
        .parse::<u32>()
        .context("SCRAM invalid iteration")?;

    let client_final_without_proof = format!("c=biws,r={nonce}");
    let auth_message = format!(
        "{},{},{}",
        state.client_first_bare, server_first, client_final_without_proof
    );

    let mut salted = [0_u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iter, &mut salted);

    let client_key = hmac_sha256(&salted, b"Client Key")?;
    let stored_key = Sha256::digest(&client_key);
    let client_sig = hmac_sha256(&stored_key, auth_message.as_bytes())?;
    let client_proof = xor_bytes(&client_key, &client_sig);

    let server_signature =
        derive_server_signature(password.as_bytes(), &salt, iter, &auth_message)?;

    let client_final = format!(
        "{},p={}",
        client_final_without_proof,
        BASE64.encode(client_proof)
    );

    Ok((
        client_final,
        ScramState {
            client_first_bare: state.client_first_bare,
            expected_server_signature: server_signature,
        },
    ))
}

fn derive_server_signature(
    password: &[u8],
    salt: &[u8],
    iter: u32,
    auth_message: &str,
) -> Result<Vec<u8>> {
    let mut salted = [0_u8; 32];
    pbkdf2_hmac::<Sha256>(password, salt, iter, &mut salted);
    let server_key = hmac_sha256(&salted, b"Server Key")?;
    hmac_sha256(&server_key, auth_message.as_bytes())
}

fn verify_scram_server_final(state: &ScramState, server_final: &str) -> Result<()> {
    let attrs = parse_scram_attrs(server_final)?;
    if let Some(error) = attrs.get("e") {
        bail!("SCRAM server error: {error}");
    }
    let got = BASE64
        .decode(
            attrs
                .get("v")
                .ok_or_else(|| anyhow!("SCRAM missing verifier v"))?,
        )
        .context("SCRAM invalid verifier base64")?;
    if got != state.expected_server_signature {
        bail!("SCRAM verifier mismatch");
    }
    Ok(())
}

fn parse_scram_attrs(input: &str) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for part in input.split(',') {
        let (k, v) = part
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid SCRAM attribute: {part}"))?;
        out.insert(k.to_string(), v.to_string());
    }
    Ok(out)
}

fn xor_bytes(a: &[u8], b: &[u8]) -> Vec<u8> {
    a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).context("invalid hmac key")?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

async fn write_sasl_initial_response(
    stream: &mut TcpStream,
    mechanism: &str,
    initial_response: &str,
) -> Result<()> {
    let mut payload = Vec::new();
    payload.extend_from_slice(mechanism.as_bytes());
    payload.push(0);
    payload.extend_from_slice(&(initial_response.len() as i32).to_be_bytes());
    payload.extend_from_slice(initial_response.as_bytes());
    write_frontend_message(stream, b'p', &payload).await
}

fn proxy_from_env() -> Result<ProxyConfig> {
    for key in ["JANUS_CONNECT_PROXY_URL", "ALL_PROXY", "HTTP_PROXY"] {
        if let Some(value) = env_non_empty(key) {
            return parse_proxy_url(&value, key);
        }
    }

    bail!(
        "missing proxy env: set JANUS_CONNECT_PROXY_URL (or ALL_PROXY/HTTP_PROXY with janus token auth)"
    )
}

fn parse_proxy_url(raw: &str, source_env: &str) -> Result<ProxyConfig> {
    let parsed = Url::parse(raw).with_context(|| format!("invalid {source_env}: {raw}"))?;
    if parsed.scheme() != "http" {
        bail!("{source_env} must use http:// scheme");
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("{source_env} missing host"))?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow!("{source_env} missing port"))?;
    let token = parsed
        .password()
        .ok_or_else(|| anyhow!("{source_env} missing proxy auth token"))?
        .to_string();

    Ok(ProxyConfig {
        host,
        port,
        token,
        source_env: source_env.to_string(),
    })
}

fn env_non_empty(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_password_matches_known_shape() {
        let salt = [1_u8, 2, 3, 4];
        let out = pg_md5_password("alice", "secret", &salt);
        assert!(out.starts_with("md5"));
        assert_eq!(out.len(), 35);
    }

    #[test]
    fn startup_params_parse_basic_pairs() {
        let mut payload = Vec::new();
        write_cstr(&mut payload, "user");
        write_cstr(&mut payload, "bob");
        write_cstr(&mut payload, "database");
        write_cstr(&mut payload, "app");
        payload.push(0);

        let parsed = parse_startup_params(&payload).expect("parsed");
        assert_eq!(parsed.get("user"), Some(&"bob".to_string()));
        assert_eq!(parsed.get("database"), Some(&"app".to_string()));
    }

    #[test]
    fn proxy_url_extracts_token() {
        let parsed =
            parse_proxy_url("http://janus:t0k@127.0.0.1:9080", "HTTP_PROXY").expect("parse");
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 9080);
        assert_eq!(parsed.token, "t0k");
    }
}
