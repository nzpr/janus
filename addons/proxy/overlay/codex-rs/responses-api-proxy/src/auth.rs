use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::read_api_key::protect_bearer_auth_header;
use crate::read_api_key::read_auth_header_from_stdin;

pub(crate) const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
pub(crate) const CHATGPT_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

#[derive(Clone)]
pub(crate) struct ResolvedAuth {
    pub(crate) auth_header: &'static str,
    pub(crate) chatgpt_account_id: Option<HeaderValue>,
    pub(crate) default_upstream_url: &'static str,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum AuthMode {
    ApiKey,
    Chatgpt,
    ChatgptAuthTokens,
}

#[derive(Debug, Clone, Deserialize)]
struct AuthDotJson {
    #[serde(default)]
    auth_mode: Option<AuthMode>,

    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,

    #[serde(default)]
    tokens: Option<TokenData>,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenData {
    access_token: String,
    account_id: Option<String>,
}

impl AuthDotJson {
    fn resolved_mode(&self) -> AuthMode {
        if let Some(mode) = self.auth_mode {
            return mode;
        }
        if self.openai_api_key.is_some() {
            return AuthMode::ApiKey;
        }
        AuthMode::Chatgpt
    }
}

pub(crate) fn resolve_auth_from_stdin() -> Result<ResolvedAuth> {
    Ok(ResolvedAuth {
        auth_header: read_auth_header_from_stdin()?,
        chatgpt_account_id: None,
        default_upstream_url: OPENAI_RESPONSES_URL,
    })
}

pub(crate) fn resolve_auth_from_codex(codex_home_override: Option<&Path>) -> Result<ResolvedAuth> {
    let codex_home = resolve_codex_home(codex_home_override)?;
    let auth_path = codex_home.join("auth.json");
    let auth_json = read_auth_json(&auth_path)?;

    match auth_json.resolved_mode() {
        AuthMode::ApiKey => {
            let api_key = auth_json
                .openai_api_key
                .as_deref()
                .filter(|key| !key.is_empty())
                .ok_or_else(|| anyhow!("auth.json is in API key mode but OPENAI_API_KEY is missing"))?;

            Ok(ResolvedAuth {
                auth_header: protect_bearer_auth_header(api_key)?,
                chatgpt_account_id: None,
                default_upstream_url: OPENAI_RESPONSES_URL,
            })
        }
        AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens => {
            let tokens = auth_json.tokens.ok_or_else(|| {
                anyhow!("auth.json is in ChatGPT auth mode but tokens are missing")
            })?;

            let auth_header = protect_bearer_auth_header(&tokens.access_token)?;
            let account_id_header = tokens
                .account_id
                .as_deref()
                .filter(|account_id| !account_id.is_empty())
                .map(HeaderValue::from_str)
                .transpose()
                .context("invalid ChatGPT account id in auth.json")?;

            Ok(ResolvedAuth {
                auth_header,
                chatgpt_account_id: account_id_header,
                default_upstream_url: CHATGPT_RESPONSES_URL,
            })
        }
    }
}

fn resolve_codex_home(codex_home_override: Option<&Path>) -> Result<PathBuf> {
    match codex_home_override {
        Some(path) => Ok(path.to_path_buf()),
        None => codex_utils_home_dir::find_codex_home().context("resolving CODEX_HOME"),
    }
}

fn read_auth_json(path: &Path) -> Result<AuthDotJson> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_json_defaults_to_api_key_mode_when_key_present() {
        let auth: AuthDotJson = serde_json::from_str(
            r#"{
                "OPENAI_API_KEY": "sk-test",
                "tokens": {
                    "access_token": "tok-test",
                    "refresh_token": "refresh-test"
                }
            }"#,
        )
        .expect("parse auth json");

        assert_eq!(auth.resolved_mode(), AuthMode::ApiKey);
    }

    #[test]
    fn auth_json_defaults_to_chatgpt_mode_without_api_key() {
        let auth: AuthDotJson = serde_json::from_str(
            r#"{
                "tokens": {
                    "access_token": "tok-test",
                    "refresh_token": "refresh-test",
                    "account_id": "acc-test"
                }
            }"#,
        )
        .expect("parse auth json");

        assert_eq!(auth.resolved_mode(), AuthMode::Chatgpt);
    }
}
