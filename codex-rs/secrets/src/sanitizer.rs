use regex::Regex;
use serde_json::Value;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

struct ReplacementRule {
    regex: Regex,
    replacement: &'static str,
}

static SECRET_ASSIGNMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r#"(?i)\b(api[_-]?key|token|secret|password)\b(\s*[:=]\s*)(["']?)[^\s"']{8,}"#)
});

// Adapted from leakwall's embedded default patterns so Codex can scrub outbound
// request strings without introducing leakwall's proxy architecture.
static REDACTION_RULES: LazyLock<Vec<ReplacementRule>> = LazyLock::new(|| {
    vec![
        rule(r"\bAKIA[0-9A-Z]{16}\b", "[REDACTED_SECRET]"),
        rule(r"\bghp_[a-zA-Z0-9]{36}\b", "[REDACTED_SECRET]"),
        rule(r"\bgho_[a-zA-Z0-9]{36}\b", "[REDACTED_SECRET]"),
        rule(
            r"\bgithub_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}\b",
            "[REDACTED_SECRET]",
        ),
        rule(r"\bsk_live_[a-zA-Z0-9]{24,}\b", "[REDACTED_SECRET]"),
        rule(r"\bsk_test_[a-zA-Z0-9]{24,}\b", "[REDACTED_SECRET]"),
        rule(
            r"\bxoxb-[0-9]{10,13}-[0-9]{10,13}-[a-zA-Z0-9]{24}\b",
            "[REDACTED_SECRET]",
        ),
        rule(
            r"\bxoxp-[0-9]{10,13}-[0-9]{10,13}-[a-zA-Z0-9]{24,}\b",
            "[REDACTED_SECRET]",
        ),
        rule(r"\bnpm_[a-zA-Z0-9]{36}\b", "[REDACTED_SECRET]"),
        rule(
            r"\bsk-[a-zA-Z0-9]{20}T3BlbkFJ[a-zA-Z0-9]{20}\b",
            "[REDACTED_SECRET]",
        ),
        rule(r"\bsk-proj-[A-Za-z0-9_-]{20,}\b", "[REDACTED_SECRET]"),
        rule(r"\bsk-ant-[a-zA-Z0-9\-]{90,}\b", "[REDACTED_SECRET]"),
        rule(r"\bAIza[0-9A-Za-z_-]{35}\b", "[REDACTED_SECRET]"),
        rule(
            r"\beyJ[a-zA-Z0-9_-]{10,}\.eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\b",
            "[REDACTED_SECRET]",
        ),
        rule(
            r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
            "[REDACTED_SECRET]",
        ),
        rule(
            r"(?i)\b(?:postgres|mysql|mongodb)://[^:\s]+:[^@\s]+@[^/\s]+",
            "[REDACTED_SECRET]",
        ),
        rule(
            r"(?i)\bBearer\s+[A-Za-z0-9._\-]{16,}\b",
            "Bearer [REDACTED_SECRET]",
        ),
        rule(r"\bsk-[A-Za-z0-9]{20,}\b", "[REDACTED_SECRET]"),
    ]
});

/// Remove secret-like values from a string on a best-effort basis using the
/// same default pattern families leakwall uses for generic secret detection.
pub fn redact_secrets(input: String) -> String {
    let runtime_secret_values = discover_runtime_secret_values();
    redact_secrets_with_runtime_values(input, &runtime_secret_values)
}

fn redact_secrets_with_runtime_values(input: String, runtime_secret_values: &[String]) -> String {
    let mut redacted = input;
    for secret in runtime_secret_values {
        if secret.is_empty() {
            continue;
        }
        redacted = redacted.replace(secret, "[REDACTED_SECRET]");
    }

    for rule in REDACTION_RULES.iter() {
        redacted = rule
            .regex
            .replace_all(&redacted, rule.replacement)
            .into_owned();
    }
    redacted = SECRET_ASSIGNMENT_REGEX
        .replace_all(&redacted, "$1$2$3[REDACTED_SECRET]")
        .into_owned();

    redacted
}

/// Recursively redact secrets from all string leaves in a JSON payload.
pub fn redact_json_secrets(value: &mut Value) {
    let runtime_secret_values = discover_runtime_secret_values();
    redact_json_secrets_with_runtime_values(value, &runtime_secret_values);
}

