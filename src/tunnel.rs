use std::env;
use std::net::SocketAddr;

use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clap::Parser;
use reqwest::Url;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Parser, Debug)]
#[command(
    name = "janus-tunnel",
    version,
    about = "Local TCP bridge through Janus CONNECT proxy for wire protocols"
)]
struct Cli {
    #[arg(
        long,
        default_value = "127.0.0.1:15432",
        help = "Local listen address (agent connects here)"
    )]
    listen: SocketAddr,
    #[arg(long, help = "Target upstream host")]
    target_host: String,
    #[arg(long, help = "Target upstream port")]
    target_port: Option<u16>,
    #[arg(
        long,
        help = "Optional capability/protocol name for default port (example: postgres_wire, mysql_wire, redis)"
    )]
    protocol: Option<String>,
}

#[derive(Clone, Debug)]
struct Target {
    host: String,
    port: u16,
    protocol: Option<String>,
}

#[derive(Clone, Debug)]
struct ProxyConfig {
    host: String,
    port: u16,
    token: String,
    source_env: String,
}

pub(crate) async fn run() -> Result<()> {
    let cli = Cli::parse();
    let target = resolve_target(&cli)?;
    let proxy = proxy_from_env()?;

    let listener = TcpListener::bind(cli.listen)
        .await
        .with_context(|| format!("failed binding {}", cli.listen))?;
    let local = listener
        .local_addr()
        .context("failed resolving listener local addr")?;

    eprintln!(
        "janus-tunnel listening on {} -> {}:{}{} (proxy {}:{} from {})",
        local,
        target.host,
        target.port,
        target
            .protocol
            .as_ref()
            .map(|v| format!(" [{v}]"))
            .unwrap_or_default(),
        proxy.host,
        proxy.port,
        proxy.source_env
    );
    eprintln!("keep this process running while your client (psql/mysql/etc.) connects");

    loop {
        let (downstream, peer) = listener.accept().await.context("accept failed")?;
        let target = target.clone();
        let proxy = proxy.clone();

        tokio::spawn(async move {
            if let Err(error) = handle_connection(downstream, &target, &proxy).await {
                eprintln!("janus-tunnel connection from {peer} failed: {error}");
            }
        });
    }
}

async fn handle_connection(
    mut downstream: TcpStream,
    target: &Target,
    proxy: &ProxyConfig,
) -> Result<()> {
    let mut upstream = TcpStream::connect((proxy.host.as_str(), proxy.port))
        .await
        .with_context(|| format!("failed dialing janus proxy {}:{}", proxy.host, proxy.port))?;

    let connect = build_connect_request(target, proxy);
    upstream
        .write_all(connect.as_bytes())
        .await
        .context("failed writing CONNECT request")?;

    let leftover = read_connect_response(&mut upstream).await?;
    if !leftover.is_empty() {
        downstream
            .write_all(&leftover)
            .await
            .context("failed forwarding CONNECT leftover bytes")?;
    }

    let _ = copy_bidirectional(&mut downstream, &mut upstream)
        .await
        .context("tunnel copy failed")?;

    Ok(())
}

fn build_connect_request(target: &Target, proxy: &ProxyConfig) -> String {
    let auth = BASE64.encode(format!("janus:{}", proxy.token));
    format!(
        "CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\nProxy-Authorization: Basic {auth}\r\n\r\n",
        host = target.host,
        port = target.port,
        auth = auth,
    )
}

async fn read_connect_response(stream: &mut TcpStream) -> Result<Vec<u8>> {
    const MAX_HEADER: usize = 16 * 1024;
    let mut buffer = Vec::with_capacity(1024);
    let mut temp = [0_u8; 1024];

    let header_end = loop {
        let read = stream
            .read(&mut temp)
            .await
            .context("failed reading CONNECT response")?;
        if read == 0 {
            bail!("proxy closed connection before CONNECT response");
        }
        buffer.extend_from_slice(&temp[..read]);
        if buffer.len() > MAX_HEADER {
            bail!("CONNECT response header too large");
        }
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };

    let head = &buffer[..header_end];
    let status_line = head
        .split(|b| *b == b'\n')
        .next()
        .ok_or_else(|| anyhow!("missing CONNECT status line"))?;
    let status_line = String::from_utf8_lossy(status_line).trim().to_string();

    if !status_line.contains(" 200 ") {
        bail!("proxy CONNECT rejected: {status_line}");
    }

    Ok(buffer[header_end..].to_vec())
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|idx| idx + 4)
}

fn resolve_target(cli: &Cli) -> Result<Target> {
    let protocol = cli
        .protocol
        .as_deref()
        .map(normalize_protocol)
        .transpose()?;

    let port = match (cli.target_port, protocol.as_deref()) {
        (Some(value), _) => value,
        (None, Some(name)) => default_port_for_protocol(name)
            .ok_or_else(|| anyhow!("protocol has no default CONNECT port: {name}"))?,
        (None, None) => bail!("either --target-port or --protocol must be provided"),
    };

    Ok(Target {
        host: cli.target_host.trim().to_string(),
        port,
        protocol,
    })
}

fn normalize_protocol(value: &str) -> Result<String> {
    let raw = value.trim().to_lowercase();
    let normalized = match raw.as_str() {
        "postgres" | "postgresql" | "pgsql" => "postgres_wire",
        "mysql" => "mysql_wire",
        other => other,
    };

    let known = crate::protocols::all()
        .iter()
        .any(|spec| spec.capability == normalized && spec.connect_fallback);
    if !known {
        bail!("unknown or non-CONNECT protocol: {value}");
    }

    Ok(normalized.to_string())
}

fn default_port_for_protocol(protocol: &str) -> Option<u16> {
    crate::protocols::all()
        .iter()
        .find(|spec| spec.capability == protocol && spec.connect_fallback)
        .and_then(|spec| spec.ports.first().copied())
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
    fn parse_proxy_url_extracts_token_and_endpoint() {
        let parsed = parse_proxy_url("http://janus:tok123@127.0.0.1:9080", "HTTP_PROXY")
            .expect("proxy url parse");
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 9080);
        assert_eq!(parsed.token, "tok123");
    }

    #[test]
    fn normalize_protocol_supports_aliases() {
        assert_eq!(normalize_protocol("postgres").expect("ok"), "postgres_wire");
        assert_eq!(normalize_protocol("mysql").expect("ok"), "mysql_wire");
        assert!(normalize_protocol("git_http").is_err());
    }

    #[test]
    fn default_port_for_postgres_wire() {
        assert_eq!(default_port_for_protocol("postgres_wire"), Some(5432));
    }

    #[test]
    fn find_header_end_detects_separator() {
        let bytes = b"HTTP/1.1 200 Connection Established\r\nX-Test: 1\r\n\r\nabc";
        assert_eq!(find_header_end(bytes), Some(50));
    }
}
