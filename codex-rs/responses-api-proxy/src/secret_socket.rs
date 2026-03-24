use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::Context;
use anyhow::Result;

#[derive(Clone, Default)]
pub(crate) struct DynamicSecretSource {
    secret_values: Arc<RwLock<Vec<String>>>,
}

impl DynamicSecretSource {
    pub(crate) fn start(socket_path: Option<PathBuf>) -> Result<Self> {
        let source = Self::default();

        let Some(socket_path) = socket_path else {
            return Ok(source);
        };

        if let Some(parent) = socket_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating secret socket dir {}", parent.display()))?;
        }

        remove_stale_socket(&socket_path)?;

        let listener = UnixListener::bind(&socket_path)
            .with_context(|| format!("binding secret socket {}", socket_path.display()))?;
        let thread_source = source.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        if let Err(err) = thread_source.update_from_stream(stream) {
                            eprintln!("secret socket update error: {err}");
                        }
                    }
                    Err(err) => {
                        eprintln!("secret socket accept error: {err}");
                    }
                }
            }
        });

        Ok(source)
    }

    pub(crate) fn secret_values(&self) -> Vec<String> {
        self.secret_values
            .read()
            .map(|values| values.clone())
            .unwrap_or_default()
    }

    fn update_from_stream(&self, mut stream: UnixStream) -> Result<()> {
        let mut body = String::new();
        stream
            .read_to_string(&mut body)
            .context("reading secret socket payload")?;
        let secret_values = parse_secret_payload(&body)?;
        let mut guard = self
            .secret_values
            .write()
            .map_err(|_| anyhow::anyhow!("poisoned secret socket state"))?;
        *guard = secret_values;
        Ok(())
    }
}

fn remove_stale_socket(socket_path: &Path) -> Result<()> {
    match fs::remove_file(socket_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("removing stale socket {}", socket_path.display()))
        }
    }
}

fn parse_secret_payload(payload: &str) -> Result<Vec<String>> {
    if payload.trim().is_empty() {
        return Ok(Vec::new());
    }

    if let Ok(values) = serde_json::from_str::<Vec<String>>(payload) {
        return Ok(normalize_secret_values(values));
    }

    Ok(normalize_secret_values(
        payload
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
    ))
}

fn normalize_secret_values(values: Vec<String>) -> Vec<String> {
    let mut unique_values = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        unique_values.insert(trimmed.to_string());
    }

    let mut normalized = unique_values.into_iter().collect::<Vec<_>>();
    normalized.sort_by_key(|value| std::cmp::Reverse(value.len()));
    normalized
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    use std::time::Instant;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_secret_payload_supports_json_arrays() {
        let parsed = parse_secret_payload(r#"["second-secret","first-secret","second-secret"]"#)
            .expect("parse json array");
        assert_eq!(
            parsed,
            vec!["second-secret".to_string(), "first-secret".to_string()]
        );
    }

    #[test]
    fn parse_secret_payload_supports_newline_delimited_values() {
        let parsed = parse_secret_payload("alpha\nbeta\n\nalpha\n").expect("parse lines");
        assert_eq!(parsed, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn dynamic_secret_source_accepts_socket_updates() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let socket_path = tempdir.path().join("secrets.sock");
        let source = DynamicSecretSource::start(Some(socket_path.clone())).expect("start socket");

        let mut stream = UnixStream::connect(&socket_path).expect("connect");
        stream
            .write_all(br#"["socket-secret","another-secret"]"#)
            .expect("write");
        drop(stream);

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let values = source.secret_values();
            if values == vec!["another-secret".to_string(), "socket-secret".to_string()] {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for socket update"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