fn redact_json_secrets_with_runtime_values(value: &mut Value, runtime_secret_values: &[String]) {
    match value {
        Value::String(text) => {
            *text = redact_secrets_with_runtime_values(std::mem::take(text), runtime_secret_values);
        }
        Value::Array(items) => {
            for item in items {
                redact_json_secrets_with_runtime_values(item, runtime_secret_values);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                redact_json_secrets_with_runtime_values(value, runtime_secret_values);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn discover_runtime_secret_values() -> Vec<String> {
    let cwd = env::current_dir().ok();
    let home = env::var_os("HOME").map(PathBuf::from);
    let mut secrets = BTreeSet::new();

    for value in discover_file_secret_values(home.as_deref(), cwd.as_deref()) {
        secrets.insert(value);
    }

    for value in discover_env_secret_values() {
        secrets.insert(value);
    }

    if let Some(cwd) = cwd.as_deref() {
        for value in discover_git_remote_secret_values(cwd) {
            secrets.insert(value);
        }
    }

    let mut values = secrets.into_iter().collect::<Vec<_>>();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values
}

fn discover_file_secret_values(home: Option<&Path>, cwd: Option<&Path>) -> Vec<String> {
    let mut values = Vec::new();
    let Some(cwd) = cwd else {
        return values;
    };

    for path in secret_file_paths(home, cwd) {
        if !path.exists() {
            continue;
        }
        values.extend(scan_file_for_secret_values(&path));
    }

    values
}

fn secret_file_paths(home: Option<&Path>, cwd: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        cwd.join(".env"),
        cwd.join(".env.local"),
        cwd.join(".env.development"),
        cwd.join(".env.production"),
        cwd.join(".env.staging"),
        cwd.join(".env.test"),
    ];

    if let Some(home) = home {
        paths.push(home.join(".env"));
        paths.push(home.join(".aws/credentials"));
        paths.push(home.join(".aws/config"));
        paths.push(home.join(".azure/credentials"));
        paths.push(home.join(".config/gcloud/application_default_credentials.json"));
        paths.push(home.join(".config/gcloud/credentials.db"));
        paths.push(home.join(".npmrc"));
        paths.push(home.join(".docker/config.json"));
        paths.push(home.join(".config/gh/hosts.yml"));
        paths.push(home.join(".kube/config"));
        paths.push(home.join(".netrc"));
        paths.push(home.join(".pypirc"));
        paths.push(home.join(".git-credentials"));
    }

    paths
}

fn scan_file_for_secret_values(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    if path.extension().is_some_and(|ext| ext == "json") {
        return scan_json_file_for_secret_values(path, &content);
    }

    if path
        .extension()
        .is_some_and(|ext| ext == "yml" || ext == "yaml")
    {
        return scan_key_value_content_for_secret_values(&content, ':');
    }

    scan_key_value_content_for_secret_values(&content, '=')
}

fn scan_key_value_content_for_secret_values(content: &str, separator: char) -> Vec<String> {
    let mut values = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(separator) else {
            continue;
        };

        let key = key.trim().trim_start_matches("export ");
        let value = value.trim().trim_matches('"').trim_matches('\'');
        let min_len = min_length_for_key(key);
        if value.is_empty() || value.len() < min_len || is_common_value(value) {
            continue;
        }

        values.push(value.to_string());
    }

    values
}

fn scan_json_file_for_secret_values(path: &Path, content: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        let _ = path;
        return Vec::new();
    };

    let mut secrets = Vec::new();
    collect_json_secret_values(&value, "", &mut secrets, 0);
    secrets
}

fn collect_json_secret_values(
    value: &Value,
    key_path: &str,
    secrets: &mut Vec<String>,
    depth: usize,
) {
    if depth > 32 {
        return;
    }

    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let new_path = if key_path.is_empty() {
                    key.clone()
                } else {
                    format!("{key_path}.{key}")
                };
                collect_json_secret_values(value, &new_path, secrets, depth + 1);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_json_secret_values(
                    item,
                    &format!("{key_path}[{index}]"),
                    secrets,
                    depth + 1,
                );
            }
        }
        Value::String(text) => {
            let min_len = min_length_for_key(key_path);
            if text.len() >= min_len && !is_common_value(text) {
                secrets.push(text.clone());
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn discover_env_secret_values() -> Vec<String> {
    env::vars()
        .filter_map(|(key, value)| {
            let min_len = min_length_for_key(&key);
            if is_excluded_env_name(&key) || value.len() < min_len || is_common_value(&value) {
                return None;
            }
            Some(value)
        })
        .collect()
}

fn discover_git_remote_secret_values(cwd: &Path) -> Vec<String> {
    let git_config = cwd.join(".git/config");
    let Ok(content) = std::fs::read_to_string(git_config) else {
        return Vec::new();
    };

    let mut values = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        let Some(url) = trimmed.strip_prefix("url = ") else {
            continue;
        };
        let Some(at_pos) = url.find('@') else {
            continue;
        };
        let Some(proto_end) = url.find("://") else {
            continue;
        };
        let cred_start = proto_end + 3;
        let Some(cred_part) = url.get(cred_start..at_pos) else {
            continue;
        };
        if !cred_part.contains(':') {
            continue;
        }
        let token = cred_part.split(':').nth(1).unwrap_or(cred_part);
        if token.len() >= 8 {
            values.push(token.to_string());
        }
    }

    values
}

fn is_excluded_env_name(name: &str) -> bool {
    const EXCLUDED_NAMES: &[&str] = &[
        "PATH",
        "HOME",
        "SHELL",
        "TERM",
        "EDITOR",
        "LANG",
        "USER",
        "LOGNAME",
        "HOSTNAME",
        "PWD",
        "OLDPWD",
        "SHLVL",
        "DISPLAY",
        "XDG_",
        "LC_",
        "TERM_PROGRAM",
        "TERM_SESSION_ID",
        "COLORTERM",
        "WINDOWID",
        "DBUS_SESSION_BUS_ADDRESS",
        "DESKTOP_SESSION",
        "SESSION_MANAGER",
        "GTK_",
        "QT_",
        "GDK_",
        "GNOME_",
        "KDE_",
        "WAYLAND_",
        "SSH_AUTH_SOCK",
        "SSH_AGENT_PID",
        "GPG_AGENT_INFO",
        "LESS",
        "PAGER",
        "MANPATH",
        "INFOPATH",
        "LS_COLORS",
        "LSCOLORS",
        "CLICOLOR",
        "GREP_",
        "BROWSER",
        "VISUAL",
        "TMPDIR",
        "TEMP",
        "TMP",
        "CARGO_",
        "RUSTUP_",
        "RUSTC",
        "RUST_",
        "NVM_",
        "PYENV_",
        "GOPATH",
        "GOROOT",
        "JAVA_HOME",
        "NODE_PATH",
        "VIRTUAL_ENV",
        "CONDA_",
        "WSL_",
        "WSLENV",
        "WT_",
    ];

    EXCLUDED_NAMES
        .iter()
        .any(|excluded| name == *excluded || name.starts_with(excluded))
}

fn min_length_for_key(key: &str) -> usize {
    if is_secret_key_name(key) { 4 } else { 8 }
}

fn is_secret_key_name(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    const INDICATORS: &[&str] = &[
        "KEY",
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "CREDENTIAL",
        "AUTH",
        "API_KEY",
        "APIKEY",
        "ACCESS_KEY",
        "PRIVATE",
        "PASSWD",
        "CONN",
        "DSN",
        "URL",
    ];

    INDICATORS.iter().any(|indicator| upper.contains(indicator))
}

fn is_common_value(value: &str) -> bool {
    const COMMON_VALUES: &[&str] = &[
        "true",
        "false",
        "yes",
        "no",
        "null",
        "none",
        "undefined",
        "development",
        "production",
        "staging",
        "test",
        "testing",
        "local",
        "debug",
        "info",
        "warn",
        "warning",
        "error",
        "trace",
        "verbose",
        "localhost",
        "0.0.0.0",
        "127.0.0.1",
        "::1",
        "default",
        "auto",
        "enabled",
        "disabled",
        "on",
        "off",
        "utf-8",
        "utf8",
        "json",
        "text",
        "html",
        "https",
        "http",
    ];

    let lower = value.to_ascii_lowercase();
    COMMON_VALUES.iter().any(|common| lower == *common)
}

fn rule(pattern: &str, replacement: &'static str) -> ReplacementRule {
    ReplacementRule {
        regex: compile_regex(pattern),
        replacement,
    }
}

fn compile_regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(err) => panic!("invalid regex pattern `{pattern}`: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_regex() {
        let _ = redact_secrets("secret".to_string());
    }

    #[test]
    fn redacts_leakwall_style_patterns() {
        let input = "ghp_abcdefghijklmnopqrstuvwxyzABCDEF1234 sk_test_abcdefghijklmnopqrstuvwxyz12";
        let output = redact_secrets(input.to_string());
        assert!(!output.contains("ghp_abcdefghijklmnopqrstuvwxyzABCDEF1234"));
        assert!(!output.contains("sk_test_abcdefghijklmnopqrstuvwxyz12"));
        assert_eq!(output.matches("[REDACTED_SECRET]").count(), 2);
    }

    #[test]
    fn redacts_json_string_leaves_recursively() {
        let mut value = serde_json::json!({
            "instructions": "Bearer abcdefghijklmnopqrstuvwxyz123456",
            "input": [
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "postgres://user:supersecret@db.internal/app"
                        }
                    ]
                }
            ]
        });

        redact_json_secrets(&mut value);
        let serialized = value.to_string();
        assert!(!serialized.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!serialized.contains("supersecret"));
        assert!(serialized.contains("[REDACTED_SECRET]"));
    }
}
