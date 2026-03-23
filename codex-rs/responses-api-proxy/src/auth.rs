use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_utils_home_dir::find_codex_home;
use reqwest::header::HeaderValue;

use crate::read_api_key::protect_bearer_auth_header;

pub(crate) const CHATGPT_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
pub(crate) const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

#[derive(Clone, Debug)]
pub(crate) struct ResolvedAuth {
    pub(crate) auth_header: &'static str,
    pub(crate) chatgpt_account_id: Option<HeaderValue>,
    pub(crate) default_upstream_url: &'static str,
}

pub(crate) fn resolve_auth_from_codex(codex_home: Option<&Path>) -> Result<ResolvedAuth> {
    let codex_home = codex_home
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(|| find_codex_home().context("resolving CODEX_HOME"))?;
    let auth_manager = AuthManager::new(
        codex_home.clone(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime for Codex auth")?;
    let auth = runtime.block_on(auth_manager.auth()).ok_or_else(|| {
        anyhow!(
            "no Codex auth found in {} (expected auth.json)",
            codex_home.display()
        )
    })?;

    let auth_header = protect_bearer_auth_header(&auth.get_token().with_context(|| {
        format!(
            "loading bearer token from Codex auth in {}",
            codex_home.display()
        )
    })?)?;
    let chatgpt_account_id = auth
        .get_account_id()
        .map(|account_id| {
            HeaderValue::from_str(&account_id)
                .with_context(|| format!("building ChatGPT-Account-ID header from {account_id:?}"))
        })
        .transpose()?;
    let default_upstream_url = if auth.is_chatgpt_auth() {
        CHATGPT_RESPONSES_URL
    } else {
        OPENAI_RESPONSES_URL
    };

    Ok(ResolvedAuth {
        auth_header,
        chatgpt_account_id,
        default_upstream_url,
    })
}

pub(crate) fn resolve_auth_from_stdin() -> Result<ResolvedAuth> {
    Ok(ResolvedAuth {
        auth_header: crate::read_api_key::read_auth_header_from_stdin()?,
        chatgpt_account_id: None,
        default_upstream_url: OPENAI_RESPONSES_URL,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use codex_login::AuthDotJson;
    use codex_login::AuthMode;
    use codex_login::TokenData;
    use codex_login::login_with_api_key;
    use codex_login::save_auth;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn resolves_api_key_auth_from_codex_home() {
        let codex_home = TempDir::new().expect("tempdir");
        login_with_api_key(
            codex_home.path(),
            "sk-test-key",
            AuthCredentialsStoreMode::File,
        )
        .expect("write api key auth");

        let resolved = resolve_auth_from_codex(Some(codex_home.path())).expect("resolve auth");

        assert_eq!(resolved.auth_header, "Bearer sk-test-key");
        assert_eq!(resolved.chatgpt_account_id, None);
        assert_eq!(resolved.default_upstream_url, OPENAI_RESPONSES_URL);
    }

    #[test]
    fn resolves_chatgpt_auth_from_codex_home() {
        let codex_home = TempDir::new().expect("tempdir");
        save_auth(
            codex_home.path(),
            &AuthDotJson {
                auth_mode: Some(AuthMode::Chatgpt),
                openai_api_key: None,
                tokens: Some(TokenData {
                    id_token: codex_login::token_data::IdTokenInfo {
                        raw_jwt: "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudF9pZCJ9fQ.c2ln".to_string(),
                        chatgpt_account_id: Some("account_id".to_string()),
                        ..Default::default()
                    },
                    access_token: "header.payload.signature".to_string(),
                    refresh_token: "refresh-token".to_string(),
                    account_id: Some("account_id".to_string()),
                }),
                last_refresh: Some(Utc::now()),
            },
            AuthCredentialsStoreMode::File,
        )
        .expect("write chatgpt auth");

        let resolved = resolve_auth_from_codex(Some(codex_home.path())).expect("resolve auth");

        assert_eq!(resolved.auth_header, "Bearer header.payload.signature");
        assert_eq!(
            resolved
                .chatgpt_account_id
                .as_ref()
                .map(HeaderValue::as_bytes),
            Some("account_id".as_bytes())
        );
        assert_eq!(resolved.default_upstream_url, CHATGPT_RESPONSES_URL);
    }

    #[test]
    fn errors_when_codex_auth_is_missing() {
        let codex_home = TempDir::new().expect("tempdir");

        let err = resolve_auth_from_codex(Some(codex_home.path())).expect_err("missing auth");

        assert!(format!("{err:#}").contains("no Codex auth found"));
    }

    #[test]
    fn accepts_explicit_codex_home_paths() {
        let codex_home = TempDir::new().expect("tempdir");
        let explicit_path = PathBuf::from(codex_home.path());
        login_with_api_key(
            codex_home.path(),
            "sk-explicit",
            AuthCredentialsStoreMode::File,
        )
        .expect("write auth");

        let resolved = resolve_auth_from_codex(Some(explicit_path.as_path())).expect("auth");

        assert_eq!(resolved.auth_header, "Bearer sk-explicit");
    }
}
